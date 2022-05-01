use log::warn;
use crate::{
    client::{Client, ClientListenError, ClientListenerState, ClientListener},
    event::EventKind,
    lobby::{Lobby, LOBBIES, Id},
};

pub struct Idler(ClientListenerState);

impl ClientListener for Idler {
    fn state(&self) -> &ClientListenerState {
        &self.0
    }

    fn state_mut(&mut self) -> &mut ClientListenerState {
        &mut self.0
    }
}

impl Idler {
    pub fn spawn(client: Client) {
        let listener = Self(ClientListenerState::Listen(client));
        tokio::spawn(listener.listen());
    }

    async fn on_create_lobby(client: Client) {
        let sender = Lobby::spawn(client);
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
                    Self::spawn(error.0);

                    // This may be an unwanted behavior, so logging a warning
                    // might be a good indicator (for the future).
                    warn!("Couln't send the client through the lobby sender.");
                }
            }
        }
    }

    async fn listen(mut self) {
        while let Some(mut client) = self.take() {
            match client.listen().await {
                Ok(event) => match event.kind {
                    // Because the client is moved, the state remains `Stop`
                    // for the two arms below
                    EventKind::CreateLobby => Self::on_create_lobby(client).await,
                    EventKind::JoinLobby => Self::on_join_lobby(client, event.data).await,

                    // The state remains `Stop` so the client gets dropped.
                    EventKind::CloseConnection => {},

                    // Continue listening only if the event is ignored.
                    _ => { self.attach(client) }
                },

                // Cannot read the socket, the state remains `Stop`,
                // so the client gets dropped.
                Err(ClientListenError::SocketStreamExhausted) => {},

                // Continue listening
                _ => { self.attach(client) }
            }
        }
    }
}

