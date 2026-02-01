//! Final rounds: semi-finals and finals (single-elimination bracket). Tournament ends after finals with two winners.

use crate::models::{
    GameMatch, MatchId, PlayerId, RoundType, Team, Tournament, TournamentError, TournamentState,
};
use rand::seq::SliceRandom;

/// Generate semi-final matches (8 players → 2 matches of 2v2). Seeds randomly.
pub fn generate_semi_final_matches(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::SemiFinals {
        return Err(TournamentError::InvalidState);
    }
    if tournament.players.len() != 8 {
        return Err(TournamentError::InvalidState);
    }
    let mut players = std::mem::take(&mut tournament.players);
    players.shuffle(&mut rand::thread_rng());
    tournament.players = players;

    let p = &tournament.players;
    let matches = vec![
        GameMatch::new(
            vec![p[0].id, p[1].id],
            vec![p[2].id, p[3].id],
            RoundType::SemiFinals,
        ),
        GameMatch::new(
            vec![p[4].id, p[5].id],
            vec![p[6].id, p[7].id],
            RoundType::SemiFinals,
        ),
    ];
    tournament.matches = matches;
    tournament.final_match_results.clear();
    Ok(())
}

/// Set winner for a final-round match (semi or finals).
pub fn set_finals_match_winner(
    tournament: &mut Tournament,
    match_id: MatchId,
    team: Team,
) -> Result<(), TournamentError> {
    if !tournament.matches.iter().any(|m| m.id == match_id) {
        return Err(TournamentError::InvalidState);
    }
    tournament.final_match_results.insert(match_id, team);
    Ok(())
}

/// Apply win/loss for a single playoff match to player stats.
/// Takes team ids and winner so we don't hold a reference into tournament while mutating it.
fn apply_playoff_match_result(
    tournament: &mut Tournament,
    team_1: &[PlayerId],
    team_2: &[PlayerId],
    winner: Team,
) -> Result<(), TournamentError> {
    let (winner_ids, loser_ids) = match winner {
        Team::One => (team_1, team_2),
        Team::Two => (team_2, team_1),
    };
    for &pid in loser_ids {
        tournament
            .get_player_mut_any(pid)
            .ok_or(TournamentError::PlayerNotFound(pid))?
            .add_loss();
    }
    for &pid in winner_ids {
        tournament
            .get_player_mut_any(pid)
            .ok_or(TournamentError::PlayerNotFound(pid))?
            .add_win();
    }
    Ok(())
}

/// Process semi-final results: advance 4 winners to Finals, generate finals match.
pub fn process_semi_final_results(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::SemiFinals {
        return Err(TournamentError::InvalidState);
    }
    if tournament.matches.len() != 2 {
        return Err(TournamentError::InvalidState);
    }
    for m in &tournament.matches {
        if !tournament.final_match_results.contains_key(&m.id) {
            return Err(TournamentError::IncompleteResults);
        }
    }

    // Apply playoff win/loss to player stats before snapshot (copy match data to avoid borrow conflict)
    let match_data: Vec<_> = tournament
        .matches
        .iter()
        .map(|m| (m.team_1.clone(), m.team_2.clone(), tournament.final_match_results[&m.id]))
        .collect();
    for (team_1, team_2, w) in match_data {
        apply_playoff_match_result(tournament, &team_1, &team_2, w)?;
    }

    tournament.bracket_semi_final_players = Some(tournament.players.clone());

    let mut winner_ids: Vec<PlayerId> = Vec::new();
    for m in &tournament.matches {
        let w = tournament.final_match_results[&m.id];
        let ids = match w {
            Team::One => &m.team_1,
            Team::Two => &m.team_2,
        };
        winner_ids.extend(ids.iter().copied());
    }

    tournament.bracket_semi_final_matches = Some(tournament.matches.clone());
    tournament.bracket_semi_final_results = Some(tournament.final_match_results.clone());
    tournament.matches.clear();
    tournament.final_match_results.clear();

    let advancing: Vec<_> = tournament
        .players
        .iter()
        .filter(|p| winner_ids.contains(&p.id))
        .cloned()
        .collect();
    tournament.players = advancing;

    let p = &tournament.players;
    tournament.matches = vec![GameMatch::new(
        vec![p[0].id, p[1].id],
        vec![p[2].id, p[3].id],
        RoundType::Finals,
    )];
    tournament.state = TournamentState::Finals;
    Ok(())
}

/// Process finals result: tournament completed (two winners from the winning team).
pub fn process_finals_results(tournament: &mut Tournament) -> Result<(), TournamentError> {
    if tournament.state != TournamentState::Finals {
        return Err(TournamentError::InvalidState);
    }
    if tournament.matches.len() != 1 {
        return Err(TournamentError::InvalidState);
    }
    let team_1 = tournament.matches[0].team_1.clone();
    let team_2 = tournament.matches[0].team_2.clone();
    let w = tournament
        .final_match_results
        .get(&tournament.matches[0].id)
        .copied()
        .ok_or(TournamentError::IncompleteResults)?;

    apply_playoff_match_result(tournament, &team_1, &team_2, w)?;

    tournament.bracket_finals_match = Some(tournament.matches[0].clone());
    tournament.bracket_finals_result = Some(w);
    tournament.matches.clear();
    tournament.final_match_results.clear();
    tournament.state = TournamentState::Completed;
    Ok(())
}
