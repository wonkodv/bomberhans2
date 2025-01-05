use std::net::SocketAddr;
use std::time::Duration;

use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::GameState;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::GameId;
use bomberhans_lib::network::ServerUpdate;
use bomberhans_lib::network::Update;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::TimeStamp;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;

use crate::connection;
use crate::connection::connect;
use crate::connection::Connection;
use crate::connection::ServerInfo;
use crate::game::SinglePlayerGame;

/// Update Local Copy of Servers `GameState` and predict local `GameState`
fn synchronize_simulation(
    mut server_game_state: GameState,
    update: ServerUpdate,
    local_update: &Update,
) -> (GameState, GameState) {
    for server_time in server_game_state.time.ticks_from_start()..update.time.ticks_from_start() {
        for u in &update.updates {
            if u.time == server_game_state.time {
                server_game_state.set_player_action(u.player, u.action);
            }
        }
        server_game_state.simulate_1_update();
    }
    debug_assert_eq!(update.time, server_game_state.time);

    // TODO: if server_game_state.checksum() != update.checksum { panic!(); }

    let mut local_game_state = server_game_state.clone();

    if local_update.time < local_game_state.time {
        log::warn!("local update missed by server {local_update:?}");
        local_game_state.set_player_action(local_update.player, local_update.action);
    }
    for _ in 0..5 {
        // TODO: think about this value
        if local_update.time == local_game_state.time {
            local_game_state.set_player_action(local_update.player, local_update.action);
        }
        local_game_state.simulate_1_update();
    }

    (server_game_state, local_game_state)
}

#[derive(Debug, Clone)]
pub enum State {
    Initial,
    SpSettings,
    SpGame(SinglePlayerGame),
    MpConnecting,
    MpView(ServerInfo),
    MpOpeningNewLobby,
    MpLobbyGuest {
        settings: Settings,
        players: Vec<Player>,
        local_player_id: PlayerId,
    },
    MpLobbyHost {
        settings: Settings,
        players: Vec<Player>,
        local_player_id: PlayerId,
    },
    MpGame {
        server_game_state: GameState,
        local_game_state: GameState,
        local_update: Update,
    },

    /// Server not responding
    MpServerLost(GameState),

    /// Server disconnected us
    Disconnected,

    /// User clicked Exit
    GuiClosed,

    /// State machine in transition, should never be observed
    Invalid,
}

pub fn controller() -> (GameController, GameControllerBackend) {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let backend = GameControllerBackend::new(rx);

    let frontend = GameController { tx };
    (frontend, backend)
}

pub enum Command {
    SetUpdateCallback(Box<dyn Fn() + Send>),
    SetAction(Action),
    ConfigureLocalGame,
    StartLocalGame,
    ConnectToServer { server: SocketAddr, player_name: String },
    OpenNewLobby,
    JoinLobby(GameId),
    UpdateSettings(Settings),
    StartMultiplayerGame,
    GetState(tokio::sync::oneshot::Sender<State>),
    Disconnect,
    Quit,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::SetUpdateCallback(_) => write!(f, "SetUpdateCallback"),
            Command::SetAction(action) => write!(f, "SetAction({action:?})"),
            Command::StartLocalGame => write!(f, "StartLocalGame"),
            Command::ConnectToServer { server, player_name } => {
                write!(f, "ConnectToServer {{{server:?}, {player_name:?} }}")
            }
            Command::OpenNewLobby => write!(f, "OpenNewLobby"),
            Command::JoinLobby(game_id) => write!(f, "JoinLobby({game_id:?})"),
            Command::UpdateSettings(_settings) => {
                write!(f, "UpdateSettings(settings)")
            }
            Command::StartMultiplayerGame => write!(f, "StartMultiplayerGame"),
            Command::GetState(_tx) => write!(f, "GetState()"),
            Command::Disconnect => write!(f, "Disconnect"),
            Command::ConfigureLocalGame => write!(f, "ConfigureLocalGame"),
            Command::Quit => write!(f, "Quit"),
        }
    }
}

pub struct GameController {
    tx: Sender<Command>,
}

