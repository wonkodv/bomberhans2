use std::net::IpAddr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::LazyLock;

use bomberhans_lib::network::*;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio::time::Instant;

/// The shared Runtime for all Communication
static RUNTIME: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().unwrap());

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub server_name: String,
    pub ping: Duration,
}

type Lobbies = Vec<(GameId, String)>;

#[derive(Debug, Clone)]
enum State {
    Pinging,
    Alive {
        lobbies: Lobbies,
        server_info: ServerInfo,
    },
    OpeningNewLobby,
    Lobby,
    Game,

    Failed(String),
    Disconnected,
}

#[derive(Debug)]
struct CommunicationBackend {
    /// The state of the connection with the server
    state: Arc<std::sync::Mutex<State>>,

    /// The server this connection is for
    server: SocketAddr,

    /// Channel to receive commands from gui thread on
    rx: Receiver<GuiToCommCommands>,

    /// Socket to send to server with
    socket: UdpSocket,

    /// When did we last hear from server
    last_server_message: Instant,

    /// Name of the player
    player_name: String,

    /// Id that the server identifies us with
    client_id: Option<ClientId>,

    /// List of all sent packets for debugging
    sent_packets: Vec<(Instant, ClientMessage)>,

    /// List of all received packets for debugging
    received_packets: Vec<ServerMessage>,
}

impl CommunicationBackend {
    /// Create a connection to a server and run it
    ///
    /// TODO: having `new`  as an async that never returns is strange
    async fn new(
        state: Arc<std::sync::Mutex<State>>,
        server: SocketAddr,
        rx: Receiver<GuiToCommCommands>,
        player_name: String,
    ) {
        let addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
        let socket = UdpSocket::bind(addr)
            .await
            .expect("can bind local udp socket");
        socket
            .connect(server)
            .await
            .expect("can set socket's remote address");
        CommunicationBackend {
            state,
            server,
            rx,
            socket,
            player_name,
            last_server_message: Instant::now(), // value immediately overwritten
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            client_id: None,
        }
        .receive_commands_and_messages()
        .await
    }

