//! Shared persistence foundation used by every Nabz user interface.
//!
//! Both Tauri and Slint open the same per-user database. Keep schema creation
//! and migrations here so a UI switch never creates a second accounting ledger.

pub mod accounting;

use rusqlite::{backup::Backup, Connection, OpenFlags};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const APP_DATA_DIRECTORY: &str = "com.herotux.nabz.webprototype";
pub const DATABASE_FILE: &str = "nabz.sqlite3";
pub const SCHEMA_VERSION: i64 = 4;

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

/// Create a complete SQLite snapshot. The SQLite backup API includes every
/// table, index, trigger and schema migration (including future accounting tables).
pub fn backup_database_to(connection: &Connection, destination: impl AsRef<Path>) -> Result<(), String> {
    let destination = destination.as_ref();
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("ساخت پوشهٔ پشتیبان ناموفق بود: {e}"))?;
    }
    let result = (|| -> Result<(), String> {
        let mut output = Connection::open(destination)
            .map_err(|e| format!("ساخت فایل پشتیبان ناموفق بود: {e}"))?;
        let backup = Backup::new(connection, &mut output).map_err(|e| e.to_string())?;
        backup.run_to_completion(128, Duration::from_millis(10), None)
            .map_err(|e| format!("تهیهٔ نسخهٔ پشتیبان ناموفق بود: {e}"))?;
        drop(backup);
        let integrity: String = output.query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != "ok" {
            return Err(format!("اعتبارسنجی نسخهٔ پشتیبان ناموفق بود: {integrity}"));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}

/// Store a timestamped full-database backup in the shared Nabz data directory.
pub fn create_automatic_backup(connection: &Connection) -> Result<PathBuf, String> {
    let directory = database_path().parent()
        .ok_or("مسیر پایگاه داده معتبر نیست")?
        .join("backups");
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let path = directory.join(format!("nabz-{stamp}.sqlite3"));
    backup_database_to(connection, &path)?;
    Ok(path)
}

/// Return the newest timestamped SQLite backup in the shared data directory.
pub fn latest_backup_path() -> Result<PathBuf, String> {
    let directory = database_path().parent()
        .ok_or("مسیر پایگاه داده معتبر نیست")?
        .join("backups");
    let mut candidates = std::fs::read_dir(&directory)
        .map_err(|e| format!("پوشهٔ پشتیبان پیدا نشد: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sqlite3"))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop().ok_or_else(|| "هیچ نسخهٔ پشتیبانی برای بازیابی وجود ندارد".into())
}

/// Validate the minimum schema contract before a snapshot is allowed to replace live data.
fn validate_restore_source(source: &Connection) -> Result<(), String> {
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("فایل پشتیبان معتبر نیست: {e}"))?;
    if integrity != "ok" {
        return Err(format!("فایل پشتیبان خراب است: {integrity}"));
    }

    let version: i64 = source.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > SCHEMA_VERSION {
        return Err(format!("نسخهٔ پشتیبان ({version}) از نسخهٔ پشتیبانی‌شدهٔ برنامه ({SCHEMA_VERSION}) جدیدتر است."));
    }

    // A valid SQLite file is not necessarily a Nabz database. Reject unrelated
    // or incomplete files before touching the active ledger.
    for table in ["contacts", "products", "invoices", "invoice_items", "payments", "settings"] {
        let exists: i64 = source.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table], |row| row.get(0)
        ).map_err(|e| e.to_string())?;
        if exists != 1 {
            return Err(format!("فایل انتخاب‌شده پایگاه دادهٔ معتبر نبض نیست؛ جدول «{table}» وجود ندارد."));
        }
    }

    let foreign_key_errors: i64 = source.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if foreign_key_errors != 0 {
        return Err("فایل پشتیبان دارای ارجاع‌های ناسازگار است و بازیابی متوقف شد".into());
    }
    Ok(())
}

