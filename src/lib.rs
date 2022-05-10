#![feature(once_cell)]
#![feature(entry_insert)]

pub mod client;
pub mod game;
pub mod idler;
pub mod lobby;
pub mod secret;
pub mod message;

pub use message::{Directive, Notification};
