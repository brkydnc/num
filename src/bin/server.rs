#![feature(once_cell)]
#![feature(entry_insert)]

use log::{error, info, debug};
use num::{client::Client, idler::Idler};
use tokio::net::{TcpListener, TcpStream};

async fn handle_new_connection(tcp_stream: TcpStream) {
    match tokio_tungstenite::accept_async(tcp_stream).await {
        Ok(socket) => {
            let client = Client::new(socket);
            Idler::spawn(client);
            debug!("Connection upgraded to websocket");
        }
        Err(cause) => {
            error!("Couldn't upgrade connection to websocket: {:?}", cause);
        }
    };
}

const ADDRESS: &'static str = "127.0.0.1:7878";

#[tokio::main]
async fn main() {
    env_logger::init();

    let listener = TcpListener::bind(ADDRESS)
        .await
        .expect("Error binding to address");

    info!("Listening to address {}", ADDRESS);
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_new_connection(stream));
        debug!("Received a new connection request")
    }
}
