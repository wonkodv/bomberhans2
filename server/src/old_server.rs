use std::collections::HashMap;
use std::hash::Hash as _;
use std::hash::Hasher as _;

use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;

use bomberhans_lib::field::Field;
use bomberhans_lib::game_state::{GameState, Player};
use bomberhans_lib::network::*;
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::GameTime;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::Position;

impl Game {
    fn remove_player(&mut self, player_id: PlayerId) {
        match self {
            Game::Lobby(lobby) => {
                lobby.players.remove(
                    lobby
                        .players
                        .iter()
                        .position(|p| p.id == player_id)
                        .expect("player to remove exists"),
                );
            }
            Game::Started(game) => todo!(),
            Game::Invalid => panic!(),
        };
    }
}

#[derive(Debug)]
struct Lobby {
    id: GameId,
    host: ClientId,
    settings: Settings,
    players: Vec<Player>,
}

#[derive(Debug)]
struct StartedGame {
    id: GameId,
    game_state: GameState,
    updates: Vec<Update>,
    future_updates: Vec<Update>,
    old_updates: Vec<Update>,
}

#[derive(Debug)]
struct ClientGame {
    /// The Game/Lobby the player is in and his Player ID
    pub game_id: GameId,

    pub player_id: PlayerId,

    /// The time of the most recent information the client acknowledged having
    pub last_acknowledge_time: GameTime,
}

#[derive(Debug)]
struct Client {
    /// Session Cookie
    pub id: ClientId,

    /// Client's Player Name
    pub name: String,

    /// The client's Address, only accept packets from there, send updates there
    pub address: SocketAddr,

    /// The Client's Game if any
    game: Option<ClientGame>,

    /// Number of the most recent packet, that we have received
    last_received_packet_number: u32,

    /// The time of the most recent communication with client
    pub last_package_received: Instant,
}

#[derive(Debug)]
pub struct Server {
    name: String,
    games: HashMap<GameId, Game>,
    clients: HashMap<ClientId, Client>,

    /// Number of the packet we most recently sent
    last_sent_packet_number: u32,
}

impl Server {
    pub fn new(name: String) -> Self {
        let games = HashMap::new();
        let clients = HashMap::new();

        Self {
            name,
            games,
            clients,
            last_sent_packet_number: 1,
        }
    }

