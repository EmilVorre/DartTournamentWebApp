//! Setup phase: start tournament (transition from Setup to GroupPlay or FinalSelection).

use crate::models::{Tournament, TournamentError, TournamentState};

/// Start the tournament: require 4 players (1v1) or 8 (2v2); set state to GroupPlay if above threshold else FinalSelection.
pub fn start_tournament(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::Setup {
        return Err(TournamentError::InvalidState);
    }
    let required = tournament.players_required_to_start();
    if tournament.players.len() < required {
        return Err(TournamentError::NotEnoughPlayersToStart { required });
    }
    tournament.state = if tournament.players.len() > required {
        TournamentState::GroupPlay
    } else {
        TournamentState::FinalSelection
    };
    Ok(())
}
