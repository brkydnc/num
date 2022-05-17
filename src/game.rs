use crate::{
    client::{
        Client, ListenError, ListenResult, Listener, Bundle,
        ListenerState,
    },
    Notification, Directive, Idler, Secret,
};
use log::debug;
use tokio::{
    select,
    time::{interval, Duration, Interval},
};

pub struct Player {
    state: ListenerState,
    pub(self) secret: Secret,
}

impl Listener for Player {
    fn state(&self) -> &ListenerState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ListenerState {
        &mut self.state
    }
}

impl Player {
    pub fn new(client: Client, secret: Secret) -> Self {
        Self {
            state: ListenerState::Listen(client),
            secret,
        }
    }

    async fn on_leave(mut opponent: Bundle<'_, Self>) {
        // Notify the opponent that the player has left.
        let _ = opponent.client.notify(Notification::OpponentLeave).await;
        Idler::spawn(opponent.client);
    }

    async fn handle(
        result: ListenResult,
        mut player: Bundle<'_, Self>,
        mut opponent: Bundle<'_, Self>,
        can_guess: bool,
        turn: &mut Turn,
    ) {
        use Directive::*;

        match result {
            Ok(directive) => {
                match directive {
                    Guess { secret } => {
                        if can_guess {
                            let (correct, wrong) = opponent.listener.secret.score(&secret);

                            if correct == 3 {
                                let _ = tokio::join! {
                                    player.client.notify(Notification::Win),
                                    opponent.client.notify(Notification::Lose)
                                };

                                Idler::spawn(player.client);
                                Idler::spawn(opponent.client);

                                return;
                            } else {
                                let notification = Notification::GuessScore { correct, wrong };
                                let _ = player.client.notify(notification).await;
                                turn.next();
                            }
                        }

                        player.reunite();
                        opponent.reunite();
                    }
                    Leave => {
                        Idler::spawn(player.client);
                        Self::on_leave(opponent).await;
                    }
                    CloseConnection => {
                        Self::on_leave(opponent).await;
                    }
                    _ => {
                        player.reunite();
                        opponent.reunite();
                    }
                }
            }
            Err(ListenError::SocketExhausted) => {
                Self::on_leave(opponent).await;
            }
            _ => {
                player.reunite();
                opponent.reunite();
            }
        }
    }
}

pub struct Turn {
    record: bool,
    interval: Interval,
}

impl Turn {
    fn new(duration: u64) -> Self {
        Self {
            record: false,
            interval: interval(Duration::from_secs(duration)),
        }
    }

    fn next(&mut self) {
        self.record = !self.record;
    }

    fn of_host(&self) -> bool {
        self.record
    }

    fn of_guest(&self) -> bool {
        !self.record
    }

    async fn interval_tick(&mut self) {
        self.interval.tick().await;
    }
}

pub struct Game {
    host: Player,
    guest: Player,
    turn: Turn,
}

impl Game {
    pub fn spawn(host: Player, guest: Player) {
        let game = Self {
            host,
            guest,
            turn: Turn::new(20),
        };

        tokio::spawn(game.listen());
    }

    pub async fn listen(mut self) {
        debug!("Listening to player directives in a game");

        let _ = tokio::join! {
            self.host.client_mut().unwrap().notify(Notification::GameStart),
            self.guest.client_mut().unwrap().notify(Notification::GameStart)
        };

        while let (Some(mut host), Some(mut guest)) = (self.host.bundle(), self.guest.bundle()) {
            select! {
                _ = self.turn.interval_tick() => {
                    self.turn.next();

                    let _ = if self.turn.of_host() {
                        host.client.notify(Notification::NextTurn).await
                    } else {
                        guest.client.notify(Notification::NextTurn).await
                    };

                    host.reunite();
                    guest.reunite();
                },
                result = host.client.listen() => {
                    Player::handle(result, host, guest, self.turn.of_host(), &mut self.turn).await;
                },
                result = guest.client.listen() => {
                    Player::handle(result, guest, host, self.turn.of_guest(), &mut self.turn).await;
                },
            }
        }

        debug!("Dropping a game listener");
    }
}
