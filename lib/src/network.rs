use std::num::NonZeroU32;

use serde::Deserialize;
use serde::Serialize;

use crate::game_state::Action;
use crate::game_state::Player;
use crate::settings::Settings;
use crate::utils::GameTime;
use crate::utils::PlayerId;

pub const BOMBERHANS_MAGIC_NO_V1: u32 = 0x1f4a3__001; // 💣

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientId(u32);

impl ClientId {
    pub fn new(val: u32) -> Self {
        Self(val)
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct GameId(u32);
impl GameId {
    pub fn new(val: u32) -> Self {
        Self(val)
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, Eq, PartialEq, Serialize, Deserialize)]
pub struct PacketNumber(NonZeroU32);
impl PacketNumber {
    pub fn new() -> Self {
        Self(NonZeroU32::new(1).unwrap())
    }
    pub fn next(&mut self) -> Self {
        let p = self.0;
        self.0 = p.checked_add(1).expect("packet_number fits 32bit");
        return Self(p);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLobbyList {
    pub server_name: String,

    pub lobbies: Vec<(GameId, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientJoinLobby {
    pub lobby: GameId,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLobbyUpdate {
    pub id: GameId,
    pub settings: Settings,
    pub players: Vec<Player>,
    pub client_player_id: PlayerId,
}

/// Periodic Client to Server update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientUpdate {
    pub client_id: ClientId,

    /// Time of the most recently received server update
    pub last_server_update: GameTime,

    /// action the player is currently taking
    pub current_player_action: Action,

    /// When did the player start this action
    pub current_action_start_time: GameTime,
}

/// Periodic Server to Client update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerUpdate {
    /// Current Server Time
    pub time: GameTime,

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
    pub time: GameTime,
}

/// A Message from Client to Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    GetLobbyList,

    /// Open a new lobby, with Player Name
    OpenNewLobby(String),
    /// Join lobby, with Player Name
    JoinLobby(GameId, String),
    LobbySettingsUpdate(ClientId, Settings),
    LobbyReady(ClientId),
    GameStart(ClientId),
    GameUpdate(ClientUpdate),
    Bye(ClientId),
    Ping,
}

/// A Client Packet wrapping a Client Message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPacket {
    pub magic: u32,
    pub packet_number: PacketNumber,
    pub message: ClientMessage,
}

/// A Message from Server to Client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    LobbyList(ServerLobbyList),
    LobbyJoined(ClientId, ServerLobbyUpdate),
    LobbyUpdate(ServerLobbyUpdate),
    Update(ServerUpdate),
    Pong,
    Bye,
}

/// A Client Packet wrapping a Server Message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPacket {
    pub magic: u32,
    pub packet_number: PacketNumber,
    pub ack_packet_number: Option<PacketNumber>,
    pub message: ServerMessage,
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
    postcard::from_bytes::<T>(data).ok()
}
