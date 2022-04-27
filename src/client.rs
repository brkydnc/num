use crate::event::{Event, EventKind};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::{Message, Error as TungsteniteError };

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
            Message::Close(_) => Ok(Event::from(EventKind::CloseConnection)),
            _ => Err(ClientListenError::UnknownEvent),
        }
    }

    pub async fn emit(&mut self, event: &Event) -> Result<(), TungsteniteError> {
        let json = serde_json::to_string(event)
            .expect("Couldn't parse event into json string");

        self.socket.send(Message::Text(json)).await
    }
}
