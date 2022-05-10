use crate::{
    Directive, Notification,
    client::{Client, ClientListenError, ClientListenResult, ClientListener, ClientListenerState},
    game::{Game, Player},
    idler::Idler,
    secret::Secret,
};
use futures_util::future::OptionFuture;
use log::{warn, debug};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
};
use tokio::{select, sync::mpsc::{channel, Receiver, Sender}};

pub type LobbyId = usize;
type LobbyIndex = Arc<RwLock<HashMap<LobbyId, Sender<Client>>>>;

static LOBBIES: SyncLazy<LobbyIndex> = SyncLazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub struct Lobby {
    id: LobbyId,
    host: Member<Host>,
    guest: Member<Guest>,
}

impl Lobby {
    fn new(creator: Client) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ID.fetch_add(1, Ordering::Relaxed),
            host: Member::new(Host::new(creator)),
            guest: Member::new(Guest::new()),
        }
    }

    pub async fn send(id: LobbyId, client: Client) {
        // Try to acquire the Sender of the lobby of the corresponding id.
        let client_sender = {
            LOBBIES
                .read()
                .expect("Error acquiring the lobby index lock")
                .get(&id)
                .cloned()
        };

        if let Some(sender) = client_sender {
            if let Err(error) = sender.send(client).await {
                // If the send was unsuccessful, spawn an idle handler for
                // the client.
                Idler::spawn(error.0);

                // This may be an unwanted behavior, so logging a warning
                // might be a good indicator (for the future).
                warn!("Couln't send the client through the lobby sender.");
            } else {
                debug!("A member has just been sent to a lobby");
            }
        }
    }

    pub fn spawn(creator: Client) {
        let (sender, receiver) = channel(1);
        let lobby = Lobby::new(creator);

        {
            LOBBIES
                .write()
                .expect("Error acquiring the lobby index lock")
                .entry(lobby.id)
                .insert_entry(sender);
        }

        tokio::spawn(lobby.listen(receiver));
    }

    async fn listen(mut self, mut receiver: Receiver<Client>) {
        debug!("Listening to member directives in a lobby");

        let _ = self.host.listener
            .client_mut()
            .unwrap()
            .notify(Notification::LobbyCreation { lobby_id: self.id })
            .await;

        // A lobby is guaranteed to have a host connected. Therefore the lobby
        // task must live as long as the host is being listened. A `while let`
        // is handy for this case.
        while let Some(mut host_client) = self.host.listener.take() {
            // A guest may or may not to be connected. Also, a guest must be
            // listened with a host concurrently. Thus, they must be put into the
            // same select body. The `OptionFuture` utility allows listening a
            // possibly connected guest along with the host. If there is a guest,
            // and the guest sends a directive, the future below will return
            // Some(result).
            //
            // The future accesses guest_client with a mutable reference, If the
            // guest_client was moved, and the `host_client` handlers get executed,
            // the handlers would try to access the guest_client via guest listener
            // (self.guest.listener), and there would be no client in it, since
            // it was moved.
            let guest_listen_future: OptionFuture<_> = self
                .guest
                .listener
                .client_mut()
                .map(|client| client.listen())
                .into();

            // Here, the host_client has already been moved from its listener,
            // and the guest_client will be moved if it is relevant.
            //
            // Handler functions will attach the clients to the appropriate listeners
            // if necessarry. So no need to attach them here by hand.
            select! {
                result = host_client.listen() => {
                    Host::handle(host_client, result, &mut self.host, &mut self.guest).await;
                }
                Some(result) = guest_listen_future => {
                    let guest_client = self.guest.listener.take().unwrap();
                    Guest::handle(guest_client, &mut host_client, result, &mut self.guest).await;
                    self.host.listener.attach(host_client);
                }
                Some(mut guest_client) = receiver.recv() => {
                    // If there is already a guest, spawn an idle handler for
                    // the incoming client.
                    if self.guest.listener.is_listening() {
                        Idler::spawn(guest_client);
                        debug!("Guest join rejected, the lobby is full");
                    } else {
                        let _ = tokio::join!{
                            host_client .notify(Notification::GuestJoin),
                            guest_client.notify(Notification::LobbyJoin { lobby_id: self.id }),
                        };

                        self.guest.listener.attach(guest_client);
                        self.host.listener.attach(host_client);

                        debug!("Guest join accepted");
                    }
                },
            }
        }

        {
            LOBBIES
                .write()
                .expect("Error acquiring the lobby index lock")
                .remove(&self.id);
        }

        debug!("Dropping a lobby listener");
    }
}

