use std::collections::HashMap;
use std::net::SocketAddr;

use bomberhans_lib::network::*;

type Answer = (ServerMessage, Option<PacketNumber>, SocketAddr);

enum Message {
    Request(Box<(ClientPacket, SocketAddr)>),
}

struct Game {
    name: String,
    manager: Manager<game::Message>,
}

struct Lobby {
    name: String,
    manager: Manager<game::Message>,
}

struct Client {
    manager: Manager<client::Message>,
}

pub struct Server {
    server_name: String,
    games: HashMap<GameId, Game>,
    lobbies: HashMap<GameId, Lobby>,
    clients: HashMap<ClientId, Client>,
    answer: Manager<Answer>,
}

impl Server {
    pub fn new(server_name: String, answer: Actor<ClientPacket>) -> Self {
        Self {
            server_name,
            games: HashMap::new(),
            lobbies: HashMap::new(),
            clients: HashMap::new(),
            answer,
        }
    }

    async fn handle_request(&self, request: Box<(ClientPacket, SocketAddr)>) {
        let (packet, client_address) = (&message.0, &message.1);

        if packet.magic != BOMBERHANS_MAGIC_NO_V1 {
            log::warn!("ignoring unknown protocol {packet:?}");
            return;
        }

        match packet.message {
            ClientMessage::GetLobbyList => {
                let server_name = self.name.clone();
                let lobbies = self
                    .lobbies
                    .values()
                    .map(|lob| (lob.id, lob.name))
                    .collect();
                self.answer
                    .send((
                        ServerMessage::GameList(ServerLobbyList {
                            server_name,
                            lobbies,
                        }),
                        Some(packet.packet_number),
                        client_address,
                    ))
                    .await;
                return;
            }
            ClientMessage::Ping => {
                self.answer
                    .send((
                        ServerMessage::Pong,
                        Some(packet.packet_number),
                        client_address,
                    ))
                    .await;
                return;
            }

            ClientMessage::OpenNewLobby(player_name) => {
                let client = Client::new(player_name, client_address);
                let lobby = Lobby::new(client.id);

                client
                    .manager
                    .send(client::Message::JoinGameAsHost(lobby.manager.assistant()))
                    .await;
                lobby
                    .manager
                    .send(game::Message::JoinHost(
                        client.manager.assistant(),
                        player_name,
                    ))
                    .await;

                let old = self.clients.insert(client.id, client);
                debug_assert!(old == None);
                let old = self.lobbies.insert(lobby.id, lobby);
                debug_assert!(old == None);

                log::info!("client {client:?} joined and opened a lobby {lobby:?}");
                return;
            }
            ClientMessage::JoinLobby(game_id, player_name) => {
                let lobby = self.lobbies.get(&game_id) else {
                    log::info!(
                        "{client_adress}({plater_name}) wanted to join unknown lobby {game_id}"
                    );
                    return;
                };
                let client = Client::new(player_name, client_address);

                client
                    .manager
                    .send(client::Message::JoinGame(lobby.manager.assistant()))
                    .await;
                lobby
                    .manager
                    .send(game::Message::Join(client.manager.assistant(), player_name))
                    .await;

                let old = self.clients.insert(client.id, client);
                debug_assert!(old == None);

                return;
            }

            ClientMessage::UpdateLobbySettings(client_id, settings) => {
                let Some(client) = self.clients.get(&client_id) else {
                    log::warn!("discarding packet from {client_address} for unknown client {client_id:?}: {packet:#?}");
                    return;
                };

                client
                    .manager
                    .send(client::Message::UpdateLobbySettings(settings))
                    .await;
            }

            ClientMessage::GameUpdate(client_update) => {
                let client_id = &client_update.client_id;
                let Some(client) = self.clients.get(client_id) else {
                    log::warn!("discarding packet from {client_address} for unknown client {client_id:?}: {packet:#?}");
                    return;
                };

                client
                    .manager
                    .send(client::Message::Update(client_update))
                    .await;
            }
            ClientMessage::GameStart(client_id)
            | ClientMessage::LobbyReady(client_id)
            | ClientMessage::Bye(client_id) => {
                if let Some(client) = self.clients.get(client_id) {
                    client
                } else {
                    log::warn!("discarding packet from {client_address} for unknown client {client_id:?}: {packet:#?}");
                    return;
                }
            }
        };
    }
}

impl crate::actor::Actor<Request> for Server {
    async fn handle_message(&mut self, message: Message) {
        match message {
            Message::Request(request) => self.handle_request(request).await,
        }
    }

    async fn close(self) {
        todo!()
    }
}
