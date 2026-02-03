//! Group stage: match generation and result processing.

use crate::models::{
    GameMatch, Player, PlayerId, RoundType, Tournament, TournamentError, TournamentMode,
    TournamentState,
};
use crate::Team;
use rand::seq::SliceRandom;
use rand::Rng;

/// Generate matches for the current group play round (1v1: 2 per match, 2v2: 4 per match).
///
/// 1. Filter to non-eliminated players.
/// 2. Sort by `internal_times_sat_out` (ascending).
/// 3. Take excess = len % players_per_round; first `excess` sit out.
/// 4. Shuffle and form matches: 1v1 chunks of 2, 2v2 chunks of 4.
pub fn generate_group_play_matches(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::GroupPlay {
        return Err(TournamentError::InvalidState);
    }

    let (min_players, chunk_size, excess_mod) = match tournament.mode {
        TournamentMode::OneVOne => (2, 2, 2),
        TournamentMode::TwoVTwo => (4, 4, 4),
    };

    let mut available: Vec<_> = tournament
        .players
        .iter()
        .filter(|p| !p.eliminated)
        .cloned()
        .collect();

    if available.len() < min_players {
        return Err(TournamentError::NotEnoughPlayers);
    }

    let mut rng = rand::thread_rng();
    let mut with_tiebreak: Vec<(Player, u32)> = available
        .drain(..)
        .map(|p| (p, rng.gen::<u32>()))
        .collect();
    with_tiebreak.sort_by_key(|(p, t)| (p.internal_times_sat_out, *t));
    available = with_tiebreak.into_iter().map(|(p, _)| p).collect();

    let n = available.len();
    let excess = n % excess_mod;

    let mut unused: Vec<Player> = available.drain(0..excess).collect();
    for p in &mut unused {
        p.record_sat_out();
    }

    available.shuffle(&mut rng);

    let matches: Vec<GameMatch> = available
        .chunks_exact(chunk_size)
        .map(|chunk| {
            let (team_1, team_2) = match tournament.mode {
                TournamentMode::OneVOne => (vec![chunk[0].id], vec![chunk[1].id]),
                TournamentMode::TwoVTwo => {
                    (
                        vec![chunk[0].id, chunk[1].id],
                        vec![chunk[2].id, chunk[3].id],
                    )
                }
            };
            GameMatch::new(team_1, team_2, RoundType::GroupPlay)
        })
        .collect();

    for p in &unused {
        if let Some(t) = tournament.players.iter_mut().find(|t| t.id == p.id) {
            t.times_sat_out = p.times_sat_out;
            t.internal_times_sat_out = p.internal_times_sat_out;
        }
    }

    tournament.matches = matches;
    tournament.unused_players = unused;
    tournament.match_results.clear();

    Ok(())
}

/// Process the current round's match results: apply wins/losses, eliminate if at max losses, update state.
///
/// Uses `tournament.match_results`; all match ids in `tournament.matches` must have a result.
/// After processing: clears `match_results` and `matches`/`unused_players`, and sets state to
/// `FinalSelection` if ≤8 players remain.
pub fn process_group_play_results(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::GroupPlay {
        return Err(TournamentError::InvalidState);
    }

    for m in &tournament.matches {
        if !tournament.match_results.contains_key(&m.id) {
            return Err(TournamentError::IncompleteResults);
        }
    }

    tournament.last_eliminated_players.clear();

    let max_losses = tournament.max_losses;
    let match_data: Vec<(Vec<PlayerId>, Vec<PlayerId>, Team)> = tournament
        .matches
        .iter()
        .map(|m| {
            let w = tournament.match_results[&m.id];
            (m.team_1.clone(), m.team_2.clone(), w)
        })
        .collect();

    for (team_1, team_2, winner) in match_data {
        let eliminated = apply_match_result(tournament, &team_1, &team_2, winner, max_losses)?;
        tournament.last_eliminated_players.extend(eliminated);
    }

    // Move eliminated into eliminated_players and remove from players
    tournament
        .eliminated_players
        .extend(tournament.last_eliminated_players.iter().cloned());
    tournament.players.retain(|p| !p.eliminated);

    // Clear current round state
    tournament.matches.clear();
    tournament.unused_players.clear();
    tournament.match_results.clear();

    let threshold = tournament.players_required_for_semi();
    if tournament.players.len() <= threshold {
        tournament.state = TournamentState::FinalSelection;
    }

    Ok(())
}

/// Apply a single match result: add wins/losses, mark eliminated if at max losses.
/// Returns clones of players that were eliminated this match.
fn apply_match_result(
    tournament: &mut Tournament,
    team_1: &[PlayerId],
    team_2: &[PlayerId],
    winner: Team,
    max_losses: u32,
) -> Result<Vec<Player>, TournamentError> {
    let mut eliminated = Vec::new();

    match winner {
        Team::One => {
            for &pid in team_2 {
                let p = tournament
                    .get_player_mut(pid)
                    .ok_or(TournamentError::PlayerNotFound(pid))?;
                p.add_loss();
                if p.losses >= max_losses {
                    p.eliminate();
                    eliminated.push(p.clone());
                }
            }
            for &pid in team_1 {
                tournament
                    .get_player_mut(pid)
                    .ok_or(TournamentError::PlayerNotFound(pid))?
                    .add_win();
            }
        }
        Team::Two => {
            for &pid in team_1 {
                let p = tournament
                    .get_player_mut(pid)
                    .ok_or(TournamentError::PlayerNotFound(pid))?;
                p.add_loss();
                if p.losses >= max_losses {
                    p.eliminate();
                    eliminated.push(p.clone());
                }
            }
            for &pid in team_2 {
                tournament
                    .get_player_mut(pid)
                    .ok_or(TournamentError::PlayerNotFound(pid))?
                    .add_win();
            }
        }
    }

    Ok(eliminated)
}
