//! Final selection: add players back from last eliminated to reach 4 (1v1) or 8 (2v2) for semi-finals.

use crate::models::{PlayerId, Tournament, TournamentError, TournamentState};

/// Add selected players from last_eliminated_players back to the tournament.
/// Must select exactly (required - players.len()) players, all from last_eliminated_players.
/// When we reach required (4 or 8), state becomes SemiFinals.
pub fn add_players_back_from_last_eliminated(
    tournament: &mut Tournament,
    player_ids: &[PlayerId],
) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::FinalSelection {
        return Err(TournamentError::InvalidState);
    }
    let required = tournament.players_required_for_semi();
    let current = tournament.players.len();
    if current >= required {
        return Err(TournamentError::InvalidState);
    }
    let needed = required - current;
    if player_ids.len() != needed {
        return Err(TournamentError::WrongNumberOfPlayers {
            needed,
            selected: player_ids.len(),
        });
    }

    let last_ids: std::collections::HashSet<_> = tournament
        .last_eliminated_players
        .iter()
        .map(|p| p.id)
        .collect();
    for &id in player_ids {
        if !last_ids.contains(&id) {
            return Err(TournamentError::PlayerNotInLastEliminated(id));
        }
    }

    let ids_to_add: std::collections::HashSet<_> = player_ids.iter().copied().collect();
    let mut to_add: Vec<_> = tournament
        .last_eliminated_players
        .drain(..)
        .filter(|p| ids_to_add.contains(&p.id))
        .collect();
    for p in &mut to_add {
        p.eliminated = false;
    }
    tournament.players.append(&mut to_add);

    tournament
        .eliminated_players
        .retain(|p| !ids_to_add.contains(&p.id));

    if tournament.players.len() == required {
        tournament.state = TournamentState::SemiFinals;
    }

    Ok(())
}

/// Transition from FinalSelection to SemiFinals when exactly 4 (1v1) or 8 (2v2) players (no add-back needed).
pub fn start_semi_finals(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::FinalSelection {
        return Err(TournamentError::InvalidState);
    }
    let required = tournament.players_required_for_semi();
    if tournament.players.len() != required {
        return Err(TournamentError::InvalidState);
    }
    tournament.state = TournamentState::SemiFinals;
    Ok(())
}
