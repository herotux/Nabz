use rusqlite::{params, Connection, OptionalExtension};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, env, fs, path::PathBuf, rc::Rc};

slint::include_modules!();

fn database_path() -> PathBuf {
    if let Some(path) = env::var_os("NABZ_DB_PATH") {
        return PathBuf::from(path);
    }
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data_home.join("com.herotux.nabz.webprototype").join("nabz.sqlite3")
}

fn open_database() -> Result<Connection, String> {
    let path = database_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("ساخت پوشهٔ داده ناموفق بود: {e}"))?;
    }
    let c = Connection::open(&path).map_err(|e| format!("بازکردن پایگاه داده ناموفق بود ({}): {e}", path.display()))?;
    c.execute_batch(
        "PRAGMA foreign_keys=ON;
         PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS contacts(id INTEGER PRIMARY KEY,name TEXT NOT NULL,kind TEXT NOT NULL DEFAULT 'customer',phone TEXT NOT NULL DEFAULT '',opening_balance INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL DEFAULT(datetime('now','localtime')));
         CREATE TABLE IF NOT EXISTS products(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL UNIQUE,barcode TEXT NOT NULL DEFAULT '',buy_price INTEGER NOT NULL DEFAULT 0,sell_price INTEGER NOT NULL DEFAULT 0,quantity INTEGER NOT NULL DEFAULT 0,min_quantity INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS invoices(id INTEGER PRIMARY KEY,number INTEGER NOT NULL UNIQUE,kind TEXT NOT NULL,contact_id INTEGER,contact_name TEXT NOT NULL,invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),subtotal INTEGER NOT NULL,discount INTEGER NOT NULL DEFAULT 0,total INTEGER NOT NULL,paid INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '');
         CREATE TABLE IF NOT EXISTS invoice_items(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),product_id INTEGER,item_name TEXT NOT NULL,quantity INTEGER NOT NULL,unit_price INTEGER NOT NULL,line_total INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS payments(id INTEGER PRIMARY KEY,kind TEXT NOT NULL CHECK(kind IN('receipt','payment')),contact_id INTEGER,contact_name TEXT NOT NULL,amount INTEGER NOT NULL CHECK(amount>0),method TEXT NOT NULL DEFAULT 'cash',payment_date TEXT NOT NULL DEFAULT(date('now','localtime')),notes TEXT NOT NULL DEFAULT '');
         CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);"
    ).map_err(|e| e.to_string())?;
    let has_voided: i64 = c.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name='voided'",
        [], |r| r.get(0)
    ).map_err(|e| e.to_string())?;
    if has_voided == 0 {
        c.execute("ALTER TABLE invoices ADD COLUMN voided INTEGER NOT NULL DEFAULT 0 CHECK(voided IN (0,1))", [])
            .map_err(|e| format!("افزودن وضعیت ابطال فاکتور ناموفق بود: {e}"))?;
    }
    c.execute_batch("CREATE TABLE IF NOT EXISTS sales_returns(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),return_date TEXT NOT NULL DEFAULT(date('now','localtime')),amount INTEGER NOT NULL CHECK(amount>0),notes TEXT NOT NULL DEFAULT ''); CREATE TABLE IF NOT EXISTS sales_return_items(id INTEGER PRIMARY KEY,return_id INTEGER NOT NULL REFERENCES sales_returns(id),invoice_item_id INTEGER NOT NULL REFERENCES invoice_items(id),product_id INTEGER NOT NULL REFERENCES products(id),item_name TEXT NOT NULL,quantity INTEGER NOT NULL CHECK(quantity>0),unit_price INTEGER NOT NULL,amount INTEGER NOT NULL CHECK(amount>0)); CREATE TABLE IF NOT EXISTS sales_refunds(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),return_id INTEGER NOT NULL REFERENCES sales_returns(id),refund_date TEXT NOT NULL DEFAULT(date('now','localtime')),amount INTEGER NOT NULL CHECK(amount>0),method TEXT NOT NULL DEFAULT 'cash',notes TEXT NOT NULL DEFAULT '');").map_err(|e|e.to_string())?;
    Ok(c)
}

fn digits(input: &str) -> String {
    input.chars().filter_map(|c| match c {
        '۰' => Some('0'), '۱' => Some('1'), '۲' => Some('2'), '۳' => Some('3'),
        '۴' => Some('4'), '۵' => Some('5'), '۶' => Some('6'), '۷' => Some('7'),
        '۸' => Some('8'), '۹' => Some('9'),
        '٠' => Some('0'), '١' => Some('1'), '٢' => Some('2'), '٣' => Some('3'),
        '٤' => Some('4'), '٥' => Some('5'), '٦' => Some('6'), '٧' => Some('7'),
        '٨' => Some('8'), '٩' => Some('9'),
        ',' | '٬' | ' ' => None,
        other => Some(other),
    }).collect()
}

fn parse_amount(input: &str) -> Result<i64, String> {
    let normalized = digits(input.trim());
    if normalized.is_empty() { return Ok(0); }
    normalized.parse::<i64>().map_err(|_| format!("عدد معتبر نیست: {input}"))
}

fn fmt_number(value: i64) -> String {
    let mut raw = value.to_string();
    let negative = raw.starts_with('-');
    if negative { raw.remove(0); }
    let mut grouped = String::new();
    for (i, c) in raw.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { grouped.push('٬'); }
        grouped.push(c);
    }
    let grouped: String = grouped.chars().rev().collect();
    let grouped = if negative { format!("-{grouped}") } else { grouped };
    grouped.chars().map(|c| match c {
        '0' => '۰', '1' => '۱', '2' => '۲', '3' => '۳', '4' => '۴',
        '5' => '۵', '6' => '۶', '7' => '۷', '8' => '۸', '9' => '۹', _ => c
    }).collect()
}

fn money(value: i64) -> String { format!("{} تومان", fmt_number(value)) }

fn normalize_kind(raw: &str) -> Result<&'static str, String> {
    match raw.trim() {
        "customer" | "مشتری" => Ok("customer"),
        "supplier" | "تأمین‌کننده" | "تامین‌کننده" => Ok("supplier"),
        "both" | "هر دو" | "مشتری و تأمین‌کننده" => Ok("both"),
        _ => Err("نوع طرف‌حساب معتبر نیست".into())
    }
}

fn normalize_invoice_search(input: &str) -> String {
    input.trim().chars().map(|c| match c {
        '۰' => '0', '۱' => '1', '۲' => '2', '۳' => '3', '۴' => '4',
        '۵' => '5', '۶' => '6', '۷' => '7', '۸' => '8', '۹' => '9',
        '٠' => '0', '١' => '1', '٢' => '2', '٣' => '3', '٤' => '4',
        '٥' => '5', '٦' => '6', '٧' => '7', '٨' => '8', '٩' => '9',
        other => other,
    }).collect()
}

