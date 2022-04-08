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

    async fn handle_lobby<D, R>(mut receiver: Receiver<Client>, on_destroyed: D, on_client_release: R)
    where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
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

        on_destroyed()
    }

    pub fn spawn_handler<D, R>(on_destroyed: D, on_client_release: R) -> Sender<Client>
    where
        D: FnOnce() + Send + 'static,
        R: Fn(Client) + Send + 'static,
    {
        let (sender, receiver) = channel(2);
        tokio::spawn(Self::handle_lobby(receiver, on_destroyed, on_client_release));

        sender
    }
}
