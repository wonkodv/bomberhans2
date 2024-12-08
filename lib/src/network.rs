use serde::Deserialize;
use serde::Serialize;

use crate::game_state::Action;
use crate::utils::PlayerId;
use crate::utils::TimeStamp;

const BOMBERHANS_MAGIC_NO_V1: u32 = 0x1f4a3__001; // 💣

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientHello {
    pub magic: u32,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerHello {
    pub name: String,

    pub cookie: ClientId,

    pub games: Vec<(GameId, String)>,
}

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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct LobbyId(u64);
impl LobbyId {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

/// Periodic Client to Server update
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientUpdate {
    pub cookie: ClientId,

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
#[derive(Debug, Serialize, Deserialize)]
pub struct Update {
    player: PlayerId,
    action: Action,
    time: TimeStamp,
}
