use crate::{
    client::{Client, ListenError, ListenResult, Listener, ListenerState, Bundle},
    Directive, Game, Idler, Notification, Player, Secret,
};
use futures_util::future::OptionFuture;
use log::{debug, warn};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
};
use tokio::{
    select,
    sync::mpsc::{channel, Receiver, Sender},
};

pub type LobbyId = usize;
type LobbyIndex = Arc<RwLock<HashMap<LobbyId, Sender<Client>>>>;

static LOBBIES: SyncLazy<LobbyIndex> = SyncLazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub struct Lobby {
    id: LobbyId,
    host: Host,
    guest: Guest,
}

impl Lobby {
    fn new(creator: Client) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ID.fetch_add(1, Ordering::Relaxed),
            host: Host::new(creator),
            guest: Guest::new(),
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

        let _ = self
            .host
            .client_mut()
            .unwrap()
            .notify(Notification::LobbyCreation { lobby_id: self.id })
            .await;

        // A lobby is guaranteed to have a host connected. Therefore the lobby
        // task must live as long as the host is being listened. A `while let`
        // is handy for this case.
        while let Some(mut host) = self.host.bundle() {
            // A guest may or may not to be connected. Also, a guest must be
            // listened with a host concurrently. Thus, they must be put into the
            // same select body. The `OptionFuture` utility allows listening a
            // possibly connected guest along with the host. If there is a guest,
            // and the guest sends a directive, the future below will return
            // Some(result).
            let guest_listen_future: OptionFuture<_> = self
                .guest
                .client_mut()
                .map(|client| client.listen())
                .into();

            // Here, the `host_client` has already been moved from its listener,
            // and the `guest_client` will be moved if it is relevant.
            //
            // Handler functions will attach the clients to the appropriate listeners
            // if necessarry. So no need to attach them here by hand.
            select! {
                result = host.client.listen() => {
                    Host::handle(result, host, &mut self.guest).await;
                }
                Some(result) = guest_listen_future => {
                    let guest_bundle = self.guest.bundle().unwrap();
                    Guest::handle(result, guest_bundle, &mut host.client).await;
                    host.reunite();
                }
                Some(mut client) = receiver.recv() => {
                    // If there is already a guest, spawn an idle handler for
                    // the incoming client.
                    if self.guest.is_listening() {
                        Idler::spawn(client);
                        debug!("Guest join rejected, the lobby is full");
                    } else {
                        let _ = tokio::join!{
                            host.client.notify(Notification::GuestJoin),
                            client.notify(Notification::LobbyJoin { lobby_id: self.id }),
                        };

                        self.guest.attach(client);
                        host.reunite();

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

struct Host {
    state: ListenerState,
    secret: Option<Secret>,
}

impl Listener for Host {
    fn state(&self) -> &ListenerState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ListenerState {
        &mut self.state
    }
}

impl Host {
    fn new(client: Client) -> Self {
        Self {
            state: ListenerState::Listen(client),
            secret: None,
        }
    }

    async fn on_start_game(host: Bundle<'_, Host>, guest: &mut Guest) {
        if !(host.listener.secret.is_some() && guest.secret.is_some()) {
            return host.reunite();
        }

        if let Some(guest_client) = guest.take() {
            let host = Player::new(host.client, host.listener.secret.take().unwrap());
            let guest = Player::new(guest_client, guest.secret.take().unwrap());

            Game::spawn(host, guest);
        } else {
            host.reunite();
        }
    }

    async fn on_leave(host: &mut Host, guest: &mut Guest) {
        // When the host leaves, if there is a guest, the guest becomes the host.
        if let Some(mut client) = guest.take() {
            let _ = client.notify(Notification::OpponentLeave).await;

            // Attach guest's listener to the host member.
            host.attach(client);

            // Move guest's secret to the host.
            host.secret = guest.secret.take();
        }
    }

    async fn handle(
        result: ListenResult,
        mut host: Bundle<'_, Host>,
        guest: &mut Guest,
    ) {
        use Directive::*;

        match result {
            Ok(directive) => match directive {
                SetSecret { secret } => {
                    let _ = host.client
                        .notify(Notification::SecretSet { secret: &secret })
                        .await;

                    host.listener.secret = Some(secret);
                    host.reunite();
                }
                StartGame => Self::on_start_game(host, guest).await,
                Leave => {
                    Self::on_leave(host.listener, guest).await;
                    Idler::spawn(host.client);
                }
                CloseConnection => Self::on_leave(host.listener, guest).await,
                _ => host.reunite(),
            },
            Err(ListenError::SocketExhausted) => Self::on_leave(host.listener, guest).await,
            _ => host.reunite(),
        }
    }
}

struct Guest {
    state: ListenerState,
    secret: Option<Secret>,
}

impl Listener for Guest {
    fn state(&self) -> &ListenerState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ListenerState {
        &mut self.state
    }
}

impl Guest {
    fn new() -> Self {
        Self {
            state: ListenerState::Stop,
            secret: None,
        }
    }

    async fn on_leave(guest: &mut Guest, host: &mut Client) {
        guest.secret = None;
        let _ = host.notify(Notification::OpponentLeave).await;
    }

    async fn handle(
        result: ListenResult,
        mut guest: Bundle<'_, Guest>,
        host: &mut Client,
    ) {
        use Directive::*;

        match result {
            Ok(directive) => match directive {
                SetSecret { secret } => {
                    let _ = guest.client
                        .notify(Notification::SecretSet { secret: &secret })
                        .await;

                    guest.listener.secret = Some(secret);
                    guest.reunite();
                }
                Leave => {
                    Self::on_leave(guest.listener, host).await;
                    Idler::spawn(guest.client);
                }
                CloseConnection => Self::on_leave(guest.listener, host).await,
                _ => guest.reunite(),
            },
            Err(ListenError::SocketExhausted) => Self::on_leave(guest.listener, host).await,
            _ => guest.reunite(),
        }
    }
}