    pub fn handle_client_packet(
        &mut self,
        packet: ClientPacket,
        client_address: SocketAddr,
    ) -> Option<ServerPacket> {
        if packet.magic != BOMBERHANS_MAGIC_NO_V1 {
            log::warn!("ignoring unknown protocol {packet:?}");
            return None;
        }

        match &packet.message {
            ClientMessage::OpenNewLobby(client_id)
            | ClientMessage::LobbySettingsUpdate(client_id, settings)
            | ClientMessage::GameUpdate(ClientUpdate { client_id, .. })
            | ClientMessage::JoinLobby(client_id, _)
            | ClientMessage::GameStart(client_id)
            | ClientMessage::Bye(client_id) => {
                if let Some(client) = self.clients.get_mut(client_id) {
                    if client.address != client_address {
                        log::warn!(
                            "discarding message from {} for {:#?} whose hello-address was {}",
                            client_address,
                            client_id,
                            client.address
                        );
                        return None;
                    }
                    if packet.packet_number <= client.last_received_packet_number {
                        log::warn!("ignoring out of order packet {packet:?}");
                        return None;
                    }

                    client.last_received_packet_number = packet.packet_number;
                    client.last_package_received = Instant::now();
                } else {
                    log::warn!("discarding packet from {client_address} for unknown client {client_id:?}: {packet:#?}");
                    return None;
                }
            }
            ClientMessage::GetLobbyList(_) => {}
        }

        log::debug!("Received from {client_address}: {packet:#?}");

        let message = match packet.message {
            ClientMessage::GetLobbyList(msg) => {
                let clients_packet_number = packet.packet_number;
                let mut h = std::hash::DefaultHasher::new();
                client_address.hash(&mut h);
                msg.player_name.hash(&mut h);
                let cookie = h.finish();
                let client_id = ClientId::new(cookie);

                let new_client = Client {
                    name: msg.player_name,
                    id: client_id,
                    address: client_address,
                    game: None,
                    last_received_packet_number: 0,
                    last_package_received: Instant::now(),
                };

                if let Some(old_client) = self.clients.insert(client_id, new_client) {
                    if let Some(game) = &old_client.game {
                        self.games
                            .get_mut(&game.game_id)
                            .unwrap()
                            .remove_player(game.player_id);
                    }
                }

                let server_name = self.name.clone();
                let lobbies = self
                    .games
                    .values()
                    .filter_map(|g| match g {
                        Game::Lobby(lob) => Some((lob.id, lob.settings.game_name.clone())),
                        Game::Started(_) => None,
                        Game::Invalid => panic!(),
                    })
                    .collect();

                Some(ServerMessage::GameList(ServerLobbyList {
                    clients_packet_number,
                    client_id,
                    server_name,
                    lobbies,
                }))
            }

            ClientMessage::OpenNewLobby(client_id) => {
                let client = self.clients.get_mut(&client_id).unwrap();

                if client.game.is_some() {
                    log::warn!("discarding OpenNewLobby for player in another game {client:?}");
                    return None;
                }

                let game_id = GameId::new(rand::random());
                let settings = Settings::default();

                let field = Field::new(settings.width, settings.height);
                let start_positions = field.start_positions();
                let start_position = Position::from_cell_position(start_positions[0]);
                let player_id = bomberhans_lib::utils::PlayerId(0);
                let players = vec![Player {
                    name: client.name.clone(),
                    id: player_id,
                    start_position,
                }];

                let old = self.games.insert(
                    game_id,
                    Game::Lobby(Lobby {
                        id: game_id,
                        host: client_id,
                        settings: settings.clone(),
                        players: players.clone(),
                    }),
                );
                assert!(old.is_none());

                client.game = Some(ClientGame {
                    game_id,
                    player_id,
                    last_acknowledge_time: GameTime::new(),
                });

                Some(ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                    client_player_id: player_id,
                    settings,
                    players,
                    id: game_id,
                }))
            }
            ClientMessage::JoinLobby(_, _) => todo!(),
            ClientMessage::GameUpdate(msg) => {
                let Some(client) = self.clients.get_mut(&msg.client_id) else {
                    log::warn!(
                        "Ignoring update for unknown client {:?} from {}",
                        msg.client_id,
                        client_address
                    );
                    return None;
                };
                if client.address != client_address {
                    log::warn!(
                        "Ignoring update for client {:?} from wrong address {}",
                        msg.client_id,
                        client_address
                    );
                    return None;
                }

                let Some(client_game) = &mut client.game else {
                    log::warn!("Ignoring Client Update for client that is not in Game {msg:?}");
                    return None;
                };

                if msg.last_server_update < client_game.last_acknowledge_time {
                    log::debug!("ignoring out of order/duplicate message {msg:?}");
                    return None;
                }

                client_game.last_acknowledge_time = msg.last_server_update;

                let Game::Started(game) = self
                    .games
                    .get_mut(&client_game.game_id)
                    .expect("game exists")
                else {
                    log::warn!("Ignoring Client Game update for Lobby {msg:?}");
                    return None;
                };

                game.future_updates.push(Update {
                    player: client_game.player_id,
                    action: msg.current_player_action,
                    time: msg.current_action_start_time,
                });
                None
            }
            ClientMessage::Bye(client_id) => {
                let client = self.clients.remove(&client_id).unwrap();

                if let Some(game) = client.game {
                    self.games
                        .get_mut(&game.game_id)
                        .unwrap()
                        .remove_player(game.player_id);
                }
                // TODO: send LobbyUpdate to all other Players
                None
            }
            ClientMessage::LobbySettingsUpdate(message) => {
                let client = &self.clients[&message.client_id];
                let Some(client_game) = &client.game else {
                    log::warn!("Ignoring LobbySettingsUpdate for client not in game {message:?}");
                    return None;
                };
                let Some(Game::Lobby(lobby)) = self.games.get_mut(&client_game.game_id) else {
                    log::warn!(
                        "discarding LobbySettingsUpdate for non-lobby {:?} {:?}",
                        message.client_id,
                        client_game.game_id
                    );
                    return None;
                };
                if lobby.host != message.client_id {
                    log::warn!(
                        "discarding LobbySettingsUpdate from non-host {:?} {:?}",
                        message.client_id,
                        client_game.game_id
                    );
                    return None;
                }
                lobby.settings = message.settings;

                // TODO: send LobbyUpdate to all other Players
                None
            }
            ClientMessage::GameStart(client_id) => {
                let client = &self.clients[&client_id];
                let Some(client_game) = &client.game else {
                    log::warn!("Ignoring GameState for client not in game {client_id:?}");
                    return None;
                };
                let game = self.games.get_mut(&client_game.game_id);
                let Some(game) = game else {
                    log::warn!(
                        "discarding Start for unknown game  {:?} {:?}",
                        client_id,
                        client_game.game_id
                    );
                    return None;
                };
                let Game::Lobby(lobby) = game else {
                    log::info!(
                        "discarding Start for started game  {:?} {:?}",
                        client_id,
                        client_game.game_id
                    );
                    return None;
                };
                if lobby.host != client_id {
                    log::warn!(
                        "discarding Start from non-host  {:?} {:?}",
                        client_id,
                        client_game.game_id
                    );
                    return None;
                }

                let lobby = std::mem::replace(game, Game::Invalid);
                let Game::Lobby(lobby) = lobby else {
                    unreachable!();
                };

                *game = Game::Started(StartedGame {
                    id: lobby.id,
                    game_state: GameState::new(lobby.settings, lobby.players),
                    updates: Vec::new(),
                    future_updates: Vec::new(),
                    old_updates: Vec::new(),
                });

                None
            }
        };

        message.map(|message| {
            self.last_sent_packet_number += 1;
            ServerPacket {
                magic: BOMBERHANS_MAGIC_NO_V1,
                packet_number: self.last_sent_packet_number,
                message,
            }
        })
    }

    pub fn periodic_update(&mut self) -> Vec<(SocketAddr, ServerPacket)> {
        for g in self.games.values_mut() {
            let Game::Started(game) = g else {
                continue;
            };

            let mut updates: Vec<Update> = Vec::new();
            std::mem::swap(&mut updates, &mut game.future_updates);

            for u in updates {
                if u.time > game.game_state.time {
                    game.future_updates.push(u);
                } else if game.game_state.set_player_action(u.player, u.action) {
                    game.updates.push(Update {
                        time: game.game_state.time,
                        ..u
                    });
                }
            }

            game.game_state.simulate_1_update();
        }

        self.clients
            .values()
            .filter_map(|client| {
                let cgs = client.game.as_ref()?;
                if client.last_package_received.elapsed() > Duration::from_secs(5) {
                    // don't send to clients we haven't heard from in a while
                    // but also do not delete them yet.
                    return None;
                }
                Some((
                    client.address,
                    match &self.games[&cgs.game_id] {
                        Game::Started(game) => ServerMessage::Update(ServerUpdate {
                            time: game.game_state.time,
                            checksum: 0, // TODO: Checksum
                            updates: game
                                .updates
                                .iter()
                                .filter(|u| u.time > cgs.last_acknowledge_time)
                                .map(Update::clone)
                                .collect(),
                        }),
                        Game::Lobby(lobby) => ServerMessage::LobbyUpdate(ServerLobbyUpdate {
                            client_player_id: cgs.player_id,
                            settings: lobby.settings.clone(),
                            players: lobby.players.clone(),
                            id: lobby.id,
                        }),

                        Game::Invalid => panic!(),
                    },
                ))
            })
            .map(|(addr, message)| {
                self.last_sent_packet_number += 1;
                (
                    addr,
                    ServerPacket {
                        magic: BOMBERHANS_MAGIC_NO_V1,
                        packet_number: self.last_sent_packet_number,
                        message,
                    },
                )
            })
            .collect()
    }

    fn handle_client_open_new_lobby(
        &mut self,
        client_id: ClientId,
        client_address: SocketAddr,
    ) -> Option<ServerLobbyUpdate> {
        let client = &self.clients[&client_id];

        let id = GameId::new(rand::random());
        let settings = Settings::default();

        let field = Field::new(settings.width, settings.height);
        let start_positions = field.start_positions();
        let start_position = Position::from_cell_position(start_positions[0]);
        let players = vec![Player {
            name: client.name.clone(),
            id: bomberhans_lib::utils::PlayerId(0),
            start_position,
        }];

        let old = self.games.insert(
            id,
            Game::Lobby(Lobby {
                id,
                host: client_id,
                settings: settings.clone(),
                players: players.clone(),
            }),
        );
        assert!(old.is_none());

        Some(ServerLobbyUpdate {
            client_player_id: bomberhans_lib::utils::PlayerId(0),
            settings,
            players,
            id,
        })
    }
}
