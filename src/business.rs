use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub sku: String,
    pub barcode: String,
    pub purchase_price: i64,
    pub sale_price: i64,
    pub stock: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasketItem {
    pub product_id: i64,
    pub name: String,
    pub unit_price: i64,
    pub quantity: i64,
}

impl BasketItem {
    pub fn line_total(&self) -> i64 {
        self.unit_price.saturating_mul(self.quantity)
    }
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            sku TEXT NOT NULL UNIQUE,
            barcode TEXT NOT NULL UNIQUE,
            purchase_price INTEGER NOT NULL CHECK(purchase_price >= 0),
            sale_price INTEGER NOT NULL CHECK(sale_price >= 0),
            stock INTEGER NOT NULL DEFAULT 0 CHECK(stock >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS sales (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            total INTEGER NOT NULL CHECK(total >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS sale_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
            product_id INTEGER NOT NULL REFERENCES products(id),
            product_name TEXT NOT NULL,
            unit_price INTEGER NOT NULL CHECK(unit_price >= 0),
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            line_total INTEGER NOT NULL CHECK(line_total >= 0)
         );"
    )
}

fn validate_product(name: &str, sku: &str, barcode: &str, purchase: i64, sale: i64, stock: i64) -> Result<(), String> {
    if name.trim().is_empty() { return Err("نام کالا الزامی است.".into()); }
    if sku.trim().is_empty() { return Err("شناسهٔ داخلی کالا الزامی است.".into()); }
    if barcode.trim().is_empty() { return Err("بارکد کالا الزامی است.".into()); }
    if barcode.len() > 128 || !barcode.is_ascii() || barcode.chars().any(char::is_control) {
        return Err("بارکد باید حداکثر ۱۲۸ نویسهٔ ASCII قابل‌چاپ داشته باشد.".into());
    }
    if purchase < 0 || sale < 0 || stock < 0 {
        return Err("قیمت و موجودی نمی‌توانند منفی باشند.".into());
    }
    Ok(())
}

pub fn add_product(conn: &Connection, name: &str, sku: &str, barcode: &str, purchase_price: i64, sale_price: i64, stock: i64) -> Result<i64, String> {
    validate_product(name, sku, barcode, purchase_price, sale_price, stock)?;
    conn.execute(
        "INSERT INTO products(name,sku,barcode,purchase_price,sale_price,stock) VALUES (?1,?2,?3,?4,?5,?6)",
        params![name.trim(), sku.trim(), barcode.trim(), purchase_price, sale_price, stock],
    ).map_err(|e| {
        if e.to_string().contains("UNIQUE constraint failed") {
            "شناسهٔ داخلی یا بارکد قبلاً برای کالای دیگری ثبت شده است.".to_string()
        } else { format!("ثبت کالا ناموفق بود: {e}") }
    })?;
    Ok(conn.last_insert_rowid())
}

pub fn list_products(conn: &Connection) -> rusqlite::Result<Vec<Product>> {
    let mut stmt = conn.prepare(
        "SELECT id,name,sku,barcode,purchase_price,sale_price,stock FROM products ORDER BY name COLLATE NOCASE, id"
    )?;
    let rows = stmt.query_map([], |r| Ok(Product {
        id: r.get(0)?, name: r.get(1)?, sku: r.get(2)?, barcode: r.get(3)?,
        purchase_price: r.get(4)?, sale_price: r.get(5)?, stock: r.get(6)?,
    }))?;
    rows.collect()
}

pub fn find_by_barcode(conn: &Connection, barcode: &str) -> rusqlite::Result<Option<Product>> {
    conn.query_row(
        "SELECT id,name,sku,barcode,purchase_price,sale_price,stock FROM products WHERE barcode=?1",
        [barcode.trim()],
        |r| Ok(Product {
            id: r.get(0)?, name: r.get(1)?, sku: r.get(2)?, barcode: r.get(3)?,
            purchase_price: r.get(4)?, sale_price: r.get(5)?, stock: r.get(6)?,
        })
    ).optional()
}

/// Add one scanned product to the basket. Repeated scans increase quantity.
pub fn scan_into_basket(conn: &Connection, basket: &mut Vec<BasketItem>, barcode: &str) -> Result<(), String> {
    let product = find_by_barcode(conn, barcode)
        .map_err(|e| format!("جست‌وجوی بارکد ناموفق بود: {e}"))?
        .ok_or_else(|| format!("کالایی با بارکد «{}» پیدا نشد.", barcode.trim()))?;
    if product.stock <= 0 {
        return Err(format!("موجودی کالای «{}» تمام شده است.", product.name));
    }
    if let Some(item) = basket.iter_mut().find(|item| item.product_id == product.id) {
        if item.quantity >= product.stock {
            return Err(format!("موجودی کافی برای کالای «{}» وجود ندارد.", product.name));
        }
        item.quantity += 1;
    } else {
        basket.push(BasketItem { product_id: product.id, name: product.name, unit_price: product.sale_price, quantity: 1 });
    }
    Ok(())
}

/// Commit a sale and decrement stock atomically. On any error, the transaction rolls back.
pub fn record_sale(conn: &mut Connection, basket: &[BasketItem], date: &str) -> Result<i64, String> {
    if basket.is_empty() { return Err("سبد فروش خالی است.".into()); }
    if date.trim().is_empty() { return Err("تاریخ فروش الزامی است.".into()); }
    let mut total: i64 = 0;
    for item in basket {
        if item.quantity <= 0 || item.unit_price < 0 {
            return Err("تعداد یا قیمت کالا نامعتبر است.".into());
        }
        total = total.checked_add(item.line_total()).ok_or("مبلغ کل فروش بیش از حد مجاز است.")?;
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| format!("شروع تراکنش فروش ناموفق بود: {e}"))?;
    for item in basket {
        let stock: i64 = tx.query_row("SELECT stock FROM products WHERE id=?1", [item.product_id], |r| r.get(0))
            .map_err(|e| format!("کالا دیگر موجود نیست: {e}"))?;
        if stock < item.quantity {
            return Err(format!("موجودی «{}» کافی نیست؛ موجودی فعلی: {stock}.", item.name));
        }
    }
    tx.execute("INSERT INTO sales(date,total) VALUES (?1,?2)", params![date.trim(), total])
        .map_err(|e| format!("ثبت فاکتور ناموفق بود: {e}"))?;
    let sale_id = tx.last_insert_rowid();
    for item in basket {
        tx.execute("UPDATE products SET stock=stock-?1 WHERE id=?2 AND stock>=?1", params![item.quantity, item.product_id])
            .map_err(|e| format!("کاهش موجودی ناموفق بود: {e}"))?;
        tx.execute(
            "INSERT INTO sale_items(sale_id,product_id,product_name,unit_price,quantity,line_total) VALUES (?1,?2,?3,?4,?5,?6)",
            params![sale_id, item.product_id, item.name, item.unit_price, item.quantity, item.line_total()],
        ).map_err(|e| format!("ثبت ردیف فاکتور ناموفق بود: {e}"))?;
    }
    tx.commit().map_err(|e| format!("نهایی‌سازی فروش ناموفق بود: {e}"))?;
    Ok(sale_id)
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub phone: String,
    pub balance: i64,
}

/// Extend the existing database without deleting or rewriting personal-accounting data.
pub fn init_business_extensions(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS parties (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('customer','supplier','both')),
            phone TEXT NOT NULL DEFAULT '',
            balance INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS purchases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            supplier_name TEXT NOT NULL DEFAULT '',
            total INTEGER NOT NULL CHECK(total >= 0),
            paid INTEGER NOT NULL CHECK(paid >= 0 AND paid <= total),
            note TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS purchase_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            purchase_id INTEGER NOT NULL REFERENCES purchases(id) ON DELETE CASCADE,
            product_id INTEGER NOT NULL REFERENCES products(id),
            product_name TEXT NOT NULL,
            unit_cost INTEGER NOT NULL CHECK(unit_cost >= 0),
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            line_total INTEGER NOT NULL CHECK(line_total >= 0)
         );
         CREATE TABLE IF NOT EXISTS business_expenses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            category TEXT NOT NULL,
            amount INTEGER NOT NULL CHECK(amount > 0),
            paid_from TEXT NOT NULL DEFAULT 'cash',
            note TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS stock_movements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id INTEGER NOT NULL REFERENCES products(id),
            date TEXT NOT NULL,
            movement_type TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            reference TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );"
    )
}

