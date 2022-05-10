use serde::{Serialize, Deserialize};
use crate::{
    lobby::Id,
    secret::Secret,
};

#[non_exhaustive]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Directive {
    CloseConnection,
    CreateLobby,
    JoinLobby { lobby_id: Id },
    Leave,
    SetSecret { secret: Secret },
    StartGame,
    Guess { secret: Secret },
}

#[non_exhaustive]
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Notification<'a> {
    LobbyCreation { lobby_id: Id },
    LobbyJoin { lobby_id: Id },
    SecretSet { secret: &'a Secret },
    GuestJoin,
    OpponentLeave,
}
