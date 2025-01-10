use std::net::SocketAddr;

use bomberhans_lib::network::ClientPacket;

use crate::actor::{actor, Manager, Done};

pub enum Command {
    Request(ClientPacket, SocketAddr),
}

pub fn server(server_name: String) -> Manager<Command> {
    let mut server = Server { server_name };
    actor(move |message| server.handle_message(message))
}

struct Server {
    server_name: String,
}

impl Server {
    async fn handle_message(&mut self, command: Command) -> Done {
        match command {
            Command::Request(packet, addr) => self.handle_packet(packet, addr).await,
        }
        Done::NotDone
    }
    async fn handle_packet(&mut self, packet: ClientPacket, addr: SocketAddr) {
        todo!()
    }
}

//                Ok((received_bytes, client_address)) => {
//                    if let Some(packet) = decode::<ClientPacket>(&buf[..received_bytes]) {
//                        let response = server.handle_client_packet(packet, client_address);
//                        if let Some(response) = response {
//                            log::debug!("sending to {client_address}: {response:#?}");
//                            let data = encode(&response);
//                            socket.send_to(&data, client_address)?;
//                        }
//                    }
//                }
// PP
