use std::collections::HashMap;
use std::mem;
use std::net::SocketAddr;
use std::time::Instant;

use bomberhans2_lib::field::Field;
use bomberhans2_lib::game_state::{GameState, Player};
use bomberhans2_lib::network::{
    ClientJoinLobby, ClientLobbyReady, ClientMessage, ClientOpenLobby, GameId, Ready,
    ServerGameStart, ServerLobbyUpdate, ServerMessage, ServerUpdate, Update,
};
use bomberhans2_lib::settings::Settings;
use bomberhans2_lib::utils::PlayerId;
use bomberhans2_lib::utils::{GameTime, Idx, Position};

use crate::actor::Actor;
use crate::actor::AssistantManager;
use crate::server;
use crate::Request;
use crate::Response;

#[derive(Debug)]
struct Client {
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
    ClientRequest(Request),

    /// time has passed
    Update,
}

#[derive(Debug)]
struct StartedGame {
    game_state: GameState,
    updates: Vec<Update>,
    future_updates: Vec<Update>,
    old_updates: Vec<Update>,
}

#[derive(Debug)]
struct Lobby {
    settings: Settings,
    players: Vec<Player>,
    players_ready: Vec<Ready>,
}

#[derive(Debug)]
enum State {
    Lobby(Lobby),
    Started(StartedGame),
}
impl State {
    fn start(&mut self) {
        match self {
            State::Started(_) => panic!(),
            State::Lobby(lob) => {
                let settings = lob.settings.clone(); // muuh
                let players = mem::take(&mut lob.players);
                let game_state = GameState::new(settings, players);
                *self = State::Started(StartedGame {
                    game_state,
                    updates: Vec::new(),
                    future_updates: Vec::new(),
                    old_updates: Vec::new(),
                });
            }
        }
    }

    fn remove_player(&mut self, player_id: PlayerId) {
        match self {
            State::Lobby(lobby) => {
                lobby.players.remove(player_id.idx());
                lobby.players_ready.remove(player_id.idx());
            }
            State::Started(game) => {
                game.game_state.players.remove(&player_id).unwrap();
            }
        }
    }
}

#[derive(Debug)]
pub struct Game {
    game_id: GameId,
    state: State,
    host: SocketAddr,
    clients: HashMap<SocketAddr, Client>,
    responder: AssistantManager<Response>,
    server: AssistantManager<server::Message>,
}

impl Game {
    pub fn new(
        game_id: GameId,
        host_address: SocketAddr,
        responder: AssistantManager<Response>,
        server: AssistantManager<server::Message>,
    ) -> Self {
        let clients = HashMap::new();
        let lobby = Lobby {
            settings: Settings::default(),
            players: Vec::new(),
            players_ready: Vec::new(),
        };
        Self {
            game_id,
            state: State::Lobby(lobby),
            host: host_address,
            clients,
            responder,
            server,
        }
    }

