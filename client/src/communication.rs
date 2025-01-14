///! A Client connects to a server by calling `connect()`.
///! This returns a `Connection` and creates a `ConnectionBackend`. A Tokio Task run's the
///! Backend.
use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;

use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::PacketNumber;
use bomberhans_lib::network::*;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::GameTime;
use bomberhans_lib::utils::PlayerId;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio::time::Instant;

/// Connect to `server`, annoncing the player wants to be called `player_name`.
pub fn connect(server: SocketAddr) -> Connection {
    let (commands_to_backend, commands_from_frontend) = tokio::sync::mpsc::channel::<Command>(2);
    let (events_to_frontend, events_from_backend) = tokio::sync::mpsc::channel::<Event>(2);
    {
        tokio::spawn(async move {
            let mut comm =
                ConnectionBackend::new(server, commands_from_frontend, events_to_frontend).await;
            comm.receive_commands_and_messages().await;
        });
    }

    Connection {
        commands_to_backend,
        events_from_backend,
        server,
    }
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub server_name: String,
    pub lobbies: Vec<(GameId, String)>,
}

#[derive(Debug)]
pub enum Event {
    /// Server sent List of games
    GameListUpdated(ServerInfo),

    /// Server sent Lobby Settings
    LobbyUpdated {
        settings: Settings,
        players: Vec<Player>,
        local_player_id: PlayerId,
    },

    /// Server sent Game Update
    Update(ServerUpdate),

    /// Server not reachable anymore
    // Disconnected,

    /// Communication Error
    Error(String),

    /// We know the Ping to the Server
    Ping(Duration),
}

#[derive(Debug)]
enum Command {
    /// Open new Lobby, as Player Name
    OpenLobby(String),

    /// Join a Lobby, as Player Name
    JoinLobby(GameId, String),

    /// Update the Settings of the Lobby we host
    UpdateSettings(Settings),

    /// Start the Game of the current Lobby
    Start,

    /// Set local Players action
    SetAction(GameTime, Action),

    /// Disconnect from Server
    Leave,
    // GetState(tokio::sync::oneshot::Sender<State>),
    //
}

/// Communication with one server
#[derive(Debug)]
pub struct Connection {
    /// Send commands to `CommunicationBackend`
    commands_to_backend: Sender<Command>,

    /// Receive Events from Backend
    events_from_backend: Receiver<Event>,

    pub server: SocketAddr,
}

impl Connection {
    pub async fn get_event(&mut self) -> Event {
        self.events_from_backend
            .recv()
            .await
            .expect("comm backend doesn't panic")
    }

