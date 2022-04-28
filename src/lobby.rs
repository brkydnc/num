use crate::{
    client::{Client, ClientListenError},
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
use tungstenite::Error as TungsteniteError;
use futures_util::future::OptionFuture;
use std::future::Future;

struct Host {
    client: Client,
    secret: Option<Secret>,
}

impl Host {
    fn new(client: Client) -> Self {
        Self { client, secret: None }
    }

    fn is_ready(&self) -> bool {
        self.secret.is_some()
    }

    async fn listen(&mut self) -> Result<Event, ClientListenError> {
        self.client.listen().await
    }

    async fn emit(&mut self, event: &Event) -> Result<(), TungsteniteError> {
        self.client.emit(event).await
    }
}

struct Guest {
    pub client: Option<Client>,
    secret: Option<Secret>,
}

impl Guest {
    fn new() -> Self {
        Self { client: None, secret: None }
    }

    fn into_player(self) -> Player {
        let client = self.client.unwrap();
        let secret = self.secret.unwrap();

        Player::new(client, secret)
    }

    fn set_client(&mut self, client: Client) {
        self.client = Some(client);
        self.secret = None;
    }

    fn set_secret(&mut self, secret: Secret) {
        self.secret = Some(secret);
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    fn is_ready(&self) -> bool {
        self.client.is_some() && self.secret.is_some()
    }

    pub fn listen(
        &mut self,
    ) -> OptionFuture<impl Future<Output = Result<Event, ClientListenError>> + '_> {
        self.client.as_mut().map(|client| client.listen()).into()
    }

    pub fn emit<'a>(
        &'a mut self,
        event: &'a Event,
    ) -> OptionFuture<impl Future<Output = Result<(), TungsteniteError>> + 'a> {
        self.client.as_mut().map(|client| client.emit(event)).into()
    }
}

//TODO: There is always a host, there is no need to wrap the host `Client` with
// an `Option`.
pub struct Lobby {
    host: Host,
    guest: Guest,
}

impl Lobby {
    pub fn new(creator: Client) -> Self {
        Self {
            host: Host::new(creator),
            guest: Guest::new()
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
        let _ = self.host.emit(&Event::from(EventKind::CreateLobby)).await;

        loop {
            select! {
                Some(mut client) = receiver.recv() => {
                    // If there already is a guest, spawn an idle handler for
                    // the incoming client.
                    if self.guest.is_connected() {
                        on_client_release(client);
                    } else {
                        let guest_join_event = Event::from(EventKind::GuestJoin);
                        let notify_host = self.host.emit(&guest_join_event);

                        let join_lobby_event = Event::from(EventKind::JoinLobby);
                        let notify_guest = client.emit(&join_lobby_event);

                        let _ = join!(notify_host, notify_guest);

                        self.guest.set_client(client);

                        join!();
                    }
                },
                listen_result = self.host.listen() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecret => {
                                    if let Some(string) = event.data {
                                        match Secret::parse(string) {
                                            Some(secret_number) => {
                                                self.host.secret = Some(secret_number);
                                                let _ = self.host.emit(&Event::from(EventKind::SetSecret)).await;
                                            },
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::StartGame => {
                                    if !(self.host.is_ready() && self.guest.is_ready()) {
                                        continue;
                                    }

                                    let host = Player::new(self.host.client, self.host.secret.unwrap());
                                    let guest = self.guest.into_player();

                                    Game::new(host, guest)
                                        .spawn_handler(on_client_release);

                                    break;
                                },
                                EventKind::Leave => {
                                    if let Some(client) = self.guest.client {
                                        let host_client = self.host.client;

                                        self.host.client = client;
                                        self.host.secret = self.guest.secret;
                                        self.guest = Guest::new();

                                        on_client_release(host_client);
                                        self.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                                    } else {
                                        on_client_release(self.host.client);
                                        break;
                                    }
                                }
                                EventKind::CloseConnection => {
                                    if let Some(client) = self.guest.client {
                                        self.host.client = client;
                                        self.host.secret = self.guest.secret;
                                        self.guest = Guest::new();

                                        self.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                                    } else {
                                        break;
                                    }
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            if let Some(client) = self.guest.client {
                                self.host.client = client;
                                self.host.secret = self.guest.secret;
                                self.guest = Guest::new();
                                self.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                            } else {
                                break;
                            }
                        },
                        _ => {},
                    }
                },
                Some(listen_result) = self.guest.listen() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecret => {
                                    if let Some(string) = event.data {
                                        match Secret::parse(string) {
                                            Some(secret_number) => {
                                                self.guest.set_secret(secret_number);
                                                self.guest.emit(&Event::from(EventKind::SetSecret)).await;
                                            }
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::Leave => {
                                    on_client_release(self.guest.client.unwrap());
                                    self.guest.client = None;
                                    self.guest.secret = None;
                                    let _ = self.host.emit(&Event::from(EventKind::OpponentLeave)).await;
                                }
                                EventKind::CloseConnection => {
                                    self.guest.client = None;
                                    self.guest.secret = None;
                                    let _ = self.host.emit(&Event::from(EventKind::OpponentLeave)).await;
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            self.guest.client = None;
                            self.guest.secret = None;
                            let _ = self.host.emit(&Event::from(EventKind::OpponentLeave)).await;
                        },
                        _ => {},
                    }
                },
                else => { break; },
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