impl GameController {
    pub fn set_update_callback(&mut self, callback: Box<dyn Fn() + Send>) {
        self.tx.blocking_send(Command::SetUpdateCallback(callback)).map_err(|e| format!("{e:#?}")).unwrap();
    }
    pub fn set_action(&mut self, action: Action) {
        self.tx.blocking_send(Command::SetAction(action)).unwrap();
    }
    pub fn open_new_lobby(&mut self) {
        self.tx.blocking_send(Command::OpenNewLobby).unwrap();
    }
    //   pub fn configure_local_game(&mut self) {
    //       self.tx.blocking_send(Command::ConfigureLocalGame).unwrap();
    //   }
    pub fn start_local_game(&mut self) {
        self.tx.blocking_send(Command::StartLocalGame).unwrap();
    }
    pub fn connect_to_server(&mut self, server: SocketAddr, player_name: String) {
        self.tx.blocking_send(Command::ConnectToServer { server, player_name }).unwrap();
    }
    pub fn join_lobby(&mut self, id: GameId) {
        self.tx.blocking_send(Command::JoinLobby(id)).unwrap();
    }
    pub fn update_settings(&mut self, new_settings: Settings) {
        self.tx.blocking_send(Command::UpdateSettings(new_settings)).unwrap();
    }
    pub fn start_multiplayer_game(&mut self) {
        self.tx.blocking_send(Command::StartMultiplayerGame).unwrap();
    }
    pub fn get_state(&self) -> State {
        let (tx, rx) = tokio::sync::oneshot::channel();
        match self.tx.blocking_send(Command::GetState(tx)) {
            Ok(()) => match rx.blocking_recv() {
                Ok(state) => state,
                Err(err) => panic!("Controller Paniced"),
            },
            Err(err) => panic!("Controller Paniced"),
        }
    }
    pub fn disconnect(&self) {
        self.tx.blocking_send(Command::Disconnect).unwrap();
    }
    pub fn quit(&self) {
        self.tx.blocking_send(Command::Quit).unwrap();
    }
}

/// central controller that encodes the application's behavior.
/// The Gui draws what this controller says
pub struct GameControllerBackend {
    /// What is currently visible on the Gui
    state: State,

    /// current connection with a server
    /// TODO: the connection should live in the Mp States, but i don't want to give it to the gui,
    /// but i also dont want to duplicate State (one with, one without conn).
    /// Should be `Some()` in all `State::Mp*` states, None otherwise
    connection: Option<Connection>,

    /// callback to inform gui something new has happend
    update_callback: Box<dyn Fn() + Send>,

    rx_from_frontend: Receiver<Command>,
}

