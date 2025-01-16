use std::num::NonZeroU32;

use serde::Deserialize;
use serde::Serialize;

use crate::game_state::Action;
use crate::game_state::Player;
use crate::settings::Settings;
use crate::utils::GameTime;
use crate::utils::PlayerId;

pub const BOMBERHANS_MAGIC_NO_V1: u32 = 0x1f4a3__001; // 💣

/// The Maximum number of Bytes we send in 1 packet
/// TODO: good value:?
pub const MTU: usize = 1024;

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

/// Client joins a lobby `game_id`, calling himself `player_name`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientJoinLobby {
    pub game_id: GameId,
    pub player_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLobbyUpdate {
    pub settings: Settings,
    pub players: Vec<Player>,
    pub players_ready: Vec<Ready>,
    pub client_player_id: PlayerId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerGameStart {
    pub settings: Settings,
    pub players: Vec<Player>,
    pub client_player_id: PlayerId,
}

/// Periodic Client to Server update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientUpdate {
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

/// Client Opens a new lobby, calling himself `player_name`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientOpenLobby {
    pub player_name: String,
}

/// Client changes the settings of a game he is in
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientLobbyUpdate {
    pub settings: Settings,
}

/// An Update is when the player changed their current action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ready {
    NotReady,
    Ready,
}

/// Client sets his ready state in the lobby
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientLobbyReady {
    pub ready: Ready,
}

/// A Message from Client to Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    GetLobbyList,
    OpenNewLobby(ClientOpenLobby),
    JoinLobby(ClientJoinLobby),
    UpdateLobbySettings(ClientLobbyUpdate),
    LobbyReady(ClientLobbyReady),
    GameUpdate(ClientUpdate),
    Bye,
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
    LobbyUpdate(ServerLobbyUpdate),
    GameStart(ServerGameStart),
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
    debug_assert!(result.len() < MTU, "Message too large {value:?}");
    result
}

pub fn decode<T: for<'a> Deserialize<'a>>(data: &[u8]) -> Option<T> {
    postcard::from_bytes::<T>(data).ok()
}
