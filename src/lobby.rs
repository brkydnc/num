use crate::{
    event::EventKind,
    client::{ClientListenError, Client}
};
use futures_util::future::OptionFuture;
use tokio::{
    select,
    sync::mpsc::{channel, Receiver, Sender},
};

pub struct Lobby {
    clients: (Option<Client>, Option<Client>),
}

impl Lobby {
    fn new() -> Self {
        Self { clients: (None, None) }
    }

    async fn handle_lobby<F>(mut receiver: Receiver<Client>, on_client_release: F)
    where
        F: Fn(Client) + Send + 'static,
    {
        let mut lobby = Self::new();

        loop {
            let client0_listen_future: OptionFuture<_> = lobby.clients.0
                .as_mut()
                .map(|c| c.listen()).into();

            select! {
                Some(client) = receiver.recv() => {
                    lobby.clients.0.replace(client);
                },
                Some(listen_result) = client0_listen_future => {
                    match listen_result {
                        Ok(event) => {
                            match event.kind {
                                EventKind::CloseConnection => break,
                                _ => {}
                            }
                        },
                        Err(ClientListenError::SocketStreamExhausted) => break,
                        _ => {},
                    }
                },
                else => {},
            }
        }
    }

    pub fn spawn_handler<F>(on_client_release: F) -> Sender<Client>
    where
        F: Fn(Client) + Send + 'static,
    {
        let (sender, receiver) = channel(2);
        tokio::spawn(Self::handle_lobby(receiver, on_client_release));

        sender
    }
}
