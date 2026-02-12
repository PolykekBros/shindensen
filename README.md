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
