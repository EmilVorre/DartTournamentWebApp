package main

import (
	"log"
	"net/http"
	"github.com/gin-gonic/gin"
	"github.com/joho/godotenv"
	"os"

	// packages
	"backend/db"
)

func main() {
	// Load environment variables
	godotenv.Load()

	// Connect to database
	connectToDatabase := false
	if connectToDatabase {
		db.ConnectDatabase()
	}

	// Create router
	router := gin.Default()

	// Define a simple route
	router.GET("/ping", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"message": "pong"})
	})

	// Start server
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}
	log.Println("Starting server on port", port)
	router.Run(":" + port)
}
