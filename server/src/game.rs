use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use bomberhans_lib::game_state::*;
use bomberhans_lib::network::*;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::*;

use crate::actor::Actor;

#[derive(Debug)]
struct Client {
    /// Client's Player Name
    pub name: String,

    /// The client's Address, send updates there
    pub address: SocketAddr,

    /// The player Id of the client
    pub player_id: PlayerId,

    /// The time of the most recent information the client acknowledged having
    pub last_acknowledge_time: GameTime,

    /// The time of the most recent communication with client
    pub last_package_received: Instant,
}

#[derive(Debug)]
pub enum Message {
    ClientRequest(Box<ClientPacket>, SocketAddr),
}

#[derive(Debug)]
struct StartedGame {
    game_state: Option<GameState>,
    updates: Vec<Update>,
    future_updates: Vec<Update>,
    old_updates: Vec<Update>,
}

#[derive(Debug)]
enum State {
    Lobby,
    Started(StartedGame),
}

#[derive(Debug)]
pub struct Game {
    state: State,
    host: SocketAddr,
    clients: HashMap<SocketAddr, Client>,
}

impl Game {
    pub fn new(host_address: SocketAddr) -> Self {
        let clients = HashMap::new();
        Self {
            state: State::Lobby,
            host: host_address,
            clients,
        }
    }

    fn handle_client_request(&self, packet: Box<ClientPacket>, client_address: SocketAddr) -> ! {
        match packet.as_ref()   {
// 
//            None => {
//                let host_client = Client {
//                    name: name,
//                    address: host_address,
//                    player_id: PlayerId(0),
//                    last_acknowledge_time: GameTime::new(),
//                    last_package_received: Instant::now(),
//                };
//                clients.insert(host_address, host_client);
//            }
// 
        }    ;
    }
}

impl Actor<Message> for Game {
    async fn handle(&mut self, message: Message) {
        match message {
            Message::ClientRequest(packet, client_address) => {
                self.handle_client_request(packet, client_address)
            }
        }
    }

    async fn close(self) {
        todo!()
    }
}