impl GameControllerBackend {
    fn new(rx_from_frontend: Receiver<Command>) -> Self {
        Self { state: State::Initial, connection: None, rx_from_frontend, update_callback: Box::new(|| {}) }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                // TODO: do something smart with timeout
                () = sleep(Duration::from_millis(10)) => { self.handle_timeout().await }
                gui_command = self.rx_from_frontend.recv() => {
                    match gui_command {
                        Some(gui_command) => self.handle_gui_command(gui_command).await,
                        None => {
                            // gui closed
                            self.handle_gui_command(Command::Quit).await;
                            return
                        }
                    }
                }
                Some(server_event) = async {
                    if let Some(connection) = self.connection.as_mut() {
                        Some(connection.get_event().await)
                    } else { None }
                } => {
                    self.handle_server_event(server_event).await;
                }
            }
        }
    }

    async fn handle_server_event(&mut self, event: connection::Event) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (event, previous_state) {
            (_, State::Invalid) => panic!("Invalid State"),
            (_, State::Disconnected) | (connection::Event::Disconnected, _) => State::Disconnected,
            //
            (connection::Event::GameListUpdated(server_info), State::MpConnecting)
            | (connection::Event::GameListUpdated(server_info), State::MpView(_)) => State::MpView(server_info),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpOpeningNewLobby) => {
                State::MpLobbyHost { settings, players, local_player_id }
            }
            (connection::Event::Update(update), State::MpLobbyGuest { settings, players, local_player_id })
            | (connection::Event::Update(update), State::MpLobbyHost { settings, players, local_player_id }) => {
                let server_game_state = GameState::new(settings, players);

                let local_update = Update { player: local_player_id, action: Action::idle(), time: TimeStamp::new() };
                let (server_game_state, local_game_state) =
                    synchronize_simulation(server_game_state, update, &local_update);
                log::info!("First Server Update received, local state updated");

                State::MpGame { server_game_state, local_game_state, local_update }
            }
            (
                connection::Event::Update(update),
                State::MpGame { server_game_state, local_update, local_game_state: old_local_game_state },
            ) => {
                let (server_game_state, local_game_state) =
                    synchronize_simulation(server_game_state, update, &local_update);
                if old_local_game_state.players[&local_update.player].1
                    != local_game_state.players[&local_update.player].1
                {
                    log::info!(
                        "Server Update received. proposed local player state changed:\n   {:?}\n   {:?}",
                        old_local_game_state.players[&local_update.player].1,
                        local_game_state.players[&local_update.player].1
                    );
                }
                State::MpGame { server_game_state, local_game_state, local_update }
            }
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpLobbyHost { .. }) => {
                State::MpLobbyHost { settings, players, local_player_id }
            }

            //
            (connection::Event::GameListUpdated(_), State::Initial) => todo!(),
            (connection::Event::GameListUpdated(_), State::SpSettings) => todo!(),
            (connection::Event::GameListUpdated(_), State::SpGame(_)) => todo!(),
            (connection::Event::GameListUpdated(_), State::MpOpeningNewLobby) => todo!(),
            (connection::Event::GameListUpdated(_), State::MpLobbyGuest { settings, players, local_player_id }) => {
                todo!()
            }
            (connection::Event::GameListUpdated(_), State::MpLobbyHost { settings, players, local_player_id }) => {
                todo!()
            }
            (connection::Event::GameListUpdated(_), State::MpGame { .. }) => todo!(),
            (connection::Event::GameListUpdated(_), State::MpServerLost(_)) => todo!(),
            (connection::Event::GameListUpdated(_), State::GuiClosed) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::Initial) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::SpSettings) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::SpGame(_)) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpConnecting) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpView(_)) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpLobbyGuest { .. }) => {
                todo!()
            }
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpGame { .. }) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::MpServerLost(_)) => todo!(),
            (connection::Event::LobbyUpdated { settings, players, local_player_id }, State::GuiClosed) => todo!(),
            (connection::Event::Update(_), State::Initial) => todo!(),
            (connection::Event::Update(_), State::SpSettings) => todo!(),
            (connection::Event::Update(_), State::SpGame(_)) => todo!(),
            (connection::Event::Update(_), State::MpConnecting) => todo!(),
            (connection::Event::Update(_), State::MpView(_)) => todo!(),
            (connection::Event::Update(_), State::MpOpeningNewLobby) => todo!(),
            (connection::Event::Update(_), State::MpServerLost(_)) => todo!(),
            (connection::Event::Update(_), State::GuiClosed) => todo!(),
        };

        self.update_gui().await;
    }

    async fn handle_gui_command(&mut self, command: Command) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (command, previous_state) {
            (_, State::Invalid) => panic!("Invalid State"),
            (Command::SetUpdateCallback(callback), state) => {
                self.update_callback = callback;
                state
            }
            (Command::GetState(return_channel), state) => {
                return_channel.send(state.clone()).unwrap();
                state
            }
            (Command::Disconnect, _) => {
                if let Some(connection) = self.connection.as_ref() {
                    connection.disconnect().await;
                }
                State::Initial
            }
            (Command::Quit, _) => {
                if let Some(connection) = self.connection.as_ref() {
                    connection.disconnect().await;
                }
                State::GuiClosed
            }
            (Command::StartLocalGame, State::Initial) => {
                State::SpGame(SinglePlayerGame::new(Settings::default() /*TODO: make settings configurable*/))
            }
            (Command::SetAction(action), State::SpGame(mut game_state)) => {
                game_state.set_local_player_action(action);
                State::SpGame(game_state)
            }
            (Command::ConnectToServer { server, player_name }, State::Initial) => {
                self.connection = Some(connect(server, player_name));
                State::MpConnecting
            }
            (Command::OpenNewLobby, State::MpView(_)) => {
                self.connection.as_ref().unwrap().open_new_lobby().await;
                State::MpOpeningNewLobby
            }
            //(Command::UpdateSettings(settings), State::SpSettings(_)) => SpSettings(settings),
            (Command::UpdateSettings(settings), State::MpLobbyHost { players, local_player_id, .. }) => {
                self.connection.as_ref().unwrap().update_settings(settings.clone()).await;
                State::MpLobbyHost { settings, players, local_player_id }
            }
            (Command::StartMultiplayerGame, State::MpLobbyHost { players, local_player_id, settings }) => {
                self.connection.as_ref().unwrap().start().await;
                let local_update = Update { player: local_player_id, action: Action::idle(), time: TimeStamp::new() };
                let server_game_state = GameState::new(settings, players);
                let local_game_state = server_game_state.clone(); // TODO: simulate some ticks into the
                                                                  // future?
                State::MpGame { server_game_state, local_game_state, local_update }
            }
            (
                Command::SetAction(action),
                State::MpGame { server_game_state, mut local_game_state, local_update: old_local_update },
            ) => {
                let local_update = Update { action, time: local_game_state.time, player: old_local_update.player };
                if local_game_state.set_player_action(local_update.player, action) {
                    self.connection.as_ref().unwrap().set_action(local_update.time, local_update.action).await;
                }
                State::MpGame { server_game_state, local_game_state, local_update }
            }
            //
            (Command::SetAction(_), State::Initial) => todo!(),
            (Command::SetAction(_), State::SpSettings) => todo!(),
            (Command::SetAction(_), State::MpConnecting) => todo!(),
            (Command::SetAction(_), State::MpView(_)) => todo!(),
            (Command::SetAction(_), State::MpOpeningNewLobby) => todo!(),
            (Command::SetAction(_), State::MpLobbyGuest { .. }) => todo!(),
            (Command::SetAction(_), State::MpLobbyHost { .. }) => todo!(),
            (Command::SetAction(_), State::MpServerLost(_)) => todo!(),
            (Command::SetAction(_), State::GuiClosed) => todo!(),
            (Command::ConfigureLocalGame, State::Initial) => todo!(),
            (Command::ConfigureLocalGame, State::SpSettings) => todo!(),
            (Command::ConfigureLocalGame, State::SpGame(_)) => todo!(),
            (Command::ConfigureLocalGame, State::MpConnecting) => todo!(),
            (Command::ConfigureLocalGame, State::MpView(_)) => todo!(),
            (Command::ConfigureLocalGame, State::MpOpeningNewLobby) => todo!(),
            (Command::ConfigureLocalGame, State::MpLobbyGuest { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::MpLobbyHost { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::MpGame { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::MpServerLost(_)) => todo!(),
            (Command::ConfigureLocalGame, State::GuiClosed) => todo!(),
            (Command::StartLocalGame, State::SpSettings) => todo!(),
            (Command::StartLocalGame, State::SpGame(_)) => todo!(),
            (Command::StartLocalGame, State::MpConnecting) => todo!(),
            (Command::StartLocalGame, State::MpView(_)) => todo!(),
            (Command::StartLocalGame, State::MpOpeningNewLobby) => todo!(),
            (Command::StartLocalGame, State::MpLobbyGuest { .. }) => todo!(),
            (Command::StartLocalGame, State::MpLobbyHost { .. }) => todo!(),
            (Command::StartLocalGame, State::MpGame { .. }) => todo!(),
            (Command::StartLocalGame, State::MpServerLost(_)) => todo!(),
            (Command::StartLocalGame, State::GuiClosed) => todo!(),
            (Command::ConnectToServer { .. }, State::SpSettings) => todo!(),
            (Command::ConnectToServer { .. }, State::SpGame(_)) => todo!(),
            (Command::ConnectToServer { .. }, State::MpConnecting) => todo!(),
            (Command::ConnectToServer { .. }, State::MpView(_)) => todo!(),
            (Command::ConnectToServer { .. }, State::MpOpeningNewLobby) => todo!(),
            (Command::ConnectToServer { .. }, State::MpLobbyGuest { .. }) => todo!(),
            (Command::ConnectToServer { .. }, State::MpLobbyHost { .. }) => todo!(),
            (Command::ConnectToServer { .. }, State::MpGame { .. }) => todo!(),
            (Command::ConnectToServer { .. }, State::MpServerLost(_)) => todo!(),
            (Command::ConnectToServer { .. }, State::GuiClosed) => todo!(),
            (Command::OpenNewLobby, State::Initial) => todo!(),
            (Command::OpenNewLobby, State::SpSettings) => todo!(),
            (Command::OpenNewLobby, State::SpGame(_)) => todo!(),
            (Command::OpenNewLobby, State::MpConnecting) => todo!(),
            (Command::OpenNewLobby, State::MpOpeningNewLobby) => todo!(),
            (Command::OpenNewLobby, State::MpLobbyGuest { .. }) => todo!(),
            (Command::OpenNewLobby, State::MpLobbyHost { .. }) => todo!(),
            (Command::OpenNewLobby, State::MpGame { .. }) => todo!(),
            (Command::OpenNewLobby, State::MpServerLost(_)) => todo!(),
            (Command::OpenNewLobby, State::GuiClosed) => todo!(),
            (Command::JoinLobby(_), State::Initial) => todo!(),
            (Command::JoinLobby(_), State::SpSettings) => todo!(),
            (Command::JoinLobby(_), State::SpGame(_)) => todo!(),
            (Command::JoinLobby(_), State::MpConnecting) => todo!(),
            (Command::JoinLobby(_), State::MpView(_)) => todo!(),
            (Command::JoinLobby(_), State::MpOpeningNewLobby) => todo!(),
            (Command::JoinLobby(_), State::MpLobbyGuest { .. }) => todo!(),
            (Command::JoinLobby(_), State::MpLobbyHost { .. }) => todo!(),
            (Command::JoinLobby(_), State::MpGame { .. }) => todo!(),
            (Command::JoinLobby(_), State::MpServerLost(_)) => todo!(),
            (Command::JoinLobby(_), State::GuiClosed) => todo!(),
            (Command::UpdateSettings(_), State::Initial) => todo!(),
            (Command::UpdateSettings(_), State::SpSettings) => todo!(),
            (Command::UpdateSettings(_), State::SpGame(_)) => todo!(),
            (Command::UpdateSettings(_), State::MpConnecting) => todo!(),
            (Command::UpdateSettings(_), State::MpView(_)) => todo!(),
            (Command::UpdateSettings(_), State::MpOpeningNewLobby) => todo!(),
            (Command::UpdateSettings(_), State::MpLobbyGuest { .. }) => todo!(),
            (Command::UpdateSettings(_), State::MpGame { .. }) => todo!(),
            (Command::UpdateSettings(_), State::MpServerLost(_)) => todo!(),
            (Command::UpdateSettings(_), State::GuiClosed) => todo!(),
            (Command::StartMultiplayerGame, State::Initial) => todo!(),
            (Command::StartMultiplayerGame, State::SpSettings) => todo!(),
            (Command::StartMultiplayerGame, State::SpGame(_)) => todo!(),
            (Command::StartMultiplayerGame, State::MpConnecting) => todo!(),
            (Command::StartMultiplayerGame, State::MpView(_)) => todo!(),
            (Command::StartMultiplayerGame, State::MpOpeningNewLobby) => todo!(),
            (Command::StartMultiplayerGame, State::MpLobbyGuest { .. }) => todo!(),
            (Command::StartMultiplayerGame, State::MpGame { .. }) => todo!(),
            (Command::StartMultiplayerGame, State::MpServerLost(_)) => todo!(),
            (Command::StartMultiplayerGame, State::GuiClosed) => todo!(),
            (Command::SetAction(_), State::Disconnected) => todo!(),
            (Command::ConfigureLocalGame, State::Disconnected) => todo!(),
            (Command::StartLocalGame, State::Disconnected) => todo!(),
            (Command::ConnectToServer { server, player_name }, State::Disconnected) => todo!(),
            (Command::OpenNewLobby, State::Disconnected) => todo!(),
            (Command::JoinLobby(_), State::Disconnected) => todo!(),
            (Command::UpdateSettings(_), State::Disconnected) => todo!(),
            (Command::StartMultiplayerGame, State::Disconnected) => todo!(),
        };
    }

    async fn handle_timeout(&mut self) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match previous_state {
            State::Invalid => panic!("Invalid State"),
            State::SpGame(mut spg) => {
                spg.update_simulation_realtime();
                self.update_gui().await;
                State::SpGame(spg)
            }

            state => state,
            // state => todo!("state {:?}", &state,),
        };
    }

    async fn update_gui(&mut self) {
        (self.update_callback)();
    }
}
