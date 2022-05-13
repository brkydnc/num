use crate::{LobbyId, Secret};
use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Directive {
    CloseConnection,
    CreateLobby,
    JoinLobby { lobby_id: LobbyId },
    Leave,
    SetSecret { secret: Secret },
    StartGame,
    Guess { secret: Secret },
}

#[non_exhaustive]
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Notification<'a> {
    LobbyCreate { lobby_id: LobbyId },
    LobbyJoin { lobby_id: LobbyId },
    SecretSet { secret: &'a Secret },
    GuestJoin,
    OpponentLeave,
    GameStart,
    NextTurn,
    GuessScore { correct: u8, wrong: u8 },
    Win,
    Lose
}
