use std::ops::RangeInclusive;

use serde::Deserialize;
use serde::Serialize;

use crate::field::Cell;
use crate::field::Upgrade;
use crate::utils::GameTimeDiff;

/// Ratios of Wood turning into those cell types:
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct Ratios {
    pub power: u32,
    pub speed: u32,
    pub bombs: u32,
    pub teleport: u32,
    pub wall: u32,
    pub wood: u32,
    pub clear: u32,
}

impl Default for Ratios {
    fn default() -> Self {
        Self {
            power: 8,
            speed: 9,
            bombs: 7,
            teleport: 2,
            wall: 0,
            wood: 1,
            clear: 20,
        }
    }
}

impl Ratios {
    pub fn new(
        power: u32,
        speed: u32,
        bombs: u32,
        teleport: u32,
        wall: u32,
        wood: u32,
        clear: u32,
    ) -> Self {
        Self {
            power,
            speed,
            bombs,
            teleport,
            wall,
            wood,
            clear,
        }
    }

    pub fn sum(&self) -> u32 {
        self.power + self.speed + self.bombs + self.teleport + self.wall + self.wood + self.clear
    }
    pub fn random(&self, random: u32) -> Cell {
        let sum = self.sum();

        let mut random = random % sum;

        if random < self.power {
            return Cell::Upgrade(Upgrade::Power);
        }
        random -= self.power;

        if random < self.speed {
            return Cell::Upgrade(Upgrade::Speed);
        }
        random -= self.speed;

        if random < self.bombs {
            return Cell::Upgrade(Upgrade::Bombs);
        }
        random -= self.bombs;

        if random < self.teleport {
            return Cell::Teleport;
        }
        random -= self.teleport;

        if random < self.wood {
            return Cell::Wood;
        }
        random -= self.wood;
        if random < self.wall {
            return Cell::Wall;
        }
        random -= self.wall;

        assert!(random < self.clear);
        Cell::Empty
    }

