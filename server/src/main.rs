#![cfg_attr(
    debug_assertions,
    allow(
        dead_code,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_imports,
        unused_macros,
        unused_extern_crates,
        missing_docs,
    )
)]
use actor::Actor;
use std::future::Future;
use std::io::Write;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{channel, Sender};
use tokio::task::JoinHandle;

mod actor;
mod game;
mod server;

#[derive(Debug)]
struct Request {
    client_addr: SocketAddr,
    data: Box<[u8]>,
}

#[derive(Debug)]
struct Response {
    client_addr: SocketAddr,
    data: Box<[u8]>,
}

#[derive(Debug)]
struct Responder<'s> {
    socket: &'s UdpSocket,
}

impl<'s> Actor<Response> for Responder<'s> {
    async fn handle(&mut self, response: Response) {
        self.socket
            .send_to(response.data.as_ref(), response.client_addr);
    }

    async fn close(self) {
        // nothing to do
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, rec| {
            writeln!(
                buf,
                "{file}:{line}: {module} {args}",
                file = rec.file().unwrap(),
                line = rec.line().unwrap(),
                module = rec.module_path().unwrap(),
                args = rec.args()
            )
        })
        .init();
    log::info!("Running Bomberhans Server {}", bomberhans_lib::VERSION);

    let addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 4267); // TODO: make port / ip configurable
    let socket = tokio::net::UdpSocket::bind(addr);
    log::info!("Listening on {addr}");

    let responder_manager = sign_on(Responder { socket: &socket });

    let server_manager = sign_on(server::Server::new(
        "HansServer".to_string(),
        responder_manager.assistant(),
    ));

    loop {
        let buf = Box::<_>::new([0u8; 1024]);
        tokio::select! {
            _ =  tokio::signal::ctrl_c() => { break }

            (len, client_addr) = socket.recv(buf) => {
                server_manager.handle_message(Request{buf,len,client_addr}).await;
             }

        };
    }

    server_manager.close(); // close all games, sending nice disconnect messages to all clients.
}
