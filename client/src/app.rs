use std::net::SocketAddr;
use std::time::Duration;

use bomberhans_lib::game_state::Action;
use bomberhans_lib::game_state::GameState;
use bomberhans_lib::game_state::Player;
use bomberhans_lib::network::GameId;
use bomberhans_lib::network::Ready;
use bomberhans_lib::network::ServerGameStart;
use bomberhans_lib::network::ServerLobbyList;
use bomberhans_lib::network::ServerLobbyUpdate;
use bomberhans_lib::network::ServerUpdate;
use bomberhans_lib::network::Update;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::GameTime;
use bomberhans_lib::utils::PlayerId;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;

use crate::communication;
use crate::communication::connect;
use crate::communication::Connection;
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
    MpView(ServerLobbyList),
    MpOpeningNewLobby,
    MpLobby {
        host: bool,
        settings: Settings,
        players: Vec<Player>,
        players_ready: Vec<Ready>,
        local_player_id: PlayerId,
    },
    MpGame {
        server_game_state: GameState,
        local_game_state: GameState,
        local_update: Update,
    },

    /// Server not responding
    MpServerLost(GameState),

    /// Connection Lost (reason)
    Disconnected(String),

    /// User clicked Exit
    GuiClosed,

    /// State machine in transition, should never be observed
    Invalid,
    MpJoiningLobby {
        game_id: GameId,
    },
}

pub fn controller() -> (GameController, GameControllerBackend) {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let backend = GameControllerBackend::new(rx);

    let frontend = GameController { tx };
    (frontend, backend)
}

struct UpdateCallback(Box<dyn Fn() + Send>);

impl std::fmt::Debug for UpdateCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Callback {}", self as *const _ as usize)
    }
}

#[derive(Debug)]
#[allow(private_interfaces)]
pub enum Command {
    SetUpdateCallback(UpdateCallback),
    SetAction(Action),
    ConfigureLocalGame,
    StartLocalGame,
    ConnectToServer(SocketAddr),
    OpenNewLobby(String),
    JoinLobby(GameId, String),
    UpdateSettings(Settings),
    SetMpReady(Ready),
    GetState(tokio::sync::oneshot::Sender<State>),
    GetPing(tokio::sync::oneshot::Sender<Option<Duration>>),
    Disconnect,
    Quit,
}

pub struct GameController {
    tx: Sender<Command>,
}

