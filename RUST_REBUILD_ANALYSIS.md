# Dart Tournament Organizer - Rust Web Application Rebuild Analysis

## Table of Contents
1. [Application Overview](#application-overview)
2. [Data Structures](#data-structures)
3. [Core Logic & Algorithms](#core-logic--algorithms)
4. [Application Workflow](#application-workflow)
5. [UI Components & Features](#ui-components--features)
6. [Rust Implementation Requirements](#rust-implementation-requirements)
7. [Technical Stack Recommendations](#technical-stack-recommendations)

---

## Application Overview

**Purpose**: A tournament management system for dart tournaments with 2v2 team matches.

**Key Features**:
- Player registration and management
- Automatic match generation (2v2 teams)
- Elimination system based on max losses
- Tournament progression: Group Play → Semi-Finals → Finals → Grand Finals
- Statistics tracking (wins, losses, times sat out)
- Manual tournament controls (edit losses, eliminate players)

---

## Data Structures

### Player
```rust
struct Player {
    id: Uuid,                    // Unique identifier
    name: String,                // Player name
    losses: u32,                  // Current loss count
    wins: u32,                    // Current win count
    times_sat_out: u32,           // Times player sat out
    internal_times_sat_out: i32, // Internal counter (can be negative)
    seed: u32,                    // Random seed for matchmaking
    eliminated: bool,             // Elimination status
    stats: PlayerStats,           // Statistics object
}
```

### PlayerStats
```rust
struct PlayerStats {
    losses: u32,
    wins: u32,
    times_sat_out: u32,
    eliminated_status: bool,
}
```

### Match
```rust
struct Match {
    id: Uuid,
    team_1: Vec<PlayerId>,  // 2 players
    team_2: Vec<PlayerId>,  // 2 players
    winner: Option<Team>,  // None = not played, Team::One or Team::Two
    round: RoundType,      // GroupPlay, SemiFinals, Finals, GrandFinals
}

enum Team {
    One,
    Two,
}
```

### Tournament
```rust
struct Tournament {
    id: Uuid,
    players: Vec<Player>,
    eliminated_players: Vec<Player>,
    last_eliminated_players: Vec<Player>,  // Last round's eliminations
    matches: Vec<Match>,
    unused_players: Vec<Player>,           // Players sitting out current round
    max_losses: u32,                      // Losses before elimination
    state: TournamentState,
    match_results: HashMap<MatchId, Team>, // Current round results
    final_match_results: HashMap<MatchId, Team>, // Final rounds results
}

enum TournamentState {
    Setup,              // Adding players
    GroupPlay,          // Main tournament phase (>8 players)
    FinalSelection,     // 8 or fewer players, need to select final 8
    SemiFinals,         // 8 players
    Finals,             // 4 players
    GrandFinals,        // 2 players
    Completed,          // Tournament finished
}
```

---

## Core Logic & Algorithms

### 1. Match Generation Algorithm (`generate_gruppeplay_matches`)

**Purpose**: Create 2v2 matches from available players.

**Steps**:
1. Filter out eliminated players
2. Sort by `internal_times_sat_out` (ascending) to prioritize players who haven't sat out
3. Calculate excess players: `excess = available_players.len() % 4`
4. Remove excess players (they sit out this round)
5. Randomize remaining players:
   - Assign random seed to each player
   - Shuffle the list
6. Group into teams of 4:
   - Split into groups of 4
   - Each group becomes a match: first 2 vs last 2
7. Update sit-out count for unused players

**Rust Implementation**:
```rust
fn generate_matches(players: &mut Vec<Player>) -> (Vec<Match>, Vec<PlayerId>) {
    let mut available: Vec<_> = players.iter_mut()
        .filter(|p| !p.eliminated)
        .collect();
    
    // Sort by times sat out (ascending), then randomize
    available.sort_by_key(|p| (p.internal_times_sat_out, thread_rng().gen::<u32>()));
    
    let excess = available.len() % 4;
    let mut unused: Vec<PlayerId> = available
        .iter()
        .rev()
        .take(excess)
        .map(|p| p.id)
        .collect();
    
    let mut playing: Vec<_> = available
        .into_iter()
        .rev()
        .skip(excess)
        .collect();
    
    // Randomize
    playing.shuffle(&mut thread_rng());
    
    // Create matches
    let mut matches = Vec::new();
    for chunk in playing.chunks_exact(4) {
        let team1 = vec![chunk[0].id, chunk[1].id];
        let team2 = vec![chunk[2].id, chunk[3].id];
        matches.push(Match {
            id: Uuid::new_v4(),
            team_1: team1,
            team_2: team2,
            winner: None,
            round: RoundType::GroupPlay,
        });
    }
    
    (matches, unused)
}
```

### 2. Result Processing (`handle_match_results`)

**Purpose**: Process match results and update player stats.

**Steps**:
1. Validate all matches have results
2. For each match:
   - If Team 1 wins: Team 2 players get a loss, Team 1 players get a win
   - If Team 2 wins: Team 1 players get a loss, Team 2 players get a win
3. Check elimination: if `player.losses >= max_losses`, eliminate player
4. Track eliminated players in `last_eliminated_players`
5. Update UI state

**Rust Implementation**:
```rust
fn process_match_results(
    tournament: &mut Tournament,
    results: HashMap<MatchId, Team>,
) -> Result<(), TournamentError> {
    // Validate all matches have results
    for match_ in &tournament.matches {
        if !results.contains_key(&match_.id) {
            return Err(TournamentError::IncompleteResults);
        }
    }
    
    tournament.last_eliminated_players.clear();
    
    for match_ in &tournament.matches {
        let winner = results[&match_.id];
        
        match winner {
            Team::One => {
                // Team 2 loses
                for player_id in &match_.team_2 {
                    let player = tournament.get_player_mut(*player_id)?;
                    player.add_loss();
                    if player.losses >= tournament.max_losses {
                        player.eliminate();
                        tournament.last_eliminated_players.push(player.clone());
                    }
                }
                // Team 1 wins
                for player_id in &match_.team_1 {
                    tournament.get_player_mut(*player_id)?.add_win();
                }
            }
            Team::Two => {
                // Team 1 loses
                for player_id in &match_.team_1 {
                    let player = tournament.get_player_mut(*player_id)?;
                    player.add_loss();
                    if player.losses >= tournament.max_losses {
                        player.eliminate();
                        tournament.last_eliminated_players.push(player.clone());
                    }
                }
                // Team 2 wins
                for player_id in &match_.team_2 {
                    tournament.get_player_mut(*player_id)?.add_win();
                }
            }
        }
    }
    
    // Move eliminated players
    tournament.eliminated_players.extend(
        tournament.last_eliminated_players.iter().cloned()
    );
    tournament.players.retain(|p| !p.eliminated);
    
    // Check tournament state
    if tournament.players.len() <= 8 {
        tournament.state = TournamentState::FinalSelection;
    }
    
    Ok(())
}
```

### 3. Final Rounds Logic (`show_final_matches`)

**Purpose**: Handle knockout stages (Semi-Finals, Finals, Grand Finals).

**Stages**:
- **8 players** → Semi-Finals (2 matches, 4 players each → 2 winners each)
- **4 players** → Finals (1 match, 2 players each → 1 winner)
- **2 players** → Grand Finals (1 match, 1v1 → 1 winner)

**Match Structure**:
- 8 players: Split into 2 groups of 4, each group = 1 match (2v2)
- 4 players: 1 match (2v2)
- 2 players: 1 match (1v1)

**Rust Implementation**:
```rust
fn generate_final_matches(players: &[Player], stage: FinalStage) -> Vec<Match> {
    match stage {
        FinalStage::SemiFinals => {
            // 8 players -> 2 matches of 2v2
            let mut matches = Vec::new();
            for chunk in players.chunks_exact(4) {
                matches.push(Match {
                    id: Uuid::new_v4(),
                    team_1: vec![chunk[0].id, chunk[1].id],
                    team_2: vec![chunk[2].id, chunk[3].id],
                    winner: None,
                    round: RoundType::SemiFinals,
                });
            }
            matches
        }
        FinalStage::Finals => {
            // 4 players -> 1 match of 2v2
            vec![Match {
                id: Uuid::new_v4(),
                team_1: vec![players[0].id, players[1].id],
                team_2: vec![players[2].id, players[3].id],
                winner: None,
                round: RoundType::Finals,
            }]
        }
        FinalStage::GrandFinals => {
            // 2 players -> 1 match of 1v1
            vec![Match {
                id: Uuid::new_v4(),
                team_1: vec![players[0].id],
                team_2: vec![players[1].id],
                winner: None,
                round: RoundType::GrandFinals,
            }]
        }
    }
}
```

### 4. Fill Missing Players (`handle_missing_players`)

**Purpose**: When <8 players remain, allow selecting from last eliminated to fill to 8.

**Logic**:
- Calculate needed: `8 - current_players.len()`
- Show checkboxes for `last_eliminated_players`
- User selects exactly the needed amount
- Add selected players back to tournament

---

## Application Workflow

### State Machine

```
[Setup]
  ↓ (Add players, set max losses, click "Start Tournament")
[GroupPlay] (players > 8)
  ↓
  [Generate Matches] → [Display Matches] → [Select Winners] → [Submit Results]
  ↓
  [Process Results] → [Check Elimination] → [Update Stats]
  ↓
  (if players <= 8) → [FinalSelection]
  (else) → [GroupPlay] (loop)
  ↓
[FinalSelection] (players < 8)
  ↓
  [Select from Last Eliminated] → [Add Players] → (if players == 8) → [SemiFinals]
  ↓ (if players == 8)
[SemiFinals] (8 players)
  ↓
  [Generate Matches] → [Select Winners] → [Submit Results]
  ↓
  [Process Results] → (4 players remain) → [Finals]
  ↓
[Finals] (4 players)
  ↓
  [Generate Matches] → [Select Winners] → [Submit Results]
  ↓
  [Process Results] → (2 players remain) → [GrandFinals]
  ↓
[GrandFinals] (2 players)
  ↓
  [Generate Matches] → [Select Winners] → [Submit Results]
  ↓
  [Process Results] → [Completed]
  ↓
[Completed]
  ↓
  [Display Winners] → [Show Stats Table] → [Restart/Reset/Exit]
```

### Detailed Flow

1. **Setup Phase**
   - User enters player names
   - Sets max losses (default: 3)
   - Can add/remove players
   - Click "Start Tournament"

2. **Group Play Phase** (>8 players)
   - Click "Generate Matches"
   - System creates 2v2 matches
   - Players who can't form a team sit out
   - User clicks on winning team for each match
   - Click "Submit Results"
   - System processes results:
     - Winners get +1 win
     - Losers get +1 loss
     - Players with `losses >= max_losses` are eliminated
   - Repeat until ≤8 players remain

3. **Final Selection** (<8 players)
   - If exactly 8: proceed to Semi-Finals
   - If <8: show "Start Extra Game" button
   - User selects from `last_eliminated_players` to fill to 8
   - Selected players rejoin tournament

4. **Semi-Finals** (8 players)
   - Randomly seed 8 players
   - Create 2 matches (2v2 each)
   - Process results → 4 winners advance

5. **Finals** (4 players)
   - Create 1 match (2v2)
   - Process results → 2 winners advance

6. **Grand Finals** (2 players)
   - Create 1 match (1v1)
   - Process results → 1 winner (or 2 if it's a team)

7. **Completed**
   - Display winners
   - Show full stats table
   - Options: Restart, Reset, Exit

---

## UI Components & Features

### 1. Start Page
- **Input**: Player name (text field)
- **Input**: Max losses (spinbox, default: 3)
- **List**: Player names with remove buttons
- **Button**: "Add Player"
- **Button**: "Start Tournament"

### 2. Main Tournament View (Group Play)

**Left Side**:
- **Table**: Players
  - Columns: Name, Losses, Wins, Times Sat Out
  - Alternating row colors
  - Light blue headers

**Right Side**:
- **Table**: Matches (2 columns: Team 1, Team 2)
  - Clickable cells to select winner
  - Green = winner, Red = loser
- **Table**: Players that Sit Out
- **Table**: Eliminated Players

**Buttons**:
- "Generate Matches"
- "Submit Results" (disabled until matches generated)
- "Edit Losses" (select players, remove 1 loss)
- "Eliminate Player" (manual elimination)
- "Restart Tournament" (same players, reset stats)
- "End Tournament" (back to start page)

### 3. Final Selection View (<8 players)
- **Table**: Remaining Players
- **Table**: Last Eliminated Players
- **Button**: "Start Extra Game" (if <8)
- **Button**: "Proceed to Final Matches" (if == 8)

### 4. Extra Game Selection (<8 players)
- **Label**: "Select X players from last eliminated..."
- **Checkboxes**: One per last eliminated player
- **Button**: "Submit Selection"
- **Validation**: Must select exactly the required amount

### 5. Final Rounds View (Semi-Finals, Finals, Grand Finals)
- **Label**: Stage name (e.g., "Knockout Stage - Semi-Finals")
- **Table**: Matches (Team 1 vs Team 2)
  - Clickable to select winner
- **Button**: "Submit Final Results"

### 6. Tournament Complete View
- **Label**: Winner(s) announcement
- **Table**: All players with stats
  - Columns: Name, Losses, Wins, Times Sat Out
- **Button**: "Restart Tournament"
- **Button**: "Go Back to Start Page"
- **Button**: "Exit"

### 7. Dialogs
- **Player Selection Dialog**: Checkboxes for selecting players
  - Used for: Edit Losses, Eliminate Player
- **Confirmation Dialogs**: Yes/No for destructive actions

---

## Rust Implementation Requirements

### Backend (Server)

#### 1. Web Framework
- **Recommended**: Axum, Actix-Web, or Rocket
- **Why**: Modern, async, good ecosystem

#### 2. State Management
- **In-Memory Store**: For single-tournament sessions
  ```rust
  struct AppState {
      tournaments: Arc<RwLock<HashMap<TournamentId, Tournament>>>,
  }
  ```
- **Database** (Optional): For persistence
  - SQLite (simple) or PostgreSQL (production)
  - Use `sqlx` or `diesel`

#### 3. API Endpoints

**Tournament Management**:
- `POST /api/tournament` - Create tournament
- `GET /api/tournament/{id}` - Get tournament state
- `POST /api/tournament/{id}/players` - Add player
- `DELETE /api/tournament/{id}/players/{player_id}` - Remove player
- `PUT /api/tournament/{id}/max-losses` - Update max losses

**Match Management**:
- `POST /api/tournament/{id}/matches/generate` - Generate matches
- `GET /api/tournament/{id}/matches` - Get current matches
- `PUT /api/tournament/{id}/matches/{match_id}/winner` - Set match winner
- `POST /api/tournament/{id}/matches/submit` - Submit all results

**Tournament Control**:
- `POST /api/tournament/{id}/players/{player_id}/eliminate` - Manual elimination
- `PUT /api/tournament/{id}/players/{player_id}/losses` - Edit losses
- `POST /api/tournament/{id}/restart` - Restart tournament
- `POST /api/tournament/{id}/reset` - Reset tournament
- `POST /api/tournament/{id}/final-selection` - Select players for final 8

**Final Rounds**:
- `POST /api/tournament/{id}/finals/seed` - Seed final 8 players
- `POST /api/tournament/{id}/finals/matches` - Generate final matches
- `POST /api/tournament/{id}/finals/submit` - Submit final results

#### 4. WebSocket (Optional but Recommended)
- Real-time updates for multi-user scenarios
- Use `tokio-tungstenite` or `axum::extract::ws`

#### 5. Serialization
- Use `serde` with `serde_json`
- Define JSON schemas for API requests/responses

### Frontend (Client)

#### 1. Framework Options
- **Recommended**: Leptos, Yew, or Dioxus (Rust-based)
- **Alternative**: React/Vue with TypeScript (call Rust API)

#### 2. State Management
- Tournament state from API
- Local UI state (forms, dialogs)
- Reactive updates on state changes

#### 3. UI Components Needed

**Reusable Components**:
- `PlayerTable` - Display players with stats
- `MatchTable` - Display matches (clickable)
- `PlayerList` - Add/remove players
- `Button` - Styled buttons
- `Dialog` - Modal dialogs
- `CheckboxList` - Multi-select players
- `Input` - Text input, number input

**Pages/Views**:
- `StartPage` - Initial setup
- `TournamentView` - Main tournament interface
- `FinalSelectionView` - Select final 8
- `FinalRoundsView` - Semi-Finals, Finals, Grand Finals
- `CompletedView` - Tournament complete

#### 4. Styling
- CSS framework (Tailwind, Bootstrap) or custom CSS
- Match color scheme: Light blue headers (#68CDFE), alternating row colors

### Data Models (Shared)

Create a `models.rs` or separate files:

```rust
// models/player.rs
pub struct Player { ... }
pub struct PlayerStats { ... }

// models/match.rs
pub struct Match { ... }
pub enum Team { ... }
pub enum RoundType { ... }

// models/tournament.rs
pub struct Tournament { ... }
pub enum TournamentState { ... }
pub enum FinalStage { ... }

// models/api.rs
pub struct CreateTournamentRequest { ... }
pub struct TournamentResponse { ... }
pub struct MatchResultRequest { ... }
// etc.
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum TournamentError {
    #[error("Tournament not found")]
    NotFound,
    #[error("Incomplete match results")]
    IncompleteResults,
    #[error("Invalid player selection")]
    InvalidSelection,
    #[error("Tournament already completed")]
    AlreadyCompleted,
    // ... more errors
}
```

### Testing

- Unit tests for core logic (match generation, result processing)
- Integration tests for API endpoints
- E2E tests for full tournament flow

---
## Technical Stack Recommendations

### Backend
- **Framework**: Axum (modern, async, good docs)
- **Serialization**: `serde` + `serde_json`
- **UUID**: `uuid` crate
- **Random**: `rand` crate
- **Database** (optional): `sqlx` with SQLite/PostgreSQL
- **WebSocket**: `tokio-tungstenite` or Axum's built-in WS

### Frontend (Rust-based)
- **Framework**: **Leptos** (recommended - modern, fast, good DX)
  - Or **Yew** (mature, component-based)
  - Or **Dioxus** (React-like)
- **HTTP Client**: `reqwest` or `leptos-use` (for Leptos)
- **Styling**: Tailwind CSS or custom CSS

### Frontend (Alternative - JS/TS)
- **Framework**: React + TypeScript
- **HTTP Client**: `fetch` or `axios`
- **State**: React Query or Zustand
- **Styling**: Tailwind CSS

### Development Tools
- **Cargo**: Package manager
- **Cargo-watch**: Auto-reload during development
- **cargo-expand**: Macro expansion debugging
- **rustfmt**: Code formatting
- **clippy**: Linting

---

## Implementation Checklist

### Phase 1: Core Data Models
- [ ] Define `Player` struct
- [ ] Define `PlayerStats` struct
- [ ] Define `Match` struct
- [ ] Define `Tournament` struct
- [ ] Define enums (`TournamentState`, `Team`, `RoundType`)
- [ ] Implement serialization (`serde`)

### Phase 2: Core Logic
- [ ] Match generation algorithm
- [ ] Result processing logic
- [ ] Elimination logic
- [ ] Final rounds logic
- [ ] Player selection logic

### Phase 3: Backend API
- [ ] Set up web framework
- [ ] Implement tournament CRUD endpoints
- [ ] Implement match endpoints
- [ ] Implement control endpoints
- [ ] Error handling
- [ ] API documentation

### Phase 4: Frontend
- [ ] Set up frontend framework
- [ ] Create reusable components
- [ ] Implement Start Page
- [ ] Implement Tournament View
- [ ] Implement Final Selection View
- [ ] Implement Final Rounds View
- [ ] Implement Completed View
- [ ] Styling and UI polish

### Phase 5: Integration & Testing
- [ ] Connect frontend to backend
- [ ] Test full tournament flow
- [ ] Error handling and validation
- [ ] Edge case testing
- [ ] Performance optimization

### Phase 6: Polish
- [ ] UI/UX improvements
- [ ] Loading states
- [ ] Error messages
- [ ] Responsive design
- [ ] Documentation

---

## Key Algorithms Summary

1. **Match Generation**: Sort by sit-out count → randomize → group into 4s → create 2v2 matches
2. **Result Processing**: Update wins/losses → check elimination threshold → move eliminated players
3. **State Transitions**: Check player count → transition to appropriate tournament phase
4. **Final Rounds**: Different match structures based on player count (8→4→2)

---

## Notes for Rust Implementation

- Use `Arc<RwLock<>>` for shared state (thread-safe)
- Use `Uuid` for all IDs (type safety)
- Use `Result<T, TournamentError>` for error handling
- Consider using `async`/`await` for I/O operations
- Use `serde` for all serialization needs
- Implement `Clone` for data structures that need copying
- Use `HashMap` for O(1) lookups (match results, player lookups)

---

This document provides a complete blueprint for rebuilding the Dart Tournament Organizer as a Rust web application. Follow the phases and use the code examples as starting points.
