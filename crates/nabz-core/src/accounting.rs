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
    let changed = connection.execute(
        "UPDATE contacts SET name=?1,phone=?2 WHERE id=?3",
        params![name, phone.trim(), id],
    ).map_err(|e| e.to_string())?;
    if changed == 0 {
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
