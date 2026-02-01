//! Tournament and TournamentState.

use crate::models::game::{GameMatch, MatchId, Team};
use crate::models::player::{Player, PlayerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Errors that can occur during tournament operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TournamentError {
    /// Not all matches have a result selected.
    IncompleteResults,
    /// Not enough players to generate matches (need at least 4).
    NotEnoughPlayers,
    /// Not enough players to start (need at least 4).
    NotEnoughPlayersToStart,
    /// Tournament is not in a state that allows this action.
    InvalidState,
    /// Player not found in active or unused list.
    PlayerNotFound(PlayerId),
    /// A player with this name already exists (names are unique, case-insensitive).
    DuplicatePlayerName,
    /// Wrong number of players selected for final selection (must select exactly N to reach 8).
    WrongNumberOfPlayers { needed: usize, selected: usize },
    /// A selected player is not in the last eliminated list.
    PlayerNotInLastEliminated(PlayerId),
}

impl std::fmt::Display for TournamentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TournamentError::IncompleteResults => write!(f, "Not all matches have a result"),
            TournamentError::NotEnoughPlayers => write!(f, "Need at least 4 players to generate matches"),
            TournamentError::NotEnoughPlayersToStart => write!(f, "Need at least 8 players to start"),
            TournamentError::InvalidState => write!(f, "Invalid state for this action"),
            TournamentError::PlayerNotFound(_) => write!(f, "Player not found"),
            TournamentError::DuplicatePlayerName => write!(f, "A player with this name already exists"),
            TournamentError::WrongNumberOfPlayers { needed, selected } => {
                write!(f, "Must select exactly {} players to rejoin (selected {})", needed, selected)
            }
            TournamentError::PlayerNotInLastEliminated(_) => {
                write!(f, "Selected player is not in the last eliminated list")
            }
        }
    }
}

/// Unique identifier for a tournament.
pub type TournamentId = Uuid;

/// Current phase of the tournament.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TournamentState {
    /// Adding players, setting max losses; not started.
    #[default]
    Setup,
    /// Main phase: >8 players, group play rounds.
    GroupPlay,
    /// 8 or fewer players; may need to select from last eliminated to reach 8.
    FinalSelection,
    /// 8 players; semi-finals (2 matches, 2v2).
    SemiFinals,
    /// 4 players; finals (1 match, 2v2). Submitting completes the tournament (two winners).
    Finals,
    /// Tournament finished; show winners and stats.
    Completed,
}

/// Full tournament state: players, matches, results, and phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tournament {
    pub id: TournamentId,
    /// Active (non-eliminated) players.
    pub players: Vec<Player>,
    /// Players eliminated so far.
    pub eliminated_players: Vec<Player>,
    /// Players eliminated in the most recent round (for "last eliminated" UI).
    pub last_eliminated_players: Vec<Player>,
    /// Current round's matches.
    pub matches: Vec<GameMatch>,
    /// Players sitting out the current round (group play).
    pub unused_players: Vec<Player>,
    /// Losses before a player is eliminated.
    pub max_losses: u32,
    pub state: TournamentState,
    /// Current round: which team won each match (before submit).
    pub match_results: HashMap<MatchId, Team>,
    /// Final rounds (semi/finals/grand): results per match.
    pub final_match_results: HashMap<MatchId, Team>,
    /// Bracket display: semi-final matches (when in Finals or later).
    pub bracket_semi_final_matches: Option<Vec<GameMatch>>,
    /// Bracket display: semi-final results.
    pub bracket_semi_final_results: Option<HashMap<MatchId, Team>>,
    /// Bracket display: finals match (when in Finals or Completed).
    pub bracket_finals_match: Option<GameMatch>,
    /// Bracket display: finals result.
    pub bracket_finals_result: Option<Team>,
    /// Bracket display: 8 players at semi-finals (for name lookup).
    pub bracket_semi_final_players: Option<Vec<Player>>,
}