    async fn receive_commands_and_messages(&mut self) {
        self.send_hello().await;
        let mut buf = [0; 1024];
        loop {
            //            TODO: I dont want the timeout signal every single loop.
            //            once it happened, back of for another interval if the state can deal with
            //            that, otherwise 🤷
            //            let timeout_in_current_state: i32 = {
            //                match *self.state.lock().unwrap() {
            //                    State::Pinging | State::Alive { .. } => 100,
            //                    State::OpeningNewLobby => 100,
            //                    State::Lobby => 5000,
            //                    State::Game => 1000,
            //                    State::Failed(_) | State::Disconnected => 86400000,
            //                }
            //            };
            //            let sleep_time:i32 = timeout_in_current_state
            //                - self
            //                    .last_server_message
            //                    .elapsed()
            //                    .as_millis()
            //                    .try_into()
            //                    .unwrap();
            //            if sleep_time < 0 {self.handle_timeout()

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
                        Ok(len)=> {self.handle_message(&buf[0..len]).await;}
                        Err(err) => {
                            *self.state.lock().unwrap() = State::Failed(format!("can not receive {err:?}"));
                        }
                    }
                }
            }

            let state = { self.state.lock().unwrap().clone() };
            match state {
                State::Failed(_) | State::Disconnected => {
                    return;
                }
                _ => {}
            };
        }
    }

    async fn handle_command(&mut self, cmd: GuiToCommCommands) {
        let state = {
            self.state.lock().unwrap().clone() // TODO: that clone :/
        };
        match cmd {
            GuiToCommCommands::OpenLobby => match state {
                State::Alive { .. } => {
                    *self.state.lock().unwrap() = State::OpeningNewLobby;
                    self.send_open_lobby().await;
                }
                _ => panic!("unexpected command {cmd:#?}  in state {state:#?}"),
            },
            GuiToCommCommands::JoinLobby(_) => todo!(),
        }
    }

    async fn handle_message(&mut self, data: &[u8]) {
        let Some(msg) = decode(data) else {
            log::warn!("unparseable data: {data:?}");
            return;
        };
        log::debug!("received: {msg:#?}");
        match &msg {
            ServerMessage::Hello(msg) => self.handle_server_hello(msg),
            ServerMessage::Update(msg) => self.handle_server_update(msg),
            ServerMessage::LobbyUpdate(msg) => self.handle_server_lobby_update(msg),
        }
        self.received_packets.push(msg);
    }

    fn handle_server_hello(&mut self, msg: &ServerHello) {
        let state: &mut State = &mut *self.state.lock().unwrap();
        match { &state } {
            State::Pinging | State::Alive { .. } => {
                let (packet_time, _) = self
                    .sent_packets
                    .iter()
                    .rfind(|(_, p)| {
                        if let ClientMessage::Hello(hello) = p {
                            hello.nonce == msg.clients_nonce
                        } else {
                            false
                        }
                    })
                    .expect("the server responded to our hello, not something else");
                let ping = packet_time.elapsed();
                let lobbies = msg.lobbies.clone();

                let server_info = ServerInfo {
                    ping,
                    server_name: msg.server_name.clone(),
                };
                log::info!(
                    "Received Server Hello from {} \"{}\" Ping: {}ms, Lobbies {}",
                    &self.server,
                    &server_info.server_name,
                    ping.as_millis(),
                    lobbies.len()
                );
                *state = State::Alive {
                    lobbies,
                    server_info,
                };

                self.client_id = Some(msg.client_id);
            }
            _ => todo!(),
        };
    }

    fn handle_server_update(&self, msg: &ServerUpdate) {
        todo!()
    }

    fn handle_server_lobby_update(&self, msg: &ServerLobbyUpdate) {
        todo!()
    }

    async fn handle_timeout(&mut self) {
        let state = self.state.lock().unwrap().clone();
        match state {
            State::Pinging | State::Alive { .. } => {
                std::mem::drop(state); // TODO: when is state dropped?
                self.send_hello().await
            }
            _ => todo!(),
        }
    }

    async fn disconnect(&mut self) {
        if let Some(client_id) = self.client_id {
            self.send(ClientMessage::Bye(client_id)).await;
            sleep(Duration::from_millis(10)).await;
            self.send(ClientMessage::Bye(client_id)).await;
        }
        *self.state.lock().unwrap() = State::Disconnected;
    }

    async fn send(&mut self, msg: ClientMessage) {
        log::debug!("Sending {msg:#?}");
        let now = Instant::now();
        match self.socket.send(&encode(&msg)).await {
            Ok(_) => {}
            Err(err) => {
                *self.state.lock().unwrap() = State::Failed(format!("can not send {err:?}"));
                return;
            }
        }
        self.sent_packets.push((now, msg));
    }

    async fn send_hello(&mut self) {
        self.send(ClientMessage::Hello(ClientHello {
            magic: BOMBERHANS_MAGIC_NO_V1,
            player_name: self.player_name.clone(),
            nonce: rand::random(),
        }))
        .await;
    }

    async fn send_open_lobby(&mut self) {
        self.send(ClientMessage::OpenNewLobby(self.client_id.unwrap()))
            .await;
    }
}

#[derive(Debug)]
enum GuiToCommCommands {
    OpenLobby,
    JoinLobby(GameId),
}

/// Communication with one server
#[derive(Debug)]
pub struct Connection {
    /// Send commands from gui to comm via this channel
    tx: Sender<GuiToCommCommands>,

    state: Arc<std::sync::Mutex<State>>,

    pub server: SocketAddr,
}

impl Connection {
    pub fn get_server_info(&self) -> Option<Result<(Lobbies, ServerInfo), String>> {
        let state: &State = &*self.state.lock().unwrap();
        match state {
            State::Alive {
                lobbies,
                server_info,
            } => Some(Ok((lobbies.clone(), server_info.clone()))),
            State::Pinging => None,
            State::Disconnected => return Some(Err("Disconnected".to_owned())),
            State::Failed(err) => return Some(Err(err.clone())),

            _ => todo!("unexpected {state:#?}"),
        }
    }

    pub fn open_new_lobby(&self) {
        self.tx.blocking_send(GuiToCommCommands::OpenLobby).unwrap();
    }
}

pub fn connect(server: SocketAddr, player_name: String) -> Connection {
    let (tx, rx) = tokio::sync::mpsc::channel::<GuiToCommCommands>(32);
    let state = State::Pinging;
    let state = std::sync::Mutex::new(state);
    let state = Arc::new(state);

    {
        let state = Arc::clone(&state);
        let foo = RUNTIME.spawn(CommunicationBackend::new(state, server, rx, player_name));
    }

    Connection { tx, state, server }
}
