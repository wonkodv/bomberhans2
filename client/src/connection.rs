use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;

use bomberhans_lib::game_state::GameState;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::*;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::PlayerId;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub server_name: String,
    pub ping: Duration,
    pub lobbies: Vec<(GameId, String)>,
}

fn in_ms(millis: u64) -> Instant {
    Instant::now() + Duration::from_millis(millis)
}

#[derive(Debug, Clone)]
enum State {
    Connecting {
        timeout: Instant,
    },
    Alive {
        server_info: ServerInfo,
        timeout: Instant,
        client_id: ClientId,
    },
    OpeningNewLobby {
        timeout: Instant,
        client_id: ClientId,
    },
    Joining {
        timeout: Instant,
        client_id: ClientId,
    },
    Lobby {
        timeout: Instant,
        lobby_id: GameId,
        client_id: ClientId,

        settings: Settings,
        players: Vec<Player>,
        local_player_id: PlayerId,
    },
    Game {
        game_id: GameId,
        client_id: ClientId,
        timeout: Instant,
        game_state: GameState,
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

    /// Channel to receive commands from gui thread on
    rx: Receiver<Command>,

    /// Socket to send to server with
    socket: UdpSocket,

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
    async fn new(server: SocketAddr, rx: Receiver<Command>, player_name: String) -> Self {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
        let socket = UdpSocket::bind(addr)
            .await
            .expect("can bind local udp socket");
        socket
            .connect(server)
            .await
            .expect("can set socket's remote address");
        CommunicationBackend {
            state: State::Connecting {
                timeout: in_ms(100),
            },
            server,
            rx,
            socket,
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
        let packet = ClientPacket {
            magic: BOMBERHANS_MAGIC_NO_V1,
            packet_number: self.last_sent_packet_number,
            message,
        };
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
        self.send_hello().await;
        let mut buf = [0; 1024];
        loop {
            let sleep_time = match self.state {
                State::Connecting { timeout }
                | State::Alive { timeout, .. }
                | State::OpeningNewLobby { timeout, .. }
                | State::Joining { timeout, .. }
                | State::Lobby { timeout, .. }
                | State::Game { timeout, .. } => timeout,
                State::Failed(_) | State::Disconnected => {
                    return;
                }
                State::Invalid => panic!("invalid state"),
            };
            tokio::select! {
                // _ = sleep(sleep_tim) => { self.handle_timeout().await }
                cmd = self.rx.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd) .await,

                        None  => { self.disconnect().await;  }

                    }
                }
                result = self.socket.recv(&mut buf) => {
                    match result {
                        Ok(len)=> {
                            if let Some(message) = self.decode_message(&buf[0..len]) {
                                self.handle_message(message);
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
            (
                Command::OpenLobby,
                State::Alive {
                    timeout, client_id, ..
                },
            ) => {
                self.send_message(ClientMessage::OpenNewLobby(client_id))
                    .await;
                State::OpeningNewLobby { timeout, client_id }
            }
            (Command::GetState(return_channel), state) => {
                return_channel.send(state.clone()).unwrap();
                state
            }
            (command, state) => {
                todo!("command/State not implemented: {command:?}/{state:?}")
            }
        };
    }

    fn handle_message(&mut self, message: ServerMessage) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (message, previous_state) {
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
                let server_info = ServerInfo {
                    ping,
                    server_name: msg.server_name,
                    lobbies: msg.lobbies,
                };
                log::info!(
                    "Received Server Hello from {} \"{}\" Ping: {}ms, Lobbies {}",
                    &self.server,
                    &server_info.server_name,
                    ping.as_millis(),
                    server_info.lobbies.len()
                );
                State::Alive {
                    server_info,
                    client_id: msg.client_id,
                    timeout: in_ms(100),
                }
            }

            (ServerMessage::LobbyUpdate(msg), State::OpeningNewLobby { timeout, client_id }) => {
                State::Lobby {
                    timeout: in_ms(100),
                    client_id,

                    lobby_id: msg.id,
                    settings: msg.settings,
                    players: msg.players,
                    local_player_id: msg.client_player_id,
                }
            }

            (message, state) => {
                todo!("Message/State not implemented: {message:?}/{state:?}")
            }
        };
    }

    async fn handle_timeout(&mut self) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match previous_state {
            State::Connecting { .. } => {
                self.send_hello().await;
                State::Connecting {
                    timeout: in_ms(100),
                }
            }
            State::Alive {
                server_info,
                timeout,
                client_id,
            } => {
                self.send_hello().await;
                State::Alive {
                    server_info,
                    timeout: in_ms(100),
                    client_id,
                }
            }
            state => {
                todo!("Timeout not implemented for state {state:#?}");
            }
        }
    }

    async fn disconnect(&mut self) {
        match self.state {
            State::Alive { client_id, .. }
            | State::OpeningNewLobby { client_id, .. }
            | State::Joining { client_id, .. }
            | State::Lobby { client_id, .. }
            | State::Game { client_id, .. } => {
                self.send_message(ClientMessage::Bye(client_id)).await;
                sleep(Duration::from_millis(10)).await;
                self.send_message(ClientMessage::Bye(client_id)).await;
            }
            _ => {}
        }
        self.state = State::Disconnected;
    }

    async fn send_hello(&mut self) {
        self.send_message(ClientMessage::Hello(ClientHello {
            player_name: self.player_name.clone(),
        }))
        .await;
    }
}

#[derive(Debug)]
pub enum Event {
    GameListUpdated,
    LobbyUpdated,
    GameUpdated,
    Disconnected,
}

#[derive(Debug)]
enum Command {
    OpenLobby,
    JoinLobby(GameId),
    GetState(tokio::sync::oneshot::Sender<State>),
}

/// Communication with one server
#[derive(Debug)]
pub struct Connection {
    /// Send commands from Game Controller to Communication via this channel
    tx: Sender<Command>,

    pub server: SocketAddr,
}

impl Connection {
    async fn get_state(&self) -> State {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.blocking_send(Command::GetState(tx)).unwrap();
        rx.await.expect("Connection backend send state")
    }

    pub async fn get_server_info(&self) -> Option<Result<ServerInfo, String>> {
        match self.get_state().await {
            State::Alive { server_info, .. } => Some(Ok(server_info)),
            State::Connecting { .. } => None,
            State::Disconnected => return Some(Err("Disconnected".to_owned())),
            State::Failed(err) => return Some(Err(err.clone())),

            state => todo!("unexpected {state:#?}"),
        }
    }

    pub async fn open_new_lobby(&self) {
        self.tx.send(Command::OpenLobby).await.unwrap();
    }
}

pub fn connect(server: SocketAddr, player_name: String) -> Connection {
    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(32);
    {
        let server = server.clone();
        let foo = tokio::spawn(async move {
            let mut comm = CommunicationBackend::new(server, rx, player_name).await;
            comm.send_hello().await;
            comm.receive_commands_and_messages().await;
        });
    }

    Connection { tx, server }
}
