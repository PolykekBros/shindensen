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
    - Returns: 
      ```json
      {
        "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
      }
      ```
    - Creates user if not exists.

- `GET /users/:username`
    - Returns information about a specific user.
    - Returns 404 if user does not exist.
    - Sample response:
      ```json
      {
        "id": 1,
        "username": "alice",
        "display_name": "Alice Wonderland", // Optional
        "bio": "Curiouser and curiouser!", // Optional
        "image_id": 42 // Optional
      }
      ```

### Chats

- `POST /chats/initiate` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Body: `{ "target_username": "bob" }`
    - Returns:
      ```json
      {
        "chat_id": 1,
        "status": "created" // or "exists"
      }
      ```
    - Starts a direct chat with another user.

- `GET /chats` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Returns: A list of all chats the user is a participant in.
      ```json
      [
        {
          "id": 1,
          "name": "General", // Optional
          "chat_type": "group",
          "created_at": "2026-02-19T12:00:00Z"
        }
      ]
      ```

- `GET /chats/:chat_id/messages` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - Returns list of messages in the chat.
    - User must be a participant of the chat.
    - Sample response:
      ```json
      {
        "chat_id": 1,
        "messages": [
          {
            "id": 1,
            "chat_id": 1,
            "sender_id": 1,
            "content": "Hello!", // Optional
            "timestamp": "2026-02-19T12:00:00Z",
            "files": [
              {
                "id": 10,
                "type": "picture",
                "url": "/uploads/uuid.ext",
                "filename": "image.png",
                "mime_type": "image/png", // Optional
                "size_bytes": 12345,
                "created_at": "2026-02-19T12:00:00Z"
              }
            ]
          }
        ]
      }
      ```

### Files

- `POST /upload` (Protected)
    - Headers: `Authorization: Bearer <token>`, `Content-Type: multipart/form-data`
    - Body: Multi-part form with a `file` field.
    - Returns: Metadata about the uploaded file.
      ```json
      {
        "url": "/uploads/uuid.ext",
        "filename": "original_name.ext",
        "mime_type": "image/png", // Optional
        "size_bytes": 12345
      }
      ```
    - Note: Files are served from `/uploads/*`.

### WebSocket

- `GET /ws` (Protected)
    - Headers: `Authorization: Bearer <token>`
    - **Bidirectional**:
        - **Receive**: Real-time stream of incoming messages from ALL chats.
            - Format:
              ```json
              {
                "id": 123,
                "chat_id": 1,
                "sender_id": 45,
                "content": "Hello", // Optional
                "timestamp": "2026-02-19T12:00:00Z",
                "files": [
                  {
                    "id": 10,
                    "type": "picture",
                    "url": "/uploads/uuid.ext",
                    "filename": "image.png",
                    "mime_type": "image/png", // Optional
                    "size_bytes": 12345,
                    "created_at": "..."
                  }
                ]
              }
              ```
        - **Send**: Send messages to a specific chat, optionally with attachments.
            - Format:
              ```json
              {
                "chat_id": 1,
                "content": "Check this out!", // Optional
                "files": [ // Optional
                  {
                    "type": "picture",
                    "url": "/uploads/uuid.ext",
                    "filename": "image.png",
                    "mime_type": "image/png", // Optional
                    "size_bytes": 12345
                  }
                ]
              }
              ```

## Testing

1. **Login** (`POST /login`) to get a token.
2. **Initiate Chat** (`POST /chats/initiate`) to get a `chat_id`.
3. **Upload File** (`POST /upload`) to get a file URL if you want to send attachments.
4. **Connect WebSocket** (`GET /ws`) with token.
5. **Send Message** via WS: `{ "chat_id": <id>, "content": "Hello", "files": [...] }`.
