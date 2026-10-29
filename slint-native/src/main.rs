use rusqlite::Connection;
use slint::{ComponentHandle, SharedString};
use std::{env, fs, path::PathBuf};

slint::include_modules!();

fn database_path() -> PathBuf {
    if let Some(path) = env::var_os("NABZ_DB_PATH") {
        return PathBuf::from(path);
    }

    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));

    data_home
        .join("com.herotux.nabz.webprototype")
        .join("nabz.sqlite3")
}

fn open_database() -> Result<Connection, String> {
    let path = database_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("ساخت پوشهٔ داده ناموفق بود: {e}"))?;
    }

    let connection = Connection::open(&path)
        .map_err(|e| format!("بازکردن پایگاه داده ناموفق بود ({}): {e}", path.display()))?;

    connection.execute_batch(
        "PRAGMA foreign_keys=ON;
         PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS contacts(
             id INTEGER PRIMARY KEY, name TEXT NOT NULL,
             kind TEXT NOT NULL DEFAULT 'customer', phone TEXT NOT NULL DEFAULT '',
             opening_balance INTEGER NOT NULL DEFAULT 0, notes TEXT NOT NULL DEFAULT '',
             created_at TEXT NOT NULL DEFAULT(datetime('now','localtime'))
         );
         CREATE TABLE IF NOT EXISTS products(
             id INTEGER PRIMARY KEY, name TEXT NOT NULL, code TEXT NOT NULL UNIQUE,
             barcode TEXT NOT NULL DEFAULT '', buy_price INTEGER NOT NULL DEFAULT 0,
             sell_price INTEGER NOT NULL DEFAULT 0, quantity INTEGER NOT NULL DEFAULT 0,
             min_quantity INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS invoices(
             id INTEGER PRIMARY KEY, number INTEGER NOT NULL UNIQUE, kind TEXT NOT NULL,
             contact_id INTEGER, contact_name TEXT NOT NULL,
             invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),
             subtotal INTEGER NOT NULL, discount INTEGER NOT NULL DEFAULT 0,
             total INTEGER NOT NULL, paid INTEGER NOT NULL DEFAULT 0, notes TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS invoice_items(
             id INTEGER PRIMARY KEY, invoice_id INTEGER NOT NULL REFERENCES invoices(id),
             product_id INTEGER, item_name TEXT NOT NULL, quantity INTEGER NOT NULL,
             unit_price INTEGER NOT NULL, line_total INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS payments(
             id INTEGER PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN('receipt','payment')),
             contact_id INTEGER, contact_name TEXT NOT NULL, amount INTEGER NOT NULL CHECK(amount>0),
             method TEXT NOT NULL DEFAULT 'cash',
             payment_date TEXT NOT NULL DEFAULT(date('now','localtime')), notes TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
    .map_err(|e| format!("آماده‌سازی پایگاه داده ناموفق بود: {e}"))?;

    Ok(connection)
}

fn rial(value: i64) -> String {
    let mut digits = value.to_string();
    let negative = digits.starts_with('-');
    if negative {
        digits.remove(0);
    }
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push('٬');
        }
        grouped.push(ch);
    }
    let grouped: String = grouped.chars().rev().collect();
    let grouped = if negative { format!("-{grouped}") } else { grouped };
    let persian_digits: String = grouped
        .chars()
        .map(|ch| match ch {
            '0' => '۰', '1' => '۱', '2' => '۲', '3' => '۳', '4' => '۴',
            '5' => '۵', '6' => '۶', '7' => '۷', '8' => '۸', '9' => '۹',
            _ => ch,
        })
        .collect();
    format!("{persian_digits} تومان")
}

fn update_summary(ui: &MainWindow, db: &Connection) -> Result<(), String> {
    let sales: i64 = db.query_row(
        "SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let receipts: i64 = db.query_row(
        "SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let balance: i64 = db.query_row(
        "SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale'),0)
         + COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    let inventory: i64 = db.query_row(
        "SELECT COALESCE(SUM(quantity*buy_price),0) FROM products",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    ui.set_sales_today(SharedString::from(rial(sales)));
    ui.set_receipts_today(SharedString::from(rial(receipts)));
    ui.set_customer_balance(SharedString::from(rial(balance)));
    ui.set_inventory_value(SharedString::from(rial(inventory)));
    ui.set_status_message(SharedString::from("داده‌ها از SQLite خوانده شدند"));
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    let database = match open_database() {
        Ok(db) => Some(db),
        Err(error) => {
            ui.set_status_message(SharedString::from(error));
            None
        }
    };

    if let Some(db) = database.as_ref() {
        if let Err(error) = update_summary(&ui, db) {
            ui.set_status_message(SharedString::from(format!("خطای خواندن خلاصه: {error}")));
        }
    }

    let weak_ui = ui.as_weak();
    ui.on_refresh_requested(move || {
        if let Some(ui) = weak_ui.upgrade() {
            match open_database().and_then(|db| update_summary(&ui, &db)) {
                Ok(()) => {}
                Err(error) => ui.set_status_message(SharedString::from(error)),
            }
        }
    });

    ui.run()?;
    Ok(())
}