    async fn handle_client_request(&mut self, request: Request) {
        let game_id = self.game_id;
        log::trace!("{game_id:?}: Handling {request:?}");
        let client_address = request.client_address;
        match &request.packet.message {
            ClientMessage::OpenNewLobby(ClientOpenLobby { player_name })
            | ClientMessage::JoinLobby(ClientJoinLobby { player_name, .. }) => {
                let State::Lobby(lobby) = &mut self.state else {
                    log::warn!(
                        "{game_id:?}: rejecting join from {client_address} for started game"
                    );
                    self.responder
                        .send(request.response(ServerMessage::Bye("Game Started".to_owned())))
                        .await;
                    return;
                };

                if self.clients.len() as u32 == lobby.settings.players {
                    log::warn!("{game_id:?}: rejecting join from {client_address} for full game");
                    self.responder
                        .send(request.response(ServerMessage::Bye("Game Full".to_owned())))
                        .await;
                    return;
                }

                let player_id = if let Some(client) = self.clients.get(&client_address) {
                    client.player_id
                } else {
                    let player_id = PlayerId(self.clients.len() as u32);

                    let client = Client {
                        address: client_address,
                        player_id,
                        last_acknowledge_time: GameTime::new(),
                        last_package_received: Instant::now(),
                    };
                    self.clients.insert(client_address, client);
                    let field = Field::new(lobby.settings.width, lobby.settings.height);
                    let start_positions = field.start_positions();
                    let start_position =
                        Position::from_cell_position(start_positions[player_id.idx()]);
                    let player = Player {
                        name: player_name.clone(),
                        id: player_id,
                        start_position,
                    };
                    lobby.players.push(player);
                    lobby.players_ready.push(Ready::NotReady);

                    player_id
                };
                self.responder
                    .send(
                        request.response(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                            settings: lobby.settings.clone(),
                            players: lobby.players.clone(),
                            players_ready: lobby.players_ready.clone(),
                            client_player_id: player_id,
                        })),
                    )
                    .await;
            }
            ClientMessage::UpdateLobbySettings(_) => todo!(),
            ClientMessage::LobbyReady(ClientLobbyReady { ready }) => {
                let client = &self.clients[&client_address];

                if let State::Lobby(lobby) = &mut self.state {
                    lobby.players_ready[client.player_id.idx()] = *ready;

                    if !lobby.players_ready.iter().all(Ready::is_ready) {
                        self.responder
                            .send(
                                request.response(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                                    settings: lobby.settings.clone(),
                                    players: lobby.players.clone(),
                                    players_ready: lobby.players_ready.clone(),
                                    client_player_id: client.player_id,
                                })),
                            )
                            .await;
                        return;
                    } else {
                        log::info!("{game_id:?}: All players ready, starting Game");

                        self.state.start();
                    }
                } else {
                    log::info!(
                        "{game_id:?}: Client set ready for started game, sending GameStart again"
                    );
                    // Game Started message was lost
                }

                let State::Started(game) = &self.state else {
                    unreachable!()
                };

                self.responder
                    .send(
                        request.response(ServerMessage::GameStart(ServerGameStart {
                            settings: game.game_state.settings.clone(),
                            players: game
                                .game_state
                                .players
                                .values()
                                .map(|(p, s)| p.clone())
                                .collect(),
                            client_player_id: client.player_id,
                        })),
                    )
                    .await;
            }
            ClientMessage::PollLobby => {
                let client = &self.clients[&client_address];
                match &self.state {
                    State::Lobby(lobby) => {
                        self.responder
                            .send(
                                request.response(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                                    settings: lobby.settings.clone(),
                                    players: lobby.players.clone(),
                                    players_ready: lobby.players_ready.clone(),
                                    client_player_id: client.player_id,
                                })),
                            )
                            .await;
                    }
                    State::Started(game) => {
                        self.responder
                            .send(
                                request.response(ServerMessage::GameStart(ServerGameStart {
                                    settings: game.game_state.settings.clone(),
                                    players: game
                                        .game_state
                                        .players
                                        .values()
                                        .map(|(p, s)| p.clone())
                                        .collect(),
                                    client_player_id: client.player_id,
                                })),
                            )
                            .await;
                    }
                };
            }
            ClientMessage::GameUpdate(msg) => {
                let client = self
                    .clients
                    .get_mut(&client_address)
                    .expect("server would not send a message to a game that client hadn't joined");

                if msg.last_server_update < client.last_acknowledge_time {
                    log::debug!("{game_id:?}: Client ACK'd Game Time odler than the one in a previous (by packetnumber) packet");
                    self.responder
                        .send(
                            request.response(ServerMessage::Bye(
                                "Cheating LastServerUpdate".to_owned(),
                            )),
                        )
                        .await;
                    return;
                }

                client.last_acknowledge_time = msg.last_server_update;

                let State::Started(game) = &mut self.state else {
                    log::warn!("{game_id:?}: Ignoring Client Game update for Lobby {msg:?}");
                    return;
                };

                game.future_updates.push(Update {
                    player: client.player_id,
                    action: msg.current_player_action,
                    time: msg.current_action_start_time,
                });
            }

            ClientMessage::Bye => {
                log::warn!("{game_id:?}: Disconnecting {client_address:?}");
                let client = self
                    .clients
                    .remove(&client_address)
                    .expect("server would not send a message to a game that client hadn't joined");

                let player_id = client.player_id;

                self.state.remove_player(player_id);

                self.clients
                    .values_mut()
                    .filter(|c| c.player_id > player_id)
                    .for_each(|c| c.player_id = PlayerId(c.player_id.0 - 1));
            }
            ClientMessage::GetLobbyList | ClientMessage::Ping => {
                unreachable!("Handled by server")
            }
        }
        //
        //            None => {
        //            }
        //
    }

    async fn handle_update(&mut self) {
        match &mut self.state {
            State::Started(game) => {
                let mut updates: Vec<Update> = Vec::new();
                std::mem::swap(&mut updates, &mut game.future_updates);

                for u in updates {
                    if u.time > game.game_state.time {
                        game.future_updates.push(u);
                    } else {
                        assert_eq!(u.time, game.game_state.time);
                        if game.game_state.set_player_action(u.player, u.action) {
                            log::trace!("GAME PLAYER ACTION: {u:?}");
                            game.updates.push(Update {
                                time: game.game_state.time,
                                ..u
                            });
                        } else {
                            log::trace!("GAME PLAYER ACTION REDUNDANT, not forwarded: {u:?}");
                        }
                    }
                }

                game.game_state.simulate_1_update();

                let checksum = game.game_state.checksum();

                log::trace!("GAME UPDATE: {:X} {:?}", checksum, game.game_state);

                for client in self.clients.values() {
                    let message = ServerMessage::Update(ServerUpdate {
                        time: game.game_state.time,
                        checksum,
                        updates: game
                            .updates
                            .iter()
                            .filter(|u| u.time > client.last_acknowledge_time)
                            .map(Update::clone)
                            .collect(),
                    });

                    self.responder
                        .send(Response {
                            client_addr: client.address,
                            message,
                            ack: None,
                        })
                        .await;
                }
            }
            State::Lobby(lobby) => {
                for client in self.clients.values() {
                    let message = ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                        client_player_id: client.player_id,
                        settings: lobby.settings.clone(),
                        players: lobby.players.clone(),
                        players_ready: lobby.players_ready.clone(),
                    });

                    self.responder
                        .send(Response {
                            client_addr: client.address,
                            message,
                            ack: None,
                        })
                        .await;
                }
            }
        }
    }
}

impl Actor<Message> for Game {
    async fn handle(&mut self, message: Message) {
        match message {
            Message::ClientRequest(request) => self.handle_client_request(request).await,
            Message::Update => self.handle_update().await,
        }
    }

    async fn close(self) {
        todo!()
    }
}
