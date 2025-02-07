backend/
│── cmd/               # Main entry point for the app
│   ├── main.go        # Main application file
│── internal/          # Business logic (optional, avoids import issues)
│── pkg/               # Shared utilities
│── api/               # Handlers and routes
│   ├── handlers/      # Route handlers
│── config/            # Configuration files (e.g., env)
│── db/                # Database models & interactions
│── middleware/        # Middleware functions (e.g., auth, logging)
│── go.mod             # Dependency management
│── go.sum             # Checksums for dependencies
