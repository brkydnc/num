use futures_util::future::{BoxFuture, FutureExt};
use num::{
    client::{Client, ClientListenError},
    event::EventKind,
    lobby::Lobby,
};
use tokio::net::{TcpListener, TcpStream};
use tungstenite::Error as TungsteniteError;

async fn handle_new_connection(tcp_stream: TcpStream) -> Result<(), TungsteniteError> {
    let socket = tokio_tungstenite::accept_async(tcp_stream).await?;
    let client = Client::new(socket);
    tokio::spawn(handle_idle_client(client));
    Ok(())
}

fn handle_idle_client(mut client: Client) -> BoxFuture<'static, ()> {
    async move {
        loop {
            match client.listen().await {
                Ok(event) => match event.kind {
                    EventKind::CreateLobby => {
                        let sender = Lobby::spawn_handler(|client| {
                            tokio::spawn(handle_idle_client(client));
                        });

                        // The error contains the same socket.
                        if let Err(error) = sender.send(client).await {
                            client = error.0;
                        } else {
                            break;
                        };
                    }
                    EventKind::CloseConnection => break,
                    _ => {}
                },
                Err(ClientListenError::SocketStreamExhausted) => break,
                _ => {}
            }
        }
    }
    .boxed()
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878")
        .await
        .expect("Error binding to address");

    eprintln!("Listening...");
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_new_connection(stream));
    }
}
