use nabz_core::{open_database_at, preferred_frontend, set_preferred_frontend, Frontend};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "nabz-ui-lifecycle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn both_frontends_reopen_the_same_ledger_after_either_ui_is_removed() {
    let root = test_root();
    let data_root = root.join("xdg-data").join("com.herotux.nabz.webprototype");
    let database = data_root.join("nabz.sqlite3");
    let slint_install = root.join("opt/nabz-slint");
    let tauri_install = root.join("opt/nabz-tauri");

    std::fs::create_dir_all(&slint_install).unwrap();
    std::fs::create_dir_all(&tauri_install).unwrap();
    std::fs::write(slint_install.join("frontend.marker"), "slint").unwrap();
    std::fs::write(tauri_install.join("frontend.marker"), "tauri").unwrap();

    // First frontend creates the ledger and writes representative accounting data.
    {
        let db = open_database_at(&database).unwrap();
        db.execute(
            "INSERT INTO contacts(name, kind) VALUES('Lifecycle customer', 'customer')",
            [],
        ).unwrap();
        db.execute(
            "INSERT INTO products(name, code, buy_price, quantity, average_cost)
             VALUES('Lifecycle item', 'LIFE-001', 125, 8, 120)",
            [],
        ).unwrap();
        set_preferred_frontend(&db, Frontend::Slint).unwrap();
    }

    // Removing Slint's installation directory must not touch the shared data directory.
    std::fs::remove_dir_all(&slint_install).unwrap();
    assert!(database.is_file());
    assert!(tauri_install.join("frontend.marker").is_file());

    // Tauri opens the exact same database and sees the existing records.
    {
        let db = open_database_at(&database).unwrap();
        let contact: String = db.query_row(
            "SELECT name FROM contacts WHERE name='Lifecycle customer'",
            [],
            |row| row.get(0),
        ).unwrap();
        let (code, quantity): (String, i64) = db.query_row(
            "SELECT code, quantity FROM products WHERE code='LIFE-001'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(contact, "Lifecycle customer");
        assert_eq!((code.as_str(), quantity), ("LIFE-001", 8));
        assert_eq!(preferred_frontend(&db).unwrap(), Some(Frontend::Slint));
        set_preferred_frontend(&db, Frontend::Tauri).unwrap();
    }

    // Removing Tauri also leaves the ledger intact; reinstalling Slint reuses it.
    std::fs::remove_dir_all(&tauri_install).unwrap();
    std::fs::create_dir_all(&slint_install).unwrap();
    let db = open_database_at(&database).unwrap();
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM contacts WHERE name='Lifecycle customer'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(count, 1);
    assert_eq!(preferred_frontend(&db).unwrap(), Some(Frontend::Tauri));

    drop(db);
    drop(Connection::open_in_memory().unwrap());
    std::fs::remove_dir_all(root).unwrap();
}
