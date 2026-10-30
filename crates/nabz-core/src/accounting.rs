//! Shared accounting write operations used by the Tauri and Slint frontends.
//! Keep validation and persistence rules here; UI layers only parse user input.

use rusqlite::{params, Connection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactInput {
    pub name: String,
    pub kind: String,
    pub phone: String,
    pub opening_balance: i64,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductInput {
    pub name: String,
    pub code: String,
    pub barcode: String,
    pub buy_price: i64,
    pub sell_price: i64,
    pub quantity: i64,
    pub min_quantity: i64,
}

fn validate_contact(input: &ContactInput) -> Result<(), String> {
    if input.name.trim().is_empty() {
        return Err("نام طرف‌حساب الزامی است".into());
    }
    if !["customer", "supplier", "both"].contains(&input.kind.as_str()) {
        return Err("نوع طرف‌حساب معتبر نیست".into());
    }
    if input.kind == "both" && input.opening_balance != 0 {
        return Err("برای طرف‌حساب دوگانه، ماندهٔ اولیه باید صفر باشد".into());
    }
    Ok(())
}

pub fn create_contact(connection: &Connection, input: &ContactInput) -> Result<i64, String> {
    validate_contact(input)?;
    let name = input.name.trim();
    let duplicate: i64 = connection.query_row(
        "SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE",
        [name],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if duplicate > 0 {
        return Err("طرف‌حسابی با این نام قبلاً ثبت شده است".into());
    }
    connection.execute(
        "INSERT INTO contacts(name,kind,phone,opening_balance,notes) VALUES(?1,?2,?3,?4,?5)",
        params![name, input.kind, input.phone.trim(), input.opening_balance, input.notes.trim()],
    ).map_err(|e| e.to_string())?;
    Ok(connection.last_insert_rowid())
}

pub fn update_contact(connection: &Connection, id: i64, name: &str, phone: &str) -> Result<(), String> {
    let name = name.trim();
    if id <= 0 || name.is_empty() {
        return Err("شناسه و نام طرف‌حساب معتبر نیست".into());
    }
    let exists: i64 = connection.query_row(
        "SELECT COUNT(*) FROM contacts WHERE id=?1", [id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err("طرف‌حساب موردنظر پیدا نشد".into());
    }
    let duplicate: i64 = connection.query_row(
        "SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE AND id<>?2",
        params![name, id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    if duplicate > 0 {
        return Err("طرف‌حساب دیگری با این نام وجود دارد".into());
    }
    connection.execute(
        "UPDATE contacts SET name=?1,phone=?2 WHERE id=?3",
        params![name, phone.trim(), id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_product(input: &ProductInput) -> Result<(), String> {
    if input.name.trim().is_empty() || input.code.trim().is_empty() {
        return Err("نام و کد کالا الزامی است".into());
    }
    if input.quantity < 0 || input.buy_price < 0 || input.sell_price < 0 || input.min_quantity < 0 {
        return Err("موجودی، حداقل موجودی و قیمت‌ها نمی‌توانند منفی باشند".into());
    }
    Ok(())
}

pub fn create_product(connection: &Connection, input: &ProductInput) -> Result<i64, String> {
    validate_product(input)?;
    connection.execute(
        "INSERT INTO products(name,code,barcode,buy_price,sell_price,quantity,min_quantity) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![input.name.trim(), input.code.trim(), input.barcode.trim(), input.buy_price, input.sell_price, input.quantity, input.min_quantity],
    ).map_err(|e| if e.to_string().contains("UNIQUE") {
        "کد کالا تکراری است".to_string()
    } else {
        e.to_string()
    })?;
    Ok(connection.last_insert_rowid())
}

pub fn update_product(connection: &Connection, id: i64, input: &ProductInput) -> Result<(), String> {
    if id <= 0 || input.name.trim().is_empty() || input.code.trim().is_empty() {
        return Err("شناسه، نام و کد کالا معتبر نیست".into());
    }
    if input.buy_price < 0 || input.sell_price < 0 || input.min_quantity < 0 {
        return Err("قیمت‌ها و حداقل موجودی نمی‌توانند منفی باشند".into());
    }
    let changed = connection.execute(
        "UPDATE products SET name=?1,code=?2,barcode=?3,buy_price=?4,sell_price=?5,min_quantity=?6 WHERE id=?7",
        params![input.name.trim(), input.code.trim(), input.barcode.trim(), input.buy_price, input.sell_price, input.min_quantity, id],
    ).map_err(|e| if e.to_string().contains("UNIQUE") {
        "کد کالا تکراری است".to_string()
    } else {
        e.to_string()
    })?;
    if changed == 0 {
        return Err("کالای موردنظر پیدا نشد".into());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentInput {
    pub kind: String,
    pub contact_name: String,
    pub amount: i64,
    pub method: String,
    pub date: String,
    pub notes: String,
}

/// Record a receipt/payment and allocate it to opening balance then oldest
/// outstanding invoices in one transaction. Both UIs use this exact policy.
pub fn record_payment(connection: &mut Connection, input: &PaymentInput) -> Result<i64, String> {
    if !["receipt", "payment"].contains(&input.kind.as_str())
        || input.contact_name.trim().is_empty() || input.amount <= 0 {
        return Err("طرف‌حساب و مبلغ مثبت الزامی است".into());
    }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let contact_name = input.contact_name.trim();
    let contact_id: i64 = tx.query_row(
        "SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1",
        [contact_name], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let role: String = tx.query_row(
        "SELECT kind FROM contacts WHERE id=?1", [contact_id], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if input.kind == "receipt" && role == "supplier" {
        return Err("برای دریافت، مشتری انتخاب کنید".into());
    }
    if input.kind == "payment" && role == "customer" {
        return Err("برای پرداخت، تأمین‌کننده انتخاب کنید".into());
    }

    let invoice_kind = if input.kind == "receipt" { "sale" } else { "purchase" };
    let invoice_outstanding: i64 = tx.query_row(
        "SELECT COALESCE(SUM(total-paid),0) FROM invoices WHERE contact_id=?1 AND kind=?2 AND voided=0",
        params![contact_id, invoice_kind], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let opening: i64 = tx.query_row(
        "SELECT opening_balance FROM contacts WHERE id=?1", [contact_id], |row| row.get(0)
    ).map_err(|e| e.to_string())?.max(0);
    let outstanding = invoice_outstanding.checked_add(opening)
        .ok_or("ماندهٔ حساب بیش از حد بزرگ است")?;
    if input.amount > outstanding {
        return Err(format!("مبلغ از ماندهٔ قابل تخصیص بیشتر است: {outstanding}"));
    }

    tx.execute(
        "INSERT INTO payments(kind,contact_id,contact_name,amount,method,payment_date,notes)
         VALUES(?1,?2,?3,?4,COALESCE(NULLIF(?5,''),'cash'),COALESCE(NULLIF(?6,''),date('now','localtime')),?7)",
        params![input.kind, contact_id, contact_name, input.amount, input.method.trim(), input.date.trim(), input.notes.trim()],
    ).map_err(|e| e.to_string())?;
    let payment_id = tx.last_insert_rowid();

    let mut remaining = input.amount;
    let opening_applied = remaining.min(opening);
    if opening_applied > 0 {
        tx.execute("UPDATE contacts SET opening_balance=opening_balance-?1 WHERE id=?2",
            params![opening_applied, contact_id]).map_err(|e| e.to_string())?;
        remaining -= opening_applied;
    }

    let invoices = {
        let mut statement = tx.prepare(
            "SELECT id,total,paid FROM invoices
             WHERE contact_id=?1 AND kind=?2 AND voided=0 AND paid<total
             ORDER BY invoice_date,id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map(params![contact_id, invoice_kind], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };
    for (invoice_id, total, paid) in invoices {
        if remaining == 0 { break; }
        let applied = remaining.min(total - paid);
        if applied > 0 {
            tx.execute("UPDATE invoices SET paid=paid+?1 WHERE id=?2",
                params![applied, invoice_id]).map_err(|e| e.to_string())?;
            remaining -= applied;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(payment_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::initialize;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection
    }

    #[test]
    fn both_frontends_share_contact_validation_and_storage() {
        let connection = database();
        let contact = ContactInput {
            name: "مشتری مشترک".into(), kind: "customer".into(),
            phone: "09120000000".into(), opening_balance: 120, notes: "یادداشت".into(),
        };
        let id = create_contact(&connection, &contact).unwrap();
        assert!(create_contact(&connection, &contact).unwrap_err().contains("قبلاً"));
        update_contact(&connection, id, "مشتری ویرایش‌شده", "09121111111").unwrap();
        let row: (String, i64, String) = connection.query_row(
            "SELECT name,opening_balance,notes FROM contacts WHERE id=?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(row, ("مشتری ویرایش‌شده".into(), 120, "یادداشت".into()));
    }

    #[test]
    fn dual_role_contact_cannot_have_ambiguous_opening_balance() {
        let connection = database();
        let input = ContactInput {
            name: "طرف دوگانه".into(), kind: "both".into(),
            phone: String::new(), opening_balance: 10, notes: String::new(),
        };
        assert!(create_contact(&connection, &input).unwrap_err().contains("باید صفر"));
    }


    #[test]
    fn shared_payment_service_allocates_opening_balance_then_oldest_invoice() {
        let mut connection = database();
        let contact = ContactInput {
            name: "مشتری پرداخت".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 30, notes: String::new(),
        };
        let contact_id = create_contact(&connection, &contact).unwrap();
        connection.execute(
            "INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,total,paid)
             VALUES(1,'sale',?1,'مشتری پرداخت','2026-01-01',100,100,0)",
            [contact_id],
        ).unwrap();
        connection.execute(
            "INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,total,paid)
             VALUES(2,'sale',?1,'مشتری پرداخت','2026-01-02',100,100,0)",
            [contact_id],
        ).unwrap();
        let payment = PaymentInput {
            kind: "receipt".into(), contact_name: "مشتری پرداخت".into(),
            amount: 80, method: "cash".into(), date: "2026-01-03".into(), notes: String::new(),
        };
        record_payment(&mut connection, &payment).unwrap();
        let opening: i64 = connection.query_row(
            "SELECT opening_balance FROM contacts WHERE id=?1", [contact_id], |r| r.get(0)
        ).unwrap();
        let paid: Vec<i64> = {
            let mut s = connection.prepare("SELECT paid FROM invoices ORDER BY id").unwrap();
            s.query_map([], |r| r.get(0)).unwrap().collect::<Result<_,_>>().unwrap()
        };
        assert_eq!(opening, 0);
        assert_eq!(paid, vec![50, 0]);
    }

    #[test]
    fn shared_payment_service_rejects_overpayment_without_partial_mutation() {
        let mut connection = database();
        let contact = ContactInput {
            name: "مشتری بدون بدهی".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 10, notes: String::new(),
        };
        create_contact(&connection, &contact).unwrap();
        let payment = PaymentInput {
            kind: "receipt".into(), contact_name: "مشتری بدون بدهی".into(),
            amount: 11, method: "cash".into(), date: String::new(), notes: String::new(),
        };
        assert!(record_payment(&mut connection, &payment).unwrap_err().contains("بیشتر"));
        let (balance, payments): (i64, i64) = connection.query_row(
            "SELECT opening_balance,(SELECT COUNT(*) FROM payments) FROM contacts WHERE name='مشتری بدون بدهی'",
            [], |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap();
        assert_eq!((balance, payments), (10, 0));
    }

    #[test]
    fn product_edit_preserves_stock_and_rejects_duplicate_codes() {
        let connection = database();
        let input = ProductInput {
            name: "کالای اول".into(), code: "SKU-1".into(), barcode: "123".into(),
            buy_price: 50, sell_price: 80, quantity: 12, min_quantity: 2,
        };
        let id = create_product(&connection, &input).unwrap();
        let second = ProductInput { name: "کالای دوم".into(), code: "SKU-2".into(), ..input.clone() };
        create_product(&connection, &second).unwrap();
        let edited = ProductInput { name: "کالای ویرایش‌شده".into(), code: "SKU-1".into(),
            barcode: "456".into(), buy_price: 60, sell_price: 90, quantity: 0, min_quantity: 3 };
        update_product(&connection, id, &edited).unwrap();
        let stock: i64 = connection.query_row("SELECT quantity FROM products WHERE id=?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(stock, 12);
        let duplicate = ProductInput { code: "SKU-2".into(), ..edited };
        assert!(create_product(&connection, &duplicate).is_err());
    }
}
