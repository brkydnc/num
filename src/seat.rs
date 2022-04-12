use crate::{
    client::{Client, ClientListenError},
    event::Event,
    secret::SecretNumber,
};
use futures_util::future::OptionFuture;
use std::future::Future;

pub struct Seat {
    client: Option<Client>,
    secret: Option<SecretNumber>,
}

impl Seat {
    pub fn new() -> Self {
        Self {
            secret: None,
            client: None,
        }
    }

    pub fn secret_number(&self) -> Option<&SecretNumber> {
        self.secret.as_ref()
    }

    pub fn set_secret_number(&mut self, number: SecretNumber) {
        self.secret = Some(number);
    }

    pub fn occupy(&mut self, client: Client) {
        self.client = Some(client);
        self.secret = None;
    }

    pub fn empty(&mut self) {
        self.client = None;
        self.secret = None;
    }

    pub fn take(&mut self) -> Option<Client> {
        self.secret = None;
        self.client.take()
    }

    pub fn is_occupied(&self) -> bool {
        self.client.is_some()
    }

    pub fn listen_if_occupied(
        &mut self,
    ) -> OptionFuture<impl Future<Output = Result<Event, ClientListenError>> + '_> {
        self.client.as_mut().map(|client| client.listen()).into()
    }

    pub fn release_if_occupied<R>(&mut self, on_release: &R)
    where
        R: Fn(Client) + Send + 'static,
    {
        if let Some(client) = self.client.take() {
            self.secret = None;
            on_release(client);
        }
    }
}
