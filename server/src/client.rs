pub enum Command {}

pub fn client() -> Actor<Command> {}

pub struct Client {
    /// Session Cookie
    pub id: ClientId,

    /// Client's Player Name
    pub name: String,

    /// The client's Address, only accept packets from there, send updates there
    pub address: SocketAddr,

    /// The Client's Game if any
    game: Option<ClientGame>,

    /// Number of the most recent packet, that we have received
    last_received_packet_number: u32,

    /// The time of the most recent communication with client
    pub last_package_received: Instant,

    pub player_id: PlayerId,

    /// The time of the most recent information the client acknowledged having
    pub last_acknowledge_time: GameTime,
}

impl Client {
    async fn run(mut self) {
        
    }
}



impl crate::actor::Actor<Request> for client {
    async fn handle_message(&mut self, message: Message) {

                    if packet.packet_number <= client.last_received_packet_number {
                        log::warn!("ignoring out of order packet {packet:?}");
                        return None;
                    }

                    client.last_received_packet_number = packet.packet_number;
                    client.last_package_received = Instant::now();






    }
}
