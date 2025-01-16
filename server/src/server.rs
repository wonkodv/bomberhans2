use std::collections::HashMap;
use std::net::SocketAddr;

use bomberhans_lib::network::*;

use crate::actor::launch;
use crate::actor::Actor;
use crate::actor::Manager;
use crate::game;
use crate::Request;
use crate::Response;

pub enum Message {
    Request(Request),
}

struct Game {
    name: String,
    started: bool,
    manager: Manager<game::Message>,
}

pub struct Server {
    server_name: String,
    games: HashMap<GameId, Game>,
    client_games: HashMap<SocketAddr, GameId>,
    responder: Manager<Response>,
}

impl Server {
    pub fn new(server_name: String, responder: Manager<Response>) -> Self {
        Self {
            server_name,
            games: HashMap::new(),
            client_games: HashMap::new(),
            responder,
        }
    }

    async fn handle_request(&mut self, request: Request) {
        let Request {
            client_address,
            packet,
        } = &request;

        // answer those request we can immediately.
        // the rest is sent to the client's game.
        match &packet.message {
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
                    .send(request.respond(ServerMessage::LobbyList(ServerLobbyList {
                        server_name,
                        lobbies,
                    })))
                    .await;
                return;
            }
            ClientMessage::Ping => {
                self.responder
                    .send(request.respond(ServerMessage::Pong))
                    .await;
                return;
            }
            ClientMessage::Bye => {
                if self.client_games.get(&client_address).is_none() {
                    return;
                }
            }

            ClientMessage::OpenNewLobby(message) => {
                let game_actor = game::Game::new(*client_address);
                let manager = launch(game_actor);
                let game = Game {
                    name: "Untitled Game".to_owned(),
                    started: false,
                    manager,
                };

                let id = GameId::new(rand::random());

                let old = self.client_games.insert(*client_address, id);
                debug_assert!(old.is_none());
                let old = self.games.insert(id, game);
                debug_assert!(old.is_none());
            }
            ClientMessage::JoinLobby(_) => {}
            ClientMessage::UpdateLobbySettings(_) => {}
            ClientMessage::LobbyReady(_) => {}
            ClientMessage::GameUpdate(_) => {}
        };

        let game_id = self.client_games[&client_address];
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
        }
    }

    async fn close(self) {
        todo!()
    }
}
