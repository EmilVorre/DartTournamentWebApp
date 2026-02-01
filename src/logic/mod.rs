//! Tournament business logic: setup, group play, finals, etc.

mod final_selection;
mod finals;
mod group_play;
mod setup;

pub use final_selection::{add_players_back_from_last_eliminated, start_semi_finals};
pub use finals::{
    generate_semi_final_matches, process_finals_results, process_semi_final_results,
    set_finals_match_winner,
};
pub use group_play::{generate_group_play_matches, process_group_play_results};
pub use setup::start_tournament;
