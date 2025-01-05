use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;

use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::{
    decode, encode, ClientHello, ClientId, ClientLobbySettingsUpdate, ClientMessage, ClientPacket, ClientUpdate,
    GameId, ServerMessage, ServerPacket, ServerUpdate, BOMBERHANS_MAGIC_NO_V1,
};
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::TimeStamp;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio::time::Instant;

pub fn connect(server: SocketAddr, player_name: String) -> Connection {
    let (commands_to_backend, commands_from_frontend) = tokio::sync::mpsc::channel::<Command>(2);
    let (events_to_frontend, events_from_backend) = tokio::sync::mpsc::channel::<Event>(2);
    {
        tokio::spawn(async move {
            let mut comm =
                CommunicationBackend::new(server, commands_from_frontend, events_to_frontend, player_name).await;
            {
                let this = &mut comm;
                async move {
                    this.send_message(ClientMessage::Hello(ClientHello { player_name: this.player_name.clone() }))
                        .await;
                }
            }
            .await;
            comm.receive_commands_and_messages().await;
        });
    }

    Connection { commands_to_backend, events_from_backend, server }
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub server_name: String,
    pub ping: Duration,
    pub lobbies: Vec<(GameId, String)>,
}

#[derive(Debug)]
pub enum Event {
    GameListUpdated(ServerInfo),
    LobbyUpdated { settings: Settings, players: Vec<Player>, local_player_id: PlayerId },
    Disconnected,
    Update(ServerUpdate),
}

#[derive(Debug)]
enum Command {
    /// Open new Lobby
    OpenLobby,

    /// Join a Lobby
    JoinLobby(GameId),

    /// Update the Settings of the Lobby we host
    UpdateSettings(Settings),

    /// Start the Game of the current Lobby
    Start,

    /// Set local Players action
    SetAction(TimeStamp, Action),

    /// Disconnect from Server
    Disconnect,
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
        self.events_from_backend.recv().await.expect("comm backend doesn't panic")
    }

    pub async fn open_new_lobby(&self) {
        self.commands_to_backend.send(Command::OpenLobby).await.unwrap();
    }

    pub async fn disconnect(&self) {
        self.commands_to_backend.send(Command::Disconnect).await.unwrap();
    }

    pub async fn update_settings(&self, settings: Settings) {
        self.commands_to_backend.send(Command::UpdateSettings(settings)).await.unwrap();
    }

    pub async fn start(&self) {
        self.commands_to_backend.send(Command::Start).await.unwrap();
    }

    pub async fn set_action(&self, time: TimeStamp, action: Action) {
        self.commands_to_backend.send(Command::SetAction(time, action)).await.unwrap();
    }
}

fn in_ms(millis: u64) -> Instant {
    Instant::now() + Duration::from_millis(millis)
}

#[derive(Debug, Clone)]
enum State {
    Connecting {},
    Alive {
        client_id: ClientId,
    },
    OpeningNewLobby {
        client_id: ClientId,
    },
    Joining {
        client_id: ClientId,
    },
    Lobby {
        client_id: ClientId,
    },
    //LobbyReady {
    //    client_id: ClientId,
    //},
    GameStarting {
        client_id: ClientId,
    },
    Game {
        client_id: ClientId,
        last_server_update: TimeStamp,
    },

    Failed(String),
    Disconnected,

    /// Invalid State, should never be observed
    Invalid,
}

#[derive(Debug)]
struct CommunicationBackend {
    /// The state of the connection with the server
    state: State,

    /// The server this connection is for
    server: SocketAddr,

    /// Channel to receive commands from `Communication`
    commands_from_frontend: Receiver<Command>,

    /// Channel to send events to Frontend
    events_to_frontend: Sender<Event>,

    /// Socket to send to server with
    socket: UdpSocket,

    /// When to resend a Packet
    timeout: Option<Instant>,

    /// Name of the player
    player_name: String,

    /// Number of the packet we most recently sent
    last_sent_packet_number: u32,

    /// Number of the most recent packet, that we have received
    last_received_packet_number: u32,

    /// List of all sent packets for debugging
    sent_packets: Vec<(Instant, ClientPacket)>,

    /// List of all received packets for debugging
    received_packets: Vec<(Instant, ServerPacket)>,
}

