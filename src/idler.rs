use crate::{
    client::{Client, ListenError, Listener, ListenerState},
    Directive, Lobby,
};
use log::debug;

pub struct Idler(ListenerState);

impl Listener for Idler {
    fn state(&self) -> &ListenerState {
        &self.0
    }

    fn state_mut(&mut self) -> &mut ListenerState {
        &mut self.0
    }
}

impl Idler {
    pub fn spawn(client: Client) {
        let listener = Self(ListenerState::Listen(client));
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
                Err(ListenError::SocketExhausted) => {}

                // Continue listening
                _ => self.attach(client),
            }
        }

        debug!("An idler listener dropped");
    }
}