pub fn add_party(conn: &Connection, name: &str, kind: &str, phone: &str) -> Result<i64, String> {
    if name.trim().is_empty() { return Err("نام مشتری یا تأمین‌کننده الزامی است.".into()); }
    if !["customer", "supplier", "both"].contains(&kind) { return Err("نوع طرف حساب نامعتبر است.".into()); }
    conn.execute("INSERT INTO parties(name,kind,phone) VALUES (?1,?2,?3)",
        params![name.trim(), kind, phone.trim()])
        .map_err(|e| format!("ثبت طرف حساب ناموفق بود: {e}"))?;
    Ok(conn.last_insert_rowid())
}

pub fn list_parties(conn: &Connection) -> rusqlite::Result<Vec<Party>> {
    let mut stmt = conn.prepare("SELECT id,name,kind,phone,balance FROM parties ORDER BY name COLLATE NOCASE")?;
    stmt.query_map([], |r| Ok(Party { id:r.get(0)?, name:r.get(1)?, kind:r.get(2)?, phone:r.get(3)?, balance:r.get(4)? }))?.collect()
}

/// Purchase, inventory increase, cost update and supplier balance post atomically.
pub fn record_purchase(conn: &mut Connection, product_id: i64, quantity: i64, unit_cost: i64,
    supplier: &str, date: &str, paid: i64, note: &str) -> Result<i64, String> {
    if quantity <= 0 || unit_cost < 0 || paid < 0 || date.trim().is_empty() {
        return Err("تعداد، مبلغ پرداختی یا تاریخ خرید نامعتبر است.".into());
    }
    let total = unit_cost.checked_mul(quantity).ok_or("مبلغ خرید بیش از حد مجاز است.")?;
    if paid > total { return Err("پرداخت نباید از مبلغ کل خرید بیشتر باشد.".into()); }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
    let name: String = tx.query_row("SELECT name FROM products WHERE id=?1", [product_id], |r| r.get(0))
        .map_err(|_| "کالای انتخاب‌شده پیدا نشد.".to_string())?;
    tx.execute("INSERT INTO purchases(date,supplier_name,total,paid,note) VALUES (?1,?2,?3,?4,?5)",
        params![date.trim(), supplier.trim(), total, paid, note.trim()]).map_err(|e| format!("ثبت فاکتور خرید ناموفق بود: {e}"))?;
    let purchase_id = tx.last_insert_rowid();
    tx.execute("INSERT INTO purchase_items(purchase_id,product_id,product_name,unit_cost,quantity,line_total) VALUES (?1,?2,?3,?4,?5,?6)",
        params![purchase_id, product_id, name, unit_cost, quantity, total]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE products SET stock=stock+?1,purchase_price=?2 WHERE id=?3", params![quantity, unit_cost, product_id])
        .map_err(|e| format!("به‌روزرسانی موجودی ناموفق بود: {e}"))?;
    tx.execute("INSERT INTO stock_movements(product_id,date,movement_type,quantity,reference,note) VALUES (?1,?2,'purchase',?3,?4,?5)",
        params![product_id, date.trim(), quantity, format!("خرید #{purchase_id}"), note.trim()]).map_err(|e| e.to_string())?;
    if !supplier.trim().is_empty() && total > paid {
        tx.execute("INSERT INTO parties(name,kind,balance) VALUES (?1,'supplier',?2)
            ON CONFLICT(name) DO UPDATE SET balance=balance+excluded.balance",
            params![supplier.trim(), total-paid]).or_else(|_| {
                tx.execute("UPDATE parties SET balance=balance+?1 WHERE name=?2 AND kind IN ('supplier','both')",
                    params![total-paid, supplier.trim()])
            }).map_err(|e| format!("به‌روزرسانی حساب تأمین‌کننده ناموفق بود: {e}"))?;
    }
    tx.commit().map_err(|e| format!("نهایی‌سازی خرید ناموفق بود: {e}"))?;
    Ok(purchase_id)
}

pub fn record_expense(conn: &Connection, date: &str, category: &str, amount: i64, paid_from: &str, note: &str) -> Result<i64, String> {
    if date.trim().is_empty() || category.trim().is_empty() || amount <= 0 {
        return Err("تاریخ، دسته‌بندی و مبلغ مثبت برای هزینه لازم است.".into());
    }
    conn.execute("INSERT INTO business_expenses(date,category,amount,paid_from,note) VALUES (?1,?2,?3,?4,?5)",
        params![date.trim(), category.trim(), amount, paid_from, note.trim()])
        .map_err(|e| format!("ثبت هزینه ناموفق بود: {e}"))?;
    Ok(conn.last_insert_rowid())
}

pub fn business_totals(conn: &Connection) -> rusqlite::Result<(i64, i64, i64, i64)> {
    let sales: i64 = conn.query_row("SELECT COALESCE(SUM(total),0) FROM sales", [], |r| r.get(0))?;
    let purchases: i64 = conn.query_row("SELECT COALESCE(SUM(total),0) FROM purchases", [], |r| r.get(0))?;
    let expenses: i64 = conn.query_row("SELECT COALESCE(SUM(amount),0) FROM business_expenses", [], |r| r.get(0))?;
    let low_stock: i64 = conn.query_row("SELECT COUNT(*) FROM products WHERE stock <= 3", [], |r| r.get(0))?;
    Ok((sales, purchases, expenses, low_stock))
}

pub fn sales_csv(conn: &Connection) -> rusqlite::Result<String> {
    let mut csv = String::from("sale_id,date,total_toman,product_name,barcode,quantity,unit_price_toman,line_total_toman\n");
    let mut stmt = conn.prepare("SELECT s.id,s.date,s.total,si.product_name,p.barcode,si.quantity,si.unit_price,si.line_total
        FROM sales s JOIN sale_items si ON si.sale_id=s.id JOIN products p ON p.id=si.product_id ORDER BY s.id,si.id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,
        r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,i64>(7)?)))?;
    for row in rows {
        let (id,date,total,name,barcode,qty,price,line) = row?;
        let safe = |s: String| format!("\"{}\"", s.replace('"', "\"\""));
        csv.push_str(&format!("{id},{},{total},{},{},{qty},{price},{line}\n",safe(date),safe(name),safe(barcode)));
    }
    Ok(csv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn adding_product_and_scanning_twice_increments_basket_quantity() {
        let conn = setup();
        add_product(&conn, "کالای آزمایشی", "SKU-1", "123456", 100, 150, 3).unwrap();
        let mut basket = Vec::new();
        scan_into_basket(&conn, &mut basket, "123456").unwrap();
        scan_into_basket(&conn, &mut basket, "123456").unwrap();
        assert_eq!(basket.len(), 1);
        assert_eq!(basket[0].quantity, 2);
        assert_eq!(basket[0].line_total(), 300);
    }

    #[test]
    fn unknown_barcode_is_reported_without_mutating_basket() {
        let conn = setup();
        let mut basket = Vec::new();
        assert!(scan_into_basket(&conn, &mut basket, "missing").unwrap_err().contains("پیدا نشد"));
        assert!(basket.is_empty());
    }

    #[test]
    fn sale_decrements_stock_and_records_invoice_atomically() {
        let mut conn = setup();
        let id = add_product(&conn, "کالا", "SKU-2", "234567", 50, 80, 4).unwrap();
        let basket = vec![BasketItem { product_id: id, name: "کالا".into(), unit_price: 80, quantity: 2 }];
        let sale_id = record_sale(&mut conn, &basket, "1405/07/02").unwrap();
        let stock: i64 = conn.query_row("SELECT stock FROM products WHERE id=?1", [id], |r| r.get(0)).unwrap();
        let (total, items): (i64, i64) = conn.query_row(
            "SELECT s.total,(SELECT COUNT(*) FROM sale_items WHERE sale_id=s.id) FROM sales s WHERE s.id=?1",
            [sale_id], |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap();
        assert_eq!(stock, 2);
        assert_eq!(total, 160);
        assert_eq!(items, 1);
    }

    #[test]
    fn insufficient_stock_does_not_create_sale_or_change_stock() {
        let mut conn = setup();
        let id = add_product(&conn, "کالا", "SKU-3", "345678", 20, 30, 1).unwrap();
        let basket = vec![BasketItem { product_id: id, name: "کالا".into(), unit_price: 30, quantity: 2 }];
        assert!(record_sale(&mut conn, &basket, "1405/07/02").is_err());
        let (stock, sales): (i64, i64) = conn.query_row(
            "SELECT stock,(SELECT COUNT(*) FROM sales) FROM products WHERE id=?1",
            [id], |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap();
        assert_eq!(stock, 1);
        assert_eq!(sales, 0);
    }

    #[test]
    fn duplicate_sku_or_barcode_is_rejected() {
        let conn = setup();
        add_product(&conn, "کالا اول", "SKU-4", "456789", 1, 2, 1).unwrap();
        assert!(add_product(&conn, "کالا دوم", "SKU-4", "567890", 1, 2, 1).is_err());
        assert!(add_product(&conn, "کالا سوم", "SKU-5", "456789", 1, 2, 1).is_err());
    }
}