fn refresh_ui(ui: &MainWindow, c: &Connection) -> Result<(), String> {
    let q = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string());
    ui.set_sales_today(SharedString::from(money(q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')")?)));
    ui.set_receipts_today(SharedString::from(money(q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date=date('now','localtime')),0)")?)));
    let balance = q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)")?;
    ui.set_customer_balance(SharedString::from(money(balance)));
    ui.set_inventory_value(SharedString::from(money(q("SELECT COALESCE(SUM(quantity*buy_price),0) FROM products")?)));
    ui.set_contacts_count(q("SELECT COUNT(*) FROM contacts")? as i32);
    ui.set_products_count(q("SELECT COUNT(*) FROM products")? as i32);
    ui.set_invoices_count(q("SELECT COUNT(*) FROM invoices WHERE voided=0")? as i32);

    let mut contact_rows = Vec::new();
    {
        let mut st = c.prepare("SELECT id,name,kind,phone,opening_balance FROM contacts ORDER BY id DESC").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok(ContactRow {
            id: r.get::<_, i64>(0)? as i32,
            name: r.get::<_, String>(1)?.into(),
            kind: match r.get::<_, String>(2)?.as_str() { "supplier" => "تأمین‌کننده", "both" => "هر دو", _ => "مشتری" }.into(),
            phone: r.get::<_, String>(3)?.into(),
            balance: money(r.get::<_, i64>(4)?).into(),
        })).map_err(|e| e.to_string())?;
        contact_rows = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }
    ui.set_contacts(ModelRc::new(VecModel::from(contact_rows)));

    let mut product_rows = Vec::new();
    {
        let mut st = c.prepare("SELECT id,name,code,quantity,sell_price,min_quantity FROM products ORDER BY id DESC").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| {
            let quantity: i64 = r.get(3)?;
            let min: i64 = r.get(5)?;
            Ok(ProductRow {
                id: r.get::<_, i64>(0)? as i32,
                name: r.get::<_, String>(1)?.into(),
                code: r.get::<_, String>(2)?.into(),
                quantity: fmt_number(quantity).into(),
                price: money(r.get::<_, i64>(4)?).into(),
                stock: if quantity <= min { "کم‌موجود".into() } else { "موجود".into() },
            })
        }).map_err(|e| e.to_string())?;
        product_rows = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }
    ui.set_products(ModelRc::new(VecModel::from(product_rows)));

    let mut invoice_rows = Vec::new();
    {
        let mut st = c.prepare("SELECT id,number,kind,contact_name,total,paid,invoice_date,voided FROM invoices WHERE (?1='' OR CAST(number AS TEXT) LIKE '%'||?1||'%' OR contact_name LIKE '%'||?1||'%' OR notes LIKE '%'||?1||'%') AND (?2='همه' OR (?2='فروش' AND kind='sale') OR (?2='خرید' AND kind='purchase') OR (?2='باطل‌شده' AND voided=1)) ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
        let search = normalize_invoice_search(&ui.get_invoice_search());
        let filter = ui.get_invoice_filter_kind();
        let rows = st.query_map(params![search, filter.as_str()], |r| {
            let kind: String = r.get(2)?;
            let total: i64 = r.get(4)?;
            let paid: i64 = r.get(5)?;
            let voided: bool = r.get::<_, i64>(7)? != 0;
            Ok(InvoiceRow {
                id: r.get::<_, i64>(0)? as i32,
                label: format!("فاکتور {} · {}", fmt_number(r.get::<_, i64>(1)?), r.get::<_, String>(3)?).into(),
                kind: if kind == "purchase" { "خرید".into() } else { "فروش".into() },
                total: money(total).into(),
                status: if voided { "باطل".into() } else if paid >= total { "تسویه".into() } else { format!("مانده {}", money(total-paid)).into() },
                date: r.get::<_, String>(6)?.into(),
            })
        }).map_err(|e| e.to_string())?;
        invoice_rows = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }
    ui.set_invoices(ModelRc::new(VecModel::from(invoice_rows)));

    let mut payment_rows = Vec::new();
    {
        let mut st = c.prepare("SELECT id,contact_name,kind,amount,method,payment_date FROM payments ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| {
            let kind: String = r.get(2)?;
            Ok(PaymentRow {
                id: r.get::<_, i64>(0)? as i32,
                label: r.get::<_, String>(1)?.into(),
                kind: if kind == "receipt" { "دریافت".into() } else { "پرداخت".into() },
                amount: money(r.get::<_, i64>(3)?).into(),
                method: r.get::<_, String>(4)?.into(),
                date: r.get::<_, String>(5)?.into(),
            })
        }).map_err(|e| e.to_string())?;
        payment_rows = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    }
    ui.set_payments(ModelRc::new(VecModel::from(payment_rows)));

    let store_name: Option<String> = c.query_row("SELECT value FROM settings WHERE key='store_name'", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let currency: Option<String> = c.query_row("SELECT value FROM settings WHERE key='currency'", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if let Some(name) = store_name { ui.set_settings_store_name(name.into()); }
    if let Some(currency) = currency { ui.set_settings_currency(currency.into()); }
    Ok(())
}

fn save_contact(c: &Connection, name: &str, kind: &str, phone: &str, balance_text: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() { return Err("نام طرف‌حساب الزامی است".into()); }
    let kind = normalize_kind(kind)?;
    let balance = parse_amount(balance_text)?;
    let count: i64 = c.query_row("SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE", [name], |r| r.get(0)).map_err(|e| e.to_string())?;
    if count > 0 { return Err("طرف‌حسابی با این نام قبلاً ثبت شده است".into()); }
    if kind == "both" && balance != 0 { return Err("برای طرف‌حساب دوگانه، ماندهٔ اولیه فعلاً باید صفر باشد".into()); }
    c.execute("INSERT INTO contacts(name,kind,phone,opening_balance) VALUES(?1,?2,?3,?4)", params![name, kind, phone.trim(), balance]).map_err(|e| e.to_string())?;
    Ok(())
}

fn save_product(c: &Connection, name: &str, code: &str, barcode: &str, qty: &str, buy: &str, sell: &str, minimum: &str) -> Result<(), String> {
    let name = name.trim();
    let code = code.trim();
    if name.is_empty() || code.is_empty() { return Err("نام و کد کالا الزامی است".into()); }
    let qty = parse_amount(qty)?; let buy = parse_amount(buy)?; let sell = parse_amount(sell)?; let minimum = parse_amount(minimum)?;
    if qty < 0 || buy < 0 || sell < 0 || minimum < 0 { return Err("موجودی و قیمت‌ها نمی‌توانند منفی باشند".into()); }
    c.execute("INSERT INTO products(name,code,barcode,buy_price,sell_price,quantity,min_quantity) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![name, code, barcode.trim(), buy, sell, qty, minimum])
        .map_err(|e| if e.to_string().contains("UNIQUE") { "کد کالا تکراری است".into() } else { e.to_string() })?;
    Ok(())
}

fn save_invoice(c: &mut Connection, kind: &str, contact: &str, item: &str, qty_text: &str, price_text: &str, discount_text: &str, paid_text: &str, method: &str, notes: &str) -> Result<(), String> {
    if kind != "sale" && kind != "purchase" { return Err("نوع فاکتور معتبر نیست".into()); }
    let contact = contact.trim();
    let item = item.trim();
    if item.is_empty() { return Err("نام یا کد کالا الزامی است".into()); }
    let qty = parse_amount(qty_text)?;
    let price = parse_amount(price_text)?;
    let discount = parse_amount(discount_text)?;
    let paid_input = parse_amount(paid_text)?;
    if qty <= 0 || price < 0 || discount < 0 { return Err("تعداد باید مثبت و قیمت و تخفیف نامنفی باشند".into()); }
    let subtotal = qty.checked_mul(price).ok_or("مبلغ ردیف بیش از حد بزرگ است")?;
    if discount > subtotal { return Err("تخفیف از مبلغ کالا بیشتر است".into()); }
    let total = subtotal - discount;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let cid: Option<i64> = if contact.is_empty() { None } else { tx.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1", [contact], |r| r.get(0)).optional().map_err(|e| e.to_string())? };
    if kind == "purchase" && cid.is_none() { return Err("برای خرید، تأمین‌کنندهٔ ثبت‌شده الزامی است".into()); }
    if kind == "sale" && !contact.is_empty() && cid.is_none() { return Err("مشتری پیدا نشد؛ ابتدا طرف‌حساب را ثبت کنید".into()); }
    if let Some(id) = cid {
        let role: String = tx.query_row("SELECT kind FROM contacts WHERE id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if kind == "sale" && role == "supplier" { return Err("برای فاکتور فروش، مشتری انتخاب کنید".into()); }
        if kind == "purchase" && role == "customer" { return Err("برای فاکتور خرید، تأمین‌کننده انتخاب کنید".into()); }
    }
    let paid = if kind == "sale" { paid_input } else { 0 };
    if paid < 0 || paid > total { return Err("مبلغ دریافتی باید بین صفر و مبلغ نهایی باشد".into()); }
    if kind == "sale" && paid < total && cid.is_none() { return Err("فروش گذری باید کامل تسویه شود".into()); }

    let product: Option<(i64, i64)> = tx.query_row("SELECT id,quantity FROM products WHERE name=?1 COLLATE NOCASE OR code=?1 OR barcode=?1 LIMIT 1", [item], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e| e.to_string())?;
    let (pid, item_name) = if let Some((id, stock)) = product {
        if kind == "sale" {
            if stock < qty { return Err(format!("موجودی کافی نیست؛ موجودی فعلی: {stock}")); }
            tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2", params![qty,id]).map_err(|e| e.to_string())?;
        } else {
            tx.execute("UPDATE products SET quantity=quantity+?1,buy_price=?2 WHERE id=?3", params![qty,price,id]).map_err(|e| e.to_string())?;
        }
        (Some(id), item.to_string())
    } else if kind == "purchase" {
        let code = format!("AUTO-{}", tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM products", [], |r| r.get::<_,i64>(0)).map_err(|e| e.to_string())?);
        tx.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity) VALUES(?1,?2,?3,?3,?4)", params![item,code,price,qty]).map_err(|e| e.to_string())?;
        (Some(tx.last_insert_rowid()), item.to_string())
    } else {
        return Err("کالای فروش در انبار پیدا نشد؛ ابتدا کالا را ثبت کنید".into());
    };
    let number: i64 = tx.query_row("SELECT COALESCE(MAX(number),1000)+1 FROM invoices", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let contact_name = if contact.is_empty() { "مشتری گذری" } else { contact };
    tx.execute("INSERT INTO invoices(number,kind,contact_id,contact_name,subtotal,discount,total,paid,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![number,kind,cid,contact_name,subtotal,discount,total,paid,notes.trim()]).map_err(|e| e.to_string())?;
    let invoice_id = tx.last_insert_rowid();
    tx.execute("INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total) VALUES(?1,?2,?3,?4,?5,?6)", params![invoice_id,pid,item_name,qty,price,qty*price]).map_err(|e| e.to_string())?;
    if kind == "sale" && paid > 0 {
        tx.execute("INSERT INTO payments(kind,contact_id,contact_name,amount,method,notes) VALUES('receipt',?1,?2,?3,COALESCE(NULLIF(?4,''),'cash'),?5)", params![cid,contact_name,paid,method.trim(),format!("دریافت بابت فاکتور فروش #{number}")]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn save_invoice_multi(c: &mut Connection, kind: &str, contact: &str, rows_text: &str, discount_text: &str, paid_text: &str, method: &str, notes: &str) -> Result<(), String> {
    if kind != "sale" && kind != "purchase" { return Err("نوع فاکتور معتبر نیست".into()); }
    let contact = contact.trim();
    let discount = parse_amount(discount_text)?;
    let paid_input = parse_amount(paid_text)?;
    if discount < 0 || paid_input < 0 { return Err("تخفیف و مبلغ دریافتی نمی‌توانند منفی باشند".into()); }

    let mut rows: Vec<(String, i64, i64, i64)> = Vec::new();
    let mut subtotal: i64 = 0;
    for (line_no, raw) in rows_text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() != 3 || parts[0].is_empty() {
            return Err(format!("ردیف {}: قالب باید «کد یا نام کالا | تعداد | قیمت واحد» باشد", line_no + 1));
        }
        let qty = parse_amount(parts[1]).map_err(|e| format!("ردیف {}: {e}", line_no + 1))?;
        let price = parse_amount(parts[2]).map_err(|e| format!("ردیف {}: {e}", line_no + 1))?;
        if qty <= 0 || price < 0 {
            return Err(format!("ردیف {}: تعداد باید مثبت و قیمت نامنفی باشد", line_no + 1));
        }
        let line_total = qty.checked_mul(price).ok_or_else(|| format!("ردیف {}: مبلغ بیش از حد بزرگ است", line_no + 1))?;
        subtotal = subtotal.checked_add(line_total).ok_or("جمع فاکتور بیش از حد بزرگ است")?;
        rows.push((parts[0].to_string(), qty, price, line_total));
    }
    if rows.is_empty() { return Err("حداقل یک ردیف کالا وارد کنید".into()); }
    if discount > subtotal { return Err("تخفیف از جمع فاکتور بیشتر است".into()); }
    let total = subtotal.checked_sub(discount).ok_or("مبلغ نهایی نامعتبر است")?;
    let paid = if kind == "sale" { paid_input } else { 0 };
    if paid > total { return Err("مبلغ دریافتی از مبلغ نهایی بیشتر است".into()); }

    let tx = c.transaction().map_err(|e| e.to_string())?;
    let cid: Option<i64> = if contact.is_empty() { None } else {
        tx.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1", [contact], |r| r.get(0))
            .optional().map_err(|e| e.to_string())?
    };
    if kind == "purchase" && cid.is_none() { return Err("برای خرید، تأمین‌کنندهٔ ثبت‌شده الزامی است".into()); }
    if kind == "sale" && !contact.is_empty() && cid.is_none() { return Err("مشتری پیدا نشد؛ ابتدا طرف‌حساب را ثبت کنید".into()); }
    if let Some(id) = cid {
        let role: String = tx.query_row("SELECT kind FROM contacts WHERE id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if kind == "sale" && role == "supplier" { return Err("برای فاکتور فروش، مشتری انتخاب کنید".into()); }
        if kind == "purchase" && role == "customer" { return Err("برای فاکتور خرید، تأمین‌کننده انتخاب کنید".into()); }
    }
    if kind == "sale" && paid < total && cid.is_none() { return Err("فروش گذری باید کامل تسویه شود".into()); }

    let number: i64 = tx.query_row("SELECT COALESCE(MAX(number),1000)+1 FROM invoices", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let contact_name = if contact.is_empty() { "مشتری گذری" } else { contact };
    tx.execute("INSERT INTO invoices(number,kind,contact_id,contact_name,subtotal,discount,total,paid,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![number,kind,cid,contact_name,subtotal,discount,total,paid,notes.trim()]).map_err(|e| e.to_string())?;
    let invoice_id = tx.last_insert_rowid();

    for (item, qty, price, line_total) in rows {
        let product: Option<(i64, i64)> = tx.query_row(
            "SELECT id,quantity FROM products WHERE name=?1 COLLATE NOCASE OR code=?1 OR barcode=?1 LIMIT 1",
            [&item], |r| Ok((r.get(0)?, r.get(1)?))
        ).optional().map_err(|e| e.to_string())?;
        let (pid, item_name) = if let Some((id, stock)) = product {
            if kind == "sale" {
                if stock < qty { return Err(format!("موجودی «{item}» کافی نیست؛ موجودی فعلی: {stock}")); }
                let changed = tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2 AND quantity>=?1", params![qty,id]).map_err(|e| e.to_string())?;
                if changed != 1 { return Err(format!("موجودی «{item}» کافی نیست")); }
            } else {
                tx.execute("UPDATE products SET quantity=quantity+?1,buy_price=?2 WHERE id=?3", params![qty,price,id]).map_err(|e| e.to_string())?;
            }
            (id, item)
        } else if kind == "purchase" {
            let code = format!("AUTO-{}", tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM products", [], |r| r.get::<_,i64>(0)).map_err(|e| e.to_string())?);
            tx.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity) VALUES(?1,?2,?3,?3,?4)", params![item,code,price,qty]).map_err(|e| e.to_string())?;
            (tx.last_insert_rowid(), item)
        } else {
            return Err(format!("کالای «{item}» در انبار پیدا نشد؛ ابتدا کالا را ثبت کنید"));
        };
        tx.execute("INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total) VALUES(?1,?2,?3,?4,?5,?6)",
            params![invoice_id,pid,item_name,qty,price,line_total]).map_err(|e| e.to_string())?;
    }
    if kind == "sale" && paid > 0 {
        tx.execute("INSERT INTO payments(kind,contact_id,contact_name,amount,method,notes) VALUES('receipt',?1,?2,?3,COALESCE(NULLIF(?4,''),'cash'),?5)",
            params![cid,contact_name,paid,method.trim(),format!("دریافت بابت فاکتور فروش #{number}")]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn invoice_details(c: &Connection, invoice_id: i64) -> Result<String, String> {
    let header: (i64, String, String, String, i64, i64, i64, i64, String) = c.query_row(
        "SELECT number,kind,contact_name,invoice_date,subtotal,discount,total,paid,notes FROM invoices WHERE id=?1",
        [invoice_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))
    ).optional().map_err(|e| e.to_string())?.ok_or("فاکتور پیدا نشد")?;
    let mut out = format!(
        "فاکتور {} · {}\nطرف‌حساب: {}\nتاریخ: {}\n",
        fmt_number(header.0),
        if header.1 == "purchase" { "خرید" } else { "فروش" },
        header.2,
        header.3
    );
    let mut st = c.prepare("SELECT item_name,quantity,unit_price,line_total FROM invoice_items WHERE invoice_id=?1 ORDER BY id").map_err(|e| e.to_string())?;
    let rows = st.query_map([invoice_id], |r| Ok((
        r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?
    ))).map_err(|e| e.to_string())?;
    let lines = rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    if lines.is_empty() { out.push_str("ردیف کالا ثبت نشده است\n"); }
    for (index,(name,qty,price,line_total)) in lines.iter().enumerate() {
        out.push_str(&format!("{}. {} | تعداد {} | فی {} | جمع {}\n", index+1, name, fmt_number(*qty), money(*price), money(*line_total)));
    }
    let mut return_st=c.prepare("SELECT sr.return_date,sri.item_name,sri.quantity,sri.amount FROM sales_returns sr JOIN sales_return_items sri ON sri.return_id=sr.id WHERE sr.invoice_id=?1 ORDER BY sr.id,sri.id").map_err(|e|e.to_string())?;
    let returns=return_st.query_map([invoice_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    if !returns.is_empty() {
        out.push_str("برگشت‌های ثبت‌شده:\\n");
        for (date,name,qty,amount) in returns { out.push_str(&format!("{} · {} | تعداد {} | مبلغ خام {}\\n",date,name,fmt_number(qty),money(amount))); }
    }
    out.push_str(&format!(
        "جمع: {}\\nتخفیف: {}\\nمبلغ نهایی: {}\\nپرداخت‌شده: {}\\nمانده: {}\\nشرح: {}",
        money(header.4), money(header.5), money(header.6), money(header.7),
        money(header.6.saturating_sub(header.7)), if header.8.trim().is_empty() { "—" } else { header.8.as_str() }
    ));
    Ok(out)
}

fn return_sale_items(c: &mut Connection, invoice_id: i64, rows_text: &str, refund_method: &str) -> Result<(i64,i64), String> {
    if invoice_id <= 0 { return Err("ابتدا فاکتور فروش را انتخاب کنید".into()); }
    let mut requested: Vec<(String,i64)> = Vec::new();
    for (line_no, raw) in rows_text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() != 2 || parts[0].is_empty() { return Err(format!("ردیف {}: قالب باید «کد یا نام کالا | تعداد برگشتی» باشد", line_no+1)); }
        let qty = parse_amount(parts[1])?;
        if qty <= 0 { return Err(format!("ردیف {}: تعداد برگشتی باید مثبت باشد", line_no+1)); }
        if requested.iter().any(|(name,_)| name.eq_ignore_ascii_case(parts[0])) {
            return Err(format!("کالای «{}» را فقط یک بار در فهرست برگشت وارد کنید", parts[0]));
        }
        requested.push((parts[0].to_string(),qty));
    }
    if requested.is_empty() { return Err("حداقل یک ردیف برگشت وارد کنید".into()); }
    let refund_method = if refund_method.trim().is_empty() { "cash" } else { refund_method.trim() };
    let tx = c.transaction().map_err(|e|e.to_string())?;
    let invoice: (String,i64,i64,i64,i64) = tx.query_row("SELECT kind,subtotal,discount,total,paid FROM invoices WHERE id=?1 AND voided=0",[invoice_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(|e|e.to_string())?.ok_or("فاکتور پیدا نشد یا باطل شده است")?;
    if invoice.0 != "sale" { return Err("برگشت جزئی فقط برای فاکتور فروش مجاز است".into()); }
    let mut lines: Vec<(i64,i64,i64,i64,String)> = Vec::new();
    let mut gross_return = 0i64;
    for (lookup, qty) in requested {
        let mut st=tx.prepare("SELECT ii.id,ii.product_id,ii.quantity,ii.unit_price,ii.item_name FROM invoice_items ii JOIN products p ON p.id=ii.product_id WHERE ii.invoice_id=?1 AND (ii.item_name=?2 COLLATE NOCASE OR p.code=?2 OR p.barcode=?2) ORDER BY ii.id").map_err(|e|e.to_string())?;
        let matches=st.query_map(params![invoice_id,lookup],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<i64>>(1)?.unwrap_or(0),r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        drop(st);
        if matches.len()!=1 { return Err(format!("کالای «{lookup}» در این فاکتور یکتا نیست یا پیدا نشد؛ از کد یکتا استفاده کنید")); }
        let (item_id,pid,original_qty,unit_price,item_name)=matches.into_iter().next().unwrap();
        if pid<=0 { return Err(format!("کالای «{item_name}» شناسهٔ انبار ندارد؛ برگشت ممکن نیست")); }
        let already:i64=tx.query_row("SELECT COALESCE(SUM(quantity),0) FROM sales_return_items WHERE invoice_item_id=?1",[item_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let remaining=original_qty.checked_sub(already).ok_or("سابقهٔ برگشت نامعتبر است")?;
        if qty>remaining { return Err(format!("برای «{item_name}» فقط {} عدد قابل برگشت است",remaining)); }
        let gross=qty.checked_mul(unit_price).ok_or("مبلغ برگشت بیش از حد بزرگ است")?;
        gross_return=gross_return.checked_add(gross).ok_or("جمع برگشت بیش از حد بزرگ است")?;
        lines.push((item_id,pid,qty,unit_price,item_name));
    }
    if invoice.1<=0 || gross_return>invoice.1 { return Err("مبلغ ردیف‌های برگشتی از جمع اولیهٔ فاکتور بیشتر است".into()); }
    let credit=gross_return.checked_mul(invoice.3).ok_or("محاسبهٔ مبلغ برگشت بیش از حد بزرگ است")? / invoice.1;
    if credit<=0 { return Err("مبلغ خالص برگشت صفر است؛ مقدار برگشت را بررسی کنید".into()); }
    let new_subtotal=invoice.1-gross_return;
    let new_total=invoice.3.checked_sub(credit).ok_or("مبلغ نهایی نامعتبر است")?;
    let refund = invoice.4.saturating_sub(new_total);
    let new_discount=new_subtotal-new_total;
    tx.execute("INSERT INTO sales_returns(invoice_id,amount,notes) VALUES(?1,?2,?3)",params![invoice_id,credit,format!("برگشت جزئی فاکتور #{}",invoice_id)]).map_err(|e|e.to_string())?;
    let return_id=tx.last_insert_rowid();
    if refund > 0 {
        tx.execute("INSERT INTO sales_refunds(invoice_id,return_id,amount,method,notes) VALUES(?1,?2,?3,?4,?5)", params![invoice_id,return_id,refund,refund_method,format!("بازپرداخت بابت برگشت فاکتور #{}",invoice_id)]).map_err(|e|e.to_string())?;
    }
    for (item_id,pid,qty,unit_price,item_name) in lines {
        let amount=qty.checked_mul(unit_price).ok_or("مبلغ ردیف برگشت بیش از حد بزرگ است")?;
        tx.execute("UPDATE products SET quantity=quantity+?1 WHERE id=?2",params![qty,pid]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO sales_return_items(return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![return_id,item_id,pid,item_name,qty,unit_price,amount]).map_err(|e|e.to_string())?;
    }
    tx.execute("UPDATE invoices SET subtotal=?1,discount=?2,total=?3,paid=MIN(paid,?3) WHERE id=?4",params![new_subtotal,new_discount,new_total,invoice_id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok((credit,refund))
}

fn void_invoice(c: &mut Connection, invoice_id: i64) -> Result<(), String> {
    if invoice_id <= 0 { return Err("ابتدا یک فاکتور را انتخاب کنید".into()); }
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let invoice: Option<(i64,String,Option<i64>,String,i64,i64)> = tx.query_row(
        "SELECT number,kind,contact_id,contact_name,total,paid FROM invoices WHERE id=?1 AND voided=0",
        [invoice_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
    ).optional().map_err(|e| e.to_string())?;
    let (number,kind,contact_id,contact_name,total,paid)=invoice.ok_or("فاکتور پیدا نشد یا قبلاً باطل شده است")?;

    let initial_receipt: i64 = if kind == "sale" {
        tx.query_row("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND notes=?1",
            [format!("دریافت بابت فاکتور فروش #{number}")], |r| r.get(0)).map_err(|e| e.to_string())?
    } else { 0 };
    if kind == "sale" && paid != initial_receipt {
        return Err("این فاکتور پرداخت‌های تخصیص‌یافتهٔ بعدی دارد؛ برای جلوگیری از مغایرت، ابتدا گردش‌های مرتبط را اصلاح کنید".into());
    }
    if kind == "purchase" && paid != 0 {
        return Err("برای این فاکتور خرید پرداخت تخصیص یافته است؛ ابتدا گردش مالی آن را اصلاح کنید".into());
    }
    if kind != "sale" && kind != "purchase" { return Err("نوع فاکتور ناشناخته است".into()); }

    let mut st = tx.prepare("SELECT product_id,item_name,quantity FROM invoice_items WHERE invoice_id=?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let mapped = st.query_map([invoice_id], |r| Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?)))
        .map_err(|e| e.to_string())?;
    let lines = mapped.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    drop(st);
    if lines.is_empty() { return Err("فاکتور ردیف کالا ندارد؛ ابطال خودکار ممکن نیست".into()); }

    for (product_id,item_name,qty) in lines {
        if qty <= 0 { return Err(format!("تعداد نامعتبر برای کالای «{item_name}»؛ ابطال لغو شد")); }
        if let Some(pid) = product_id {
            let stock: i64 = tx.query_row("SELECT quantity FROM products WHERE id=?1", [pid], |r| r.get(0))
                .optional().map_err(|e| e.to_string())?.ok_or(format!("کالای «{item_name}» حذف شده؛ ابطال امن ممکن نیست"))?;
            if kind == "sale" {
                let next = stock.checked_add(qty).ok_or("موجودی پس از برگشت بیش از حد بزرگ می‌شود")?;
                tx.execute("UPDATE products SET quantity=?1 WHERE id=?2", params![next,pid]).map_err(|e| e.to_string())?;
            } else {
                if stock < qty {
                    return Err(format!("موجودی «{item_name}» برای برگشت خرید کافی نیست؛ موجودی فعلی {} و مقدار برگشت {}", stock, qty));
                }
                tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2", params![qty,pid]).map_err(|e| e.to_string())?;
            }
        } else if kind == "purchase" {
            return Err(format!("کالای «{item_name}» شناسهٔ انبار ندارد؛ ابطال امن ممکن نیست"));
        }
    }
    if initial_receipt > 0 {
        tx.execute("DELETE FROM payments WHERE kind='receipt' AND notes=?1",
            [format!("دریافت بابت فاکتور فروش #{number}")]).map_err(|e| e.to_string())?;
    }
    tx.execute("UPDATE invoices SET voided=1 WHERE id=?1 AND voided=0", [invoice_id]).map_err(|e| e.to_string())?;
    let _ = (contact_id,contact_name,total);
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn save_payment(c: &mut Connection, kind: &str, contact: &str, amount_text: &str, method: &str, notes: &str) -> Result<(), String> {
    let amount = parse_amount(amount_text)?;
    let contact = contact.trim();
    if !["receipt","payment"].contains(&kind) || contact.is_empty() || amount <= 0 { return Err("طرف‌حساب و مبلغ مثبت الزامی است".into()); }
    let tx = c.transaction().map_err(|e| e.to_string())?;
    let cid: i64 = tx.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1", [contact], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("طرف‌حساب ثبت‌شده‌ای با این نام پیدا نشد")?;
    let role: String = tx.query_row("SELECT kind FROM contacts WHERE id=?1", [cid], |r| r.get(0)).map_err(|e| e.to_string())?;
    if kind == "receipt" && role == "supplier" { return Err("برای دریافت، مشتری انتخاب کنید".into()); }
    if kind == "payment" && role == "customer" { return Err("برای پرداخت، تأمین‌کننده انتخاب کنید".into()); }
    let invoice_kind = if kind == "receipt" { "sale" } else { "purchase" };
    let invoice_outstanding: i64 = tx.query_row("SELECT COALESCE(SUM(total-paid),0) FROM invoices WHERE contact_id=?1 AND kind=?2 AND voided=0", params![cid,invoice_kind], |r| r.get(0)).map_err(|e| e.to_string())?;
    let opening: i64 = tx.query_row("SELECT opening_balance FROM contacts WHERE id=?1", [cid], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?.max(0_i64);
    let outstanding = invoice_outstanding.checked_add(opening).ok_or("ماندهٔ حساب بیش از حد بزرگ است")?;
    if amount > outstanding { return Err(format!("مبلغ از ماندهٔ قابل تخصیص بیشتر است: {}", money(outstanding))); }
    tx.execute("INSERT INTO payments(kind,contact_id,contact_name,amount,method,notes) VALUES(?1,?2,?3,?4,COALESCE(NULLIF(?5,''),'cash'),?6)", params![kind,cid,contact,amount,method.trim(),notes.trim()]).map_err(|e| e.to_string())?;
    let mut remaining = amount.min(opening);
    if remaining > 0 { tx.execute("UPDATE contacts SET opening_balance=opening_balance-?1 WHERE id=?2", params![remaining,cid]).map_err(|e| e.to_string())?; }
    let mut st = tx.prepare("SELECT id,total,paid FROM invoices WHERE contact_id=?1 AND kind=?2 AND voided=0 AND paid<total ORDER BY invoice_date,id").map_err(|e| e.to_string())?;
    let rows = st.query_map(params![cid,invoice_kind], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).map_err(|e| e.to_string())?;
    let invoices = rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    drop(st);
    for (id,total,paid) in invoices {
        if remaining >= amount { break; }
        let applied = (amount - remaining).min(total-paid);
        if applied > 0 {
            tx.execute("UPDATE invoices SET paid=paid+?1 WHERE id=?2", params![applied,id]).map_err(|e| e.to_string())?;
            remaining += applied;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn set_status(ui: &MainWindow, message: impl Into<SharedString>) {
    ui.set_status_message(message.into());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    let db = match open_database() {
        Ok(c) => Rc::new(RefCell::new(c)),
        Err(e) => {
            set_status(&ui, e);
            return Err("Could not initialize Nabz SQLite database".into());
        }
    };
    if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای خواندن داده‌ها: {e}")); }

    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_refresh_requested(move || if let Some(ui) = weak.upgrade() {
            match refresh_ui(&ui, &db.borrow()) { Ok(()) => set_status(&ui, "داده‌ها به‌روز شد"), Err(e) => set_status(&ui, format!("خطا: {e}")) }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_contact_requested(move || if let Some(ui) = weak.upgrade() {
            let result = save_contact(&db.borrow(), &ui.get_contact_name(), &ui.get_contact_kind(), &ui.get_contact_phone(), &ui.get_contact_balance());
            match result {
                Ok(()) => { ui.set_contact_name("".into()); ui.set_contact_phone("".into()); ui.set_contact_balance("0".into()); set_status(&ui, "طرف‌حساب ثبت شد"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_product_requested(move || if let Some(ui) = weak.upgrade() {
            let result = save_product(&db.borrow(), &ui.get_product_name(), &ui.get_product_code(), &ui.get_product_barcode(), &ui.get_product_quantity(), &ui.get_product_buy_price(), &ui.get_product_sell_price(), &ui.get_product_minimum());
            match result {
                Ok(()) => { ui.set_product_name("".into()); ui.set_product_code("".into()); ui.set_product_barcode("".into()); ui.set_product_quantity("0".into()); ui.set_product_buy_price("0".into()); ui.set_product_sell_price("0".into()); ui.set_product_minimum("0".into()); set_status(&ui, "کالا ثبت شد"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_invoice_requested(move || if let Some(ui) = weak.upgrade() {
            let result = save_invoice_multi(&mut db.borrow_mut(), &ui.get_invoice_kind(), &ui.get_invoice_contact(), &ui.get_invoice_items_text(), &ui.get_invoice_discount(), &ui.get_invoice_paid(), &ui.get_invoice_method(), &ui.get_invoice_notes());
            match result {
                Ok(()) => { ui.set_invoice_items_text("".into()); ui.set_invoice_discount("0".into()); ui.set_invoice_paid("0".into()); ui.set_invoice_notes("".into()); set_status(&ui, "فاکتور ثبت شد و موجودی به‌روزرسانی شد"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_invoice_detail_requested(move |invoice_id| if let Some(ui) = weak.upgrade() {
            match invoice_details(&db.borrow(), invoice_id as i64) {
                Ok(details) => { ui.set_invoice_detail(details.into()); set_status(&ui, format!("جزئیات فاکتور {} نمایش داده شد", invoice_id)); }
                Err(e) => { ui.set_invoice_detail("".into()); set_status(&ui, format!("خطا در نمایش جزئیات: {e}")); }
            }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_invoice_return_requested(move |invoice_id, rows_text| if let Some(ui) = weak.upgrade() {
            match return_sale_items(&mut db.borrow_mut(), invoice_id as i64, &rows_text, &ui.get_return_refund_method()) {
                Ok((amount,refund)) => {
                    ui.set_return_items_text("".into());
                    set_status(&ui, if refund > 0 { format!("برگشت ثبت شد؛ اعتبار {} و بازپرداخت {} با روش انتخابی ثبت شد؛ موجودی اصلاح شد", money(amount), money(refund)) } else { format!("برگشت جزئی ثبت شد؛ اعتبار فاکتور {} کاهش یافت و موجودی اصلاح شد", money(amount)) });
                    if let Ok(details) = invoice_details(&db.borrow(), invoice_id as i64) { ui.set_invoice_detail(details.into()); }
                }
                Err(e) => set_status(&ui, format!("برگشت ثبت نشد: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_payment_requested(move || if let Some(ui) = weak.upgrade() {
            let result = save_payment(&mut db.borrow_mut(), &ui.get_payment_kind(), &ui.get_payment_contact(), &ui.get_payment_amount(), &ui.get_payment_method(), &ui.get_payment_notes());
            match result {
                Ok(()) => { ui.set_payment_amount("".into()); ui.set_payment_notes("".into()); set_status(&ui, "گردش مالی ثبت شد و به مانده تخصیص یافت"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_settings_requested(move || if let Some(ui) = weak.upgrade() {
            let result = (|| -> Result<(),String> {
                let c = db.borrow();
                c.execute("INSERT INTO settings(key,value) VALUES('store_name',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [ui.get_settings_store_name().as_str()]).map_err(|e|e.to_string())?;
                c.execute("INSERT INTO settings(key,value) VALUES('currency',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [ui.get_settings_currency().as_str()]).map_err(|e|e.to_string())?;
                Ok(())
            })();
            match result { Ok(()) => set_status(&ui, "تنظیمات ذخیره شد"), Err(e) => set_status(&ui, format!("خطا: {e}")) }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }

    ui.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE contacts(id INTEGER PRIMARY KEY,name TEXT NOT NULL,kind TEXT NOT NULL DEFAULT 'customer',phone TEXT NOT NULL DEFAULT '',opening_balance INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL DEFAULT(datetime('now','localtime')));
             CREATE TABLE products(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL UNIQUE,barcode TEXT NOT NULL DEFAULT '',buy_price INTEGER NOT NULL DEFAULT 0,sell_price INTEGER NOT NULL DEFAULT 0,quantity INTEGER NOT NULL DEFAULT 0,min_quantity INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE invoices(id INTEGER PRIMARY KEY,number INTEGER NOT NULL UNIQUE,kind TEXT NOT NULL,contact_id INTEGER,contact_name TEXT NOT NULL,invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),subtotal INTEGER NOT NULL,discount INTEGER NOT NULL DEFAULT 0,total INTEGER NOT NULL,paid INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',voided INTEGER NOT NULL DEFAULT 0 CHECK(voided IN (0,1)));
             CREATE TABLE invoice_items(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),product_id INTEGER,item_name TEXT NOT NULL,quantity INTEGER NOT NULL,unit_price INTEGER NOT NULL,line_total INTEGER NOT NULL);
             CREATE TABLE sales_returns(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),return_date TEXT NOT NULL DEFAULT(date('now','localtime')),amount INTEGER NOT NULL CHECK(amount>0),notes TEXT NOT NULL DEFAULT '');
             CREATE TABLE sales_return_items(id INTEGER PRIMARY KEY,return_id INTEGER NOT NULL REFERENCES sales_returns(id),invoice_item_id INTEGER NOT NULL REFERENCES invoice_items(id),product_id INTEGER NOT NULL REFERENCES products(id),item_name TEXT NOT NULL,quantity INTEGER NOT NULL CHECK(quantity>0),unit_price INTEGER NOT NULL,amount INTEGER NOT NULL CHECK(amount>0));
             CREATE TABLE sales_refunds(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),return_id INTEGER NOT NULL REFERENCES sales_returns(id),refund_date TEXT NOT NULL DEFAULT(date('now','localtime')),amount INTEGER NOT NULL CHECK(amount>0),method TEXT NOT NULL DEFAULT 'cash',notes TEXT NOT NULL DEFAULT '');
             CREATE TABLE payments(id INTEGER PRIMARY KEY,kind TEXT NOT NULL CHECK(kind IN('receipt','payment')),contact_id INTEGER,contact_name TEXT NOT NULL,amount INTEGER NOT NULL CHECK(amount>0),method TEXT NOT NULL DEFAULT 'cash',payment_date TEXT NOT NULL DEFAULT(date('now','localtime')),notes TEXT NOT NULL DEFAULT '');
             CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO contacts(name,kind) VALUES('Ali','customer');
             INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity) VALUES('Pen','PEN',10,20,10,2);"
        ).unwrap();
        c
    }

    fn scalar(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn persian_and_arabic_digits_parse_to_integer() {
        assert_eq!(parse_amount("۱۲۳٬۴۵۶").unwrap(), 123456);
        assert_eq!(parse_amount("١٢٣").unwrap(), 123);
    }

    #[test]
    fn rejects_invalid_and_negative_amounts() {
        assert!(parse_amount("abc").is_err());
        assert!(parse_amount("-۱۰").unwrap() < 0);
    }

    #[test]
    fn money_uses_persian_digits() {
        assert_eq!(fmt_number(12345), "۱۲٬۳۴۵");
    }

    #[test]
    fn invoice_search_normalizes_persian_and_arabic_digits() {
        assert_eq!(normalize_invoice_search("فاکتور ۱۲۳"), "فاکتور 123");
        assert_eq!(normalize_invoice_search("٤٥٦"), "456");
        assert_eq!(normalize_invoice_search("  Ali  "), "Ali");
    }

    #[test]
    fn sale_updates_stock_and_records_initial_receipt_atomically() {
        let mut c = test_db();
        save_invoice(&mut c, "sale", "Ali", "PEN", "2", "50", "0", "30", "cash", "").unwrap();
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 8);
        assert_eq!(scalar(&c, "SELECT total FROM invoices WHERE kind='sale'"), 100);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE kind='sale'"), 30);
        assert_eq!(scalar(&c, "SELECT SUM(amount) FROM payments WHERE kind='receipt'"), 30);
    }

    #[test]
    fn later_receipt_allocates_to_oldest_open_invoice() {
        let mut c = test_db();
        save_invoice(&mut c, "sale", "Ali", "PEN", "2", "50", "0", "30", "cash", "").unwrap();
        save_payment(&mut c, "receipt", "Ali", "۲۰", "cash", "").unwrap();
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE kind='sale'"), 50);
        assert_eq!(scalar(&c, "SELECT SUM(amount) FROM payments WHERE kind='receipt'"), 50);
        assert_eq!(scalar(&c, "SELECT total-paid FROM invoices WHERE kind='sale'"), 50);
    }

    #[test]
    fn insufficient_stock_rolls_back_invoice_and_payment() {
        let mut c = test_db();
        assert!(save_invoice(&mut c, "sale", "Ali", "PEN", "99", "20", "0", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM payments"), 0);
    }

    #[test]
    fn purchase_increases_stock_and_updates_buy_price() {
        let mut c = test_db();
        c.execute("INSERT INTO contacts(name,kind) VALUES('Vendor','supplier')", []).unwrap();
        save_invoice(&mut c, "purchase", "Vendor", "PEN", "5", "12", "0", "0", "cash", "").unwrap();
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 15);
        assert_eq!(scalar(&c, "SELECT buy_price FROM products WHERE code='PEN'"), 12);
        assert_eq!(scalar(&c, "SELECT total FROM invoices WHERE kind='purchase'"), 60);
    }

    #[test]
    fn anonymous_unpaid_sale_is_rejected_without_side_effects() {
        let mut c = test_db();
        assert!(save_invoice(&mut c, "sale", "", "PEN", "1", "20", "0", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
    }

    #[test]
    fn receipt_overpayment_is_rejected_without_partial_write() {
        let mut c = test_db();
        save_invoice(&mut c, "sale", "Ali", "PEN", "1", "20", "0", "0", "cash", "").unwrap();
        assert!(save_payment(&mut c, "receipt", "Ali", "21", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM payments"), 0);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE kind='sale'"), 0);
    }


    #[test]
    fn multi_row_sale_records_all_lines_and_updates_stock() {
        let mut c = test_db();
        c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity) VALUES('Cable','CAB',30,60,5,1)", []).unwrap();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 50\nCAB | 3 | 60", "10", "100", "cash", "").unwrap();
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoice_items"), 2);
        assert_eq!(scalar(&c, "SELECT subtotal FROM invoices WHERE kind='sale'"), 280);
        assert_eq!(scalar(&c, "SELECT total FROM invoices WHERE kind='sale'"), 270);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 8);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='CAB'"), 2);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE kind='sale'"), 100);
    }

    #[test]
    fn multi_row_sale_rolls_back_everything_when_later_row_has_insufficient_stock() {
        let mut c = test_db();
        c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity) VALUES('Cable','CAB',30,60,1,1)", []).unwrap();
        assert!(save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 50\nCAB | 2 | 60", "0", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoice_items"), 0);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM payments"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='CAB'"), 1);
    }

    #[test]
    fn multi_row_invoice_rejects_malformed_rows_without_side_effects() {
        let mut c = test_db();
        assert!(save_invoice_multi(&mut c, "sale", "Ali", "PEN | 1", "0", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
    }


    #[test]
    fn invoice_details_show_all_lines_and_balance() {
        let mut c = test_db();
        c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity) VALUES('Cable','CAB',30,60,5,1)", []).unwrap();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 50\nCAB | 1 | 60", "10", "20", "cash", "test detail").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        let details = invoice_details(&c, id).unwrap();
        assert!(details.contains("PEN"));
        assert!(details.contains("Cable"));
        assert!(details.contains("جمع: ۱۶۰ تومان"));
        assert!(details.contains("تخفیف: ۱۰ تومان"));
        assert!(details.contains("مبلغ نهایی: ۱۵۰ تومان"));
        assert!(details.contains("مانده: ۱۳۰ تومان"));
        assert!(details.contains("test detail"));
    }


    #[test]
    fn partial_sale_return_restores_stock_and_reduces_invoice_total() {
        let mut c = test_db();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 4 | 20", "0", "20", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        let (credit,refund) = return_sale_items(&mut c, id, "PEN | 1", "cash").unwrap();
        assert_eq!(credit, 20);
        assert_eq!(refund, 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 7);
        assert_eq!(scalar(&c, "SELECT total FROM invoices WHERE id=1"), 60);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE id=1"), 20);
        assert_eq!(scalar(&c, "SELECT SUM(quantity) FROM sales_return_items"), 1);
    }

    #[test]
    fn partial_sale_return_rejects_over_return_atomically() {
        let mut c = test_db();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 20", "0", "30", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        let result = return_sale_items(&mut c, id, "PEN | 1", "cash");
        assert_eq!(result.unwrap(), (20,10));
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 9);
        assert_eq!(scalar(&c, "SELECT total FROM invoices WHERE id=1"), 20);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM sales_returns"), 1);
        assert_eq!(scalar(&c, "SELECT amount FROM sales_refunds"), 10);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE id=1"), 20);
        assert!(return_sale_items(&mut c, id, "PEN | 3", "cash").is_err());
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 9);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM sales_returns"), 1);
    }

    #[test]
    fn partial_sale_return_refund_tracks_method_and_reduces_paid_balance() {
        let mut c = test_db();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 20", "0", "30", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        let (credit, refund) = return_sale_items(&mut c, id, "PEN | 1", "bank transfer").unwrap();
        assert_eq!((credit, refund), (20, 10));
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM sales_refunds WHERE method='bank transfer'"), 1);
        assert_eq!(scalar(&c, "SELECT paid FROM invoices WHERE id=1"), 20);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 9);
    }

    #[test]
    fn void_sale_restores_stock_and_removes_its_initial_receipt() {
        let mut c = test_db();
        save_invoice(&mut c, "sale", "Ali", "PEN", "2", "50", "0", "30", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        void_invoice(&mut c, id).unwrap();
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
        assert_eq!(scalar(&c, "SELECT voided FROM invoices WHERE id=1"), 1);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM payments WHERE kind='receipt'"), 0);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices WHERE voided=0"), 0);
    }

    #[test]
    fn void_purchase_is_rejected_if_stock_was_already_sold() {
        let mut c = test_db();
        c.execute("INSERT INTO contacts(name,kind) VALUES('Vendor','supplier')", []).unwrap();
        save_invoice(&mut c, "purchase", "Vendor", "PEN", "5", "15", "0", "0", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        c.execute("UPDATE products SET quantity=2 WHERE code='PEN'", []).unwrap();
        assert!(void_invoice(&mut c, id).is_err());
        assert_eq!(scalar(&c, "SELECT voided FROM invoices WHERE id=1"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 2);
    }

    #[test]
    fn void_sale_is_rejected_after_later_receipt_allocation() {
        let mut c = test_db();
        save_invoice(&mut c, "sale", "Ali", "PEN", "2", "50", "0", "10", "cash", "").unwrap();
        save_payment(&mut c, "receipt", "Ali", "20", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        assert!(void_invoice(&mut c, id).is_err());
        assert_eq!(scalar(&c, "SELECT voided FROM invoices WHERE id=1"), 0);
    }

    #[test]
    fn duplicate_contact_and_product_code_are_rejected() {
        let c = test_db();
        assert!(save_contact(&c, "Ali", "customer", "", "0").is_err());
        assert!(save_product(&c, "Another pen", "PEN", "", "1", "1", "2", "0").is_err());
    }
}
