use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Clone, Copy, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
#[non_exhaustive]
pub enum EventKind {
    CloseConnection,
    CreateLobby,
    JoinLobby,
    Leave,
    SetSecret,
    StartGame,
    Guess,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl Event {
    pub fn with(mut self, data: String) -> Self {
        self.data = Some(data);

        self
    }

    pub fn kind(&self) -> EventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&String> {
        self.data.as_ref()
    }
}

impl From<EventKind> for Event {
    fn from(kind: EventKind) -> Self {
        Self { kind, data: None }
    }
}