/// Restore a full database snapshot into the already-open shared connection.
///
/// A verified emergency snapshot is created before the live ledger is changed.
/// If SQLite replacement or post-restore migration fails, the emergency
/// snapshot is restored before returning an error. The safety snapshot is kept
/// after success so the user can recover the pre-restore state manually.
pub fn restore_database_from(connection: &mut Connection, source_path: impl AsRef<Path>) -> Result<(), String> {
    let source_path = source_path.as_ref();
    let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("فایل پشتیبان باز نشد: {e}"))?;
    validate_restore_source(&source)?;

    let canonical_db = database_path();
    let safety_dir = if canonical_db.exists() {
        canonical_db.parent().ok_or("مسیر پایگاه داده معتبر نیست")?.join("backups")
    } else {
        // In-memory and isolated tests must never write into a developer's real
        // Nabz data directory.
        std::env::temp_dir().join("nabz-pre-restore-backups")
    };
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let safety_path = safety_dir.join(format!("pre-restore-{stamp}.sqlite3"));
    backup_database_to(connection, &safety_path)
        .map_err(|e| format!("ساخت نسخهٔ ایمنی پیش از بازیابی ناموفق بود؛ دیتابیس فعلی تغییر نکرد: {e}"))?;

    let restore_result = (|| -> Result<(), String> {
        let backup = Backup::new(&source, connection).map_err(|e| e.to_string())?;
        backup.run_to_completion(128, Duration::from_millis(10), None)
            .map_err(|e| format!("بازیابی پایگاه داده ناموفق بود: {e}"))?;
        drop(backup);
        initialize(connection)?;
        Ok(())
    })();

    if let Err(restore_error) = restore_result {
        let safety = Connection::open_with_flags(&safety_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("{restore_error}; بازکردن نسخهٔ ایمنی برای بازگشت ناموفق بود: {e}. فایل ایمنی: {}", safety_path.display()))?;
        let rollback = Backup::new(&safety, connection)
            .and_then(|backup| backup.run_to_completion(128, Duration::from_millis(10), None));
        if let Err(rollback_error) = rollback {
            return Err(format!("{restore_error}; بازگردانی نسخهٔ ایمنی نیز ناموفق بود: {rollback_error}. فایل ایمنی را نگه دارید: {}", safety_path.display()));
        }
        drop(safety);
        let rollback_validation = initialize(connection);
        if let Err(validation_error) = rollback_validation {
            return Err(format!("{restore_error}; دادهٔ قبلی بازیابی شد اما بررسی نهایی خطا داد: {validation_error}. فایل ایمنی: {}", safety_path.display()));
        }
        return Err(format!("{restore_error}; دیتابیس قبلی بازیابی شد. نسخهٔ ایمنی: {}", safety_path.display()));
    }

    Ok(())
}

