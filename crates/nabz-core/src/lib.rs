//! Shared persistence foundation used by every Nabz user interface.
//!
//! Both Tauri and Slint open the same per-user database. Keep schema creation
//! and migrations here so a UI switch never creates a second accounting ledger.

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const APP_DATA_DIRECTORY: &str = "com.herotux.nabz.webprototype";
pub const DATABASE_FILE: &str = "nabz.sqlite3";
pub const SCHEMA_VERSION: i64 = 1;

/// Normalize Persian and English contact roles consistently across frontends.
pub fn normalize_contact_kind(raw: &str) -> Option<&'static str> {
    match raw.trim().to_lowercase().as_str() {
        "" | "customer" | "مشتری" => Some("customer"),
        "supplier" | "vendor" | "تامین‌کننده" | "تأمین‌کننده"
        | "تامین کننده" | "تأمین کننده" => Some("supplier"),
        "both" | "customer_supplier" | "هر دو"
        | "مشتری و تامین‌کننده" | "مشتری و تأمین‌کننده"
        | "مشتری و تامین کننده" | "مشتری و تأمین کننده" => Some("both"),
        _ => None,
    }
}

/// Resolve the canonical database path. NABZ_DB_PATH is retained as an
/// explicit override for portable installs, tests, and administrators.
pub fn database_path() -> PathBuf {
    if let Some(path) = std::env::var_os("NABZ_DB_PATH") {
        return PathBuf::from(path);
    }

    // Match Tauri's Windows AppDataDir (RoamingAppData) to avoid split databases.
    #[cfg(target_os = "windows")]
    let data_home = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let data_home = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));

    data_home
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DATA_DIRECTORY)
        .join(DATABASE_FILE)
}

/// Identifies one of the independently installed Nabz frontends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Frontend {
    Tauri,
    Slint,
}

/// Start another installed frontend from the current installation directory or PATH.
/// The caller should close its own window only after this function succeeds.
pub fn launch_frontend(frontend: Frontend) -> Result<Child, String> {
    let executable = match frontend {
        Frontend::Tauri => if cfg!(windows) { "nabz-tauri.exe" } else { "nabz-tauri" },
        Frontend::Slint => if cfg!(windows) { "nabz-slint.exe" } else { "nabz-slint" },
    };

    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        let sibling = directory.join(executable);
        if sibling.is_file() {
            return Command::new(sibling)
                .spawn()
                .map_err(|e| format!("اجرای رابط دیگر ناموفق بود: {e}"));
        }
    }

    Command::new(executable)
        .spawn()
        .map_err(|e| format!("رابط درخواستی نصب نیست یا در PATH پیدا نشد ({executable}): {e}"))
}

/// Open the canonical per-user database shared by all Nabz frontends.
pub fn open_database() -> Result<Connection, String> {
    open_database_at(database_path())
}

/// Open a database at an explicit path and apply the canonical schema/migrations.
pub fn open_database_at(path: impl AsRef<Path>) -> Result<Connection, String> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("ساخت پوشهٔ داده ناموفق بود: {e}"))?;
    }

    let connection = Connection::open(path)
        .map_err(|e| format!("بازکردن پایگاه داده ناموفق بود ({}): {e}", path.display()))?;
    initialize(&connection)?;
    Ok(connection)
}

