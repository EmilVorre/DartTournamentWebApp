package utility

import (
	"backend/internal/structs"
)


func AddWin(playerMap map[string]*structs.Player, name string) {
	playerMap[name].Stats.TotalWins++
}

func AddLoss(playerMap map[string]*structs.Player, name string) {
	playerMap[name].Stats.TotalLosses++
}

func AddSatOut(playerMap map[string]*structs.Player, name string) {
	playerMap[name].Stats.TotalSatOut++
}

func AddPLayer(playerMap map[string]*structs.Player, name string) {
	// Initialize the player
	player := new(structs.Player)
	player.Name = name
	player.Losses = 0
	player.Wins = 0
	player.SatOut = 0
	player.Seed = len(playerMap) + 1
	
	// Add the player to the map
	playerMap[name] = player
}

