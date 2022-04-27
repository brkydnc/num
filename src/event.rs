use serde::{Deserialize, Serialize};
use serde_repr::*;

// TODO: Most of the events do not contain any data. And their serialized forms
// are constants. Find a way to eliminate the extra effort to serialzie
// the events.

// TODO: Client-sent events are mostly directives, and server-sent events are
// notify-oriented. Maybe separating the events as `Directive`s and `Notification`s
// help simplify the code.
#[derive(Clone, Copy, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
#[non_exhaustive]
pub enum EventKind {
    // Directives?
    CloseConnection,
    CreateLobby,
    JoinLobby,
    Leave,
    SetSecret,
    StartGame,
    Guess,

    // Notifications?
    GuestJoin,
    OpponentLeave,
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
