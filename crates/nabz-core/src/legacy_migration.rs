//! One-time, non-destructive import from the original egui shop database.
//!
//! The legacy file is opened read-only and never renamed, rewritten, or deleted.
//! Import is allowed only into an empty Tauri database; existing Tauri data is
//! never merged automatically.

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::backup_database_to;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LegacyImportSummary {
    pub products: usize,
    pub contacts: usize,
    pub sales: usize,
    pub purchases: usize,
    pub sale_items: usize,
    pub purchase_items: usize,
    pub expenses: usize,
    pub stock_adjustments: usize,
    pub warnings: usize,
}

#[derive(Clone)]
struct LegacyProduct {
    id: i64,
    name: String,
    sku: String,
    barcode: String,
    purchase_price: i64,
    sale_price: i64,
    stock: i64,
}

#[derive(Clone)]
struct LegacyParty {
    id: i64,
    name: String,
    kind: String,
    phone: String,
    balance: i64,
}

struct LegacyInvoice {
    id: i64,
    date: String,
    total: i64,
    paid: i64,
    party: String,
    note: String,
}

struct LegacyLine {
    id: i64,
    product_id: Option<i64>,
    name: String,
    quantity: i64,
    unit_price: i64,
    line_total: i64,
    unit_cost: Option<i64>,
}

struct LegacyMovement {
    id: i64,
    product_id: i64,
    date: String,
    kind: String,
    quantity: i64,
    reference: String,
    note: String,
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get::<_, i64>(0),
    ).map(|value| value != 0).map_err(|e| e.to_string())
}

fn target_is_empty(connection: &Connection) -> Result<bool, String> {
    let count: i64 = connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM contacts) +
           (SELECT COUNT(*) FROM products) +
           (SELECT COUNT(*) FROM invoices) +
           (SELECT COUNT(*) FROM payments) +
           (SELECT COUNT(*) FROM expenses) +
           (SELECT COUNT(*) FROM settings)",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    Ok(count == 0)
}

