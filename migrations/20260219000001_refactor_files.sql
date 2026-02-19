-- Refactor files into a separate entity
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL CHECK(type IN ('picture', 'video', 'audio', 'file')),
    url TEXT NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT,
    size_bytes INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Create association table for messages and files
CREATE TABLE message_files (
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, file_id)
);

-- Drop the old message_attachments table
DROP TABLE IF EXISTS message_attachments;

-- Update users table to allow a profile picture reference
ALTER TABLE users ADD COLUMN image_id INTEGER REFERENCES files(id) ON DELETE SET NULL;
