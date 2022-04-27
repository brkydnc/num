use crate::{
    client::{Client, ClientListenError},
    event::{Event, EventKind},
    seat::Seat,
    secret::Secret,
    game::Game,
};
use bmrng::{channel, RequestReceiver, RequestSender};
use tokio::{select, join};
use log::info;

pub type Sender = RequestSender<Client, Result<(), Client>>;
pub type Receiver = RequestReceiver<Client, Result<(), Client>>;

//TODO: There is always a host, there is no need to wrap the host `Client` with
// an `Option`.
pub struct Lobby {
    host: Seat,
    guest: Seat,
}

impl Lobby {
    fn new(creator: Client) -> Self {
        let mut lobby = Self {
            host: Seat::new(),
            guest: Seat::new(),
        };
        lobby.host.occupy(creator);

        lobby
    }

    fn is_full(&self) -> bool {
        self.host.is_occupied() && self.guest.is_occupied()
    }

    fn is_empty(&self) -> bool {
        self.host.is_empty() && self.guest.is_empty()
    }

    fn join(&mut self, client: Client) {
        if self.host.is_empty() {
            self.host.occupy(client);
        } else if self.guest.is_empty() {
            self.guest.occupy(client);
        }
    }

    fn ready_to_play(&self) -> bool {
        self.host.ready_to_play() && self.guest.ready_to_play()
    }

    async fn handle_lobby<D, R>(
        mut lobby: Lobby,
        mut receiver: Receiver,
        on_destroyed: D,
        on_client_release: R,
    ) where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
    {
        info!("A lobby handler has just been spawned");
        lobby.host.emit(&Event::from(EventKind::CreateLobby)).await;

        loop {
            select! {
                Ok((mut client, responder)) = receiver.recv(), if !lobby.is_empty() => {
                    if lobby.is_full() {
                        let _ = responder.respond(Err(client));
                    } else {
                        let guest_join_event = Event::from(EventKind::GuestJoin);
                        let notify_host = lobby.host.emit(&guest_join_event);

                        let join_lobby_event = Event::from(EventKind::JoinLobby);
                        let notify_guest = client.emit(&join_lobby_event);

                        let _ = join!(notify_host, notify_guest);

                        lobby.join(client);
                        let _ = responder.respond(Ok(()));

                        join!();
                    }
                },
                Some(listen_result) = lobby.host.listen() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecret => {
                                    if let Some(string) = event.data {
                                        match Secret::parse(string) {
                                            Some(secret_number) => {
                                                lobby.host.set_secret(secret_number);
                                                lobby.host.emit(&Event::from(EventKind::SetSecret)).await;
                                            },
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::StartGame => {
                                    if !lobby.ready_to_play() { continue; }

                                    let host = lobby.host.to_player();
                                    let guest = lobby.guest.to_player();

                                    Game::new(host, guest)
                                        .spawn_handler(on_client_release);

                                    break;
                                },
                                EventKind::Leave => {
                                    lobby.host.release(&on_client_release);
                                    lobby.host = lobby.guest;
                                    lobby.guest = Seat::new();

                                    lobby.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                                }
                                EventKind::CloseConnection => {
                                    lobby.host.empty();
                                    lobby.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            lobby.host.empty();
                            lobby.guest.emit(&Event::from(EventKind::OpponentLeave)).await;
                        },
                        _ => {},
                    }
                },
                Some(listen_result) = lobby.guest.listen() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecret => {
                                    if let Some(string) = event.data {
                                        match Secret::parse(string) {
                                            Some(secret_number) => {
                                                lobby.guest.set_secret(secret_number);
                                                lobby.guest.emit(&Event::from(EventKind::SetSecret)).await;
                                            }
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::Leave => {
                                    lobby.guest.release(&on_client_release);
                                    lobby.host.emit(&Event::from(EventKind::OpponentLeave)).await;
                                }
                                EventKind::CloseConnection => {
                                    lobby.guest.empty();
                                    lobby.host.emit(&Event::from(EventKind::OpponentLeave)).await;
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            lobby.guest.empty();
                            lobby.host.emit(&Event::from(EventKind::OpponentLeave)).await;
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

    pub fn spawn_handler<D, R>(creator: Client, on_destroyed: D, on_client_release: R) -> Sender
    where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
    {
        let (sender, receiver) = channel(2);

        tokio::spawn(Self::handle_lobby(
            Self::new(creator),
            receiver,
            on_destroyed,
            on_client_release,
        ));

        sender
    }
}
