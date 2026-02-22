# Agent Instructions for Antigravity

These instructions guide the agent's behavior for the Shindensen messenger backend project.

## API Documentation
- **Mandatory README updates**: Whenever you modify an API endpoint (change path, request body, response format, or authentication requirements), you **MUST** immediately update the `README.md` file.
- Ensure the sample JSON responses and headers in `README.md` are always in sync with the actual implementation in `handlers.rs` and `models.rs`.

## Database Migrations
- **Tooling**: Always use `sqlx-cli` for database operations.
- **Workflow**:
    1. Create a new migration using `sqlx migrate add <description>`.
    2. Write the SQL for the migration.
    3. Apply the migration using `sqlx migrate run`.
- **Data Preservation**:
    - **Prioritize Stability**: When changing the schema, try to preserve existing data. 
    - **Non-destructive Changes**: Use `ALTER TABLE` to add columns or change types where supported by SQLite.
    - **Complex Changes**: If a change requires dropping/recreating tables (e.g., renaming a column in older SQLite versions or changing constraints), use temporary tables to migrate existing data:
        1. Create a new table with the desired schema.
        2. `INSERT INTO new_table SELECT ... FROM old_table`.
        3. `DROP TABLE old_table`.
        4. `ALTER TABLE new_table RENAME TO old_table`.
    - **Verify**: Always check the impact of a migration on existing data before applying it.

## Testing & Verification
- After making changes or applying migrations, run `cargo check` to ensure compile-time SQL queries (if any) are still valid.
- If possible, verify the API changes with manual requests or by checking the auto-generated types if using a frontend integration.
