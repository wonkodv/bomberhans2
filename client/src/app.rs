use std::net::SocketAddr;
use std::time::Duration;

use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::GameState;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::GameId;
use bomberhans_lib::settings::Settings;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;

use crate::connection;
use crate::connection::Connection;
use crate::connection::ServerInfo;
use crate::game::SinglePlayerGame;

#[derive(Debug, Clone)]
pub enum State {
    Initial,
    SpSettings,
    SpGame(SinglePlayerGame),
    MpConnecting,
    MpView(ServerInfo),
    MpOpeningLobby,
    MpLobbyGuest(Settings, Vec<Player>),
    MpLobbyHost(Settings, Vec<Player>),
    MpGame(GameState),

    Invalid,
}

pub fn controller() -> (GameController, GameControllerBackend) {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let backend = GameControllerBackend::new(rx);

    let frontend = GameController { tx };
    return (frontend, backend);
}

pub enum Command {
    SetUpdateCallback(Box<dyn Fn() -> () + Send>),
    SetAction(Action),
    StartLocalGame(Settings),
    ConnectToServer {
        server: SocketAddr,
        player_name: String,
    },
    OpenNewLobby,
    JoinLobby(GameId),
    UpdateLobbySettings(Settings),
    StartMultiplayerGame,
    GetState(tokio::sync::oneshot::Sender<State>),
    Disconnect,
}

pub struct GameController {
    tx: Sender<Command>,
}

impl GameController {
    pub fn set_update_callback(&mut self, callback: Box<dyn Fn() -> () + Send>) {
        self.tx
            .blocking_send(Command::SetUpdateCallback(callback))
            .map_err(|e| format!("{e:#?}"))
            .unwrap();
    }
    pub fn set_action(&mut self, action: Action) {
        self.tx.blocking_send(Command::SetAction(action)).unwrap();
    }
    pub fn open_new_lobby(&mut self) {
        self.tx.blocking_send(Command::OpenNewLobby).unwrap();
    }
    pub fn start_local_game(&mut self, settings: Settings) {
        self.tx
            .blocking_send(Command::StartLocalGame(settings))
            .unwrap();
    }
    pub fn connect_to_server(&mut self, server: SocketAddr, player_name: String) {
        self.tx
            .blocking_send(Command::ConnectToServer {
                server,
                player_name,
            })
            .unwrap();
    }
    pub fn join_lobby(&mut self, id: GameId) {
        self.tx.blocking_send(Command::JoinLobby(id)).unwrap();
    }
    pub fn update_lobby_settings(&mut self, new_settings: Settings) {
        self.tx
            .blocking_send(Command::UpdateLobbySettings(new_settings))
            .unwrap();
    }
    pub fn start_multiplayer_game(&mut self) {
        self.tx
            .blocking_send(Command::StartMultiplayerGame)
            .unwrap();
    }
    pub fn get_state(&self) -> State {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.blocking_send(Command::GetState(tx)).unwrap();
        rx.blocking_recv()
            .expect("GameControllerBackend backend sent state")
    }
    pub fn disconnect(&self) {
        self.tx.blocking_send(Command::Disconnect).unwrap();
    }
}

/// central controller that encodes the application's behavior.
/// The Gui draws what this controller says
pub struct GameControllerBackend {
    /// What is currently visible on the Gui
    state: State,

    /// current connection with a server
    connection: Option<Connection>,

    /// callback to inform gui soemthign new has happend
    update_callback: Option<Box<dyn Fn() -> () + Send>>,

    rx_from_frontend: Receiver<Command>,
    rx_from_comm: Receiver<connection::Event>,
    tx_to_hand_to_comm: Sender<connection::Event>,
}

impl GameControllerBackend {
    fn new(rx_from_frontend: Receiver<Command>) -> Self {
        let (tx_to_hand_to_comm, rx_from_comm) = tokio::sync::mpsc::channel(32);
        Self {
            state: State::Initial,
            connection: None,
            rx_from_frontend,
            rx_from_comm,
            tx_to_hand_to_comm,
            update_callback: None,
        }
    }

    pub async fn run(&mut self) {
        loop {
            // TODO: if in-game timeout every frame
            tokio::select! {
                 _ = sleep(Duration::from_millis(100)) => { self.handle_timeout().await }
                comm_event = self.rx_from_comm.recv() => {
                    self.handle_comm_update(comm_event.expect("comm chnnel never closes")) .await;

                }
                gui_command = self.rx_from_frontend.recv() => {
                    self.handle_gui_command(gui_command.expect("gui doesn't just close th channel")).await;
                }
            }
        }
    }

    async fn handle_comm_update(&mut self, event: connection::Event) {
        todo!("Depending on state, update the game");
    }

    async fn handle_gui_command(&mut self, command: Command) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (command, previous_state) {
            (Command::SetUpdateCallback(callback), state) => {
                self.update_callback = Some(callback);
                state
            }
            (Command::GetState(return_channel), state) => {
                return_channel.send(state.clone()).unwrap();
                state
            }
            (_, State::Invalid) => panic!("Invalid State"),
            (command, state) => todo!(
                "command/state not implemented: {:?} / {:?}",
                &command,
                &state,
            ),
        };
    }

    async fn handle_timeout(&mut self) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match previous_state {
            State::Initial | State::SpSettings => previous_state,
            State::SpGame(mut spg) => {
                spg.update_simulation_realtime();
                self.update_callback.as_ref().unwrap()();
                State::SpGame(spg)
            }
            State::Invalid => panic!("Invalid State"),
            state => todo!("state {:?}", &state,),
        };
    }
}
