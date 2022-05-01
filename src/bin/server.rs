#![feature(once_cell)]
#![feature(entry_insert)]

use num::{
    client::{Client, ClientListenError, ClientListenerState, ClientListener},
    event::EventKind,
    lobby::Lobby,
    id::{Id, IdGenerator},
};
use std::{
    collections::HashMap,
    lazy::SyncLazy,
    sync::{Arc, RwLock},
};
use tokio::{
    sync::mpsc::Sender,
    net::{TcpListener, TcpStream}
};
use log::{info, error, warn};

type LobbyIndex = Arc<RwLock<HashMap<Id, Sender<Client>>>>;

static LOBBIES: SyncLazy<LobbyIndex> = SyncLazy::new(|| Arc::new(RwLock::new(HashMap::new())));
static ID_GENERATOR: SyncLazy<IdGenerator> = SyncLazy::new(|| IdGenerator::new());

struct Idler(ClientListenerState);

impl ClientListener for Idler {
    fn state(&self) -> &ClientListenerState {
        &self.0
    }

    fn state_mut(&mut self) -> &mut ClientListenerState {
        &mut self.0
    }
}

impl Idler {
    fn new(client: Client) -> Self {
        Self(ClientListenerState::Listen(client))
    }

    async fn spawn(mut self) {
        // Replace self.state with `Stop` temporarily.
        while let Some(mut client) = self.take() {
            match client.listen().await {
                Ok(event) => match event.kind {
                    // Because the client is moved, self.state remains `Stop`
                    // for the two arms below
                    EventKind::CreateLobby => Self::on_create_lobby(client).await,
                    EventKind::JoinLobby => Self::on_join_lobby(client, event.data).await,

                    // self.state remains `Stop` so the client gets dropped.
                    EventKind::CloseConnection => {},

                    // Continue listening only if the event is ignored.
                    _ => { self.attach(client) }
                },

                // Cannot read the socket, self.state remains `Stop`,
                // so the client gets dropped.
                Err(ClientListenError::SocketStreamExhausted) => {},

                // Continue listening
                _ => { self.attach(client) }
            }
        }
    }

    async fn on_create_lobby(client: Client) {
        let id = ID_GENERATOR.next();

        // LobbyIndex cleanup for the future destruction of the lobby.
        let on_destroyed = move || {
            LOBBIES
                .try_write()
                .expect("Error acquiring the lobby index lock")
                .remove(&id);
        };

        let sender = Lobby::new(client).spawn(on_destroyed, handle_idle_client);

        LOBBIES
            .try_write()
            .expect("Error acquiring the lobby index lock")
            .entry(id)
            .insert_entry(sender);
    }

    async fn on_join_lobby(client: Client, id_string: Option<String>) {
        // Parse the string into the corresponding id.
        let id_parse = id_string
            .map(|id| id.parse::<Id>().ok())
            .flatten();

        if let Some(id) = id_parse {
            // Try to acquire the Sender of the lobby of the corresponding id.
            let client_sender = {
                let lobbies = LOBBIES
                    .try_read()
                    .expect("Error acquiring the lobby index lock");

                lobbies.get(&id).cloned()
            };

            if let Some(sender) = client_sender {
                if let Err(error) = sender.send(client).await {
                    // If the send was unsuccessful, spawn an idle handler for
                    // the client.
                    handle_idle_client(error.0);

                    // This may be an unwanted behavior, so logging a warning
                    // might be a good indicator (for the future).
                    warn!("Couln't send the client through the lobby sender.");
                }
            }
        }
    }
}

fn handle_idle_client(client: Client) {
    tokio::spawn(Idler::new(client).spawn());
}

async fn handle_new_connection(tcp_stream: TcpStream) {
    match tokio_tungstenite::accept_async(tcp_stream).await {
        Ok(socket) => {
            let client = Client::new(socket);
            handle_idle_client(client);
        },
        Err(cause) => {
            error!("Couldn't upgrade connection to websocket: {:?}", cause);
        }
    };
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
