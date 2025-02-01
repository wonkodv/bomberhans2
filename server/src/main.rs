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
use actor::launch;
use actor::Actor;
use bomberhans2_lib::network::{
    decode, encode, ClientPacket, PacketNumber, ServerMessage, ServerPacket,
    BOMBERHANS_MAGIC_NO_V1, MTU,
};
use std::future::Future;
use std::io::Write;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{channel, Sender};
use tokio::task::JoinHandle;

mod actor;
mod game;
mod server;

#[derive(Debug)]
struct Request {
    client_address: SocketAddr,
    packet: ClientPacket,
}

impl Request {
    pub fn response(&self, message: ServerMessage) -> Response {
        Response {
            client_addr: self.client_address,
            message,
            ack: Some(self.packet.packet_number),
        }
    }
}

#[derive(Debug)]
struct Response {
    client_addr: SocketAddr,
    message: ServerMessage,
    ack: Option<PacketNumber>,
}

#[derive(Debug)]
struct Responder<'s> {
    socket: &'s UdpSocket,
    packet_number: PacketNumber,
}
impl<'s> Responder<'s> {
    fn new(socket: &'s UdpSocket) -> Self {
        Self {
            socket,
            packet_number: PacketNumber::new(),
        }
    }
}

impl<'s> Actor<Response> for Responder<'s> {
    async fn handle(&mut self, response: Response) {
        let packet = ServerPacket {
            magic: BOMBERHANS_MAGIC_NO_V1,
            packet_number: self.packet_number.next(),
            ack_packet_number: response.ack,
            message: response.message,
        };
        log::trace!("Sending to {:?} packet {:?}", response.client_addr, packet);
        let data = encode(&packet);
        self.socket
            .send_to(&data, response.client_addr)
            .await
            .expect("can send bytes");
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
    log::info!(
        "Running Bomberhans Server {}, LogLevel {}",
        bomberhans2_lib::VERSION,
        log::max_level(),
    );

    let addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 4267); // TODO: make port / ip configurable
    let socket = tokio::net::UdpSocket::bind(addr)
        .await
        .expect("can bind socket");
    let socket = Box::leak(Box::new(socket));
    log::info!("Listening on {addr}");

    let responder_manager = launch(|a| Responder::new(socket));

    let server_manager = launch(|server_manager_assistant| {
        server::Server::new(
            "HansServer".to_owned(),
            responder_manager,
            server_manager_assistant,
        )
    });

    let mut buf = [0_u8; MTU];
    let mut interval = tokio::time::interval(Duration::from_millis(16));
    loop {
        tokio::select! {
            _ =  tokio::signal::ctrl_c() => { break }
            _ =  interval.tick() => {
                        server_manager.send(server::Message::Update).await;
            }

            result = socket.recv_from(&mut buf) => {
                let (len, client_address) = result.expect("can receive");
                if let Some(packet) = decode::<ClientPacket>(&buf[0..len]) {
                    if packet.magic == BOMBERHANS_MAGIC_NO_V1 {
                        log::trace!("handeling packet from {client_address}  {packet:?}");
                        server_manager.send(server::Message::Request(Request { client_address, packet })).await;
                    } else {
                        log::warn!("ignoring unknown protocol {client_address}  {packet:?}");
                    }


                }else {
                    log::warn!("ignoring unparsable data from {client_address:?}");

                };
            }

        };
    }

    server_manager.close_and_join().await; // close all games, sending nice disconnect messages to all clients.
}