    pub fn normalize(&self) -> Self {
        let ratio = 100.0 / (self.sum() as f32);

        let power = (self.power as f32 * ratio).round() as u32;
        let speed = (self.speed as f32 * ratio).round() as u32;
        let bombs = (self.bombs as f32 * ratio).round() as u32;
        let teleport = (self.teleport as f32 * ratio).round() as u32;
        let wall = (self.wall as f32 * ratio).round() as u32;
        let wood = (self.wood as f32 * ratio).round() as u32;
        let clear = (self.clear as f32 * ratio).round() as u32;

        Self {
            power,
            speed,
            bombs,
            teleport,
            wall,
            wood,
            clear,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct Settings {
    /// Name of the game
    pub game_name: String,

    /// field width
    pub width: u32,

    /// field width
    pub height: u32,

    /// number of players that can join
    pub players: u32,

    /// time after bomb placement that the bomb explodes
    pub bomb_explode_time_ms: u32,

    /// player walking speed [cells/100/s]
    pub speed_base: u32,

    /// player walking speed increase per speed power up [cells/100/s]
    pub speed_multiplyer: u32,

    /// percentage that walking on bomb succeeds each update
    pub bomb_walking_chance: u32,

    /// percentage that walking on tombstone succeeds each update
    pub tombstone_walking_chance: u32,

    /// Power of Upgrade Packets exploding
    pub upgrade_explosion_power: u32,

    /// how long before burning wood turns into something
    pub wood_burn_time_ms: u32,

    /// how long fire burns
    pub fire_burn_time_ms: u32,

    /// how far behind the player the bomb is placed [cell/100]
    pub bomb_offset: u32,

    /// Ratios what comes out of burned down walls
    pub ratios: Ratios,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            game_name: "A Game of Bomberhans".to_owned(),
            width: Self::WIDTH_DEFAULT,
            height: Self::HEIGHT_DEFAULT,
            players: Self::PLAYERS_DEFAULT,
            bomb_offset: Self::BOMB_OFFSET_DEFAULT,
            bomb_explode_time_ms: Self::BOMB_TIME_DEFAULT,
            speed_multiplyer: Self::SPEED_MULTIPLYER_DEFAULT,
            speed_base: Self::SPEED_BASE_DEFAULT,
            bomb_walking_chance: Self::BOMB_WALKING_CHANCE_DEFAULT,
            tombstone_walking_chance: Self::TOMBSTONE_WALKING_CHANCE_DEFAULT,
            upgrade_explosion_power: Self::UPGRADE_EXPLOSION_POWER_DEFAULT,
            wood_burn_time_ms: Self::WOOD_BURN_TIME_DEFAULT,
            fire_burn_time_ms: Self::FIRE_BURN_TIME_DEFAULT,
            ratios: Ratios::default(),
        }
    }
}

impl Settings {
    pub const BOMB_OFFSET_DEFAULT: u32 = 49;
    pub const BOMB_OFFSET_RANGE: RangeInclusive<u32> = 0..=100;
    pub const BOMB_TIME_DEFAULT: u32 = 4267;
    pub const BOMB_TIME_RANGE: RangeInclusive<u32> = 100..=10_000;
    pub const BOMB_WALKING_CHANCE_DEFAULT: u32 = 80;
    pub const BOMB_WALKING_CHANCE_RANGE: RangeInclusive<u32> = 0..=100;
    pub const FIRE_BURN_TIME_DEFAULT: u32 = 400;
    pub const FIRE_BURN_TIME_RANGE: RangeInclusive<u32> = 0..=10_000;
    pub const HEIGHT_DEFAULT: u32 = 13;
    pub const HEIGHT_RANGE: RangeInclusive<u32> = Self::WIDTH_RANGE;
    pub const PLAYERS_DEFAULT: u32 = 4;
    pub const PLAYERS_RANGE: RangeInclusive<u32> = 1..=4; // TODO: generate maps with more players
    pub const RATIOS_RANGE: RangeInclusive<u32> = 0..=100;
    pub const SPEED_BASE_DEFAULT: u32 = 100;
    pub const SPEED_BASE_RANGE: RangeInclusive<u32> = 10..=500;
    pub const SPEED_MULTIPLYER_DEFAULT: u32 = 50;
    pub const SPEED_MULTIPLYER_RANGE: RangeInclusive<u32> = 0..=200;
    pub const TOMBSTONE_WALKING_CHANCE_DEFAULT: u32 = 40;
    pub const TOMBSTONE_WALKING_CHANCE_RANGE: RangeInclusive<u32> = 0..=100;
    pub const UPGRADE_EXPLOSION_POWER_DEFAULT: u32 = 1;
    pub const UPGRADE_EXPLOSION_POWER_RANGE: RangeInclusive<u32> = 0..=15;
    pub const WIDTH_DEFAULT: u32 = 17;
    pub const WIDTH_RANGE: RangeInclusive<u32> = 5..=25;
    pub const WOOD_BURN_TIME_DEFAULT: u32 = 1200;
    pub const WOOD_BURN_TIME_RANGE: RangeInclusive<u32> = 0..=10_000;

    /// Walking Speed based on `speed_powerup`
    /// returned speed is returned in `Cells/100s`
    ///
    /// Speed of input variables is Cells/100s
    pub fn get_update_walk_distance(&self, player_speed: u32) -> u32 {
        self.speed_base + (player_speed * self.speed_multiplyer)
    }

    pub fn bomb_explode_time(&self) -> GameTimeDiff {
        GameTimeDiff::from_ms(self.bomb_explode_time_ms)
    }
    pub fn wood_burn_time(&self) -> GameTimeDiff {
        GameTimeDiff::from_ms(self.wood_burn_time_ms)
    }
    pub fn fire_burn_time(&self) -> GameTimeDiff {
        GameTimeDiff::from_ms(self.fire_burn_time_ms)
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ratios() {
        let r = Ratios::new(2, 2, 2, 2, 2, 2, 2);

        assert_eq!(Cell::Upgrade(Upgrade::Power), r.random(0));
        assert_eq!(Cell::Upgrade(Upgrade::Power), r.random(1));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.random(2));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.random(3));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.random(4));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.random(5));
        assert_eq!(Cell::Teleport, r.random(6));
        assert_eq!(Cell::Teleport, r.random(7));
        assert_eq!(Cell::Wood, r.random(8));
        assert_eq!(Cell::Wood, r.random(9));
        assert_eq!(Cell::Wall, r.random(10));
        assert_eq!(Cell::Wall, r.random(11));
        assert_eq!(Cell::Empty, r.random(12));
        assert_eq!(Cell::Empty, r.random(13));
    }

    #[test]
    fn test_walking_distance() {
        let r = Settings::default();
        assert_eq!(r.get_update_walk_distance(0), 100);
        assert_eq!(r.get_update_walk_distance(1), 150);
        assert_eq!(r.get_update_walk_distance(2), 200);
    }
}
