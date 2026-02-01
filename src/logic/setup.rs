//! Setup phase: start tournament (transition from Setup to GroupPlay or FinalSelection).

use crate::models::{Tournament, TournamentError, TournamentState};

/// Start the tournament: require at least 8 players, set state to GroupPlay if >8 else FinalSelection.
pub fn start_tournament(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::Setup {
        return Err(TournamentError::InvalidState);
    }
    if tournament.players.len() < 8 {
        return Err(TournamentError::NotEnoughPlayersToStart);
    }
    tournament.state = if tournament.players.len() > 8 {
        TournamentState::GroupPlay
    } else {
        TournamentState::FinalSelection
    };
    Ok(())
}
