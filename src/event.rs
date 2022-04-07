use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
#[non_exhaustive]
pub enum EventKind {
    CloseConnection,
    CreateLobby,
    JoinLobby,
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl Event {
    pub fn new(kind: EventKind, data: Option<String>) -> Self {
        Self { kind, data }
    }
    pub fn kind(&self) -> EventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&String> {
        self.data.as_ref()
    }
}
