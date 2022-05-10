use log::debug;
use crate::{
    Directive,
    lobby::Lobby,
    client::{Client, ClientListenError, ClientListener, ClientListenerState},
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

    async fn listen(mut self) {
        debug!("Listening to an idle client");

        while let Some(mut client) = self.take() {
            use Directive::*;

            match client.listen().await {
                Ok(directive) => match directive {
                    // Because the client is moved, the state remains `Stop`
                    // for the two arms below
                    CreateLobby => Lobby::spawn(client),
                    JoinLobby { lobby_id } => Lobby::send(lobby_id, client).await,

                    // The state remains `Stop` so the client gets dropped.
                    CloseConnection => {}

                    // Continue listening only if the directive is ignored.
                    _ => self.attach(client),
                },

                // Cannot read the socket, the state remains `Stop`,
                // so the client gets dropped.
                Err(ClientListenError::SocketExhausted) => {}

                // Continue listening
                _ => self.attach(client),
            }
        }

        debug!("An idler listener dropped");
    }
}
