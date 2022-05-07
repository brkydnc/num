use crate::event::{Event, EventKind};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::{Error as TungsteniteError, Message};

pub type ClientListenResult = Result<Event, ClientListenError>;

// TODO: The `WebSocket` type is about 300 bytes. And the code has a lot of
// move semantics. Maybe it is better putting the inner socket behind a `Box`.
pub struct Client {
    pub socket: WebSocketStream<TcpStream>,
}

pub enum ClientListenError {
    SocketStreamExhausted,
    MessageRead,
    EventParse,
    UnknownEvent,
}

impl Client {
    pub fn new(socket: WebSocketStream<TcpStream>) -> Self {
        Self { socket }
    }

    pub async fn listen(&mut self) -> ClientListenResult {
        use ClientListenError::*;

        let message_read = self.socket.next().await.ok_or(SocketStreamExhausted)?;

        let message = message_read.or(Err(MessageRead))?;

        match message {
            Message::Text(ref text) => serde_json::from_str(text).or(Err(EventParse)),
            Message::Close(_) => Ok(Event::from(EventKind::CloseConnection)),
            _ => Err(UnknownEvent),
        }
    }

    pub async fn emit(&mut self, event: &Event) -> Result<(), TungsteniteError> {
        let json = serde_json::to_string(event).expect("Couldn't parse event into json string");

        self.socket.send(Message::Text(json)).await
    }
}

/// A utility enum type that wraps a client to be listened.
///
/// The purpose of this enum is to wrap a client like an `Option` does, and to
/// be used in the contextes which involve listening to a `Client`.
pub enum ClientListenerState {
    Listen(Client),
    Stop,
}

/// A utility interface for manuplating the types that contain `ClientListenerState`.
pub trait ClientListener {
    /// Returns a reference to the inner ClientListenerState.
    fn state(&self) -> &ClientListenerState;

    /// Returns a mutable reference to the inner ClientListenerState.
    fn state_mut(&mut self) -> &mut ClientListenerState;

    /// Return true if the state is `Listen(Client)`
    fn is_listening(&self) -> bool {
        matches!(self.state(), ClientListenerState::Listen(_))
    }

    /// Wraps the new client with `Listen`, drops the old one if it exists.
    fn attach(&mut self, client: Client) {
        let _ = std::mem::replace(self.state_mut(), ClientListenerState::Listen(client));
    }

    /// Returns the current client if the current state is `Listen(Client)`,
    /// and replaces the state with `Stop`.
    fn take(&mut self) -> Option<Client> {
        let state = std::mem::replace(self.state_mut(), ClientListenerState::Stop);

        match state {
            ClientListenerState::Listen(client) => Some(client),
            ClientListenerState::Stop => None,
        }
    }

    /// If a client is being listened, returns the bundle of the client and a
    /// mutable reference to the listener, replaces the listener state with `Stop`.
    fn bundle(&mut self) -> Option<ClientListenerBundle<Self>> where Self: Sized{
        self.take().map(|client| ClientListenerBundle { listener: self, client })
    }

    /// If a client is being listened, this method returns a mutable reference
    /// to the client.
    fn client_mut(&mut self) -> Option<&mut Client> {
        match self.state_mut() {
            ClientListenerState::Listen(client) => Some(client),
            ClientListenerState::Stop => None,
        }
    }
}

/// Bundles a client with its listener. Since the client is moved into the bundle,
/// the state of the listener is `Stop`. Later the client can be attached to the
/// listener again.
pub struct ClientListenerBundle<'l, L: ClientListener> {
    pub listener: &'l mut L,
    pub client: Client,
}

impl<L: ClientListener> ClientListenerBundle<'_, L> {
    /// Attaches the client to the listener.
    pub fn reunite(self) {
        self.listener.attach(self.client);
    }
}
