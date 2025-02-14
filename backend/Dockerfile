# Use Go 1.23 for building
FROM golang:1.23 AS builder

WORKDIR /app
COPY . .

# Ensure dependencies are installed
RUN go mod tidy

# Build the Go application with static linking (important for Alpine)
RUN go build -o server ./cmd/main.go

# Use a lightweight image
FROM alpine:latest

WORKDIR /app
COPY --from=builder /app/server .

# Ensure the binary is executable
RUN chmod +x server

EXPOSE 8080
CMD ["./server"]
