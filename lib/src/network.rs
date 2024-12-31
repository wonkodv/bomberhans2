use serde::Deserialize;
use serde::Serialize;

use crate::game_state::Action;
use crate::game_state::GameStatic;
use crate::utils::PlayerId;
use crate::utils::TimeStamp;

pub const BOMBERHANS_MAGIC_NO_V1: u32 = 0x1f4a3__001; // 💣

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientId(u64);

impl ClientId {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct GameId(u64);
impl GameId {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientHello {
    /// Identifying the protocol
    pub magic: u32,

    /// Unique number of this packet, to associate the server's response to a packet, to compute
    /// the ping
    pub nonce: u32,

    /// the player's name
    pub player_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerHello {
    /// nonce of the ClientHello
    pub clients_nonce: u32,

    /// Session cookie to identify the client again later
    pub client_id: ClientId,

    pub server_name: String,

    pub lobbies: Vec<(GameId, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientJoinLobby {
    pub lobby: GameId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerLobbyUpdate {
    client_player_id: PlayerId,

    game: GameStatic,
}

/// Periodic Client to Server update
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientUpdate {
    pub client_id: ClientId,

    /// Time of the most recently received server update
    pub last_server_update: TimeStamp,

    /// action the player is currently taking
    pub current_player_action: Action,

    /// When did the player start this action
    pub current_action_start_time: TimeStamp,
}

/// Periodic Server to Client update
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerUpdate {
    /// Current Server Time
    pub time: TimeStamp,

    /// Hash of the Game State
    pub checksum: u32,

    /// Everything that has happened since the client last acknowledged
    pub updates: Vec<Update>,
}

/// An Update is when the player changed their current action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub player: PlayerId,
    pub action: Action,
    pub time: TimeStamp,
}

/// A Message from Client to Server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello(ClientHello),
    OpenNewLobby(ClientId),
    Update(ClientUpdate),
    Bye(ClientId),
}

/// A Message from Server to Client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Hello(ServerHello),
    Update(ServerUpdate),
    LobbyUpdate(ServerLobbyUpdate),
}

pub fn encode<S>(value: &S) -> Vec<u8>
where
    S: Serialize,
    S: std::fmt::Debug,
{
    let result = postcard::to_allocvec(value).expect("can serialize anything");
    debug_assert!(result.len() < 1000, "Message too large {value:?}");
    result
}

pub fn decode<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Option<T> {
    postcard::from_bytes::<T>(&data).ok()
}
