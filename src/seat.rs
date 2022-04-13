use crate::{
    client::{Client, ClientListenError},
    event::Event,
    secret::Secret,
};
use futures_util::future::OptionFuture;
use std::future::Future;

pub struct Seat {
    client: Option<Client>,
    secret: Option<Secret>,
}

impl Seat {
    pub fn new() -> Self {
        Self {
            secret: None,
            client: None,
        }
    }

    pub fn secret(&self) -> Option<&Secret> {
        self.secret.as_ref()
    }

    pub fn set_secret(&mut self, number: Secret) {
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

    pub fn is_empty(&self) -> bool {
        self.client.is_none()
    }

    pub fn listen(
        &mut self,
    ) -> OptionFuture<impl Future<Output = Result<Event, ClientListenError>> + '_> {
        self.client.as_mut().map(|client| client.listen()).into()
    }

    pub fn release<R>(&mut self, on_release: &R)
    where
        R: Fn(Client) + Send + 'static,
    {
        if let Some(client) = self.client.take() {
            self.secret = None;
            on_release(client);
        }
    }
}
