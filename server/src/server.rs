use std::collections::HashMap;
use std::hash::Hash as _;
use std::hash::Hasher as _;

use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Instant;

use bomberhans_lib::game_state::*;
use bomberhans_lib::network::*;
use bomberhans_lib::utils::PlayerId;
use bomberhans_lib::utils::TimeStamp;

enum Game {
    Lobby(Lobby),
    Started(StartedGame),
}
impl Game {
    fn remove_player(&mut self, player_id: PlayerId) {
        match self {
            Game::Lobby(lobby) => lobby.game_static.players.remove(&player_id),
            Game::Started(game) => todo!(),
        };
    }
}

struct Lobby {
    id: GameId,
    game_static: GameStatic,
}

struct StartedGame {
    id: GameId,
    game_static: Rc<GameStatic>,
    game_state: GameState,
    updates: Vec<Update>,
    future_updates: Vec<Update>,
    old_updates: Vec<Update>,
}

struct ClientGame {
    /// The Game/Lobby the player is in and his Player ID
    pub game_id: GameId,

    pub player_id: PlayerId,

    /// The time of the most recent information the client acknowledged having
    pub last_acknowledge_time: TimeStamp,
}

struct Client {
    /// Session Cookie
    pub id: ClientId,

    /// Client's Player Name
    pub name: String,

    /// The client's Address, only accept packets from there, send updates there
    pub address: SocketAddr,

    /// The Client's Game if any
    game: Option<ClientGame>,
}

pub struct Server {
    name: String,
    games: HashMap<GameId, Game>,
    clients: HashMap<ClientId, Client>,
}

impl Server {
    pub fn new(name: String) -> Self {
        let games = HashMap::new();
        let clients = HashMap::new();

        Self {
            name,
            games,
            clients,
        }
    }

    pub fn handle_client_message(
        &mut self,
        msg: ClientMessage,
        client_address: SocketAddr,
    ) -> Option<ServerMessage> {
        log::debug!("Received from {client_address}: {msg:#?}");

        match msg {
            ClientMessage::OpenNewLobby(client_id)
            | ClientMessage::Update(ClientUpdate { client_id, .. })
            | ClientMessage::Bye(client_id) => {
                if let Some(client) = self.clients.get_mut(&client_id) {
                    if client.address != client_address {
                        log::warn!(
                            "discarding message from {} for {:#?} whose hello-address was {}",
                            client_address,
                            client_id,
                            client.address
                        );
                        return None;
                    }
                } else {
                    log::warn!("discarding message from {client_address} for unknown client {client_id:?}: {msg:#?}");
                    return None;
                }
            }
            _ => (),
        }

        match msg {
            ClientMessage::Hello(msg) => self
                .handle_client_helo(msg, client_address)
                .map(|msg| ServerMessage::Hello(msg)),
            ClientMessage::OpenNewLobby(msg) => self
                .handle_client_open_new_lobby(msg, client_address)
                .map(|msg| ServerMessage::LobbyUpdate(msg)),
            ClientMessage::Update(msg) => {
                self.handle_client_update(msg, client_address);
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
                None
            }
        }
    }

    fn handle_client_helo(
        &mut self,
        message: ClientHello,
        client_address: SocketAddr,
    ) -> Option<ServerHello> {
        if message.magic != BOMBERHANS_MAGIC_NO_V1 {
            return None;
        }

        let mut h = std::hash::DefaultHasher::new();
        client_address.hash(&mut h);
        message.player_name.hash(&mut h);
        let cookie = h.finish();
        let cookie = ClientId::new(cookie);

        let last_communication = Instant::now();

        let client = Client {
            name: message.player_name,
            id: cookie,
            address: client_address,
            game: None,
        };

        self.clients.insert(cookie, client);

        let server_name = self.name.clone();
        let lobbies = self
            .games
            .values()
            .filter_map(|g| match g {
                Game::Lobby(lob) => Some((lob.id, lob.game_static.settings.game_name.clone())),
                Game::Started(_) => None,
            })
            .collect();

        return Some(ServerHello {
            server_name,
            client_id: cookie,
            lobbies,
            clients_nonce: message.nonce,
        });
    }

    fn handle_client_update(&mut self, msg: ClientUpdate, client_address: SocketAddr) {
        let Some(client) = self.clients.get_mut(&msg.client_id) else {
            log::warn!(
                "update for unknown client {:?} from {}",
                msg.client_id,
                client_address
            );
            return;
        };
        if client.address != client_address {
            log::warn!(
                "update for client {:?} from wrong address {}",
                msg.client_id,
                client_address
            );
            return;
        }

        let Some(client_game) = &mut client.game else {
            log::warn!("Client Update for client that is not in Game {msg:?}");
            return;
        };

        if msg.last_server_update <= client_game.last_acknowledge_time {
            log::debug!("ignoring out of order/duplicate message {msg:?}");
            return;
        }

        client_game.last_acknowledge_time = msg.last_server_update;

        let Game::Started(game) = self
            .games
            .get_mut(&client_game.game_id)
            .expect("game exists")
        else {
            log::warn!("Client Game update for Lobby {msg:?}");
            return;
        };

        game.future_updates.push(Update {
            player: client_game.player_id,
            action: msg.current_player_action,
            time: msg.current_action_start_time,
        });
    }

    pub fn periodic_update(&mut self) -> Vec<(SocketAddr, ServerUpdate)> {
        for g in self.games.values_mut() {
            let Game::Started(game) = g else {
                continue;
            };

            let mut updates: Vec<Update> = Vec::new();
            std::mem::swap(&mut updates, &mut game.future_updates);

            game.game_state.simulate_1_update();

            for u in updates {
                if u.time > game.game_state.time {
                    game.future_updates.push(u);
                } else {
                    if game.game_state.set_player_action(u.player, u.action) {
                        game.updates.push(Update {
                            time: game.game_state.time,
                            ..u
                        });
                    }
                }
            }
        }

        self.clients
            .values()
            .filter_map(|c| {
                let cgs = c.game.as_ref()?;
                let Game::Started(game) = &self.games[&cgs.game_id] else {
                    return None;
                };
                Some((
                    c.address,
                    ServerUpdate {
                        time: game.game_state.time,
                        checksum: 0,
                        updates: game
                            .updates
                            .iter()
                            .filter(|u| u.time > cgs.last_acknowledge_time)
                            .map(Update::clone)
                            .collect(),
                    },
                ))
            })
            .collect()
    }

    fn handle_client_open_new_lobby(
        &mut self,
        msg: ClientId,
        client_address: SocketAddr,
    ) -> Option<ServerLobbyUpdate> {
        let game_id = GameId::new(rand::random());
        todo!();
    }
}