/// Basic Sending and receiving
impl CommunicationBackend {
    /// Create a connection to a server
    async fn new(
        server: SocketAddr,
        commands_from_frontend: Receiver<Command>,
        events_to_frontend: Sender<Event>,
        player_name: String,
    ) -> Self {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
        let socket = UdpSocket::bind(addr).await.expect("can bind local udp socket");
        socket.connect(server).await.expect("can set socket's remote address");
        CommunicationBackend {
            state: State::Connecting {},
            server,
            commands_from_frontend,
            events_to_frontend,
            socket,
            timeout: None,
            player_name,
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            last_sent_packet_number: 0,
            last_received_packet_number: 0,
        }
    }

    async fn send_message(&mut self, message: ClientMessage) {
        log::debug!("Sending {message:#?}");
        let now = Instant::now();

        self.last_sent_packet_number += 1;
        let packet =
            ClientPacket { magic: BOMBERHANS_MAGIC_NO_V1, packet_number: self.last_sent_packet_number, message };
        match self.socket.send(&encode(&packet)).await {
            Ok(_) => {}
            Err(err) => {
                self.state = State::Failed(format!("can not send {err:?}"));
                return;
            }
        }
        self.sent_packets.push((now, packet));
    }

    fn decode_message(&mut self, data: &[u8]) -> Option<ServerMessage> {
        let Some(packet) = decode::<ServerPacket>(data) else {
            log::warn!("ignoring unparseable data: {data:?}");
            return None;
        };

        if packet.magic != BOMBERHANS_MAGIC_NO_V1 {
            log::warn!("ignoring unknown protocol {packet:?}");
            return None;
        }

        if packet.packet_number <= self.last_received_packet_number {
            log::warn!("ignoring out of order packet {packet:?}");
            return None;
        }

        self.last_received_packet_number = packet.packet_number;
        let message = packet.message.clone();
        self.received_packets.push((Instant::now(), packet));

        log::debug!("received: {message:#?}");
        Some(message)
    }
}