    pub async fn open_new_lobby(&self, player_name: String) {
        self.commands_to_backend
            .send(Command::OpenLobby(player_name))
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn disconnect(&self) {
        self.commands_to_backend
            .send(Command::Leave)
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn update_settings(&self, settings: Settings) {
        self.commands_to_backend
            .send(Command::UpdateSettings(settings))
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn start(&self) {
        self.commands_to_backend
            .send(Command::Start)
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn set_action(&self, time: GameTime, action: Action) {
        self.commands_to_backend
            .send(Command::SetAction(time, action))
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn join_lobby(&self, game_id: GameId, player_name: String) {
        self.commands_to_backend
            .send(Command::JoinLobby(game_id, player_name))
            .await
            .expect("comm backend doesn't panic");
    }
}

fn message_timeout(message: &ClientMessage) -> Duration {
    let ms = match message {
        ClientMessage::GetLobbyList => 100,
        ClientMessage::OpenNewLobby(_) => 100,
        ClientMessage::JoinLobby(_, _) => 100,
        ClientMessage::UpdateLobbySettings(_, _) => 100,
        ClientMessage::LobbyReady(_) => 100,
        ClientMessage::GameStart(_) => 16,
        ClientMessage::GameUpdate(_) => 16,
        ClientMessage::Bye => 0,
        ClientMessage::Ping => 100,
    };
    Duration::from_millis(ms)
}

#[derive(Debug)]
struct ConnectionBackend {
    /// The server this connection is for
    server: SocketAddr,

    /// Channel to receive commands from `Communication`
    commands_from_frontend: Receiver<Command>,

    /// Channel to send events to Frontend
    events_to_frontend: Sender<Event>,

    /// Socket to send to server with
    socket: UdpSocket,

    /// Number of the packet we most recently sent
    last_sent_packet: PacketNumber,

    /// Number of the most recent packet, that we have received
    last_received_packet: PacketNumber,

    /// Time of the latest `GameState` that we received
    last_server_update: GameTime,

    /// The last Message we sent which has not been acknowledged, time it was sent and duration for
    /// resend
    unacknowledged_packet: Option<(ClientPacket, Instant, Duration)>,

    /// List of all sent packets for debugging
    sent_packets: Vec<(Instant, ClientPacket)>,

    /// List of all received packets for debugging
    received_packets: Vec<(Instant, ServerPacket)>,
}

/// Basic Sending and receiving
impl ConnectionBackend {
    /// Create a connection to a server
    async fn new(
        server: SocketAddr,
        commands_from_frontend: Receiver<Command>,
        events_to_frontend: Sender<Event>,
    ) -> Self {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
        let socket = UdpSocket::bind(addr)
            .await
            .expect("can bind local udp socket");
        socket
            .connect(server)
            .await
            .expect("can set socket's remote address");
        ConnectionBackend {
            server,
            commands_from_frontend,
            events_to_frontend,
            socket,
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            last_sent_packet: PacketNumber::new(),
            last_received_packet: PacketNumber::new(),
            unacknowledged_packet: None,
            client_id: None,
            last_server_update: GameTime::new(),
        }
    }

    async fn send_message(&mut self, message: ClientMessage) {
        log::debug!("Sending {message:#?}");
        let now = Instant::now();

        let message_timeout = message_timeout(&message);

        let packet = ClientPacket {
            magic: BOMBERHANS_MAGIC_NO_V1,
            packet_number: self.last_sent_packet.next(),
            message,
        };
        self.socket.send(&encode(&packet)).await.unwrap();

        self.sent_packets.push((now, packet.clone())); // TODO: remove
        self.unacknowledged_packet = Some((packet.clone(), now, message_timeout));
    }

    /// A message was not acknowledged in time
    async fn handle_timeout(&mut self) {
        let (packet, _, timeout) = self
            .unacknowledged_packet
            .take()
            .expect("a packet timed out");
        let now = Instant::now();
        self.unacknowledged_packet = Some((packet.clone(), now, timeout));
        self.socket.send(&encode(&packet)).await.unwrap();
    }

    fn decode_message(&mut self, data: &[u8]) -> Option<ServerPacket> {
        let Some(packet) = decode::<ServerPacket>(data) else {
            log::warn!("ignoring unparseable data: {data:?}");
            return None;
        };

        if packet.magic != BOMBERHANS_MAGIC_NO_V1 {
            log::warn!("ignoring unknown protocol {packet:?}");
            return None;
        }

        Some(packet)
    }
}

impl ConnectionBackend {
    async fn receive_commands_and_messages(&mut self) {
        self.send_message(ClientMessage::GetLobbyList).await;
        let mut buf = [0; 1024];
        loop {
            let timeout = if let Some((_, sent_time, timeout)) = self.unacknowledged_packet {
                Some(timeout - sent_time.elapsed())
            } else {
                None
            };

            tokio::select! {
                () = async { if let Some(timeout) = timeout { sleep(timeout).await } } => { self.handle_timeout().await }
                cmd = self.commands_from_frontend.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd) .await,

                        None  => { self.handle_command(Command::Leave).await;
                            return;
                        }

                    }
                }
                result = self.socket.recv(&mut buf) => {
                    match result {
                        Ok(len)=> {
                            if let Some(message) = self.decode_message(&buf[0..len]) {
                                self.handle_message(message).await;
                            }
                        }
                        Err(err) => {
                            self.send_event(Event::Error(format!("{err:?}"))).await;
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::Leave => {
                self.leave().await;
            }
            Command::JoinLobby(lobby_id, player_name) => {
                self.send_message(ClientMessage::JoinLobby(lobby_id, player_name))
                    .await;
            }
            Command::OpenLobby(player_name) => {
                self.send_message(ClientMessage::OpenNewLobby(player_name))
                    .await;
            }
            Command::UpdateSettings(settings) => {
                let client_id = self.client_id.expect("We have a client_id by now");
                self.send_message(ClientMessage::UpdateLobbySettings(client_id, settings))
                    .await;
            }
            Command::Start => {
                let client_id = self.client_id.expect("We have a client_id by now");
                self.send_message(ClientMessage::GameStart(client_id)).await;
            }
            Command::SetAction(time, action) => {
                let client_id = self.client_id.expect("We have a client_id by now");
                self.send_message(ClientMessage::GameUpdate(ClientUpdate {
                    client_id,
                    last_server_update: self.last_server_update,
                    current_player_action: action,
                    current_action_start_time: time,
                }))
                .await;
            }
        };
    }

    async fn send_event(&mut self, event: Event) {
        self.events_to_frontend
            .send(event)
            .await
            .expect("comm frontend not dropped");
    }

    async fn handle_message(&mut self, packet: ServerPacket) {
        self.last_received_packet = packet.packet_number;
        self.received_packets.push((Instant::now(), packet.clone()));

        if let Some((pending_ack_packet, sent_time, _timeout)) = self.unacknowledged_packet.as_ref()
        {
            if Some(pending_ack_packet.packet_number) == packet.ack_packet_number {
                log::trace!("Packet acked: {:?}", pending_ack_packet.packet_number);
                self.send_event(Event::Ping(sent_time.elapsed())).await;
                self.unacknowledged_packet = None;
            }
        };

        if packet.packet_number <= self.last_received_packet {
            log::trace!("ignoring out of order packet {packet:?}");
            return;
        }
        log::trace!("received {packet:?}");

        match packet.message {
            ServerMessage::LobbyList(msg) => {
                let server_info = ServerInfo {
                    server_name: msg.server_name,
                    lobbies: msg.lobbies,
                };
                log::info!(
                    "Received Server List from {} \"{}\", Lobbies: {}",
                    &self.server,
                    &server_info.server_name,
                    server_info.lobbies.len()
                );
                self.send_event(Event::GameListUpdated(server_info)).await;
            }

            ServerMessage::LobbyJoined(client_id, lobby_update) => {
                self.client_id = Some(client_id);
                self.send_event(Event::LobbyUpdated {
                    settings: lobby_update.settings,
                    players: lobby_update.players,
                    local_player_id: lobby_update.client_player_id,
                })
                .await;
            }
            ServerMessage::LobbyUpdate(lobby_update) => {
                self.send_event(Event::LobbyUpdated {
                    settings: lobby_update.settings,
                    players: lobby_update.players,
                    local_player_id: lobby_update.client_player_id,
                })
                .await;
            }

            ServerMessage::Update(update) => {
                self.last_server_update = update.time;
                self.send_event(Event::Update(update)).await;
            }
            ServerMessage::Pong => todo!(),
            ServerMessage::Bye => todo!(),
        };
    }

    /// Leave any game / lobby
    /// this connection can be used to join again or it can be dropped
    /// to not communicate with server again
    async fn leave(&mut self) {
        if let Some(client_id) = self.client_id {
            self.send_message(ClientMessage::Bye(client_id)).await;
            sleep(Duration::from_millis(10)).await;
            self.send_message(ClientMessage::Bye(client_id)).await;
        }
        self.client_id = None;
        self.unacknowledged_packet = None;
    }
}
