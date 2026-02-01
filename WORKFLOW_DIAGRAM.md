# Tournament Workflow Diagram

## Visual State Flow

```
┌─────────────────┐
│   SETUP PHASE   │
│                 │
│ • Add Players   │
│ • Set Max Losses│
│ • Start Button  │
└────────┬────────┘
         │
         ▼
┌──────────────────┐
│  GROUP PLAY      │◄────────┐
│  (>8 players)    │         │
│                  │         │
│ 1. Generate      │         │
│    Matches       │         │
│ 2. Select        │         │
│    Winners       │         │
│ 3. Submit        │         │
│    Results       │         │
│ 4. Process &     │         │
│    Eliminate     │         │
└────────┬─────────┘         │
         │                   │
         │ (if >8 players)   │
         └───────────────────┘
         │
         │ (if ≤8 players)
         ▼
┌─────────────────┐
│ FINAL SELECTION │
│                 │
│ • If == 8:      │
│   → Semi-Finals │
│ • If < 8:       │
│   → Extra Game  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  SEMI-FINALS    │
│  (8 players)    │
│                 │
│ • 2 matches     │
│ • 2v2 each      │
│ • → 4 winners   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│    FINALS       │
│  (4 players)    │
│                 │
│ • 1 match       │
│ • 2v2           │
│ • → 2 winners   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ GRAND FINALS    │
│  (2 players)    │
│                 │
│ • 1 match       │
│ • 1v1           │
│ • → 1 winner    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   COMPLETED     │
│                 │
│ • Show Winners  │
│ • Show Stats    │
│ • Restart/Reset │
└─────────────────┘
```

## Match Generation Flow

```
Available Players
       │
       ▼
Filter Eliminated
       │
       ▼
Sort by Times Sat Out (ascending)
       │
       ▼
Calculate Excess (count % 4)
       │
       ▼
Remove Excess Players → Sit Out
       │
       ▼
Randomize Remaining Players
       │
       ▼
Group into Chunks of 4
       │
       ▼
Create Matches:
  Chunk[0,1] vs Chunk[2,3]
       │
       ▼
Display Matches
```

## Result Processing Flow

```
Match Results Submitted
       │
       ▼
Validate All Matches Have Results
       │
       ▼
For Each Match:
  ├─ Winner = Team 1?
  │   ├─ Team 1: +1 Win
  │   └─ Team 2: +1 Loss
  │
  └─ Winner = Team 2?
      ├─ Team 2: +1 Win
      └─ Team 1: +1 Loss
       │
       ▼
Check Elimination:
  If Losses >= Max Losses:
    → Eliminate Player
       │
       ▼
Update Tournament State:
  ├─ Move to Eliminated List
  ├─ Remove from Active Players
  └─ Track in Last Eliminated
       │
       ▼
Check Player Count:
  ├─ > 8: Continue Group Play
  └─ ≤ 8: → Final Selection
```

## Final Rounds Flow

```
8 Players
  │
  ├─ Random Seed
  │
  ├─ Create 2 Matches:
  │   Match 1: Players[0,1] vs Players[2,3]
  │   Match 2: Players[4,5] vs Players[6,7]
  │
  └─ Process Results
      │
      ▼
4 Players (Winners)
  │
  ├─ Create 1 Match:
  │   Match: Players[0,1] vs Players[2,3]
  │
  └─ Process Results
      │
      ▼
2 Players (Winners)
  │
  ├─ Create 1 Match:
  │   Match: Player[0] vs Player[1]
  │
  └─ Process Results
      │
      ▼
Winner(s) Determined
```

## Data Flow

```
User Input
    │
    ▼
Frontend (UI)
    │
    ▼ HTTP/WebSocket
Backend API
    │
    ▼
Business Logic
    │
    ├─ Match Generation
    ├─ Result Processing
    ├─ State Management
    └─ Validation
    │
    ▼
State Update
    │
    ▼
Response to Frontend
    │
    ▼
UI Update
```
