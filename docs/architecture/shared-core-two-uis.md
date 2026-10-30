# Nabz architecture: one product, two interchangeable UIs

Nabz is one accounting product with two independently installable frontends. It is
not two separate accounting applications.

## Components

- `crates/nabz-core`: canonical SQLite schema, schema migrations, shared persistence
  foundation, and (as the refactor progresses) accounting/business operations.
- `tauri-web`: independently buildable Tauri frontend.
- `slint-native`: independently buildable native Slint frontend.

Both frontends must use the same core crate and the same per-user database. A user
may install either frontend or both, switch between them, and continue working with
the same contacts, products, invoices, payments, returns, and settings.

## Database identity

The canonical Linux database path is:

```text
$XDG_DATA_HOME/com.herotux.nabz.webprototype/nabz.sqlite3
```

When `XDG_DATA_HOME` is not set:

```text
~/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3
```

This preserves the Slint prototype's existing Linux data directory and matches the
Tauri bundle identifier's application data directory. `NABZ_DB_PATH` remains an
explicit override for portable deployments and tests. Both frontends must not
silently create a second default database.

## Switching and safe use

- Switching UI is not a data conversion: both frontends open the same database.
- Keep schema migrations exclusively in `nabz-core`.
- A frontend must refuse to open a database with a newer unsupported schema version.
- SQLite foreign keys and WAL mode are enabled by the shared initializer.
- Before shipping UI switching, add a backup/restore flow and a safe single-writer
  policy for simultaneous launches. SQLite serializes writes, but application-level
  workflows still need conflict handling and user feedback.
- Do not delete the shared data directory when uninstalling either frontend.

## Independent distribution

Build and publish separate artifacts for Tauri and Slint. Their installers and
binaries may be installed or removed independently; the shared user data directory
must remain untouched. Keep separate CI jobs so a failure in one UI does not block
developers from building the other, while requiring both to pass before a combined
release is declared ready.

## Refactor status

The initial shared persistence layer now owns the common base schema and v1 migration.
The frontends still contain duplicated accounting queries and business operations.
Those must be moved into `nabz-core` incrementally before claiming that all business
logic is shared. Preserve frontend command/callback contracts while extracting the
operations, and verify both frontends against the same copied test database.
