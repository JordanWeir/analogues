# SQLite CLI Cheat Sheet

Practical "SQLite 101" commands for inspecting a `.sqlite`, `.db`, or `.sqlite3` file from the command line.

## Open A Database

```bash
# Open an interactive SQLite shell
sqlite3 my.db

# Open read-only when you only want to inspect
sqlite3 -readonly my.db

# Open with nice tabular output
sqlite3 -header -column my.db

# Run one command and exit
sqlite3 my.db ".tables"

# Run SQL and exit
sqlite3 my.db "SELECT name FROM sqlite_schema WHERE type='table';"
```

## First Commands To Type

Once inside `sqlite3`:

```sql
.help
.databases
.tables
.schema
.quit
```

## Must-Know Dot Commands

Dot commands are SQLite shell commands, not SQL.

```sql
.help                    -- show shell help
.show                    -- show current shell settings
.databases               -- list attached/open databases
.tables                  -- list tables and views
.tables pattern*         -- filter tables by pattern
.schema                  -- show all schema DDL
.schema users            -- show schema for one table
.fullschema              -- show full schema including internals
.indexes                 -- list indexes
.indexes users           -- list indexes on one table
.dbinfo                  -- basic database file information
.dump                    -- export whole database as SQL
.dump users              -- export one table as SQL
.backup backup.db        -- make a backup
.restore backup.db       -- restore from backup into current DB
.headers on              -- print column headers
.headers off             -- hide column headers
.mode box                -- pretty boxed table output
.mode column             -- aligned columns
.mode tabs               -- tab-separated output
.mode csv                -- CSV output
.nullvalue NULL          -- display NULL values more clearly
.width 20 10 10          -- set display widths in column mode
.timer on                -- show query timing
.eqp on                  -- show query plan automatically
.stats on                -- show memory/runtime stats
.once result.txt         -- send next query output to file
.output result.txt       -- send all output to file
.output stdout           -- send output back to terminal
.read script.sql         -- run commands from a file
.expert                  -- suggest useful indexes for queries
.quit                    -- exit sqlite3
```

## Must-Know SQL For Inspection

### List Tables, Views, Indexes, And Triggers

```sql
SELECT type, name
FROM sqlite_schema
WHERE type IN ('table', 'view', 'index', 'trigger')
ORDER BY type, name;
```

### Show Table Definitions

```sql
SELECT sql
FROM sqlite_schema
WHERE type = 'table'
ORDER BY name;
```

### Show Columns For A Table

```sql
PRAGMA table_info(users);
```

Useful columns in the result:

- `cid`: column position
- `name`: column name
- `type`: declared type
- `notnull`: whether `NOT NULL` is set
- `dflt_value`: default value
- `pk`: whether it is part of the primary key

### Show Foreign Keys

```sql
PRAGMA foreign_key_list(users);
```

### Show Indexes On A Table

```sql
PRAGMA index_list(users);
```

### Show Indexed Columns

```sql
PRAGMA index_info(idx_users_email);
```

### Count Rows

```sql
SELECT COUNT(*) FROM users;
```

### Preview Rows

```sql
SELECT * FROM users LIMIT 20;
```

### Preview The Newest Rows By `rowid`

```sql
SELECT rowid, *
FROM users
ORDER BY rowid DESC
LIMIT 20;
```

### Search For A Value

```sql
SELECT *
FROM users
WHERE email LIKE '%example.com%'
LIMIT 20;
```

### Check For NULLs Or Unexpected Values

```sql
SELECT *
FROM users
WHERE email IS NULL
LIMIT 20;
```

### See Distinct Values

```sql
SELECT status, COUNT(*) AS rows
FROM users
GROUP BY status
ORDER BY rows DESC;
```

### Inspect Recent Data With Sorting

```sql
SELECT id, created_at, status
FROM orders
ORDER BY created_at DESC
LIMIT 20;
```

## Best PRAGMAs For Inspection

```sql
PRAGMA journal_mode;
PRAGMA synchronous;
PRAGMA foreign_keys;
PRAGMA page_size;
PRAGMA page_count;
PRAGMA freelist_count;
PRAGMA encoding;
PRAGMA user_version;
PRAGMA application_id;
```

## Health Checks

```sql
PRAGMA quick_check;
PRAGMA integrity_check;
```

Use `quick_check` first. `integrity_check` is more thorough.

## Query Plans

When a query is slow, inspect the plan:

```sql
EXPLAIN QUERY PLAN
SELECT *
FROM users
WHERE email = 'alice@example.com';
```

Or turn plans on for every query:

```sql
.eqp on
```

## Nice Output Formats

These are especially handy when you are inspecting by eye:

```sql
.headers on
.mode box

SELECT id, email, created_at
FROM users
LIMIT 10;
```

Other useful modes:

```sql
.mode column   -- aligned text
.mode csv      -- copy into spreadsheets
.mode json     -- JSON output if supported by your sqlite3 build
```

## Common One-Liners

```bash
# List tables
sqlite3 my.db ".tables"

# Show schema for one table
sqlite3 my.db ".schema users"

# Count rows in a table
sqlite3 my.db "SELECT COUNT(*) FROM users;"

# Show columns for a table
sqlite3 my.db "PRAGMA table_info(users);"

# Show all database objects
sqlite3 my.db "SELECT type, name FROM sqlite_schema ORDER BY type, name;"

# Run read-only with headers and columns
sqlite3 -readonly -header -column my.db "SELECT * FROM users LIMIT 20;"

# Dump the whole DB to SQL
sqlite3 my.db ".dump" > dump.sql
```

## A Good 60-Second Inspection Workflow

```bash
sqlite3 -readonly my.db
```

Then inside the shell:

```sql
.headers on
.mode box
.databases
.tables
.schema users
PRAGMA table_info(users);
SELECT COUNT(*) FROM users;
SELECT * FROM users LIMIT 10;
PRAGMA quick_check;
```

## Tips

- Prefer `sqlite3 -readonly my.db` when you only want to inspect.
- Use `.mode box` plus `.headers on` for the most readable output.
- `sqlite_schema` is your friend for discovering tables, views, indexes, and triggers.
- `PRAGMA table_info(table_name)` is usually the fastest way to understand a table.
- `EXPLAIN QUERY PLAN ...` helps you understand why a query is slow.
- `.dump` is the easiest way to export the database structure and data as SQL.
- If a database is in WAL mode, you may see companion files like `my.db-wal` and `my.db-shm`.

## Minimal Set To Memorize

If you only memorize a few commands, make them these:

```sql
.tables
.schema
.headers on
.mode box
PRAGMA table_info(table_name);
SELECT COUNT(*) FROM table_name;
SELECT * FROM table_name LIMIT 20;
PRAGMA quick_check;
.quit
```