/// Initialize the common schema and apply every outstanding migration.
pub fn initialize(connection: &Connection) -> Result<(), String> {
    // Reject newer schemas before any DDL/DML can mutate a database.
    let current_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if current_version > SCHEMA_VERSION {
        return Err(format!(
            "نسخهٔ پایگاه داده ({current_version}) از نسخهٔ پشتیبانی‌شدهٔ این برنامه ({SCHEMA_VERSION}) جدیدتر است."
        ));
    }

    connection
        .execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS contacts(
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               kind TEXT NOT NULL DEFAULT 'customer',
               phone TEXT NOT NULL DEFAULT '',
               opening_balance INTEGER NOT NULL DEFAULT 0,
               notes TEXT NOT NULL DEFAULT '',
               created_at TEXT NOT NULL DEFAULT(datetime('now','localtime'))
             );
             CREATE TABLE IF NOT EXISTS products(
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               code TEXT NOT NULL UNIQUE,
               barcode TEXT NOT NULL DEFAULT '',
               buy_price INTEGER NOT NULL DEFAULT 0,
               sell_price INTEGER NOT NULL DEFAULT 0,
               quantity INTEGER NOT NULL DEFAULT 0,
               min_quantity INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS invoices(
               id INTEGER PRIMARY KEY,
               number INTEGER NOT NULL UNIQUE,
               kind TEXT NOT NULL,
               contact_id INTEGER,
               contact_name TEXT NOT NULL,
               invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               subtotal INTEGER NOT NULL,
               discount INTEGER NOT NULL DEFAULT 0,
               total INTEGER NOT NULL,
               paid INTEGER NOT NULL DEFAULT 0,
               notes TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS invoice_items(
               id INTEGER PRIMARY KEY,
               invoice_id INTEGER NOT NULL REFERENCES invoices(id),
               product_id INTEGER,
               item_name TEXT NOT NULL,
               quantity INTEGER NOT NULL,
               unit_price INTEGER NOT NULL,
               line_total INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS payments(
               id INTEGER PRIMARY KEY,
               kind TEXT NOT NULL CHECK(kind IN('receipt','payment')),
               contact_id INTEGER,
               contact_name TEXT NOT NULL,
               amount INTEGER NOT NULL CHECK(amount>0),
               method TEXT NOT NULL DEFAULT 'cash',
               payment_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               notes TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS settings(
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             UPDATE contacts SET kind='customer' WHERE lower(trim(kind)) IN ('مشتری');
             UPDATE contacts SET kind='supplier'
               WHERE lower(trim(kind)) IN ('vendor','تامین‌کننده','تأمین‌کننده','تامین کننده','تأمین کننده');
             UPDATE contacts SET kind='both'
               WHERE trim(kind) IN ('مشتری و تامین‌کننده','مشتری و تأمین‌کننده','مشتری و تامین کننده','مشتری و تأمین کننده');",
        )
        .map_err(|e| e.to_string())?;

    migrate(connection)
}

fn migrate(connection: &Connection) -> Result<(), String> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    if version > SCHEMA_VERSION {
        return Err(format!(
            "نسخهٔ پایگاه داده ({version}) از نسخهٔ پشتیبانی‌شدهٔ این برنامه ({SCHEMA_VERSION}) جدیدتر است."
        ));
    }

    if version < 1 {
        let tx = connection.unchecked_transaction().map_err(|e| e.to_string())?;
        let has_voided: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name='voided'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if has_voided == 0 {
            tx.execute_batch(
                "ALTER TABLE invoices ADD COLUMN voided INTEGER NOT NULL DEFAULT 0 CHECK(voided IN (0,1));",
            )
            .map_err(|e| format!("افزودن وضعیت ابطال فاکتور ناموفق بود: {e}"))?;
        }

        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS sales_returns(
               id INTEGER PRIMARY KEY,
               invoice_id INTEGER NOT NULL REFERENCES invoices(id),
               return_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               amount INTEGER NOT NULL CHECK(amount>0),
               notes TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS sales_return_items(
               id INTEGER PRIMARY KEY,
               return_id INTEGER NOT NULL REFERENCES sales_returns(id),
               invoice_item_id INTEGER NOT NULL REFERENCES invoice_items(id),
               product_id INTEGER NOT NULL REFERENCES products(id),
               item_name TEXT NOT NULL,
               quantity INTEGER NOT NULL CHECK(quantity>0),
               unit_price INTEGER NOT NULL,
               amount INTEGER NOT NULL CHECK(amount>0)
             );
             CREATE TABLE IF NOT EXISTS sales_refunds(
               id INTEGER PRIMARY KEY,
               invoice_id INTEGER NOT NULL REFERENCES invoices(id),
               return_id INTEGER NOT NULL REFERENCES sales_returns(id),
               refund_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               amount INTEGER NOT NULL CHECK(amount>0),
               method TEXT NOT NULL DEFAULT 'cash',
               notes TEXT NOT NULL DEFAULT ''
             );",
        )
        .map_err(|e| e.to_string())?;

        tx.pragma_update(None, "user_version", 1).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_roles_are_normalized_consistently() {
        assert_eq!(normalize_contact_kind("مشتری"), Some("customer"));
        assert_eq!(normalize_contact_kind("تأمین‌کننده"), Some("supplier"));
        assert_eq!(normalize_contact_kind("هر دو"), Some("both"));
        assert_eq!(normalize_contact_kind("unknown"), None);
    }

    #[test]
    fn fresh_database_gets_shared_schema_and_version() {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();

        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        let has_returns: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sales_returns'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(has_returns, 1);
    }

    #[test]
    fn initialization_is_idempotent_and_preserves_existing_records() {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection.execute(
            "INSERT INTO contacts(name,kind) VALUES('Customer','customer')",
            [],
        ).unwrap();

        initialize(&connection).unwrap();

        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM contacts WHERE name='Customer'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn two_frontends_opening_the_canonical_file_share_one_ledger() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nabz-shared-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let db_path = root.join(DATABASE_FILE);

        let first_frontend = open_database_at(&db_path).unwrap();
        first_frontend.execute(
            "INSERT INTO contacts(name,kind) VALUES('Shared customer','customer')",
            [],
        ).unwrap();

        // A second frontend process opens the same canonical file and sees
        // the exact same record; it does not initialize a parallel ledger.
        let second_frontend = open_database_at(&db_path).unwrap();
        let count: i64 = second_frontend.query_row(
            "SELECT COUNT(*) FROM contacts WHERE name='Shared customer'",
            [], |row| row.get(0)
        ).unwrap();
        assert_eq!(count, 1);

        second_frontend.execute(
            "INSERT INTO products(name,code) VALUES('Shared item','SHARED-1')",
            [],
        ).unwrap();
        let product_count: i64 = first_frontend.query_row(
            "SELECT COUNT(*) FROM products WHERE code='SHARED-1'",
            [], |row| row.get(0)
        ).unwrap();
        assert_eq!(product_count, 1);

        drop(second_frontend);
        drop(first_frontend);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_database_from_a_newer_schema_without_mutating_it() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE future_data(value TEXT); INSERT INTO future_data VALUES('keep');").unwrap();
        connection.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
        assert!(initialize(&connection).unwrap_err().contains("جدیدتر"));
        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        let contacts_table: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='contacts'", [], |r| r.get(0)
        ).unwrap();
        let value: String = connection.query_row("SELECT value FROM future_data", [], |r| r.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION + 1);
        assert_eq!(contacts_table, 0);
        assert_eq!(value, "keep");
    }
}
