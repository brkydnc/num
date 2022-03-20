use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::unbounded_channel,
};
use tungstenite::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:7878").await?;

    eprintln!("Listening...");
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let websocket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("Error during the websocket handshake occurred");

            eprintln!("CONNECTION");
            let (outgoing, mut incoming) = websocket.split();

            while let Some(result) = incoming.next().await {
                let message = result.map_err(|_| "Error unwrapping message")?;

                match message {
                    Message::Text(text) => {
                        eprintln!("MESSAGE: {}", text);
                    }
                    Message::Close(_) => {
                        eprintln!("DISCONNECTION");
                        break;
                    }
                    _ => {}
                }
            }

            Ok::<_, &str>(())
        });
    }

    Ok(())
}
