use crate::{
    client::{Client, ClientListenError, ClientListenerState, ClientListener, ClientListenResult, ClientListenerBundle},
    event::EventKind,
    secret::Secret,
    idler::Idler,
};
use tokio::{
    select,
    time::{interval, Interval, Duration},
};

pub struct Player {
    state: ClientListenerState,
    pub(self) secret: Secret,
}

impl ClientListener for Player {
    fn state(&self) -> &ClientListenerState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ClientListenerState {
        &mut self.state
    }
}

impl Player {
    pub fn new(client: Client, secret: Secret) -> Self {
        Self {
            state: ClientListenerState::Listen(client),
            secret,
        }
    }

    // TODO: Notify leave.
    async fn on_leave(opponent: ClientListenerBundle<'_, Self>) {
        Idler::spawn(opponent.client);
    }

    async fn handle(
        result: ClientListenResult,
        player: ClientListenerBundle<'_, Self>,
        opponent: ClientListenerBundle<'_, Self>,
        can_guess: bool,
        turn: &mut Turn,
    ) {
        match result {
            Ok(event) => {
                match event.kind {
                    EventKind::Guess => {
                        if !can_guess {
                            player.reunite();
                            opponent.reunite();
                            return;
                        }

                        // TODO: Notify guesses

                        if let Some(string) = event.data {
                            if let Some(guess) = Secret::parse(string) {
                                let (correct, _wrong) = player.listener.secret.score(&guess);

                                // The winner is the host
                                if correct == 3 {
                                    Idler::spawn(player.client);
                                    Idler::spawn(opponent.client);
                                    return;
                                }

                                turn.next();
                            }
                        }
                    }
                    EventKind::Leave => {
                        Idler::spawn(player.client);
                        Self::on_leave(opponent).await;
                    }
                    EventKind::CloseConnection => {
                        Self::on_leave(opponent).await;
                    }
                    _ => {
                        player.reunite();
                        opponent.reunite();
                    }
                }
            }
            Err(ClientListenError::SocketStreamExhausted) => {
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
        let game = Self { host, guest, turn: Turn::new(20) };
        tokio::spawn(game.listen());
    }

    pub async fn listen(mut self) {
        while let (Some(mut host), Some(mut guest)) = (self.host.bundle(), self.guest.bundle()) {
            select! {
                _ = self.turn.interval_tick() => {
                    self.turn.next();
                },
                result = host.client.listen() => {
                    Player::handle(result, host, guest, self.turn.of_host(), &mut self.turn).await;
                },
                result = guest.client.listen() => {
                    Player::handle(result, guest, host, self.turn.of_guest(), &mut self.turn).await;
                },
            }
        }
    }
}
