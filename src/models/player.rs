//! Player and PlayerStats data structures.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a player (used in matches and lookups).
pub type PlayerId = Uuid;

/// Statistics view of a player (for API / display).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub losses: u32,
    pub wins: u32,
    pub times_sat_out: u32,
    pub eliminated_status: bool,
}

impl PlayerStats {
    pub fn from_player(p: &Player) -> Self {
        Self {
            losses: p.losses,
            wins: p.wins,
            times_sat_out: p.times_sat_out,
            eliminated_status: p.eliminated,
        }
    }
}

/// A player in the tournament.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub losses: u32,
    pub wins: u32,
    pub times_sat_out: u32,
    /// Internal counter for sit-out fairness (can go negative when we "owe" a sit-out).
    pub internal_times_sat_out: i32,
    /// Random seed for matchmaking (shuffle).
    pub seed: u32,
    pub eliminated: bool,
}

impl Player {
    /// Create a new player with the given name. Other fields start at zero/false.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: Uuid::new_v4(),
            name,
            losses: 0,
            wins: 0,
            times_sat_out: 0,
            internal_times_sat_out: 0,
            seed: 0,
            eliminated: false,
        }
    }

    /// Current stats as a separate struct (for API responses).
    pub fn stats(&self) -> PlayerStats {
        PlayerStats::from_player(self)
    }

    /// Record a win for this player.
    pub fn add_win(&mut self) {
        self.wins += 1;
    }

    /// Record a loss for this player.
    pub fn add_loss(&mut self) {
        self.losses += 1;
    }

    /// Mark the player as eliminated.
    pub fn eliminate(&mut self) {
        self.eliminated = true;
    }

    /// Record that this player sat out one round.
    pub fn record_sat_out(&mut self) {
        self.times_sat_out += 1;
        self.internal_times_sat_out += 1;
    }
}
