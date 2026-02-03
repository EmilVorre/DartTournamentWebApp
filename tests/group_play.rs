//! Integration tests for group play: match generation and result processing.

use dart_tournament_web::{
    generate_group_play_matches, process_group_play_results, Player, RoundType, Team,
    Tournament, TournamentError, TournamentMode, TournamentState,
};

fn tournament_with_players(n: usize) -> Tournament {
    let players: Vec<Player> = (0..n).map(|i| Player::new(format!("P{i}"))).collect();
    let mut t = Tournament::with_players(players, 2, TournamentMode::TwoVTwo);
    t.state = TournamentState::GroupPlay;
    t
}

#[test]
fn generate_requires_at_least_4_players() {
    let mut t = tournament_with_players(3);
    assert!(matches!(
        generate_group_play_matches(&mut t),
        Err(TournamentError::NotEnoughPlayers)
    ));
}

#[test]
fn generate_creates_matches_and_unused() {
    let mut t = tournament_with_players(10); // 10 % 4 = 2 sit out, 8 play -> 2 matches
    generate_group_play_matches(&mut t).unwrap();
    assert_eq!(t.matches.len(), 2);
    assert_eq!(t.unused_players.len(), 2);
    for m in &t.matches {
        assert_eq!(m.team_1.len(), 2);
        assert_eq!(m.team_2.len(), 2);
        assert_eq!(m.round, RoundType::GroupPlay);
    }
}

#[test]
fn process_results_updates_wins_losses_and_eliminates() {
    let mut t = tournament_with_players(4); // 1 match
    generate_group_play_matches(&mut t).unwrap();
    let m = &t.matches[0];
    let team1_ids = m.team_1.clone();
    let team2_ids = m.team_2.clone();
    t.match_results.insert(m.id, Team::One);

    process_group_play_results(&mut t).unwrap();

    // Team 1 won: team1 +1 win each, team2 +1 loss each; max_losses=2 so no elimination yet
    for pid in &team1_ids {
        let p = t.players.iter().find(|x| x.id == *pid).unwrap();
        assert_eq!(p.wins, 1);
        assert_eq!(p.losses, 0);
    }
    for pid in &team2_ids {
        let p = t.players.iter().find(|x| x.id == *pid).unwrap();
        assert_eq!(p.wins, 0);
        assert_eq!(p.losses, 1);
    }
    // 4 players <= 8, so state moves to FinalSelection
    assert_eq!(t.state, TournamentState::FinalSelection);
}
