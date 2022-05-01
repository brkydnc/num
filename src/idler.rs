use crate::{
    client::{Client, ClientListenError, ClientListener, ClientListenerState},
    event::EventKind,
    lobby::{Id, Lobby},
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

    async fn on_join_lobby(client: Client, id_string: Option<String>) {
        // Parse the string into the corresponding id.
        let id_parse = id_string.map(|id| id.parse::<Id>().ok()).flatten();

        if let Some(id) = id_parse {
            Lobby::send(id, client).await;
        }
    }

    async fn listen(mut self) {
        while let Some(mut client) = self.take() {
            match client.listen().await {
                Ok(event) => match event.kind {
                    // Because the client is moved, the state remains `Stop`
                    // for the two arms below
                    EventKind::CreateLobby => Lobby::spawn(client),
                    EventKind::JoinLobby => Self::on_join_lobby(client, event.data).await,

                    // The state remains `Stop` so the client gets dropped.
                    EventKind::CloseConnection => {}

                    // Continue listening only if the event is ignored.
                    _ => self.attach(client),
                },

                // Cannot read the socket, the state remains `Stop`,
                // so the client gets dropped.
                Err(ClientListenError::SocketStreamExhausted) => {}

                // Continue listening
                _ => self.attach(client),
            }
        }
    }
}
