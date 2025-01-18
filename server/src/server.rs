use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use bomberhans_lib::network::*;
use tokio::task::JoinHandle;

use crate::actor::launch;
use crate::actor::Actor;
use crate::actor::AssistantManager;
use crate::actor::Manager;
use crate::game;
use crate::Request;
use crate::Response;

#[derive(Debug)]
pub enum Message {
    Request(Request),
    GameStarted(GameId),
    GameClosed(GameId),
}

#[derive(Debug)]
struct Game {
    name: String,
    started: bool,
    manager: Manager<game::Message>,
}

#[derive(Debug)]
pub struct Server {
    server_name: String,
    games: HashMap<GameId, Game>,
    client_games: HashMap<SocketAddr, GameId>,
    responder: Manager<Response>,
    server: AssistantManager<Message>,
}

impl Server {
    pub fn new(
        server_name: String,
        responder: Manager<Response>,
        server: AssistantManager<Message>,
    ) -> Self {
        Self {
            server_name,
            games: HashMap::new(),
            client_games: HashMap::new(),
            responder,
            server,
        }
    }

    async fn handle_request(&mut self, request: Request) {
        log::trace!("handling {request:?}");

        let Request {
            client_address,
            packet,
        } = &request;

        // answer those request we can immediately.
        // the rest is sent to the client's game.
        let game_id: GameId = match &packet.message {
            ClientMessage::GetLobbyList => {
                let server_name = self.server_name.clone();
                let lobbies = self
                    .games
                    .iter()
                    .filter_map(|(game_id, game)| {
                        if !game.started {
                            Some((*game_id, game.name.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                self.responder
                    .send(request.response(ServerMessage::LobbyList(ServerLobbyList {
                        server_name,
                        lobbies,
                    })))
                    .await;
                return;
            }
            ClientMessage::Ping => {
                self.responder
                    .send(request.response(ServerMessage::Pong))
                    .await;
                return;
            }
            ClientMessage::Bye => {
                let Some(game_id) = self.client_games.remove(&client_address) else {
                    log::trace!("Bye from client not in any game {client_address:?}");
                    return;
                };
                log::info!("Removed client from client_game {client_address:?}");
                // send request to Game to remove player form Game
                game_id
            }

            ClientMessage::OpenNewLobby(message) => {
                if let Some(game_id) = self.client_games.get(client_address) {
                    //already in game, our answer was lost. Let game send a Lobby Update
                    *game_id
                } else {
                    // create new Game and new Client
                    let game_id = GameId::new(rand::random());
                    let game_actor = game::Game::new(
                        game_id,
                        *client_address,
                        self.responder.assistant(),
                        self.server.assistant(),
                    );
                    let manager = launch(|tx| game_actor);
                    let game = Game {
                        name: "Untitled Game".to_owned(),
                        started: false,
                        manager,
                    };

                    let old = self.client_games.insert(*client_address, game_id);
                    debug_assert!(old.is_none());
                    let old = self.games.insert(game_id, game);
                    debug_assert!(old.is_none());

                    // Let game send the Lobby Update
                    game_id
                }
            }
            ClientMessage::JoinLobby(ClientJoinLobby {
                game_id,
                player_name,
            }) => {
                if let Some(client_game) = self.client_games.get(client_address) {
                    //already in game, our answer was lost. Let game send a Lobby Update
                    *game_id
                } else {
                    let Some(game) = self.games.get(game_id) else {
                        log::warn!(
                            "Ignoring {client_address:?} wants to join unknown game {game_id:?}"
                        );
                        return;
                    };

                    let old = self.client_games.insert(*client_address, *game_id);
                    debug_assert!(old.is_none());
                    *game_id
                }
            }
            ClientMessage::UpdateLobbySettings(_)
            | ClientMessage::LobbyReady(_)
            | ClientMessage::GameUpdate(_) => {
                let Some(game_id) = self.client_games.get(client_address) else {
                    log::warn!("ignore Game message for {client_address}, not in a game");
                    return;
                };
                *game_id
            }
        };

        log::trace!("Sending request to game {game_id:?}");
        self.games[&game_id]
            .manager
            .send(game::Message::ClientRequest(request))
            .await;
    }
}

impl Actor<Message> for Server {
    async fn handle(&mut self, message: Message) {
        match message {
            Message::Request(request) => self.handle_request(request).await,
            Message::GameStarted(game_id) => {
                self.games.get_mut(&game_id).unwrap().started = true;
            }
            Message::GameClosed(game_id) => {
                self.games.remove(&game_id).unwrap();

                debug_assert!(!self.client_games.values().any(|&gid| gid == game_id));
            }
        }
    }

    async fn close(self) {
        for game in self.games.values() {
            game.manager.close().await;
        }
        for (_game_id, game) in self.games {
            game.manager.join().await;
        }
        self.responder.close_and_join().await;
    }
}
