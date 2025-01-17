use std::collections::HashMap;
use std::mem;
use std::net::SocketAddr;
use std::time::Instant;

use bomberhans_lib::field::Field;
use bomberhans_lib::game_state::*;
use bomberhans_lib::network::*;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::*;

use crate::actor::Actor;
use crate::actor::AssistantManager;
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
    player_ready: Vec<Ready>,
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
}

#[derive(Debug)]
pub struct Game {
    state: State,
    host: SocketAddr,
    clients: HashMap<SocketAddr, Client>,
    responder: AssistantManager<Response>,
}

impl Game {
    pub fn new(host_address: SocketAddr, responder: AssistantManager<Response>) -> Self {
        let clients = HashMap::new();
        let lobby = Lobby {
            settings: Settings::default(),
            players: Vec::new(),
            player_ready: Vec::new(),
        };
        Self {
            state: State::Lobby(lobby),
            host: host_address,
            clients,
            responder,
        }
    }

    async fn handle_client_request(&mut self, request: Request) {
        let client_address = request.client_address;
        match &request.packet.message {
            ClientMessage::GetLobbyList => todo!(),
            ClientMessage::OpenNewLobby(ClientOpenLobby { player_name })
            | ClientMessage::JoinLobby(ClientJoinLobby { player_name, .. }) => {
                let State::Lobby(lobby) = &mut self.state else {
                    log::warn!("rejecting join from {client_address} for started game");
                    self.responder
                        .send(request.response(ServerMessage::Bye("Game Started".to_owned())))
                        .await;
                    return;
                };

                if self.clients.len() as u32 == lobby.settings.players {
                    log::warn!("rejecting join from {client_address} for full game");
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
                    lobby.player_ready.push(Ready::NotReady);

                    player_id
                };
                self.responder
                    .send(
                        request.response(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                            settings: lobby.settings.clone(),
                            players: lobby.players.clone(),
                            players_ready: lobby.player_ready.clone(),
                            client_player_id: player_id,
                        })),
                    )
                    .await;
            }
            ClientMessage::UpdateLobbySettings(_) => todo!(),
            ClientMessage::LobbyReady(ClientLobbyReady { ready }) => {
                let client = &self.clients[&client_address];

                if let State::Lobby(lobby) = &mut self.state {
                    lobby.player_ready[client.player_id.idx()] = *ready;

                    if !lobby.player_ready.iter().all(Ready::is_ready) {
                        self.responder
                            .send(
                                request.response(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                                    settings: lobby.settings.clone(),
                                    players: lobby.players.clone(),
                                    players_ready: lobby.player_ready.clone(),
                                    client_player_id: client.player_id,
                                })),
                            )
                            .await;
                        return;
                    } else {
                        log::info!("All players ready, starting Game");

                        self.state.start();
                    }
                } else {
                    log::info!("Client set ready for started game, sending GameStart again");
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
            ClientMessage::GameUpdate(msg) => {
                let client = self
                    .clients
                    .get_mut(&client_address)
                    .expect("server would not send a message to a game that client hadn't joined");

                if msg.last_server_update < client.last_acknowledge_time {
                    log::debug!("Client ACK'd Game Time odler than the one in a previous (by packetnumber) packet");
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
                    log::warn!("Ignoring Client Game update for Lobby {msg:?}");
                    return;
                };

                game.future_updates.push(Update {
                    player: client.player_id,
                    action: msg.current_player_action,
                    time: msg.current_action_start_time,
                });
            }
            ClientMessage::Bye | ClientMessage::Ping => {
                unreachable!("Handled by server")
            }
        }
        //
        //            None => {
        //            }
        //
    }
}

impl Actor<Message> for Game {
    async fn handle(&mut self, message: Message) {
        match message {
            Message::ClientRequest(request) => self.handle_client_request(request).await,
        }
    }

    async fn close(self) {
        todo!()
    }
}