impl GameController {
    pub fn set_update_callback(&mut self, callback: Box<dyn Fn() + Send>) {
        self.tx
            .blocking_send(Command::SetUpdateCallback(UpdateCallback(callback)))
            .map_err(|e| format!("{e:#?}"))
            .unwrap();
    }
    pub fn set_action(&mut self, action: Action) {
        self.tx.blocking_send(Command::SetAction(action)).unwrap();
    }
    pub fn open_new_lobby(&mut self, player_name: String) {
        self.tx
            .blocking_send(Command::OpenNewLobby(player_name))
            .unwrap();
    }
    //   pub fn configure_local_game(&mut self) {
    //       self.tx.blocking_send(Command::ConfigureLocalGame).unwrap();
    //   }
    pub fn start_local_game(&mut self) {
        self.tx.blocking_send(Command::StartLocalGame).unwrap();
    }
    pub fn connect_to_server(&mut self, server: SocketAddr) {
        self.tx
            .blocking_send(Command::ConnectToServer(server))
            .unwrap();
    }
    pub fn join_lobby(&mut self, lobby_id: GameId, player_name: String) {
        self.tx
            .blocking_send(Command::JoinLobby(lobby_id, player_name))
            .unwrap();
    }
    pub fn update_settings(&mut self, new_settings: Settings) {
        self.tx
            .blocking_send(Command::UpdateSettings(new_settings))
            .unwrap();
    }
    pub fn set_ready(&mut self, ready: Ready) {
        self.tx.blocking_send(Command::SetMpReady(ready)).unwrap();
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
    pub fn get_ping(&self) -> Option<Duration> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .blocking_send(Command::GetPing(tx))
            .expect("Controller doesn't panic");
        rx.blocking_recv().expect("controller doesn't panic")
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
    update_callback: UpdateCallback,

    rx_from_frontend: Receiver<Command>,

    /// Server's response time
    ping: Option<Duration>,
}

impl GameControllerBackend {
    fn new(rx_from_frontend: Receiver<Command>) -> Self {
        Self {
            state: State::Initial,
            connection: None,
            rx_from_frontend,
            update_callback: UpdateCallback(Box::new(|| panic!("no gui callback set"))),
            ping: None,
        }
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

    async fn handle_server_event(&mut self, event: communication::Event) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match (event, previous_state) {
            (_, State::Invalid) => panic!("Invalid State"),
            (communication::Event::Error(e), _) => {
                // TODO: display something
                State::Disconnected(format!("Communication Error {e}"))
            }
            //           (communication::Event::Disconnected, _) => {
            //               State::Disconnected(format!("server kicked us? "))
            //           }
            (_, State::Disconnected(msg)) => State::Disconnected(msg),
            (communication::Event::Disconnect(reason), _) => State::Disconnected(reason),
            (communication::Event::Ping(ping), state) => {
                self.ping = Some(ping);
                state
            }

            (
                communication::Event::GameListUpdated(server_lobby_list),
                State::MpConnecting | State::MpView(_),
            ) => State::MpView(server_lobby_list),

            (
                communication::Event::LobbyUpdated(server_lobby_update),
                State::MpJoiningLobby { .. },
            ) => {
                let ServerLobbyUpdate {
                    settings,
                    players,
                    players_ready,
                    client_player_id,
                } = server_lobby_update;
                State::MpLobby {
                    host: false,
                    settings,
                    players,
                    players_ready,
                    local_player_id: client_player_id,
                }
            }
            (communication::Event::LobbyUpdated(server_lobby_update), State::MpOpeningNewLobby) => {
                let ServerLobbyUpdate {
                    settings,
                    players,
                    players_ready,
                    client_player_id,
                } = server_lobby_update;
                State::MpLobby {
                    host: true,
                    settings,
                    players,
                    players_ready,
                    local_player_id: client_player_id,
                }
            }
            (
                communication::Event::LobbyUpdated(server_lobby_update),
                State::MpLobby { host, .. },
            ) => {
                let ServerLobbyUpdate {
                    settings,
                    players,
                    players_ready,
                    client_player_id,
                } = server_lobby_update;
                State::MpLobby {
                    host,
                    settings,
                    players,
                    players_ready,
                    local_player_id: client_player_id,
                }
            }

            (communication::Event::GameStart(server_game_start), State::MpLobby { .. }) => {
                let ServerGameStart {
                    settings,
                    players,
                    client_player_id,
                } = server_game_start;

                log::info!("Game Started");

                let server_game_state = GameState::new(settings, players);

                let local_update = Update {
                    player: client_player_id,
                    action: Action::idle(),
                    time: GameTime::new(),
                };

                let local_game_state = server_game_state.clone();

                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                }
            }

            (
                communication::Event::Update(update),
                State::MpGame {
                    server_game_state,
                    local_update,
                    local_game_state: old_local_game_state,
                },
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
                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                }
            }
            //
            (communication::Event::GameListUpdated(_), State::Initial) => todo!(),
            (communication::Event::GameListUpdated(_), State::SpSettings) => todo!(),
            (communication::Event::GameListUpdated(_), State::SpGame(_)) => todo!(),
            (communication::Event::GameListUpdated(_), State::MpOpeningNewLobby) => todo!(),
            (
                communication::Event::GameListUpdated(_),
                State::MpLobby {
                    host,
                    settings,
                    players,
                    players_ready,
                    local_player_id,
                },
            ) => todo!(),
            (
                communication::Event::GameListUpdated(_),
                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                },
            ) => todo!(),
            (communication::Event::GameListUpdated(_), State::MpServerLost(_)) => todo!(),
            (communication::Event::GameListUpdated(_), State::GuiClosed) => todo!(),
            (communication::Event::GameListUpdated(_), State::MpJoiningLobby { game_id }) => {
                todo!()
            }
            (communication::Event::LobbyUpdated(_), State::Initial) => todo!(),
            (communication::Event::LobbyUpdated(_), State::SpSettings) => todo!(),
            (communication::Event::LobbyUpdated(_), State::SpGame(_)) => todo!(),
            (communication::Event::LobbyUpdated(_), State::MpConnecting) => todo!(),
            (communication::Event::LobbyUpdated(_), State::MpView(_)) => todo!(),
            (
                communication::Event::LobbyUpdated(_),
                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                },
            ) => todo!(),
            (communication::Event::LobbyUpdated(_), State::MpServerLost(_)) => todo!(),
            (communication::Event::LobbyUpdated(_), State::GuiClosed) => todo!(),
            (communication::Event::GameStart(_), State::Initial) => todo!(),
            (communication::Event::GameStart(_), State::SpSettings) => todo!(),
            (communication::Event::GameStart(_), State::SpGame(_)) => todo!(),
            (communication::Event::GameStart(_), State::MpConnecting) => todo!(),
            (communication::Event::GameStart(_), State::MpView(_)) => todo!(),
            (communication::Event::GameStart(_), State::MpOpeningNewLobby) => todo!(),
            (
                communication::Event::GameStart(_),
                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                },
            ) => todo!(),
            (communication::Event::GameStart(_), State::MpServerLost(_)) => todo!(),
            (communication::Event::GameStart(_), State::GuiClosed) => todo!(),
            (communication::Event::GameStart(_), State::MpJoiningLobby { game_id }) => todo!(),
            (communication::Event::Update(_), State::Initial) => todo!(),
            (communication::Event::Update(_), State::SpSettings) => todo!(),
            (communication::Event::Update(_), State::SpGame(_)) => todo!(),
            (communication::Event::Update(_), State::MpConnecting) => todo!(),
            (communication::Event::Update(_), State::MpView(_)) => todo!(),
            (communication::Event::Update(_), State::MpOpeningNewLobby) => todo!(),
            (
                communication::Event::Update(_),
                State::MpLobby {
                    host,
                    settings,
                    players,
                    players_ready,
                    local_player_id,
                },
            ) => todo!(),
            (communication::Event::Update(_), State::MpServerLost(_)) => todo!(),
            (communication::Event::Update(_), State::GuiClosed) => todo!(),
            (communication::Event::Update(_), State::MpJoiningLobby { game_id }) => todo!(),
        };

        self.update_gui();
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
            (Command::GetPing(return_channel), state) => {
                return_channel.send(self.ping).unwrap();
                state
            }
            (Command::Disconnect, _) => {
                if let Some(connection) = self.connection.take() {
                    connection.disconnect().await;
                }
                self.ping = None;
                State::Initial
            }
            (Command::Quit, _) => {
                if let Some(connection) = self.connection.take() {
                    connection.disconnect().await;
                }
                self.ping = None;
                State::GuiClosed
            }
            (Command::StartLocalGame, State::Initial) => {
                State::SpGame(SinglePlayerGame::new(
                    Settings::default(), /*TODO: make settings configurable*/
                ))
            }
            (Command::SetAction(action), State::SpGame(mut game_state)) => {
                game_state.set_local_player_action(action);
                State::SpGame(game_state)
            }
            (Command::ConnectToServer(server), State::Initial) => {
                self.connection = Some(connect(server));
                State::MpConnecting
            }
            (Command::JoinLobby(game_id, player_name), State::MpView(_)) => {
                self.connection
                    .as_ref()
                    .unwrap()
                    .join_lobby(game_id, player_name)
                    .await;
                State::MpJoiningLobby { game_id }
            }
            (Command::OpenNewLobby(player_name), State::MpView(_)) => {
                self.connection
                    .as_ref()
                    .unwrap()
                    .open_new_lobby(player_name)
                    .await;
                State::MpOpeningNewLobby
            }
            //(Command::UpdateSettings(settings), State::SpSettings(_)) => SpSettings(settings),
            (
                Command::UpdateSettings(settings),
                State::MpLobby {
                    host,
                    players,
                    players_ready,
                    local_player_id,
                    ..
                },
            ) if host => {
                self.connection
                    .as_ref()
                    .unwrap()
                    .update_settings(settings.clone())
                    .await;
                State::MpLobby {
                    settings,
                    host,
                    players,
                    players_ready,
                    local_player_id,
                }
            }
            (Command::SetMpReady(ready), state @ State::MpLobby { .. }) => {
                self.connection.as_ref().unwrap().set_ready(ready).await;
                state
            }
            (
                Command::SetAction(action),
                State::MpGame {
                    server_game_state,
                    mut local_game_state,
                    local_update: old_local_update,
                },
            ) => {
                let local_update = Update {
                    action,
                    time: local_game_state.time,
                    player: old_local_update.player,
                };
                if local_game_state.set_player_action(local_update.player, action) {
                    self.connection
                        .as_ref()
                        .unwrap()
                        .set_action(local_update.time, local_update.action)
                        .await;
                }
                State::MpGame {
                    server_game_state,
                    local_game_state,
                    local_update,
                }
            }

            //
            (Command::SetAction(_), State::Initial) => todo!(),
            (Command::SetAction(_), State::SpSettings) => todo!(),
            (Command::SetAction(_), State::MpConnecting) => todo!(),
            (Command::SetAction(_), State::MpView(_)) => todo!(),
            (Command::SetAction(_), State::MpOpeningNewLobby) => todo!(),
            (Command::SetAction(_), State::MpLobby { .. }) => todo!(),
            (Command::SetAction(_), State::MpServerLost(_)) => todo!(),
            (Command::SetAction(_), State::Disconnected(_)) => todo!(),
            (Command::SetAction(_), State::GuiClosed) => todo!(),
            (Command::SetAction(_), State::MpJoiningLobby { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::Initial) => todo!(),
            (Command::ConfigureLocalGame, State::SpSettings) => todo!(),
            (Command::ConfigureLocalGame, State::SpGame(_)) => todo!(),
            (Command::ConfigureLocalGame, State::MpConnecting) => todo!(),
            (Command::ConfigureLocalGame, State::MpView(_)) => todo!(),
            (Command::ConfigureLocalGame, State::MpOpeningNewLobby) => todo!(),
            (Command::ConfigureLocalGame, State::MpLobby { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::MpGame { .. }) => todo!(),
            (Command::ConfigureLocalGame, State::MpServerLost(_)) => todo!(),
            (Command::ConfigureLocalGame, State::Disconnected(_)) => todo!(),
            (Command::ConfigureLocalGame, State::GuiClosed) => todo!(),
            (Command::ConfigureLocalGame, State::MpJoiningLobby { .. }) => todo!(),
            (Command::StartLocalGame, State::SpSettings) => todo!(),
            (Command::StartLocalGame, State::SpGame(_)) => todo!(),
            (Command::StartLocalGame, State::MpConnecting) => todo!(),
            (Command::StartLocalGame, State::MpView(_)) => todo!(),
            (Command::StartLocalGame, State::MpOpeningNewLobby) => todo!(),
            (Command::StartLocalGame, State::MpLobby { .. }) => todo!(),
            (Command::StartLocalGame, State::MpGame { .. }) => todo!(),
            (Command::StartLocalGame, State::MpServerLost(_)) => todo!(),
            (Command::StartLocalGame, State::Disconnected(_)) => todo!(),
            (Command::StartLocalGame, State::GuiClosed) => todo!(),
            (Command::StartLocalGame, State::MpJoiningLobby { .. }) => todo!(),
            (Command::ConnectToServer(_), State::SpSettings) => todo!(),
            (Command::ConnectToServer(_), State::SpGame(_)) => todo!(),
            (Command::ConnectToServer(_), State::MpConnecting) => todo!(),
            (Command::ConnectToServer(_), State::MpView(_)) => todo!(),
            (Command::ConnectToServer(_), State::MpOpeningNewLobby) => todo!(),
            (Command::ConnectToServer(_), State::MpLobby { .. }) => todo!(),
            (Command::ConnectToServer(_), State::MpGame { .. }) => todo!(),
            (Command::ConnectToServer(_), State::MpServerLost(_)) => todo!(),
            (Command::ConnectToServer(_), State::Disconnected(_)) => todo!(),
            (Command::ConnectToServer(_), State::GuiClosed) => todo!(),
            (Command::ConnectToServer(_), State::MpJoiningLobby { .. }) => todo!(),
            (Command::OpenNewLobby(_), State::Initial) => todo!(),
            (Command::OpenNewLobby(_), State::SpSettings) => todo!(),
            (Command::OpenNewLobby(_), State::SpGame(_)) => todo!(),
            (Command::OpenNewLobby(_), State::MpConnecting) => todo!(),
            (Command::OpenNewLobby(_), State::MpOpeningNewLobby) => todo!(),
            (Command::OpenNewLobby(_), State::MpLobby { .. }) => todo!(),
            (Command::OpenNewLobby(_), State::MpGame { .. }) => todo!(),
            (Command::OpenNewLobby(_), State::MpServerLost(_)) => todo!(),
            (Command::OpenNewLobby(_), State::Disconnected(_)) => todo!(),
            (Command::OpenNewLobby(_), State::GuiClosed) => todo!(),
            (Command::OpenNewLobby(_), State::MpJoiningLobby { .. }) => todo!(),
            (Command::JoinLobby(_, _), State::Initial) => todo!(),
            (Command::JoinLobby(_, _), State::SpSettings) => todo!(),
            (Command::JoinLobby(_, _), State::SpGame(_)) => todo!(),
            (Command::JoinLobby(_, _), State::MpConnecting) => todo!(),
            (Command::JoinLobby(_, _), State::MpOpeningNewLobby) => todo!(),
            (Command::JoinLobby(_, _), State::MpLobby { .. }) => todo!(),
            (Command::JoinLobby(_, _), State::MpGame { .. }) => todo!(),
            (Command::JoinLobby(_, _), State::MpServerLost(_)) => todo!(),
            (Command::JoinLobby(_, _), State::Disconnected(_)) => todo!(),
            (Command::JoinLobby(_, _), State::GuiClosed) => todo!(),
            (Command::JoinLobby(_, _), State::MpJoiningLobby { .. }) => todo!(),
            (Command::UpdateSettings(_), State::Initial) => todo!(),
            (Command::UpdateSettings(_), State::SpSettings) => todo!(),
            (Command::UpdateSettings(_), State::SpGame(_)) => todo!(),
            (Command::UpdateSettings(_), State::MpConnecting) => todo!(),
            (Command::UpdateSettings(_), State::MpView(_)) => todo!(),
            (Command::UpdateSettings(_), State::MpOpeningNewLobby) => todo!(),
            (Command::UpdateSettings(_), State::MpLobby { .. }) => todo!(),
            (Command::UpdateSettings(_), State::MpGame { .. }) => todo!(),
            (Command::UpdateSettings(_), State::MpServerLost(_)) => todo!(),
            (Command::UpdateSettings(_), State::Disconnected(_)) => todo!(),
            (Command::UpdateSettings(_), State::GuiClosed) => todo!(),
            (Command::UpdateSettings(_), State::MpJoiningLobby { .. }) => todo!(),
            (Command::SetMpReady(_), State::Initial) => todo!(),
            (Command::SetMpReady(_), State::SpSettings) => todo!(),
            (Command::SetMpReady(_), State::SpGame(_)) => todo!(),
            (Command::SetMpReady(_), State::MpConnecting) => todo!(),
            (Command::SetMpReady(_), State::MpView(_)) => todo!(),
            (Command::SetMpReady(_), State::MpOpeningNewLobby) => todo!(),
            (Command::SetMpReady(_), State::MpGame { .. }) => todo!(),
            (Command::SetMpReady(_), State::MpServerLost(_)) => todo!(),
            (Command::SetMpReady(_), State::Disconnected(_)) => todo!(),
            (Command::SetMpReady(_), State::GuiClosed) => todo!(),
            (Command::SetMpReady(_), State::MpJoiningLobby { .. }) => todo!(),
        };
    }

    async fn handle_timeout(&mut self) {
        let previous_state = std::mem::replace(&mut self.state, State::Invalid);
        self.state = match previous_state {
            State::Invalid => panic!("Invalid State"),
            State::SpGame(mut spg) => {
                spg.update_simulation_realtime();
                self.update_gui();
                State::SpGame(spg)
            }

            state => state,
            // state => todo!("state {:?}", &state,),
        };
    }

    fn update_gui(&mut self) {
        (self.update_callback.0)();
    }
}
