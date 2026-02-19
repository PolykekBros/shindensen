-- Migration to add attachments support
-- Make content nullable and add message_attachments table

-- Disable foreign keys temporarily to handle table recreation
PRAGMA foreign_keys=OFF;

-- Create new messages table with nullable content
CREATE TABLE messages_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    sender_id INTEGER NOT NULL REFERENCES users(id),
    content TEXT,
    timestamp TEXT NOT NULL
);

-- Copy data from old table
INSERT INTO messages_new (id, chat_id, sender_id, content, timestamp)
SELECT id, chat_id, sender_id, content, timestamp FROM messages;

-- Drop old table and rename new one
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;

-- Create message_attachments table
CREATE TABLE message_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    type TEXT NOT NULL CHECK(type IN ('picture', 'video', 'audio', 'file')),
    url TEXT NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT,
    size_bytes INTEGER NOT NULL
);

PRAGMA foreign_keys=ON;