fn legacy_lines(
    source: &Connection,
    table: &str,
    parent_column: &str,
    parent_id: i64,
    costs_table_exists: bool,
) -> Result<Vec<LegacyLine>, String> {
    // Table and column identifiers are selected only from this private fixed
    // call-site list; values remain bound parameters.
    let sql = match (table, parent_column, costs_table_exists) {
        ("sale_items", "sale_id", true) =>
            "SELECT si.id,si.product_id,si.product_name,si.quantity,si.unit_price,si.line_total,sc.unit_cost
             FROM sale_items si LEFT JOIN sale_item_costs sc ON sc.sale_item_id=si.id
             WHERE si.sale_id=?1 ORDER BY si.id",
        ("sale_items", "sale_id", false) =>
            "SELECT id,product_id,product_name,quantity,unit_price,line_total,NULL
             FROM sale_items WHERE sale_id=?1 ORDER BY id",
        ("purchase_items", "purchase_id", _) =>
            "SELECT id,product_id,product_name,quantity,unit_cost,line_total,unit_cost
             FROM purchase_items WHERE purchase_id=?1 ORDER BY id",
        _ => return Err("ساختار ردیف‌های دیتابیس قدیمی پشتیبانی نمی‌شود".into()),
    };
    let mut statement = source.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement.query_map([parent_id], |row| Ok(LegacyLine {
        id: row.get(0)?,
        product_id: row.get(1)?,
        name: row.get(2)?,
        quantity: row.get(3)?,
        unit_price: row.get(4)?,
        line_total: row.get(5)?,
        unit_cost: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn load_movements(source: &Connection) -> Result<Vec<LegacyMovement>, String> {
    if !table_exists(source, "stock_movements")? {
        return Ok(Vec::new());
    }
    let mut statement = source.prepare(
        "SELECT id,product_id,date,movement_type,quantity,reference,note
         FROM stock_movements ORDER BY date,id"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok(LegacyMovement {
        id: row.get(0)?,
        product_id: row.get(1)?,
        date: row.get(2)?,
        kind: row.get(3)?,
        quantity: row.get(4)?,
        reference: row.get(5)?,
        note: row.get(6)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn import_stock_adjustments(
    tx: &rusqlite::Transaction<'_>,
    products: &[LegacyProduct],
    movements: &[LegacyMovement],
    summary: &mut LegacyImportSummary,
) -> Result<(), String> {
    let final_stock = products.iter().map(|product| (product.id, product.stock))
        .collect::<HashMap<_, _>>();
    let mut grouped = HashMap::<i64, Vec<LegacyMovement>>::new();
    for movement in movements {
        grouped.entry(movement.product_id).or_default().push(movement.clone());
    }

    for (product_id, mut rows) in grouped {
        let Some(stock) = final_stock.get(&product_id).copied() else {
            summary.warnings += rows.len();
            continue;
        };
        rows.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        let movement_sum = rows.iter().try_fold(0_i64, |sum, row| {
            sum.checked_add(row.quantity).ok_or("جمع گردش قدیمی انبار بیش از ظرفیت عددی است")
        })?;
        let Some(mut running) = stock.checked_sub(movement_sum) else {
            summary.warnings += rows.len();
            continue;
        };
        if running < 0 {
            summary.warnings += rows.len();
            continue;
        }

        let mut pending = Vec::new();
        let mut valid = true;
        for row in rows {
            let Some(next) = running.checked_add(row.quantity) else {
                valid = false;
                break;
            };
            if next < 0 {
                valid = false;
                break;
            }
            let kind = row.kind.trim().to_ascii_lowercase();
            if kind != "sale" && kind != "purchase" && row.quantity != 0 {
                let reason = [row.kind.as_str(), row.reference.as_str(), row.note.as_str()]
                    .into_iter().filter(|part| !part.trim().is_empty())
                    .collect::<Vec<_>>().join(" · ");
                pending.push((running, next, row.quantity, reason, row.date));
            }
            running = next;
        }
        if !valid || running != stock {
            summary.warnings += pending.len().max(1);
            continue;
        }

        let product_name: String = tx.query_row(
            "SELECT name FROM products WHERE id=?1",
            [product_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        for (previous, next, delta, reason, date) in pending {
            tx.execute(
                "INSERT INTO inventory_adjustments(
                   product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at
                 ) VALUES(?1,?2,?3,?4,?5,?6,COALESCE(NULLIF(?7,''),datetime('now','localtime')))",
                params![product_id, product_name, previous, next, delta,
                    if reason.trim().is_empty() { "مهاجرت گردش قدیمی انبار" } else { &reason }, date],
            ).map_err(|e| e.to_string())?;
            summary.stock_adjustments += 1;
        }
    }
    Ok(())
}

/// Import legacy egui business data into an empty Tauri database.
///
/// Returns None when the source has no supported shop schema, has already been
/// imported, or the destination contains user data. No merge is ever attempted.
pub fn import_legacy_egui_database(
    target: &mut Connection,
    legacy_path: impl AsRef<Path>,
    target_path: impl AsRef<Path>,
) -> Result<Option<LegacyImportSummary>, String> {
    let legacy_path = legacy_path.as_ref();
    let target_path = target_path.as_ref();
    if !legacy_path.is_file() {
        return Ok(None);
    }
    if legacy_path == target_path {
        return Err("مسیر دیتابیس قدیمی و مقصد یکسان است؛ مهاجرت متوقف شد".into());
    }

    let already_imported: Option<String> = target.query_row(
        "SELECT value FROM settings WHERE key='legacy_egui_import_v1'",
        [],
        |row| row.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if already_imported.is_some() {
        return Ok(None);
    }
    if !target_is_empty(target)? {
        return Ok(None);
    }

    let source = Connection::open_with_flags(
        legacy_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).map_err(|e| format!("بازکردن دیتابیس قدیمی فقط‌خواندنی ناموفق بود: {e}"))?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("بررسی سلامت دیتابیس قدیمی ناموفق بود: {e}"))?;
    if integrity != "ok" {
        return Err(format!("دیتابیس قدیمی سالم نیست و مهاجرت متوقف شد: {integrity}"));
    }
    if !table_exists(&source, "products")? {
        return Ok(None);
    }

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let backup_dir = target_path.parent().unwrap_or_else(|| Path::new(".")).join("backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("ساخت پوشهٔ پشتیبان مهاجرت ناموفق بود: {e}"))?;
    let backup_path = backup_dir.join(format!("legacy-egui-{stamp}.sqlite3"));
    backup_database_to(&source, &backup_path)
        .map_err(|e| format!("مهاجرت متوقف شد؛ پشتیبان دیتابیس قدیمی ساخته نشد: {e}"))?;

    let mut products = Vec::new();
    {
        let mut statement = source.prepare(
            "SELECT id,name,sku,barcode,purchase_price,sale_price,stock FROM products ORDER BY id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok(LegacyProduct {
            id: row.get(0)?, name: row.get(1)?, sku: row.get(2)?, barcode: row.get(3)?,
            purchase_price: row.get(4)?, sale_price: row.get(5)?, stock: row.get(6)?,
        })).map_err(|e| e.to_string())?;
        products = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }

    let mut parties = Vec::new();
    if table_exists(&source, "parties")? {
        let mut statement = source.prepare(
            "SELECT id,name,kind,phone,balance FROM parties ORDER BY id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok(LegacyParty {
            id: row.get(0)?, name: row.get(1)?, kind: row.get(2)?, phone: row.get(3)?, balance: row.get(4)?,
        })).map_err(|e| e.to_string())?;
        parties = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }

    let mut sales = Vec::new();
    if table_exists(&source, "sales")? {
        let mut statement = source.prepare("SELECT id,date,total FROM sales ORDER BY date,id")
            .map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok(LegacyInvoice {
            id: row.get(0)?, date: row.get(1)?, total: row.get(2)?, paid: 0,
            party: String::new(), note: String::new(),
        })).map_err(|e| e.to_string())?;
        sales = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }

    let mut purchases = Vec::new();
    if table_exists(&source, "purchases")? {
        let mut statement = source.prepare(
            "SELECT id,date,supplier_name,total,paid,note FROM purchases ORDER BY date,id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok(LegacyInvoice {
            id: row.get(0)?, date: row.get(1)?, party: row.get(2)?, total: row.get(3)?,
            paid: row.get(4)?, note: row.get(5)?,
        })).map_err(|e| e.to_string())?;
        purchases = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }

    let movements = load_movements(&source)?;
    let has_sale_costs = table_exists(&source, "sale_item_costs")?;
    let has_sale_items = table_exists(&source, "sale_items")?;
    let has_purchase_items = table_exists(&source, "purchase_items")?;
    let mut summary = LegacyImportSummary::default();

    let tx = target.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if !target_is_empty(&tx)? {
        return Err("دیتابیس مقصد هنگام شروع مهاجرت خالی نبود؛ هیچ داده‌ای ادغام نشد".into());
    }

    for product in &products {
        if product.id <= 0 || product.name.trim().is_empty() || product.sku.trim().is_empty()
            || product.stock < 0 || product.purchase_price < 0 || product.sale_price < 0 {
            return Err(format!("اطلاعات کالای قدیمی با شناسهٔ {} معتبر نیست؛ مهاجرت متوقف شد", product.id));
        }
        tx.execute(
            "INSERT INTO products(id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost)
             VALUES(?1,?2,?3,?4,?5,?6,?7,0,CASE WHEN ?7>0 THEN ?5 ELSE 0 END)",
            params![product.id, product.name, product.sku, product.barcode,
                product.purchase_price, product.sale_price, product.stock],
        ).map_err(|e| format!("انتقال کالای «{}» ناموفق بود: {e}", product.name))?;
        summary.products += 1;
    }

    let mut contact_ids = HashMap::<String, i64>::new();
    for party in &parties {
        let kind = crate::normalize_contact_kind(&party.kind)
            .ok_or_else(|| format!("نوع طرف‌حساب قدیمی «{}» شناخته‌شده نیست", party.kind))?;
        let (opening, notes) = if party.balance >= 0 && kind != "both" {
            (party.balance, String::new())
        } else {
            (0, format!("ماندهٔ نسخهٔ قدیمی که جهت آن نیازمند تطبیق دستی است: {}", party.balance))
        };
        tx.execute(
            "INSERT INTO contacts(id,name,kind,phone,opening_balance,notes)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![party.id, party.name, kind, party.phone, opening, notes],
        ).map_err(|e| format!("انتقال طرف‌حساب «{}» ناموفق بود: {e}", party.name))?;
        contact_ids.insert(party.name.to_lowercase(), party.id);
        summary.contacts += 1;
    }

    let mut next_number = 1001_i64;
    for sale in &sales {
        if sale.total < 0 {
            return Err(format!("مبلغ فروش قدیمی #{} نامعتبر است", sale.id));
        }
        let lines = if has_sale_items {
            legacy_lines(&source, "sale_items", "sale_id", sale.id, has_sale_costs)?
        } else {
            Vec::new()
        };
        let line_sum = lines.iter().try_fold(0_i64, |sum, line| {
            sum.checked_add(line.line_total).ok_or("جمع ردیف‌های فروش قدیمی بیش از ظرفیت عددی است")
        })?;
        let subtotal = line_sum.max(sale.total);
        let discount = subtotal - sale.total;
        tx.execute(
            "INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,credit_applied)
             VALUES(?1,'sale',NULL,'مشتری گذری',COALESCE(NULLIF(?2,''),date('now','localtime')),?3,?4,?5,?5,?6,0)",
            params![next_number, sale.date, subtotal, discount, sale.total,
                format!("مهاجرت از فروش egui شمارهٔ {}", sale.id)],
        ).map_err(|e| e.to_string())?;
        let invoice_id = tx.last_insert_rowid();
        let mut imported_line_sum = 0_i64;
        for line in &lines {
            tx.execute(
                "INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![invoice_id, line.product_id, line.name, line.quantity,
                    line.unit_price, line.line_total, line.unit_cost],
            ).map_err(|e| e.to_string())?;
            imported_line_sum = imported_line_sum.checked_add(line.line_total)
                .ok_or("جمع ردیف‌های فروش قدیمی بیش از ظرفیت عددی است")?;
            summary.sale_items += 1;
        }
        if imported_line_sum < sale.total {
            let difference = sale.total - imported_line_sum;
            tx.execute(
                "INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost)
                 VALUES(?1,NULL,'مبلغ تعدیلی فروش قدیمی',1,?2,?2,NULL)",
                params![invoice_id, difference],
            ).map_err(|e| e.to_string())?;
            summary.sale_items += 1;
            summary.warnings += 1;
        } else if lines.is_empty() && sale.total > 0 {
            summary.warnings += 1;
        }
        if sale.total > 0 {
            tx.execute(
                "INSERT INTO payments(kind,contact_id,contact_name,amount,method,payment_date,notes)
                 VALUES('receipt',NULL,'مشتری گذری',?1,'cash',COALESCE(NULLIF(?2,''),date('now','localtime')),?3)",
                params![sale.total, sale.date, format!("دریافت فرض‌شدهٔ نقدی بابت فروش قدیمی #{}", sale.id)],
            ).map_err(|e| e.to_string())?;
        }
        next_number += 1;
        summary.sales += 1;
    }

    for purchase in &purchases {
        if purchase.total < 0 || purchase.paid < 0 || purchase.paid > purchase.total {
            return Err(format!("مبلغ خرید قدیمی #{} نامعتبر است", purchase.id));
        }
        let supplier = purchase.party.trim();
        if supplier.is_empty() {
            return Err(format!("نام تأمین‌کنندهٔ خرید قدیمی #{} خالی است؛ مهاجرت مالی متوقف شد", purchase.id));
        }
        let contact_id = if let Some(id) = contact_ids.get(&supplier.to_lowercase()).copied() {
            id
        } else {
            tx.execute(
                "INSERT INTO contacts(name,kind,phone,opening_balance,notes)
                 VALUES(?1,'supplier','',0,'ایجادشده هنگام مهاجرت فاکتور خرید قدیمی')",
                [supplier],
            ).map_err(|e| e.to_string())?;
            let id = tx.last_insert_rowid();
            contact_ids.insert(supplier.to_lowercase(), id);
            id
        };
        let lines = if has_purchase_items {
            legacy_lines(&source, "purchase_items", "purchase_id", purchase.id, false)?
        } else {
            Vec::new()
        };
        let line_sum = lines.iter().try_fold(0_i64, |sum, line| {
            sum.checked_add(line.line_total).ok_or("جمع ردیف‌های خرید قدیمی بیش از ظرفیت عددی است")
        })?;
        let subtotal = line_sum.max(purchase.total);
        let discount = subtotal - purchase.total;
        tx.execute(
            "INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,credit_applied)
             VALUES(?1,'purchase',?2,?3,COALESCE(NULLIF(?4,''),date('now','localtime')),?5,?6,?7,?8,?9,0)",
            params![next_number, contact_id, supplier, purchase.date, subtotal, discount,
                purchase.total, purchase.paid,
                format!("مهاجرت از خرید egui شمارهٔ {}: {}", purchase.id, purchase.note)],
        ).map_err(|e| e.to_string())?;
        let invoice_id = tx.last_insert_rowid();
        let mut imported_line_sum = 0_i64;
        for line in &lines {
            tx.execute(
                "INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![invoice_id, line.product_id, line.name, line.quantity,
                    line.unit_price, line.line_total, line.unit_cost],
            ).map_err(|e| e.to_string())?;
            imported_line_sum = imported_line_sum.checked_add(line.line_total)
                .ok_or("جمع ردیف‌های خرید قدیمی بیش از ظرفیت عددی است")?;
            summary.purchase_items += 1;
        }
        if imported_line_sum < purchase.total {
            let difference = purchase.total - imported_line_sum;
            tx.execute(
                "INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost)
                 VALUES(?1,NULL,'مبلغ تعدیلی خرید قدیمی',1,?2,?2,NULL)",
                params![invoice_id, difference],
            ).map_err(|e| e.to_string())?;
            summary.purchase_items += 1;
            summary.warnings += 1;
        }
        if purchase.paid > 0 {
            tx.execute(
                "INSERT INTO payments(kind,contact_id,contact_name,amount,method,payment_date,notes)
                 VALUES('payment',?1,?2,?3,'cash',COALESCE(NULLIF(?4,''),date('now','localtime')),?5)",
                params![contact_id, supplier, purchase.paid, purchase.date,
                    format!("پرداخت ثبت‌شده در خرید قدیمی #{}", purchase.id)],
            ).map_err(|e| e.to_string())?;
        }
        next_number += 1;
        summary.purchases += 1;
    }

    if table_exists(&source, "business_expenses")? {
        let mut statement = source.prepare(
            "SELECT date,category,amount,paid_from,note FROM business_expenses ORDER BY date,id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?, row.get::<_, String>(4)?,
        ))).map_err(|e| e.to_string())?;
        let expenses = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        for (date, category, amount, paid_from, note) in expenses {
            if amount <= 0 || category.trim().is_empty() {
                summary.warnings += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO expenses(category,description,amount,expense_date)
                 VALUES(?1,?2,?3,COALESCE(NULLIF(?4,''),date('now','localtime')))",
                params![category, format!("{} · پرداخت از: {}", note, paid_from), amount, date],
            ).map_err(|e| e.to_string())?;
            summary.expenses += 1;
        }
    }

    import_stock_adjustments(&tx, &products, &movements, &mut summary)?;

    let marker = format!(
        "source=legacy-egui;products={};contacts={};sales={};purchases={};sale_items={};purchase_items={};expenses={};stock_adjustments={};warnings={};backup={}",
        summary.products, summary.contacts, summary.sales, summary.purchases,
        summary.sale_items, summary.purchase_items, summary.expenses,
        summary.stock_adjustments, summary.warnings, backup_path.display()
    );
    tx.execute(
        "INSERT INTO settings(key,value) VALUES('legacy_egui_import_v1',?1)",
        [marker],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::initialize;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("nabz-{label}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn legacy_shop_database_import_is_complete_non_destructive_and_idempotent() {
        let root = temp_dir("legacy-import");
        std::fs::create_dir_all(&root).unwrap();
        let legacy_path = root.join("legacy.sqlite3");
        let target_path = root.join("target").join("nabz.sqlite3");
        let source = Connection::open(&legacy_path).unwrap();
        source.execute_batch(
            "CREATE TABLE products(id INTEGER PRIMARY KEY,name TEXT,sku TEXT,barcode TEXT,purchase_price INTEGER,sale_price INTEGER,stock INTEGER);
             CREATE TABLE parties(id INTEGER PRIMARY KEY,name TEXT,kind TEXT,phone TEXT,balance INTEGER);
             CREATE TABLE sales(id INTEGER PRIMARY KEY,date TEXT,total INTEGER);
             CREATE TABLE sale_items(id INTEGER PRIMARY KEY,sale_id INTEGER,product_id INTEGER,product_name TEXT,unit_price INTEGER,quantity INTEGER,line_total INTEGER);
             CREATE TABLE sale_item_costs(sale_item_id INTEGER,unit_cost INTEGER,total_cost INTEGER);
             CREATE TABLE purchases(id INTEGER PRIMARY KEY,date TEXT,supplier_name TEXT,total INTEGER,paid INTEGER,note TEXT);
             CREATE TABLE purchase_items(id INTEGER PRIMARY KEY,purchase_id INTEGER,product_id INTEGER,product_name TEXT,unit_cost INTEGER,quantity INTEGER,line_total INTEGER);
             CREATE TABLE business_expenses(id INTEGER PRIMARY KEY,date TEXT,category TEXT,amount INTEGER,paid_from TEXT,note TEXT);
             CREATE TABLE stock_movements(id INTEGER PRIMARY KEY,product_id INTEGER,date TEXT,movement_type TEXT,quantity INTEGER,reference TEXT,note TEXT);
             CREATE TABLE transactions(id INTEGER PRIMARY KEY,amount INTEGER);"
        ).unwrap();
        source.execute("INSERT INTO products VALUES(7,'کالای قدیمی','SKU-7','123456',12,25,6)", []).unwrap();
        source.execute("INSERT INTO parties VALUES(3,'تأمین‌کننده قدیمی','supplier','0912',5)", []).unwrap();
        source.execute("INSERT INTO sales VALUES(2,'2026-10-01',60)", []).unwrap();
        source.execute("INSERT INTO sale_items VALUES(11,2,7,'کالای قدیمی',30,2,60)", []).unwrap();
        source.execute("INSERT INTO sale_item_costs VALUES(11,12,24)", []).unwrap();
        source.execute("INSERT INTO purchases VALUES(4,'2026-10-02','تأمین‌کننده قدیمی',30,10,'خرید آزمایشی')", []).unwrap();
        source.execute("INSERT INTO purchase_items VALUES(21,4,7,'کالای قدیمی',15,2,30)", []).unwrap();
        source.execute("INSERT INTO business_expenses VALUES(1,'2026-10-03','حمل',20,'cash','هزینه آزمایشی')", []).unwrap();
        source.execute("INSERT INTO stock_movements VALUES(1,7,'2026-09-01','opening',5,'افتتاحیه','موجودی اول دوره')", []).unwrap();
        source.execute("INSERT INTO stock_movements VALUES(2,7,'2026-10-01','sale',-2,'فروش #2','')", []).unwrap();
        source.execute("INSERT INTO stock_movements VALUES(3,7,'2026-10-02','purchase',2,'خرید #4','')", []).unwrap();
        source.execute("INSERT INTO transactions VALUES(1,999)", []).unwrap();
        drop(source);

        let mut target = Connection::open_in_memory().unwrap();
        initialize(&target).unwrap();
        let summary = import_legacy_egui_database(&mut target, &legacy_path, &target_path).unwrap().unwrap();
        assert_eq!((summary.products, summary.contacts, summary.sales, summary.purchases, summary.expenses), (1,1,1,1,1));
        assert_eq!(target.query_row("SELECT quantity*average_cost FROM products WHERE id=7", [], |r| r.get::<_,i64>(0)).unwrap(), 72);
        assert_eq!(target.query_row("SELECT unit_cost FROM invoice_items WHERE item_name='کالای قدیمی' AND invoice_id=(SELECT id FROM invoices WHERE kind='sale')", [], |r| r.get::<_,i64>(0)).unwrap(), 12);
        assert_eq!(target.query_row("SELECT paid FROM invoices WHERE kind='sale'", [], |r| r.get::<_,i64>(0)).unwrap(), 60);
        assert_eq!(target.query_row("SELECT paid FROM invoices WHERE kind='purchase'", [], |r| r.get::<_,i64>(0)).unwrap(), 10);
        assert_eq!(target.query_row("SELECT opening_balance FROM contacts WHERE id=3", [], |r| r.get::<_,i64>(0)).unwrap(), 5);
        assert_eq!(target.query_row("SELECT COUNT(*) FROM inventory_adjustments", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
        assert_eq!(target.query_row("SELECT COUNT(*) FROM payments", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
        assert!(target.query_row("SELECT value FROM settings WHERE key='legacy_egui_import_v1'", [], |r| r.get::<_,String>(0)).is_ok());
        assert!(root.join("target/backups").read_dir().is_ok());
        let again = import_legacy_egui_database(&mut target, &legacy_path, &target_path).unwrap();
        assert!(again.is_none());

        let source = Connection::open_with_flags(&legacy_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(source.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
        assert_eq!(source.query_row("SELECT stock FROM products WHERE id=7", [], |r| r.get::<_,i64>(0)).unwrap(), 6);
        drop(source);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_import_never_merges_into_a_nonempty_tauri_database() {
        let root = temp_dir("legacy-no-merge");
        std::fs::create_dir_all(&root).unwrap();
        let legacy_path = root.join("legacy.sqlite3");
        let source = Connection::open(&legacy_path).unwrap();
        source.execute_batch("CREATE TABLE products(id INTEGER PRIMARY KEY,name TEXT,sku TEXT,barcode TEXT,purchase_price INTEGER,sale_price INTEGER,stock INTEGER);").unwrap();
        source.execute("INSERT INTO products VALUES(1,'Old','OLD-1','',10,20,1)", []).unwrap();
        drop(source);

        let mut target = Connection::open_in_memory().unwrap();
        initialize(&target).unwrap();
        target.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,average_cost) VALUES('New','NEW-1',20,30,2,20)", []).unwrap();
        let result = import_legacy_egui_database(&mut target, &legacy_path, root.join("target.sqlite3")).unwrap();
        assert!(result.is_none());
        assert_eq!(target.query_row("SELECT COUNT(*) FROM products", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
