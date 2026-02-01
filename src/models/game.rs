//! Match (game), Team, and RoundType for 2v2 / 1v1 games.

use crate::models::player::PlayerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a match.
pub type MatchId = Uuid;

/// Which team won the match.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Team {
    #[default]
    One,
    Two,
}

/// Phase of the tournament this match belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundType {
    GroupPlay,
    SemiFinals,
    Finals,
    GrandFinals,
}

/// A single match: two teams (usually 2v2, or 1v1 in grand finals).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GameMatch {
    pub id: MatchId,
    /// Team 1 player IDs (2 for 2v2, 1 for 1v1 grand finals).
    pub team_1: Vec<PlayerId>,
    /// Team 2 player IDs.
    pub team_2: Vec<PlayerId>,
    /// None if not yet played.
    pub winner: Option<Team>,
    pub round: RoundType,
}

impl GameMatch {
    pub fn new(team_1: Vec<PlayerId>, team_2: Vec<PlayerId>, round: RoundType) -> Self {
        Self {
            id: Uuid::new_v4(),
            team_1,
            team_2,
            winner: None,
            round,
        }
    }
}
