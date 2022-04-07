use crate::event::{Event, EventKind};
use futures_util::stream::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::Message;

pub type WebSocket = WebSocketStream<TcpStream>;

pub struct Client {
    pub socket: WebSocket,
}

pub enum ClientListenError {
    SocketStreamExhausted,
    MessageRead,
    EventParse,
    UnknownEvent,
}

impl Client {
    pub fn new(socket: WebSocket) -> Self {
        Self { socket }
    }

    pub async fn listen(&mut self) -> Result<Event, ClientListenError> {
        let message_read = self
            .socket
            .next()
            .await
            .ok_or(ClientListenError::SocketStreamExhausted)?;
        let message = message_read.or(Err(ClientListenError::MessageRead))?;

        match message {
            Message::Text(ref text) => {
                serde_json::from_str::<Event>(text).or(Err(ClientListenError::EventParse))
            }
            Message::Close(_) => Ok(Event::new(EventKind::CloseConnection, None)),
            _ => Err(ClientListenError::UnknownEvent),
        }
    }
}