impl CommunicationBackend {
    async fn receive_commands_and_messages(&mut self) {
        {
            let this = &mut *self;
            async move {
                this.send_message(ClientMessage::Hello(ClientHello { player_name: this.player_name.clone() })).await;
            }
        }
        .await;
        let mut buf = [0; 1024];
        loop {
            match self.state {
                State::Invalid => panic!("invalid state"),
                State::Failed(_) | State::Disconnected => {
                    return;
                }
                _ => {}
            };

            tokio::select! {
                () = async { if let Some(to) = self.timeout { sleep(to - Instant::now()).await } } => { self.handle_timeout().await }
                cmd = self.commands_from_frontend.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd) .await,

                        None  => { self.handle_command(Command::Disconnect).await;}

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
                            self.state = State::Failed(format!("can not receive {err:?}"));
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (cmd, previous_state) {
            (_, State::Invalid) => panic!("Invalid State"),
            (Command::OpenLobby, State::Alive { client_id, .. }) => {
                self.send_message(ClientMessage::OpenNewLobby(client_id)).await;
                State::OpeningNewLobby { client_id }
            }
            (Command::UpdateSettings(settings), State::Lobby { client_id, .. }) => {
                self.send_message(ClientMessage::LobbySettingsUpdate(ClientLobbySettingsUpdate {
                    client_id,
                    settings,
                }))
                .await;
                State::Lobby { client_id }
            }
            // (Command::GetState(return_channel), state) => {
            //     return_channel.send(state.clone()).unwrap();
            //     state
            // }
            (Command::Disconnect, State::Connecting { .. }) | (Command::Disconnect, State::Disconnected) => {
                State::Disconnected
            }
            (Command::Disconnect, State::Failed(msg)) => State::Failed(msg),

            (Command::Disconnect, State::Alive { client_id, .. })
            | (Command::Disconnect, State::OpeningNewLobby { client_id, .. })
            | (Command::Disconnect, State::Joining { client_id, .. })
            | (Command::Disconnect, State::Lobby { client_id, .. })
            | (Command::Disconnect, State::Game { client_id, .. }) => {
                self.send_disconnect(client_id).await;
                State::Disconnected
            }
            (Command::Start, State::Lobby { client_id }) => {
                self.send_message(ClientMessage::GameStart(client_id)).await;
                State::GameStarting { client_id }
            }
            (Command::SetAction(time, action), State::Game { client_id, last_server_update }) => {
                self.send_message(ClientMessage::GameUpdate(ClientUpdate {
                    client_id,
                    last_server_update,
                    current_player_action: action,
                    current_action_start_time: time,
                }))
                .await;

                State::Game { client_id, last_server_update }
            }

            //
            (Command::OpenLobby, State::Connecting { .. }) => todo!(),
            (Command::OpenLobby, State::OpeningNewLobby { .. }) => todo!(),
            (Command::OpenLobby, State::Joining { .. }) => todo!(),
            (Command::OpenLobby, State::Lobby { .. }) => todo!(),
            (Command::OpenLobby, State::Game { .. }) => todo!(),
            (Command::OpenLobby, State::Failed(_)) => todo!(),
            (Command::OpenLobby, State::Disconnected) => todo!(),
            (Command::JoinLobby(_), State::Connecting { .. }) => todo!(),
            (Command::JoinLobby(_), State::Alive { .. }) => todo!(),
            (Command::JoinLobby(_), State::OpeningNewLobby { .. }) => todo!(),
            (Command::JoinLobby(_), State::Joining { .. }) => todo!(),
            (Command::JoinLobby(_), State::Lobby { .. }) => todo!(),
            (Command::JoinLobby(_), State::Game { .. }) => todo!(),
            (Command::JoinLobby(_), State::Failed(_)) => todo!(),
            (Command::JoinLobby(_), State::Disconnected) => todo!(),
            (Command::UpdateSettings(_), State::Connecting { .. }) => todo!(),
            (Command::UpdateSettings(_), State::Alive { .. }) => todo!(),
            (Command::UpdateSettings(_), State::OpeningNewLobby { .. }) => todo!(),
            (Command::UpdateSettings(_), State::Joining { .. }) => todo!(),
            (Command::UpdateSettings(_), State::Game { .. }) => todo!(),
            (Command::UpdateSettings(_), State::Failed(_)) => todo!(),
            (Command::UpdateSettings(_), State::Disconnected) => todo!(),
            (Command::Start, State::Connecting { .. }) => todo!(),
            (Command::Start, State::Alive { .. }) => todo!(),
            (Command::Start, State::OpeningNewLobby { .. }) => todo!(),
            (Command::Start, State::Joining { .. }) => todo!(),
            (Command::Start, State::Game { .. }) => todo!(),
            (Command::Start, State::Failed(_)) => todo!(),
            (Command::Start, State::Disconnected) => todo!(),
            (Command::OpenLobby, State::GameStarting { .. }) => todo!(),
            (Command::JoinLobby(_), State::GameStarting { .. }) => todo!(),
            (Command::UpdateSettings(_), State::GameStarting { .. }) => todo!(),
            (Command::Start, State::GameStarting { .. }) => todo!(),
            (Command::Disconnect, State::GameStarting { .. }) => todo!(),
            (Command::SetAction(_, _), State::Connecting {}) => todo!(),
            (Command::SetAction(_, _), State::Alive { client_id }) => todo!(),
            (Command::SetAction(_, _), State::OpeningNewLobby { client_id }) => todo!(),
            (Command::SetAction(_, _), State::Joining { client_id }) => todo!(),
            (Command::SetAction(_, _), State::Lobby { client_id }) => todo!(),
            (Command::SetAction(t, a), State::GameStarting { client_id }) => todo!("{a:#?}"),
            (Command::SetAction(_, _), State::Failed(_)) => todo!(),
            (Command::SetAction(_, _), State::Disconnected) => todo!(),
        };
        self.refresh_timeout();
    }

    async fn handle_message(&mut self, message: ServerMessage) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (message, previous_state) {
            (_, State::Invalid) => panic!("Invalid State"),
            (ServerMessage::Hello(msg), State::Connecting { .. })
            | (ServerMessage::Hello(msg), State::Alive { .. }) => {
                let ping = self
                    .sent_packets
                    .iter()
                    .rfind(|(_, p)| {
                        if let ClientMessage::Hello(_) = p.message {
                            p.packet_number == msg.clients_packet_number
                        } else {
                            false
                        }
                    })
                    .expect("the server responded to our hello, not something else")
                    .0
                    .elapsed();
                let server_info = ServerInfo { ping, server_name: msg.server_name, lobbies: msg.lobbies };
                log::info!(
                    "Received Server Hello from {} \"{}\" Ping: {}ms, Lobbies {}",
                    &self.server,
                    &server_info.server_name,
                    ping.as_millis(),
                    server_info.lobbies.len()
                );
                self.events_to_frontend.send(Event::GameListUpdated(server_info)).await.unwrap();
                State::Alive { client_id: msg.client_id }
            }

            (ServerMessage::LobbyUpdate(msg), State::Lobby { client_id, .. })
            | (ServerMessage::LobbyUpdate(msg), State::OpeningNewLobby { client_id }) => {
                self.events_to_frontend
                    .send(Event::LobbyUpdated {
                        settings: msg.settings,
                        players: msg.players,
                        local_player_id: msg.client_player_id,
                    })
                    .await
                    .unwrap();

                State::Lobby { client_id }
            }

            (ServerMessage::Update(update), State::GameStarting { client_id, .. })
            | (ServerMessage::Update(update), State::Game { client_id, .. }) => {
                let last_server_update = update.time;
                self.events_to_frontend.send(Event::Update(update)).await.unwrap();
                State::Game { last_server_update, client_id }
            }

            (_, State::Disconnected) => State::Disconnected,
            //
            (ServerMessage::Update(_), State::Alive { .. }) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::Alive { .. }) => todo!(),
            (ServerMessage::Update(_), State::Connecting { .. }) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::Connecting { .. }) => todo!(),
            (ServerMessage::Hello(_), State::Failed(_)) => todo!(),
            (ServerMessage::Update(_), State::Failed(_)) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::Failed(_)) => todo!(),
            (ServerMessage::Hello(_), State::Game { .. }) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::Game { .. }) => todo!(),
            (ServerMessage::Hello(_), State::GameStarting { .. }) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::GameStarting { .. }) => todo!(),
            (ServerMessage::Hello(_), State::Joining { .. }) => todo!(),
            (ServerMessage::Update(_), State::Joining { .. }) => todo!(),
            (ServerMessage::LobbyUpdate(_), State::Joining { .. }) => todo!(),
            (ServerMessage::Hello(_), State::Lobby { .. }) => todo!(),
            (ServerMessage::Hello(_), State::OpeningNewLobby { .. }) => todo!(),
            (ServerMessage::Update(_), State::OpeningNewLobby { .. }) => todo!(),
            (ServerMessage::Update(_), State::Lobby { .. }) => todo!(),
        };
        self.refresh_timeout();
    }

    async fn handle_timeout(&mut self) {
        log::info!("Timeout in {:?}", self.state);

        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match previous_state {
            State::Invalid => panic!("Invalid State"),
            State::Connecting { .. } => {
                {
                    let this = &mut *self;
                    async move {
                        this.send_message(ClientMessage::Hello(ClientHello { player_name: this.player_name.clone() }))
                            .await;
                    }
                }
                .await;
                State::Connecting {}
            }
            State::Alive { client_id } => {
                {
                    let this = &mut *self;
                    async move {
                        this.send_message(ClientMessage::Hello(ClientHello { player_name: this.player_name.clone() }))
                            .await;
                    }
                }
                .await;
                State::Alive { client_id }
            }
            State::GameStarting { client_id } => {
                self.send_message(ClientMessage::GameStart(client_id)).await;
                State::GameStarting { client_id }
            }
            State::OpeningNewLobby { client_id } => todo!(),
            State::Joining { client_id } => todo!(),
            State::Lobby { client_id } => todo!(),
            State::Game { client_id, last_server_update } => todo!(),
            State::Failed(_) => todo!(),
            State::Disconnected => todo!(),
        };
        self.refresh_timeout();
    }

    fn refresh_timeout(&mut self) {
        self.timeout = match &self.state {
            State::Connecting {} => Some(in_ms(100)),
            State::Alive { client_id } => Some(in_ms(5000)),
            State::OpeningNewLobby { client_id } => Some(in_ms(100)),
            State::Joining { client_id } => Some(in_ms(100)),
            State::Lobby { client_id } => Some(in_ms(100)),
            State::GameStarting { client_id } => Some(in_ms(100)),
            State::Game { client_id, last_server_update } => Some(in_ms(5000)),
            State::Failed(_) => None,
            State::Disconnected => None,
            State::Invalid => None,
        }
    }

    async fn send_disconnect(&mut self, client_id: ClientId) {
        self.send_message(ClientMessage::Bye(client_id)).await;
        sleep(Duration::from_millis(10)).await;
        self.send_message(ClientMessage::Bye(client_id)).await;
    }
}