/// Restore the newest automatic backup.
pub fn restore_latest_backup(connection: &mut Connection) -> Result<PathBuf, String> {
    let path = latest_backup_path()?;
    restore_database_from(connection, &path)?;
    Ok(path)
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

    if version < 2 {
        let tx=connection.unchecked_transaction().map_err(|e|e.to_string())?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS purchase_returns(
               id INTEGER PRIMARY KEY,
               invoice_id INTEGER NOT NULL REFERENCES invoices(id),
               return_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               amount INTEGER NOT NULL CHECK(amount>0),
               notes TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS purchase_return_items(
               id INTEGER PRIMARY KEY,
               return_id INTEGER NOT NULL REFERENCES purchase_returns(id),
               invoice_item_id INTEGER NOT NULL REFERENCES invoice_items(id),
               product_id INTEGER NOT NULL REFERENCES products(id),
               item_name TEXT NOT NULL,
               quantity INTEGER NOT NULL CHECK(quantity>0),
               unit_price INTEGER NOT NULL,
               amount INTEGER NOT NULL CHECK(amount>0)
             );
             CREATE TABLE IF NOT EXISTS purchase_refunds(
               id INTEGER PRIMARY KEY,
               invoice_id INTEGER NOT NULL REFERENCES invoices(id),
               return_id INTEGER NOT NULL REFERENCES purchase_returns(id),
               refund_date TEXT NOT NULL DEFAULT(date('now','localtime')),
               amount INTEGER NOT NULL CHECK(amount>0),
               method TEXT NOT NULL DEFAULT 'cash',
               notes TEXT NOT NULL DEFAULT ''
             );"
        ).map_err(|e|e.to_string())?;
        tx.pragma_update(None,"user_version",2).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
    }

    if version < 3 {
        let tx=connection.unchecked_transaction().map_err(|e|e.to_string())?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS inventory_adjustments(
               id INTEGER PRIMARY KEY,
               product_id INTEGER NOT NULL REFERENCES products(id),
               product_name TEXT NOT NULL,
               previous_quantity INTEGER NOT NULL CHECK(previous_quantity>=0),
               new_quantity INTEGER NOT NULL CHECK(new_quantity>=0),
               delta INTEGER NOT NULL CHECK(delta<>0),
               reason TEXT NOT NULL CHECK(length(trim(reason))>=3),
               created_at TEXT NOT NULL DEFAULT(datetime('now','localtime'))
             );
             CREATE INDEX IF NOT EXISTS idx_inventory_adjustments_product_date
               ON inventory_adjustments(product_id,created_at DESC);"
        ).map_err(|e|e.to_string())?;
        tx.pragma_update(None,"user_version",3).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
    }

    if version < 4 {
        let tx = connection.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS inventory_adjustments_no_update
               BEFORE UPDATE ON inventory_adjustments
               BEGIN
                 SELECT RAISE(ABORT, 'inventory adjustment history is immutable');
               END;
             CREATE TRIGGER IF NOT EXISTS inventory_adjustments_no_delete
               BEFORE DELETE ON inventory_adjustments
               BEGIN
                 SELECT RAISE(ABORT, 'inventory adjustment history is immutable');
               END;"
        ).map_err(|e| e.to_string())?;
        tx.pragma_update(None, "user_version", 4).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_adjustment_audit_is_immutable_after_v4_migration() {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection.execute(
            "INSERT INTO products(name,code,quantity) VALUES('کالا','IMM-1',5)", []
        ).unwrap();
        connection.execute(
            "INSERT INTO inventory_adjustments(product_id,product_name,previous_quantity,new_quantity,delta,reason)
             VALUES(1,'کالا',4,5,1,'انبارگردانی')", []
        ).unwrap();

        let update = connection.execute(
            "UPDATE inventory_adjustments SET reason='تغییر' WHERE id=1", []
        );
        let delete = connection.execute("DELETE FROM inventory_adjustments WHERE id=1", []);
        assert!(update.is_err());
        assert!(delete.is_err());
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM inventory_adjustments", [], |row| row.get(0)
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn contact_roles_are_normalized_consistently() {
        assert_eq!(normalize_contact_kind("مشتری"), Some("customer"));
        assert_eq!(normalize_contact_kind("تأمین‌کننده"), Some("supplier"));
        assert_eq!(normalize_contact_kind("هر دو"), Some("both"));
        assert_eq!(normalize_contact_kind("unknown"), None);
    }

    #[test]
    fn version_one_database_migrates_purchase_return_tables_without_data_loss() {
        let connection=Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection.execute("INSERT INTO products(name,code,quantity) VALUES('موجودی قدیمی','OLD-1',7)",[]).unwrap();
        connection.pragma_update(None,"user_version",1).unwrap();
        connection.execute_batch("DROP TABLE purchase_refunds; DROP TABLE purchase_return_items; DROP TABLE purchase_returns;").unwrap();
        initialize(&connection).unwrap();
        let version:i64=connection.query_row("PRAGMA user_version",[],|r|r.get(0)).unwrap();
        let stock:i64=connection.query_row("SELECT quantity FROM products WHERE code='OLD-1'",[],|r|r.get(0)).unwrap();
        let tables:i64=connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('purchase_returns','purchase_return_items','purchase_refunds')",[],|r|r.get(0)).unwrap();
        assert_eq!(version,3);
        assert_eq!(stock,7);
        assert_eq!(tables,3);
        let audit_table:i64=connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='inventory_adjustments'",[],|r|r.get(0)).unwrap();
        assert_eq!(audit_table,1);
    }

    #[test]
    fn version_two_database_migrates_inventory_adjustment_history() {
        let connection=Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection.execute("INSERT INTO products(name,code,quantity) VALUES('کالای موجود','STOCK-2',4)",[]).unwrap();
        connection.execute_batch("DROP TABLE inventory_adjustments; DROP INDEX IF EXISTS idx_inventory_adjustments_product_date;").unwrap();
        connection.pragma_update(None,"user_version",2).unwrap();
        initialize(&connection).unwrap();
        let version:i64=connection.query_row("PRAGMA user_version",[],|r|r.get(0)).unwrap();
        let count:i64=connection.query_row("SELECT COUNT(*) FROM inventory_adjustments",[],|r|r.get(0)).unwrap();
        let stock:i64=connection.query_row("SELECT quantity FROM products WHERE code='STOCK-2'",[],|r|r.get(0)).unwrap();
        assert_eq!((version,count,stock),(3,0,4));
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
    fn full_database_backup_and_restore_preserve_every_accounting_table() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nabz-backup-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let backup_path = root.join("complete.sqlite3");

        let source = Connection::open_in_memory().unwrap();
        initialize(&source).unwrap();
        source.execute("INSERT INTO contacts(name,kind) VALUES('Backup customer','customer')", []).unwrap();
        source.execute("INSERT INTO products(name,code) VALUES('Backup item','BACKUP-1')", []).unwrap();
        source.execute("INSERT INTO invoices(number,kind,contact_name,subtotal,total) VALUES(1001,'sale','Backup customer',50,50)", []).unwrap();
        source.execute("INSERT INTO invoice_items(invoice_id,item_name,quantity,unit_price,line_total) VALUES(1,'Backup item',1,50,50)", []).unwrap();
        source.execute("INSERT INTO sales_returns(invoice_id,amount) VALUES(1,10)", []).unwrap();
        source.execute("INSERT INTO sales_return_items(return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount) VALUES(1,1,1,'Backup item',1,10,10)", []).unwrap();
        source.execute("INSERT INTO sales_refunds(invoice_id,return_id,amount) VALUES(1,1,10)", []).unwrap();
        source.execute("INSERT INTO payments(kind,contact_name,amount) VALUES('receipt','Backup customer',40)", []).unwrap();
        source.execute("INSERT INTO settings(key,value) VALUES('store_name','Backup shop')", []).unwrap();

        backup_database_to(&source, &backup_path).unwrap();

        let mut target = Connection::open_in_memory().unwrap();
        initialize(&target).unwrap();
        target.execute("INSERT INTO contacts(name,kind) VALUES('Old data','customer')", []).unwrap();
        restore_database_from(&mut target, &backup_path).unwrap();

        let contacts: i64 = target.query_row("SELECT COUNT(*) FROM contacts", [], |r| r.get(0)).unwrap();
        let returns: i64 = target.query_row("SELECT COUNT(*) FROM sales_returns", [], |r| r.get(0)).unwrap();
        let return_items: i64 = target.query_row("SELECT COUNT(*) FROM sales_return_items", [], |r| r.get(0)).unwrap();
        let refunds: i64 = target.query_row("SELECT COUNT(*) FROM sales_refunds", [], |r| r.get(0)).unwrap();
        let settings: String = target.query_row("SELECT value FROM settings WHERE key='store_name'", [], |r| r.get(0)).unwrap();
        assert_eq!(contacts, 1);
        assert_eq!(returns, 1);
        assert_eq!(return_items, 1);
        assert_eq!(refunds, 1);
        assert_eq!(settings, "Backup shop");

        drop(target);
        drop(source);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_rejects_unrelated_sqlite_file_without_changing_live_data() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nabz-invalid-restore-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let invalid_path = root.join("not-nabz.sqlite3");
        let invalid = Connection::open(&invalid_path).unwrap();
        invalid.execute("CREATE TABLE unrelated(value TEXT)", []).unwrap();
        drop(invalid);

        let mut live = Connection::open_in_memory().unwrap();
        initialize(&live).unwrap();
        live.execute("INSERT INTO contacts(name,kind) VALUES('Keep me','customer')", []).unwrap();

        let error = restore_database_from(&mut live, &invalid_path).unwrap_err();
        let name: String = live.query_row("SELECT name FROM contacts LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(error.contains("پایگاه دادهٔ معتبر نبض نیست"));
        assert_eq!(name, "Keep me");

        drop(live);
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
