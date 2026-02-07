# Messenger MVP Backend

A simple messenger backend built with Axum, Tokio, sqlx (SQLite), and jsonwebtoken.

## Stack & Features

- **Axum**: Web framework
- **Tokio**: Async runtime
- **sqlx**: Database access (SQLite) with compile-time checking
- **DashMap**: Concurrency-friendly map for WebSocket connections
- **jsonwebtoken**: Authentication

## Installation

1. Ensure you have Rust installed.
2. The SQLite database `messenger.db` is already initialized. If not, install `sqlite3` and run:
   ```sh
   sqlite3 messenger.db "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE NOT NULL); CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY AUTOINCREMENT, sender_id INTEGER NOT NULL, receiver_id INTEGER NOT NULL, content TEXT NOT NULL, timestamp DATETIME DEFAULT CURRENT_TIMESTAMP);"
   ```
3. Create `.env` file (already provided):
   ```
   DATABASE_URL="sqlite:messenger.db"
   JWT_SECRET="secret"
   ```

## Running

```sh
cargo run
```

The server listens on `0.0.0.0:3000`.

## API Endpoints

### Authentication

- `POST /login`
    - Body: `{ "username": "alice" }`
    - Returns: `{ "token": "..." }`
    - Creates user if not exists.

### Messaging

- `POST /send` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Body: `{ "receiver_username": "bob", "content": "Hello" }`
    - Saves message and pushes to Bob if he is connected via WebSocket.

- `GET /history/:username` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Returns list of messages between authenticated user and target user.

### WebSocket

- `GET /ws` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - **Bidirectional**:
        - **Receive**: Real-time stream of incoming messages.
        - **Send**: Send messages as JSON strings over the WebSocket.
        - **Format**: `{ "receiver_username": "bob", "content": "Hello via WS" }`

## Testing

For testing WebSocket, you can use `wscat` or a custom client. Note that passing headers in standard `wscat` might need specific flags or just use a Rust client/Postman.
