#![feature(once_cell)]
#![feature(entry_insert)]


use futures_util::future::{BoxFuture, FutureExt};
use tokio::{
    sync::mpsc::Sender,
    net::{TcpListener, TcpStream}
};
use tungstenite::Error as TungsteniteError;
use num::{
    id::{Id, IdGenerator},
    client::{Client, ClientListenError},
    event::EventKind,
    lobby::Lobby,
};
use std::{
    sync::{Arc, RwLock},
    lazy::SyncLazy,
    collections::HashMap,
};

type LobbyIndex = Arc<RwLock<HashMap<Id, Sender<Client>>>>;

static LOBBIES: SyncLazy<LobbyIndex> = SyncLazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static ID_GENERATOR: SyncLazy<IdGenerator> = SyncLazy::new(|| IdGenerator::new());

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
                        let id = ID_GENERATOR.next();

                        let on_destroyed = move || {
                            LOBBIES
                                .try_write()
                                .expect("Error acquiring the lobby index lock")
                                .remove(&id);
                        };

                        let on_client_release = |client| {
                            tokio::spawn(handle_idle_client(client));
                        };

                        let sender = Lobby::spawn_handler(on_destroyed, on_client_release);

                        // The error contains the same socket.
                        if let Err(error) = sender.send(client).await {
                            client = error.0;
                        } else {
                            LOBBIES
                                .try_write()
                                .expect("Error acquiring the lobby index lock")
                                .entry(id)
                                .insert_entry(sender);

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