impl Tournament {
    /// Create a new tournament in Setup state with no players.
    pub fn new(max_losses: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            players: Vec::new(),
            eliminated_players: Vec::new(),
            last_eliminated_players: Vec::new(),
            matches: Vec::new(),
            unused_players: Vec::new(),
            max_losses,
            state: TournamentState::Setup,
            match_results: HashMap::new(),
            final_match_results: HashMap::new(),
            bracket_semi_final_matches: None,
            bracket_semi_final_results: None,
            bracket_finals_match: None,
            bracket_finals_result: None,
            bracket_semi_final_players: None,
        }
    }

    /// Create a tournament with initial players (e.g. from setup). Still in Setup until started.
    pub fn with_players(players: Vec<Player>, max_losses: u32) -> Self {
        Self {
            players,
            ..Self::new(max_losses)
        }
    }

    /// Mutable reference to an active player by id (searches `players` only).
    pub fn get_player_mut(&mut self, id: PlayerId) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    /// Look up a player in either `players` or `unused_players` (for group play).
    pub fn get_player_mut_any(&mut self, id: PlayerId) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id).or_else(|| {
            self.unused_players.iter_mut().find(|p| p.id == id)
        })
    }

    /// Add a player (valid in Setup, GroupPlay, or FinalSelection). Names must be unique (case-insensitive).
    pub fn add_player(&mut self, name: impl Into<String>) -> Result<(), TournamentError> {
        use TournamentState::*;
        if !matches!(self.state, Setup | GroupPlay | FinalSelection) {
            return Err(TournamentError::InvalidState);
        }
        let name = name.into();
        let name_trimmed = name.trim();
        if name_trimmed.is_empty() {
            return Err(TournamentError::InvalidState);
        }
        let is_duplicate = self
            .players
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(name_trimmed));
        if is_duplicate {
            return Err(TournamentError::DuplicatePlayerName);
        }
        self.players.push(Player::new(name_trimmed));
        Ok(())
    }

    /// Remove a player by id (only valid in Setup).
    pub fn remove_player(&mut self, player_id: PlayerId) -> Result<(), TournamentError> {
        if self.state != TournamentState::Setup {
            return Err(TournamentError::InvalidState);
        }
        let idx = self
            .players
            .iter()
            .position(|p| p.id == player_id)
            .ok_or(TournamentError::PlayerNotFound(player_id))?;
        self.players.remove(idx);
        Ok(())
    }

    /// Set max losses before elimination (only valid in Setup).
    pub fn set_max_losses(&mut self, max_losses: u32) -> Result<(), TournamentError> {
        if self.state != TournamentState::Setup {
            return Err(TournamentError::InvalidState);
        }
        self.max_losses = max_losses;
        Ok(())
    }

    /// Set a player's loss count manually (GroupPlay or FinalSelection). Player must be active (in players or unused_players).
    /// When no matches have been generated yet, we do not set eliminated=true so that "Generate matches" still has enough players.
    pub fn set_player_losses(&mut self, player_id: PlayerId, losses: u32) -> Result<(), TournamentError> {
        if self.state != TournamentState::GroupPlay && self.state != TournamentState::FinalSelection {
            return Err(TournamentError::InvalidState);
        }
        let max_losses = self.max_losses;
        let has_matches = !self.matches.is_empty();
        let p = self
            .get_player_mut_any(player_id)
            .ok_or(TournamentError::PlayerNotFound(player_id))?;
        p.losses = losses;
        // Only mark eliminated once at least one round has been generated; otherwise editing losses
        // before the first "Generate matches" would shrink the pool and block generating matches.
        if has_matches && p.losses >= max_losses {
            p.eliminated = true;
        }
        Ok(())
    }

    /// Manually eliminate a player (GroupPlay or FinalSelection). Moves them from active to eliminated_players.
    pub fn eliminate_player(&mut self, player_id: PlayerId) -> Result<(), TournamentError> {
        if self.state != TournamentState::GroupPlay && self.state != TournamentState::FinalSelection {
            return Err(TournamentError::InvalidState);
        }
        let player = self
            .players
            .iter()
            .chain(self.unused_players.iter())
            .find(|p| p.id == player_id)
            .cloned()
            .ok_or(TournamentError::PlayerNotFound(player_id))?;
        let mut p = player;
        p.eliminate();
        self.players.retain(|x| x.id != player_id);
        self.unused_players.retain(|x| x.id != player_id);
        self.eliminated_players.push(p);
        Ok(())
    }

    /// Restart tournament: go back to Setup with same player names (active + eliminated). Clears matches and state.
    pub fn restart_tournament(&mut self) -> Result<(), TournamentError> {
        if self.state != TournamentState::GroupPlay && self.state != TournamentState::FinalSelection {
            return Err(TournamentError::InvalidState);
        }
        let names: Vec<String> = self
            .players
            .iter()
            .chain(self.unused_players.iter())
            .chain(self.eliminated_players.iter())
            .map(|p| p.name.clone())
            .collect();
        let max_losses = self.max_losses;
        *self = Self::new(max_losses);
        for name in names {
            let _ = self.add_player(name);
        }
        Ok(())
    }
}
