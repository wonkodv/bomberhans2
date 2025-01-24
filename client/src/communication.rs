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

#[derive(Debug)]
pub enum Event {
    /// Server sent List of games
    GameListUpdated(ServerLobbyList),

    /// Server sent Lobby Settings
    LobbyUpdated(ServerLobbyUpdate),

    /// Server sent Lobby Settings
    GameStart(ServerGameStart),

    /// Server sent Game Update
    Update(ServerUpdate),

    /// Server not reachable anymore
    // Disconnected,

    /// Communication Error
    Error(String),

    /// We know the Ping to the Server
    Ping(Duration),
    Disconnect(String),
}

#[derive(Debug)]
enum Command {
    /// Open new Lobby, as Player Name
    OpenLobby(String),

    /// Join a Lobby, as Player Name
    JoinLobby(GameId, String),

    /// Update the Settings of the Lobby we host
    UpdateSettings(Settings),

    /// Set player to ready
    SetReady(Ready),

    /// Set local Players action
    SetAction(GameTime, Action),

    /// Disconnect from Server
    Leave,

    /// Ask Server for Lobby Update
    PollLobby,
    PollGameList,
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

    async fn send(&self, command: Command) {
        self.commands_to_backend
            .send(command)
            .await
            .expect("comm backend doesn't panic");
    }

    pub async fn open_new_lobby(&self, player_name: String) {
        self.send(Command::OpenLobby(player_name)).await;
    }

    pub async fn disconnect(&self) {
        self.send(Command::Leave).await;
    }

    pub async fn update_settings(&self, settings: Settings) {
        self.send(Command::UpdateSettings(settings)).await;
    }

    pub async fn set_ready(&self, ready: Ready) {
        self.send(Command::SetReady(ready)).await;
    }

    pub async fn set_action(&self, time: GameTime, action: Action) {
        self.send(Command::SetAction(time, action)).await;
    }

    pub async fn join_lobby(&self, game_id: GameId, player_name: String) {
        self.send(Command::JoinLobby(game_id, player_name)).await;
    }

    pub async fn poll_lobby(&self) {
        self.send(Command::PollLobby).await;
    }

    pub async fn poll_game_list(&self) {
        self.send(Command::PollGameList).await;
    }
}

fn message_timeout(message: &ClientMessage) -> Duration {
    let ms = match message {
        ClientMessage::GetLobbyList => 100,
        ClientMessage::OpenNewLobby(_) => 100,
        ClientMessage::JoinLobby(_) => 100,
        ClientMessage::UpdateLobbySettings(_) => 100,
        ClientMessage::LobbyReady(_) => 100,
        ClientMessage::GameUpdate(_) => 16,
        ClientMessage::Bye => 0,
        ClientMessage::Ping => 100,
        ClientMessage::PollLobby => 500,
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
        socket.connect(server).await.unwrap();
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
            .expect("if we reach timeout, there should be something that timed out");
        let now = Instant::now();
        self.unacknowledged_packet = Some((packet.clone(), now, timeout));
        let _ = self.socket.send(&encode(&packet)).await; // TODO: do soemthing if we can not send
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
                let elapsed = sent_time.elapsed();
                if timeout < elapsed {
                    self.handle_timeout().await;
                    continue;
                }
                Some(timeout - elapsed)
            } else {
                None
            };

            tokio::select! { biased;
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
                true = async { if let Some(timeout) = timeout { sleep(timeout).await; true } else {false} } => {
                    self.handle_timeout().await;
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::Leave => {
                self.leave().await;
            }
            Command::JoinLobby(game_id, player_name) => {
                self.send_message(ClientMessage::JoinLobby(ClientJoinLobby {
                    game_id,
                    player_name,
                }))
                .await;
            }
            Command::OpenLobby(player_name) => {
                self.send_message(ClientMessage::OpenNewLobby(ClientOpenLobby { player_name }))
                    .await;
            }
            Command::UpdateSettings(settings) => {
                self.send_message(ClientMessage::UpdateLobbySettings(ClientLobbyUpdate {
                    settings,
                }))
                .await;
            }
            Command::SetReady(ready) => {
                self.send_message(ClientMessage::LobbyReady(ClientLobbyReady { ready }))
                    .await;
            }
            Command::SetAction(time, action) => {
                self.send_message(ClientMessage::GameUpdate(ClientUpdate {
                    last_server_update: self.last_server_update,
                    current_player_action: action,
                    current_action_start_time: time,
                }))
                .await;
            }
            Command::PollLobby => {
                self.send_message(ClientMessage::PollLobby).await;
            }
            Command::PollGameList => {
                self.send_message(ClientMessage::GetLobbyList).await;
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
        self.received_packets.push((Instant::now(), packet.clone()));

        log::trace!("received {packet:?}");

        if let Some((pending_ack_packet, sent_time, _timeout)) = self.unacknowledged_packet.as_ref()
        {
            if Some(pending_ack_packet.packet_number) == packet.ack_packet_number {
                log::trace!("Acknowledges: {:?}", pending_ack_packet.packet_number);
                self.send_event(Event::Ping(sent_time.elapsed())).await;
                self.unacknowledged_packet = None;
            }
        };

        if packet.packet_number <= self.last_received_packet {
            log::trace!("ignoring out of order packet {packet:?}");
            return;
        }

        self.last_received_packet = packet.packet_number;

        match packet.message {
            ServerMessage::LobbyList(lobby_list) => {
                log::info!(
                    "Received Server List from {} \"{}\", Lobbies: {}",
                    &self.server,
                    &lobby_list.server_name,
                    lobby_list.lobbies.len()
                );
                self.send_event(Event::GameListUpdated(lobby_list)).await;
            }

            ServerMessage::LobbyUpdate(lobby_update) => {
                self.send_event(Event::LobbyUpdated(lobby_update)).await;
            }
            ServerMessage::GameStart(game_start) => {
                self.send_event(Event::GameStart(game_start)).await;
            }
            ServerMessage::Update(update) => {
                self.last_server_update = update.time;
                self.send_event(Event::Update(update)).await;
            }
            ServerMessage::Pong => todo!(),
            ServerMessage::Bye(reason) => {
                log::warn!("Server disconnected because: {reason:?}");
                self.send_event(Event::Disconnect(reason)).await;
            }
        };
    }

    /// Leave any game / lobby
    /// this connection can be used to join again or it can be dropped
    /// to not communicate with server again
    async fn leave(&mut self) {
        self.send_message(ClientMessage::Bye).await;
        sleep(Duration::from_millis(10)).await;
        self.send_message(ClientMessage::Bye).await;
        self.unacknowledged_packet = None;
    }
}
