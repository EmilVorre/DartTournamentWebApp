//! Dart tournament web app: library with models and business logic.

pub mod logic;
pub mod models;

pub use logic::{
    add_players_back_from_last_eliminated, generate_group_play_matches, generate_semi_final_matches,
    process_finals_results, process_grand_finals_results, process_group_play_results,
    process_semi_final_results, set_finals_match_winner, start_semi_finals, start_tournament,
};
pub use models::{
    GameMatch, MatchId, Player, PlayerId, PlayerStats, RoundType, Team, Tournament, TournamentError,
    TournamentId, TournamentState,
};
