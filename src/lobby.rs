use crate::{
    client::{Client, ClientListenError},
    event::EventKind,
    seat::Seat,
    secret::SecretNumber,
};
use bmrng::{channel, RequestReceiver, RequestSender};
use tokio::select;

pub type Sender = RequestSender<Client, Result<(), Client>>;
pub type Receiver = RequestReceiver<Client, Result<(), Client>>;

pub struct Lobby {
    clients: (Seat, Seat),
}

impl Lobby {
    fn new(creator: Client) -> Self {
        let mut lobby = Self {
            clients: (Seat::new(), Seat::new()),
        };
        lobby.clients.0.occupy(creator);

        lobby
    }

    fn is_full(&self) -> bool {
        self.clients.0.is_occupied() && self.clients.1.is_occupied()
    }

    fn is_empty(&self) -> bool {
        !self.clients.0.is_occupied() && !self.clients.1.is_occupied()
    }

    fn join_client(&mut self, client: Client) {
        if !self.clients.0.is_occupied() {
            self.clients.0.occupy(client);
        } else if !self.clients.1.is_occupied() {
            self.clients.1.occupy(client);
        }
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
        loop {
            select! {
                Ok((client, responder)) = receiver.recv(), if !lobby.is_empty() => {
                    if lobby.is_full() {
                        let _ = responder.respond(Err(client));
                    } else {
                        lobby.join_client(client);
                        let _ = responder.respond(Ok(()));
                    }
                },
                Some(listen_result) = lobby.clients.0.listen_if_occupied() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecretNumber => {
                                    if let Some(string) = event.data {
                                        match SecretNumber::parse(string) {
                                            Some(secret_number) => lobby.clients.0.set_secret_number(secret_number),
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::StartGame => {
                                    if !lobby.is_full() { continue; }
                                },
                                EventKind::LeaveLobby => {
                                    lobby.clients.0.release_if_occupied(&on_client_release);
                                    lobby.clients.0 = lobby.clients.1;
                                    lobby.clients.1 = Seat::new();
                                }
                                EventKind::CloseConnection => {
                                    lobby.clients.0.empty();
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            lobby.clients.0.empty();
                        },
                        _ => {},
                    }
                },
                Some(listen_result) = lobby.clients.1.listen_if_occupied() => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::SetSecretNumber => {
                                    if let Some(string) = event.data {
                                        match SecretNumber::parse(string) {
                                            Some(secret_number) => lobby.clients.1.set_secret_number(secret_number),
                                            None => continue,
                                        }
                                    }
                                },
                                EventKind::LeaveLobby => {
                                    lobby.clients.1.release_if_occupied(&on_client_release);
                                }
                                EventKind::CloseConnection => {
                                    lobby.clients.1.empty();
                                },
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => {
                            lobby.clients.1.empty();
                        },
                        _ => {},
                    }
                },
                else => { break; },
            }
        }

        on_destroyed();
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
