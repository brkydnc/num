use crate::{Directive, Notification};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::{Error as TungsteniteError, Message};

pub type ListenResult = Result<Directive, ListenError>;

// TODO: The `WebSocket` type is about 300 bytes. And the code has a lot of
// move semantics. Maybe it is better putting the inner socket behind a `Box`.
pub struct Client {
    socket: WebSocketStream<TcpStream>,
}

pub enum ListenError {
    SocketExhausted,
    InvalidMessage,
    UnknownMessage,
    InvalidDirective,
}

impl Client {
    pub fn new(socket: WebSocketStream<TcpStream>) -> Self {
        Self { socket }
    }

    pub async fn listen(&mut self) -> ListenResult {
        use ListenError::*;

        let message = self
            .socket
            .next()
            .await
            .ok_or(SocketExhausted)?
            .or(Err(InvalidMessage))?;

        match message {
            Message::Text(ref text) => serde_json::from_str(text).or(Err(InvalidDirective)),
            Message::Close(_) => Ok(Directive::CloseConnection),
            _ => Err(UnknownMessage),
        }
    }

    pub async fn notify(&mut self, n: Notification<'_>) -> Result<(), TungsteniteError> {
        let json = serde_json::to_string(&n).expect("Couldn't parse notification to json");

        self.socket.send(Message::Text(json)).await
    }
}

/// A utility enum type that wraps a client to be listened.
///
/// The purpose of this enum is to wrap a client like an `Option` does, and to
/// be used in the contextes which involve listening to a `Client`.
pub enum ListenerState {
    Listen(Client),
    Stop,
}

/// A utility interface for manuplating the types that contain `ListenerState`.
pub trait Listener {
    /// Returns a reference to the inner ListenerState.
    fn state(&self) -> &ListenerState;

    /// Returns a mutable reference to the inner ListenerState.
    fn state_mut(&mut self) -> &mut ListenerState;

    /// Return true if the state is `Listen(Client)`
    fn is_listening(&self) -> bool {
        matches!(self.state(), ListenerState::Listen(_))
    }

    /// Wraps the new client with `Listen`, drops the old one if it exists.
    fn attach(&mut self, client: Client) {
        let _ = std::mem::replace(self.state_mut(), ListenerState::Listen(client));
    }

    /// Returns the current client if the current state is `Listen(Client)`,
    /// and replaces the state with `Stop`.
    fn take(&mut self) -> Option<Client> {
        let state = std::mem::replace(self.state_mut(), ListenerState::Stop);

        match state {
            ListenerState::Listen(client) => Some(client),
            ListenerState::Stop => None,
        }
    }

    /// If a client is being listened, returns the bundle of the client and a
    /// mutable reference to the listener, replaces the listener state with `Stop`.
    fn bundle(&mut self) -> Option<Bundle<Self>>
    where
        Self: Sized,
    {
        self.take().map(|client| Bundle {
            listener: self,
            client,
        })
    }

    /// If a client is being listened, this method returns a mutable reference
    /// to the client.
    fn client_mut(&mut self) -> Option<&mut Client> {
        match self.state_mut() {
            ListenerState::Listen(client) => Some(client),
            ListenerState::Stop => None,
        }
    }
}

/// Bundles a client with its listener. Since the client is moved into the bundle,
/// the state of the listener is `Stop`. Later the client can be attached to the
/// listener again.
pub struct Bundle<'l, L: Listener> {
    pub listener: &'l mut L,
    pub client: Client,
}

impl<L: Listener> Bundle<'_, L> {
    /// Attaches the client to the listener.
    pub fn reunite(self) {
        self.listener.attach(self.client);
    }
}
