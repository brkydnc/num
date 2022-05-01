use crate::{
    client::{
        Client,
        ClientListener,
        ClientListenerState,
        ClientListenResult,
        ClientListenError,
    },
    event::{Event, EventKind},
    secret::Secret,
    game::{Game, Player},
};

use tokio::{
    sync::mpsc::{
        Sender,
        Receiver,
        channel
    },
    select,
    join,
};

use log::info;
use futures_util::future::OptionFuture;

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

    async fn on_set_secret(mut client: Client, member: &mut Member<Self>, data: &Option<String>) {
        // Parse the given string and set the new secret if possible.
        if let Some(string) = data.as_ref() {
            if let Some(secret) = Secret::parse(string) {
                member.secret = Some(secret);
                let _ = client.emit(&Event::from(EventKind::SetSecret)).await;
            }
        }

        // Since this event does not require the client to be moved elsewhere,
        // attach it to the member's listener, so that its events are received
        // afterwards.
        member.listener.attach(client);
    }

    async fn on_start_game(client: Client, host: &mut Member<Self>, guest: &mut Member<Guest>) {
        if !(host.secret.is_some() && guest.secret.is_some()) { return }

        if let Some(guest_client) = guest.listener.take() {
            let host = Player::new(client, host.secret.take().unwrap());
            let guest = Player::new(guest_client, guest.secret.take().unwrap());

            todo!("Emit start game event to members and spawn without callback release mechanism");
            // Game::new(host, guest).spawn_handler();
        }
    }

    async fn on_leave(host: &mut Member<Self>, guest: &mut Member<Guest>) {
        // When the host leaves, if there is a guest, the guest becomes the host.
        if let Some(client) = guest.listener.take() {
            // Attach guest's listener to the host member.
            host.listener.attach(client);

            // Move guest's secret to the host.
            host.secret = guest.secret.take();

            // The guest member is completely empty now.

            todo!("Emit host leave to the guest");
            // let _ = guest_client.emit(&Event::from(EventKind::OpponentLeave)).await;
        }
    }

    async fn handle(client: Client, result: ClientListenResult, host: &mut Member<Self>, guest: &mut Member<Guest>) {
        match result {
            Ok(event) => {
                match event.kind {
                    EventKind::SetSecret => Self::on_set_secret(client, host, &event.data).await,
                    EventKind::StartGame => Self::on_start_game(client, host, guest).await,
                    EventKind::Leave => {
                        Self::on_leave(host, guest).await;
                        todo!("Release client without callback release mechanism");
                    }
                    EventKind::CloseConnection => Self::on_leave(host, guest).await,
                    _ => { host.listener.attach(client) }
                }
            }
            Err(ClientListenError::SocketStreamExhausted) => Self::on_leave(host, guest).await,
            _ => { host.listener.attach(client) },
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

    async fn on_set_secret(mut client: Client, member: &mut Member<Self>, data: &Option<String>) {
        // Parse the given string and set the new secret if possible.
        if let Some(string) = data.as_ref() {
            if let Some(secret) = Secret::parse(string) {
                member.secret = Some(secret);
                let _ = client.emit(&Event::from(EventKind::SetSecret)).await;
            }
        }

        // Since this event does not require the client to be moved elsewhere,
        // attach it to the member's listener, so that its events are received
        // afterwards.
        member.listener.attach(client);
    }

    async fn on_leave(host_client: &mut Client, guest: &mut Member<Guest>) {
        guest.secret = None;
        
        // Since the guest client has been moved from the listener before,
        // there is no need to detach it.

        todo!("Emit guest leave to the host");
        // let _ = host_client.emit(&Event::from(EventKind::OpponentLeave)).await;
    }

    async fn handle(
        guest_client: Client,
        host_client: &mut Client,
        result: ClientListenResult,
        guest: &mut Member<Self>
    ) -> ()
    {
        match result {
            Ok(event) => {
                match event.kind {
                    EventKind::SetSecret => Self::on_set_secret(guest_client, guest, &event.data).await,
                    EventKind::Leave => {
                        Self::on_leave(host_client, guest).await;
                        todo!("Release client without callback release mechanism");
                    }
                    EventKind::CloseConnection => Self::on_leave(host_client, guest).await,
                    _ => { guest.listener.attach(guest_client) }
                }
            }
            Err(ClientListenError::SocketStreamExhausted) => Self::on_leave(host_client, guest).await,
            _ => { guest.listener.attach(guest_client) },
        }
    }
}

struct Member<L: ClientListener> {
    listener: L,
    secret: Option<Secret>,
}

impl<L: ClientListener> Member<L> {
    fn new(listener: L) -> Self {
        Self { listener, secret: None, }
    }
}

pub struct Lobby {
    host: Member<Host>,
    guest: Member<Guest>,
}

impl Lobby {
    pub fn new(creator: Client) -> Self {
        Self {
            host: Member::new(Host::new(creator)),
            guest: Member::new(Guest::new())
        }
    }

    async fn listen<D, R>(
        mut self,
        mut receiver: Receiver<Client>,
        on_destroyed: D,
        on_client_release: R,
    ) where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
    {
        info!("A lobby handler has just been spawned");
        // let _ = self.host.emit(&Event::from(EventKind::CreateLobby)).await;

        // A lobby is guaranteed to have a host connected. Therefore the lobby
        // task must live as long as the host is being listened. A `while let`
        // is handy for this case.
        while let Some(mut host_client) = self.host.listener.take() {
            // A guest may or may not to be connected. Also, a guest must be
            // listened with a host concurrently. Thus, they must be put into the
            // same select body. The `OptionFuture` utility allows listening a
            // possibly connected guest along with the host. If there is a guest,
            // and the guest sends an event, the future below will return Some(result).
            // 
            // The future accesses guest_client with a mutable reference, If the
            // guest_client was moved, and the `host_client` handlers get executed,
            // the handlers would try to access the guest_client via guest listener
            // (self.guest.listener), and there would be no client in it, since 
            // it was moved.
            let guest_listen_future: OptionFuture<_> = self.guest.listener
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
                }
                Some(client) = receiver.recv() => {
                    // If there is already a guest, spawn an idle handler for
                    // the incoming client.
                    if self.guest.listener.is_listening() {
                        on_client_release(client);
                    } else {
                        // let guest_join_event = Event::from(EventKind::GuestJoin);
                        // let notify_host = self.host.emit(&guest_join_event);

                        // let join_lobby_event = Event::from(EventKind::JoinLobby);
                        // let notify_guest = client.emit(&join_lobby_event);

                        // let _ = join!(notify_host, notify_guest);

                        self.guest.listener.attach(client);
                    }
                },
            }

        }

        on_destroyed();
        info!("A lobby handler has just been destroyed");
    }

    pub fn spawn<D, R>(self, on_destroyed: D, on_client_release: R) -> Sender<Client>
    where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
    {
        let (sender, receiver) = channel(1);

        tokio::spawn(self.listen(receiver, on_destroyed, on_client_release));

        sender
    }
}
