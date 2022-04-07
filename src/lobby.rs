use crate::client::Client;
use tokio::{
    select,
    sync::mpsc::{channel, Receiver, Sender},
};

pub struct Lobby;

impl Lobby {
    async fn handle_lobby<F>(mut receiver: Receiver<Client>, on_client_release: F)
    where
        F: Fn(Client) + Send + 'static,
    {
        loop {
            select! {
                Some(client) = receiver.recv() => {
                    on_client_release(client);
                    break
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
