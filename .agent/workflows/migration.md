---
description: Create and run a database migration while preserving data
---

1. Plan the migration. Identify if it's a simple `ALTER TABLE` or requires a table rebuild.
2. Create the migration file:
   ```bash
   sqlx migrate add <description>
   ```
3. Open the newly created SQL file in `migrations/`.
4. Implement the SQL logic. To preserve data during complex changes:
   - Create a temporary table with the new schema.
   - Copy data: `INSERT INTO temp_table SELECT columns FROM original_table;`
   - Drop original: `DROP TABLE original_table;`
   - Rename temp: `ALTER TABLE temp_table RENAME TO original_table;`
5. // turbo
   Apply the migration:
   ```bash
   sqlx migrate run
   ```
6. Run `cargo check` to verify compile-time SQL queries.
