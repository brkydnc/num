use crate::{
    client::{Client, ClientListenError},
    event::{Event, EventKind},
    secret::Secret,
};
use log::info;
use tokio::join;
use tokio::{
    select,
    time::{interval, Duration},
};

pub struct Player {
    pub(self) client: Client,
    pub(self) secret: Secret,
}

impl Player {
    pub fn new(client: Client, secret: Secret) -> Self {
        Self { client, secret }
    }
}

pub struct Game {
    host: Player,
    guest: Player,
    host_turn: bool,
}

impl Game {
    pub fn new(host: Player, guest: Player) -> Self {
        Self {
            host,
            guest,
            host_turn: true,
        }
    }

    pub fn spawn_handler<R>(mut self, on_client_release: R)
    where
        R: Fn(Client) + Send + 'static,
    {
        info!("A game handler has just been spawned");

        tokio::spawn(async move {
            let mut turn_swap_interval = interval(Duration::from_secs(20));

            loop {
                select! {
                    _ = turn_swap_interval.tick() => {
                        self.host_turn = !self.host_turn;
                    },
                    listen_result = self.host.client.listen() => {
                        match listen_result {
                            Ok(event) => {
                                match event.kind {
                                    EventKind::Guess => {
                                        if !self.host_turn { continue };

                                        // TODO: Notify guesses
                                        if let Some(string) = event.data {
                                            if let Some(guess) = Secret::parse(string) {
                                                let (correct, _wrong) = self.host.secret.score(&guess);

                                                // The winner is the host
                                                if correct == 3 {
                                                    on_client_release(self.host.client);
                                                    on_client_release(self.guest.client);
                                                    break;
                                                }

                                                self.host_turn = false;
                                                turn_swap_interval.reset();
                                            }
                                        }
                                    },
                                    EventKind::Leave => {
                                        let leave = Event::from(EventKind::Leave);
                                        let notify_leave = self.host.client.emit(&leave);

                                        let opponent_leave = Event::from(EventKind::OpponentLeave);
                                        let notify_opponent_leave = self.guest.client.emit(&opponent_leave);

                                        let _ = join!(notify_leave, notify_opponent_leave);

                                        on_client_release(self.host.client);
                                        on_client_release(self.guest.client);
                                        break;
                                    }
                                    EventKind::CloseConnection => {
                                        let opponent_leave = Event::from(EventKind::OpponentLeave);
                                        let _ = self.guest.client.emit(&opponent_leave).await;

                                        on_client_release(self.guest.client);
                                        break;
                                    },
                                    _ => {}
                                }
                            },
                            Err(ClientListenError::SocketStreamExhausted) => {
                                let opponent_leave = Event::from(EventKind::OpponentLeave);
                                let _ = self.guest.client.emit(&opponent_leave).await;

                                on_client_release(self.guest.client);
                                break;
                            },
                            _ => {},
                        }
                    },
                    listen_result = self.guest.client.listen() => {
                        match listen_result {
                            Ok(event) => {
                                match event.kind {
                                    EventKind::Guess => {
                                        if self.host_turn { continue };

                                        // TODO: Notify guesses
                                        if let Some(string) = event.data {
                                            if let Some(guess) = Secret::parse(string) {
                                                let (correct, _wrong) = self.host.secret.score(&guess);

                                                // The winner is the guest
                                                if correct == 3 {
                                                    on_client_release(self.host.client);
                                                    on_client_release(self.guest.client);
                                                    break;
                                                }

                                                self.host_turn = true;
                                                turn_swap_interval.reset();
                                            }
                                        }
                                    },
                                    EventKind::Leave => {
                                        let leave = Event::from(EventKind::Leave);
                                        let notify_leave = self.guest.client.emit(&leave);

                                        let opponent_leave = Event::from(EventKind::OpponentLeave);
                                        let notify_opponent_leave = self.host.client.emit(&opponent_leave);

                                        let _ = join!(notify_leave, notify_opponent_leave);

                                        on_client_release(self.host.client);
                                        on_client_release(self.guest.client);
                                        break;
                                    }
                                    EventKind::CloseConnection => {
                                        let opponent_leave = Event::from(EventKind::OpponentLeave);
                                        let _ = self.host.client.emit(&opponent_leave).await;

                                        on_client_release(self.host.client);
                                        break;
                                    },
                                    _ => {}
                                }
                            },
                            Err(ClientListenError::SocketStreamExhausted) => {
                                let opponent_leave = Event::from(EventKind::OpponentLeave);
                                let _ = self.host.client.emit(&opponent_leave).await;

                                on_client_release(self.host.client);
                                break;
                            },
                            _ => {},
                        }
                    },
                }
            }
        });

        info!("A game handler has just been destroyed");
    }
}
