# Messenger MVP Backend

A simple messenger backend built with Axum, Tokio, sqlx (SQLite), and jsonwebtoken.

## Stack & Features

- **Axum**: Web framework
- **Tokio**: Async runtime
- **sqlx**: Database access (SQLite) with compile-time checking
- **DashMap**: Concurrency-friendly map for WebSocket connections
- **jsonwebtoken**: Authentication

## Installation

1. **Rust**: Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.
2. **Environment**: Create a `.env` file (one is provided in the root):
   ```env
   DATABASE_URL="sqlite:shindensen.db"
   JWT_SECRET="supersecret"
   ```
3. **Database & Migrations**: 
   Install `sqlx-cli` if you haven't already:
   ```sh
   brew install sqlx-cli
   ```
   Create the database and run migrations:
   ```sh
   sqlx database create
   sqlx migrate run
   ```

## Running

```sh
cargo run
```

## Database Migrations

This project uses `sqlx` for database migrations.

### Creating a new migration
To add a new schema change, run:
```sh
sqlx migrate add <description>
```
This will create a new SQL file in the `migrations/` directory.

### Applying migrations
```sh
sqlx migrate run
```

### Reverting migrations
```sh
sqlx migrate revert
```

The server listens on `0.0.0.0:3000`.

## API Endpoints

### Authentication

- `POST /login`
    - Body: `{ "username": "alice" }`
    - Returns: `{ "token": "..." }`
    - Creates user if not exists.

### Chats

- `POST /chats/initiate` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Body: `{ "target_username": "bob" }`
    - Returns: `{ "chat_id": 1, "status": "created" }` (or "exists")
    - Starts a direct chat with another user.

- `GET /chats/:chat_id/messages` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Returns list of messages in the chat.
    - User must be a participant of the chat.

### WebSocket

- `GET /ws` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - **Bidirectional**:
        - **Receive**: Real-time stream of incoming messages from ALL chats.
            - Format: `{ "chat_id": 1, "sender": "bob", "content": "Hello", "timestamp": "..." }`
        - **Send**: Send messages to a specific chat.
            - Format: `{ "chat_id": 1, "content": "Hello via WS" }`

## Testing

1. **Login** (`POST /login`) to get a token.
2. **Initiate Chat** (`POST /chats/initiate`) to get a `chat_id`.
3. **Connect WebSocket** (`GET /ws`) with token.
4. **Send Message** via WS: `{ "chat_id": <id>, "content": "Hello" }`.
