# Nabz shared data and uninstall policy

Nabz is one product with two independently installable frontends. The shared
SQLite database and user files belong to the product, not to either frontend.

## Linux data location

```text
${XDG_DATA_HOME:-~/.local/share}/com.herotux.nabz.webprototype/
├── nabz.sqlite3
├── backups/
├── attachments/
└── logs/
```

The existing application identifier and directory are intentionally retained so
updates do not strand databases created by earlier Tauri/Slint builds.

## Package maintainer rules

- The Slint package may remove only its own executable, desktop entry, and
  package-owned documentation.
- The Tauri package may remove only its own installed files.
- Do not add `rm -rf ~/.local/share/com.herotux.nabz.webprototype` or any
  equivalent recursive deletion to package maintainer scripts.
- Do not put the SQLite database under `/opt/nabz`, `/usr/share/nabz`, an
  AppImage mount, or a frontend-specific cache directory.
- Upgrades and reinstalls must open the existing database through
  `nabz-core::open_database()`.
- A future "delete all Nabz data" feature must be separate from uninstall,
  explain the irreversible effect, require explicit confirmation, and recommend
  a verified backup first.

## UI switching

The switch operation launches the installed target frontend against the same
canonical database. A successful handoff records the preferred UI in the shared
SQLite settings table. If launch fails, the previous preference is restored.
No database copy, export/import, or schema reset occurs during a switch.

## Verification

Core tests cover shared preference persistence, invalid preference values, the
shared directory layout, and opening one database through two frontend
connections. Before release, also test uninstall/reinstall of each package with
a disposable database and verify that the other frontend still opens every
existing record.