struct Member<L: ClientListener> {
    listener: L,
    secret: Option<Secret>,
}

impl<L: ClientListener> Member<L> {
    fn new(listener: L) -> Self {
        Self {
            listener,
            secret: None,
        }
    }
}

struct Host(ClientListenerState);

impl ClientListener for Host {
    fn state(&self) -> &ClientListenerState {
        &self.0
    }

    fn state_mut(&mut self) -> &mut ClientListenerState {
        &mut self.0
    }
}

impl Host {
    fn new(client: Client) -> Self {
        Self(ClientListenerState::Listen(client))
    }

    async fn on_set_secret(mut client: Client, member: &mut Member<Self>, secret: Secret) {
        let _ = client.notify(Notification::SecretSet { secret: &secret }).await;
        member.secret = Some(secret);
        member.listener.attach(client);
    }

    async fn on_start_game(client: Client, host: &mut Member<Self>, guest: &mut Member<Guest>) {
        if !(host.secret.is_some() && guest.secret.is_some()) {
            return host.listener.attach(client);
        }

        if let Some(guest_client) = guest.listener.take() {
            let host = Player::new(client, host.secret.take().unwrap());
            let guest = Player::new(guest_client, guest.secret.take().unwrap());

            Game::spawn(host, guest);
            warn!("Send start game notification to members and spawn without callback release mechanism");
        } else {
            host.listener.attach(client);
        }
    }

    async fn on_leave(host: &mut Member<Self>, guest: &mut Member<Guest>) {
        // When the host leaves, if there is a guest, the guest becomes the host.
        if let Some(mut client) = guest.listener.take() {
            let _ = client.notify(Notification::OpponentLeave).await;

            // Attach guest's listener to the host member.
            host.listener.attach(client);

            // Move guest's secret to the host.
            host.secret = guest.secret.take();
        }
    }

    async fn handle(
        client: Client,
        result: ClientListenResult,
        host: &mut Member<Self>,
        guest: &mut Member<Guest>,
    ) {
        use Directive::*;

        match result {
            Ok(directive) => match directive {
                SetSecret { secret } => Self::on_set_secret(client, host, secret).await,
                StartGame => Self::on_start_game(client, host, guest).await,
                Leave => {
                    Self::on_leave(host, guest).await;
                    Idler::spawn(client);
                }
                CloseConnection => Self::on_leave(host, guest).await,
                _ => host.listener.attach(client),
            },
            Err(ClientListenError::SocketExhausted) => Self::on_leave(host, guest).await,
            _ => host.listener.attach(client),
        }
    }
}

struct Guest(ClientListenerState);

impl ClientListener for Guest {
    fn state(&self) -> &ClientListenerState {
        &self.0
    }

    fn state_mut(&mut self) -> &mut ClientListenerState {
        &mut self.0
    }
}

impl Guest {
    fn new() -> Self {
        Self(ClientListenerState::Stop)
    }

    async fn on_leave(host_client: &mut Client, guest: &mut Member<Guest>) {
        guest.secret = None;
        let _ = host_client.notify(Notification::OpponentLeave).await;
    }

    async fn handle(
        mut guest_client: Client,
        host_client: &mut Client,
        result: ClientListenResult,
        guest: &mut Member<Self>,
    ) {
        use Directive::*;

        match result {
            Ok(directive) => match directive {
                SetSecret { secret } => {
                    let _ = guest_client.notify(Notification::SecretSet { secret: &secret }).await;
                    guest.secret = Some(secret);
                    guest.listener.attach(guest_client);
                },
                Leave => {
                    Self::on_leave(host_client, guest).await;
                    Idler::spawn(guest_client);
                }
                CloseConnection => Self::on_leave(host_client, guest).await,
                _ => guest.listener.attach(guest_client),
            },
            Err(ClientListenError::SocketExhausted) => {
                Self::on_leave(host_client, guest).await
            }
            _ => guest.listener.attach(guest_client),
        }
    }
}
