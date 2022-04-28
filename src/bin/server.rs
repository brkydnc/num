#![feature(once_cell)]
#![feature(entry_insert)]

use bmrng::error::RequestError;
use futures_util::future::{BoxFuture, FutureExt};
use num::{
    client::{Client, ClientListenError},
    event::EventKind,
    id::{Id, IdGenerator},
    lobby::{Lobby, Sender},
};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    sync::{Arc, RwLock},
};
use tokio::net::{TcpListener, TcpStream};
use log::{info, error};

type LobbyIndex = Arc<RwLock<HashMap<Id, Sender>>>;

static LOBBIES: SyncLazy<LobbyIndex> = SyncLazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static ID_GENERATOR: SyncLazy<IdGenerator> = SyncLazy::new(|| IdGenerator::new());

async fn handle_new_connection(tcp_stream: TcpStream) {
    match tokio_tungstenite::accept_async(tcp_stream).await {
        Ok(socket) => {
            let client = Client::new(socket);
            tokio::spawn(handle_idle_client(client));
        },
        Err(cause) => {
            error!("Couldn't upgrade connection to websocket: {:?}", cause);
        }
    };
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

                        let sender = Lobby::new(client)
                            .spawn(on_destroyed, on_client_release);

                        LOBBIES
                            .try_write()
                            .expect("Error acquiring the lobby index lock")
                            .entry(id)
                            .insert_entry(sender);

                        break;
                    }
                    EventKind::JoinLobby => {
                        let parsed_id = event.data.map(|data| data.parse::<Id>().ok()).flatten();

                        if let Some(id) = parsed_id {
                            let client_sender = {
                                let lobbies = LOBBIES
                                    .try_read()
                                    .expect("Error acquiring the lobby index lock");

                                lobbies.get(&id).cloned()
                            };

                            if let Some(sender) = client_sender {
                                client = match sender.send_receive(client).await {
                                    Ok(response) => match response {
                                        Ok(_) => break,
                                        Err(client) => client,
                                    },
                                    Err(request_error) => match request_error {
                                        RequestError::SendError(client) => client,
                                        _ => unreachable!(),
                                    },
                                };
                            }
                        }
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

const ADDRESS: &'static str = "127.0.0.1:7878";

#[tokio::main]
async fn main() {
    env_logger::init();

    let listener = TcpListener::bind(ADDRESS)
        .await
        .expect("Error binding to address");

    info!("Listening address {}", ADDRESS);
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_new_connection(stream));
    }
}
