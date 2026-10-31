use rusqlite::{params, Connection, OptionalExtension};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

slint::include_modules!();

#[cfg(test)]
fn migrate_database(c: &mut Connection) -> Result<(), String> {
    nabz_core::initialize(c)
}

fn open_database() -> Result<Connection, String> {
    nabz_core::open_database()
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
    nabz_core::normalize_contact_kind(raw).ok_or_else(|| "نوع طرف‌حساب معتبر نیست".into())
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
    let history = {
        let mut st=c.prepare("SELECT product_name,previous_quantity,new_quantity,delta,reason,created_at FROM inventory_adjustments ORDER BY id DESC LIMIT 5").map_err(|e|e.to_string())?;
        let rows=st.query_map([],|r|Ok(format!("{}: {} ← {} ({:+}) · {} · {}",r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?))).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?.join("\n")
    };
    ui.set_inventory_adjustment_history(if history.is_empty(){"هنوز سابقه‌ای ثبت نشده است.".into()}else{history.into()});
    let valuation = nabz_core::accounting::inventory_valuation(c)?;
    let total_units = valuation.iter().try_fold(0_i64, |sum, p| sum.checked_add(p.quantity))
        .ok_or("جمع تعداد موجودی بیش از ظرفیت عددی است")?;
    let total_value = valuation.iter().try_fold(0_i64, |sum, p| sum.checked_add(p.stock_value))
        .ok_or("ارزش کل انبار بیش از ظرفیت عددی است")?
    let low_stock = valuation.iter().filter(|p| p.quantity <= p.min_quantity).count();
    let zero_stock = valuation.iter().filter(|p| p.quantity == 0).count();
    ui.set_inventory_report_summary(format!(
        "تعداد کالا: {} | تعداد موجود: {} | نیازمند تأمین: {} | بدون موجودی: {} | ارزش انبار: {}",
        fmt_number(valuation.len() as i64), fmt_number(total_units), fmt_number(low_stock as i64),
        fmt_number(zero_stock as i64), money(total_value)
    ).into());
    let movements = nabz_core::accounting::inventory_movements(c)?;
    let movement_text = movements.iter().take(12).map(|m| format!(
        "{} · {} · {} · {} ({:+}) · مانده {}",
        m.movement_date, m.product_name, m.movement_type, m.reference,
        m.quantity_delta, fmt_number(m.balance_after)
    )).collect::<Vec<_>>().join("\n");
    ui.set_inventory_report_movements(if movement_text.is_empty() {
        "هنوز گردش انباری ثبت نشده است.".into()
    } else { movement_text.into() });
    let profit = nabz_core::accounting::profit_loss_report(c, "", "")?;
    ui.set_profit_loss_summary(format!(
        "فروش خالص: {} | بهای تمام‌شده: {} | سود ناخالص: {} | هزینه‌ها: {} | سود خالص: {} | فروش‌های بدون بهای ثبت‌شده: {}",
        money(profit.net_sales), money(profit.cost_of_goods_sold), money(profit.gross_profit),
        money(profit.expenses), money(profit.net_profit), money(profit.uncosted_sales)
    ).into());
    ui.set_contacts_count(q("SELECT COUNT(*) FROM contacts")? as i32);
    ui.set_products_count(q("SELECT COUNT(*) FROM products")? as i32);
    ui.set_invoices_count(q("SELECT COUNT(*) FROM invoices WHERE voided=0")? as i32);

    let mut contact_rows = Vec::new();
    {
        let mut st = c.prepare("SELECT id,name,kind,phone,opening_balance FROM contacts WHERE (?1='' OR name LIKE '%'||?1||'%' OR phone LIKE '%'||?1||'%' OR kind LIKE '%'||?1||'%') ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
        let rows = st.query_map([ui.get_contact_search().trim()], |r| Ok(ContactRow {
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
        let mut st = c.prepare("SELECT id,name,code,quantity,sell_price,min_quantity FROM products WHERE (?1='' OR name LIKE '%'||?1||'%' OR code LIKE '%'||?1||'%' OR barcode LIKE '%'||?1||'%') ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
        let rows = st.query_map([ui.get_product_search().trim()], |r| {
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
        let mut st = c.prepare("SELECT id,contact_name,kind,amount,method,payment_date FROM payments WHERE (?1='' OR contact_name LIKE '%'||?1||'%' OR method LIKE '%'||?1||'%' OR notes LIKE '%'||?1||'%' OR payment_date LIKE '%'||?1||'%') ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
        let rows = st.query_map([ui.get_payment_search().trim()], |r| {
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
    let kind = normalize_kind(kind)?;
    let balance = parse_amount(balance_text)?;
    nabz_core::accounting::create_contact(c, &nabz_core::accounting::ContactInput {
        name: name.to_owned(), kind: kind.to_owned(), phone: phone.to_owned(),
        opening_balance: balance, notes: String::new(),
    }).map(|_| ())
}


fn update_contact(c: &Connection, id: i64, name: &str, phone: &str) -> Result<(), String> {
    nabz_core::accounting::update_contact(c, id, name, phone)
}

fn update_product(c: &Connection, id: i64, name: &str, code: &str, barcode: &str, buy: &str, sell: &str, minimum: &str) -> Result<(), String> {
    let input = nabz_core::accounting::ProductInput {
        name: name.to_owned(), code: code.to_owned(), barcode: barcode.to_owned(),
        buy_price: parse_amount(buy)?, sell_price: parse_amount(sell)?, quantity: 0,
        min_quantity: parse_amount(minimum)?,
    };
    nabz_core::accounting::update_product(c, id, &input)
}

fn save_product(c: &Connection, name: &str, code: &str, barcode: &str, qty: &str, buy: &str, sell: &str, minimum: &str) -> Result<(), String> {
    nabz_core::accounting::create_product(c, &nabz_core::accounting::ProductInput {
        name: name.to_owned(), code: code.to_owned(), barcode: barcode.to_owned(),
        quantity: parse_amount(qty)?, buy_price: parse_amount(buy)?, sell_price: parse_amount(sell)?,
        min_quantity: parse_amount(minimum)?,
    }).map(|_| ())
}

fn save_invoice(c: &mut Connection, kind: &str, contact: &str, item: &str, qty_text: &str, price_text: &str, discount_text: &str, paid_text: &str, method: &str, notes: &str) -> Result<(), String> {
    nabz_core::accounting::create_invoice(c, &nabz_core::accounting::InvoiceInput {
        kind: kind.to_owned(), contact_name: contact.to_owned(), date: String::new(),
        discount: parse_amount(discount_text)?, paid: parse_amount(paid_text)?,
        method: method.to_owned(), notes: notes.to_owned(),
        lines: vec![nabz_core::accounting::InvoiceLineInput {
            product_lookup: item.to_owned(), quantity: parse_amount(qty_text)?,
            unit_price: parse_amount(price_text)?,
        }],
    }).map(|_| ())
}

fn save_invoice_multi(c: &mut Connection, kind: &str, contact: &str, rows_text: &str, discount_text: &str, paid_text: &str, method: &str, notes: &str) -> Result<(), String> {
    let discount = parse_amount(discount_text)?;
    let paid = parse_amount(paid_text)?;
    if discount < 0 || paid < 0 {
        return Err("تخفیف و مبلغ دریافتی نمی‌توانند منفی باشند".into());
    }
    let mut lines = Vec::new();
    for (line_no, raw) in rows_text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() != 3 || parts[0].is_empty() {
            return Err(format!("ردیف {}: قالب باید «کد یا نام کالا | تعداد | قیمت واحد» باشد", line_no + 1));
        }
        let quantity = parse_amount(parts[1]).map_err(|e| format!("ردیف {}: {e}", line_no + 1))?;
        let unit_price = parse_amount(parts[2]).map_err(|e| format!("ردیف {}: {e}", line_no + 1))?;
        if quantity <= 0 || unit_price < 0 {
            return Err(format!("ردیف {}: تعداد باید مثبت و قیمت نامنفی باشد", line_no + 1));
        }
        lines.push(nabz_core::accounting::InvoiceLineInput {
            product_lookup: parts[0].to_owned(), quantity, unit_price,
        });
    }
    nabz_core::accounting::create_invoice(c, &nabz_core::accounting::InvoiceInput {
        kind: kind.to_owned(), contact_name: contact.to_owned(), date: String::new(),
        discount, paid, method: method.to_owned(), notes: notes.to_owned(), lines,
    }).map(|_| ())
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
    let mut refund_st=c.prepare("SELECT refund_date,amount,method FROM sales_refunds WHERE invoice_id=?1 ORDER BY id").map_err(|e|e.to_string())?;
    let refunds=refund_st.query_map([invoice_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    if !refunds.is_empty() {
        out.push_str("بازپرداخت‌های ثبت‌شده:\\n");
        for (date,amount,method) in refunds { out.push_str(&format!("{} · {} · {}\\n",date,money(amount),method)); }
    }
    out.push_str(&format!(
        "جمع: {}\\nتخفیف: {}\\nمبلغ نهایی: {}\\nپرداخت‌شده: {}\\nمانده: {}\\nشرح: {}",
        money(header.4), money(header.5), money(header.6), money(header.7),
        money(header.6.saturating_sub(header.7)), if header.8.trim().is_empty() { "—" } else { header.8.as_str() }
    ));
    Ok(out)
}

fn return_sale_items(c: &mut Connection, invoice_id: i64, rows_text: &str, refund_method: &str) -> Result<(i64,i64), String> {
    let mut lines = Vec::new();
    for (line_no, raw) in rows_text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() != 2 || parts[0].is_empty() {
            return Err(format!("ردیف {}: قالب باید «کد یا نام کالا | تعداد برگشتی» باشد", line_no + 1));
        }
        lines.push(nabz_core::accounting::ReturnLineInput {
            product_lookup: parts[0].to_owned(),
            quantity: parse_amount(parts[1])?,
        });
    }
    let kind:String=c.query_row("SELECT kind FROM invoices WHERE id=?1 AND voided=0",[invoice_id],|r|r.get(0)).map_err(|e|e.to_string())?;
    if kind=="purchase" { nabz_core::accounting::return_purchase_items(c,invoice_id,&lines,refund_method) }
    else { nabz_core::accounting::return_sale_items(c,invoice_id,&lines,refund_method) }
}

fn void_invoice(c: &mut Connection, invoice_id: i64) -> Result<(), String> {
    nabz_core::accounting::void_invoice(c, invoice_id)
}

fn save_payment(c: &mut Connection, kind: &str, contact: &str, amount_text: &str, method: &str, notes: &str) -> Result<(), String> {
    nabz_core::accounting::record_payment(c, &nabz_core::accounting::PaymentInput {
        kind: kind.to_owned(), contact_name: contact.to_owned(), amount: parse_amount(amount_text)?,
        method: method.to_owned(), date: String::new(), notes: notes.to_owned(),
    }).map(|_| ())
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
            let result = if ui.get_edit_contact_id() > 0 { update_contact(&db.borrow(), ui.get_edit_contact_id() as i64, &ui.get_contact_name(), &ui.get_contact_phone()) } else { save_contact(&db.borrow(), &ui.get_contact_name(), &ui.get_contact_kind(), &ui.get_contact_phone(), &ui.get_contact_balance()) };
            match result {
                Ok(()) => { ui.set_contact_name("".into()); ui.set_contact_phone("".into()); ui.set_contact_balance("0".into()); ui.set_edit_contact_id(0); set_status(&ui, "اطلاعات طرف‌حساب ذخیره شد"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak=ui.as_weak(); let db=db.clone();
        ui.on_adjust_inventory_requested(move |id| if let Some(ui)=weak.upgrade() {
            let current=db.borrow().query_row("SELECT quantity FROM products WHERE id=?1",[id as i64],|r|r.get::<_,i64>(0));
            match current {
                Ok(quantity)=>{ui.set_adjustment_product_id(id);ui.set_adjustment_new_quantity(quantity.to_string().into());ui.set_adjustment_reason("انبارگردانی".into());set_status(&ui,format!("موجودی فعلی کالا {} است؛ مقدار واقعی جدید و علت را وارد کنید",fmt_number(quantity)));},
                Err(e)=>set_status(&ui,format!("بارگذاری موجودی ناموفق بود: {e}"))
            }
        });
    }
    {
        let weak=ui.as_weak(); let db=db.clone();
        ui.on_save_inventory_adjustment_requested(move || if let Some(ui)=weak.upgrade() {
            let result=nabz_core::accounting::adjust_inventory(&mut db.borrow_mut(),ui.get_adjustment_product_id() as i64,parse_amount(&ui.get_adjustment_new_quantity()).unwrap_or(-1),&ui.get_adjustment_reason());
            match result {
                Ok(change)=>{set_status(&ui,format!("موجودی اصلاح شد: {} به {}؛ اختلاف {}؛ علت: {}",fmt_number(change.previous_quantity),fmt_number(change.new_quantity),fmt_number(change.delta),change.reason));ui.set_adjustment_product_id(0);ui.set_adjustment_new_quantity("0".into());ui.set_adjustment_reason("انبارگردانی".into());},
                Err(e)=>set_status(&ui,format!("اصلاح موجودی انجام نشد: {e}"))
            }
            if let Err(e)=refresh_ui(&ui,&db.borrow()){set_status(&ui,format!("خطای تازه‌سازی: {e}"));}
        });
    }
    {
        let weak = ui.as_weak(); let db = db.clone();
        ui.on_save_product_requested(move || if let Some(ui) = weak.upgrade() {
            let result = if ui.get_edit_product_id() > 0 { update_product(&db.borrow(), ui.get_edit_product_id() as i64, &ui.get_product_name(), &ui.get_product_code(), &ui.get_product_barcode(), &ui.get_product_buy_price(), &ui.get_product_sell_price(), &ui.get_product_minimum()) } else { save_product(&db.borrow(), &ui.get_product_name(), &ui.get_product_code(), &ui.get_product_barcode(), &ui.get_product_quantity(), &ui.get_product_buy_price(), &ui.get_product_sell_price(), &ui.get_product_minimum()) };
            match result {
                Ok(()) => { ui.set_product_name("".into()); ui.set_product_code("".into()); ui.set_product_barcode("".into()); ui.set_product_quantity("0".into()); ui.set_product_buy_price("0".into()); ui.set_product_sell_price("0".into()); ui.set_product_minimum("0".into()); ui.set_edit_product_id(0); set_status(&ui, "اطلاعات کالا ذخیره شد"); }
                Err(e) => set_status(&ui, format!("خطا: {e}")),
            }
            if let Err(e) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("خطای تازه‌سازی: {e}")); }
        });
    }
    {
        let weak=ui.as_weak(); let db=db.clone();
        ui.on_edit_contact_requested(move |id| if let Some(ui)=weak.upgrade() {
            let result=db.borrow().query_row("SELECT name,phone FROM contacts WHERE id=?1",[id as i64],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)));
            match result { Ok((name,phone)) => { ui.set_edit_contact_id(id); ui.set_contact_name(name.into()); ui.set_contact_phone(phone.into()); set_status(&ui,"طرف‌حساب برای ویرایش بارگذاری شد؛ نوع و ماندهٔ اولیه حفظ می‌شوند"); }, Err(e)=>set_status(&ui,format!("خطا در بارگذاری طرف‌حساب: {e}")) }
        });
    }
    {
        let weak=ui.as_weak(); let db=db.clone();
        ui.on_edit_product_requested(move |id| if let Some(ui)=weak.upgrade() {
            let result=db.borrow().query_row("SELECT name,code,barcode,buy_price,sell_price,min_quantity,quantity FROM products WHERE id=?1",[id as i64],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?)));
            match result { Ok((name,code,barcode,buy,sell,min,qty)) => { ui.set_edit_product_id(id); ui.set_product_name(name.into()); ui.set_product_code(code.into()); ui.set_product_barcode(barcode.into()); ui.set_product_buy_price(buy.to_string().into()); ui.set_product_sell_price(sell.to_string().into()); ui.set_product_minimum(min.to_string().into()); ui.set_product_quantity(qty.to_string().into()); set_status(&ui,"کالا برای ویرایش بارگذاری شد؛ موجودی فعلی حفظ می‌شود"); }, Err(e)=>set_status(&ui,format!("خطا در بارگذاری کالا: {e}")) }
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
        ui.on_invoice_return_requested(move |invoice_id, rows_text, refund_method| if let Some(ui) = weak.upgrade() {
            match return_sale_items(&mut db.borrow_mut(), invoice_id as i64, &rows_text, &refund_method) {
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
        ui.on_save_expense_requested(move || if let Some(ui) = weak.upgrade() {
            let result = nabz_core::accounting::create_expense(&db.borrow(), &nabz_core::accounting::ExpenseInput {
                category: ui.get_expense_category().to_string(),
                description: ui.get_expense_description().to_string(),
                amount: parse_amount(&ui.get_expense_amount()).unwrap_or(0),
                date: String::new(),
            });
            match result {
                Ok(_) => { ui.set_expense_description("".into()); ui.set_expense_amount("".into()); set_status(&ui, "هزینه ثبت شد"); }
                Err(error) => set_status(&ui, format!("ثبت هزینه ناموفق بود: {error}")),
            }
            if let Err(error) = refresh_ui(&ui, &db.borrow()) { set_status(&ui, format!("تازه‌سازی ناموفق بود: {error}")); }
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

    {
        let weak = ui.as_weak();
        ui.on_switch_ui_requested(move || {
            match nabz_core::launch_frontend(nabz_core::Frontend::Tauri) {
                Ok(_child) => {
                    if let Err(error) = slint::quit_event_loop() {
                        if let Some(ui) = weak.upgrade() {
                            set_status(&ui, format!("رابط مقصد اجرا شد؛ بستن رابط فعلی ناموفق بود: {error}"));
                        }
                    }
                }
                Err(error) => {
                    if let Some(ui) = weak.upgrade() {
                        set_status(&ui, error);
                    }
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let db = db.clone();
        ui.on_create_backup_requested(move || if let Some(ui) = weak.upgrade() {
            match nabz_core::create_automatic_backup(&db.borrow()) {
                Ok(path) => set_status(&ui, format!("پشتیبان کامل ذخیره شد: {}", path.display())),
                Err(error) => set_status(&ui, format!("ساخت پشتیبان ناموفق بود: {error}")),
            }
        });
    }
    {
        let weak = ui.as_weak();
        let db = db.clone();
        ui.on_restore_backup_requested(move || if let Some(ui) = weak.upgrade() {
            match nabz_core::restore_latest_backup(&mut db.borrow_mut()) {
                Ok(path) => {
                    set_status(&ui, format!("بازیابی کامل شد: {}", path.display()));
                    if let Err(error) = refresh_ui(&ui, &db.borrow()) {
                        set_status(&ui, format!("بازیابی انجام شد؛ تازه‌سازی ناموفق بود: {error}"));
                    }
                }
                Err(error) => set_status(&ui, format!("بازیابی ناموفق بود: {error}")),
            }
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
             CREATE TABLE invoice_items(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),product_id INTEGER,item_name TEXT NOT NULL,quantity INTEGER NOT NULL,unit_price INTEGER NOT NULL,line_total INTEGER NOT NULL,unit_cost INTEGER);
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
        assert!(details.contains("Pen"));
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
    fn void_sale_is_rejected_after_partial_return_without_double_restoring_stock() {
        let mut c = test_db();
        save_invoice_multi(&mut c, "sale", "Ali", "PEN | 2 | 20", "0", "0", "cash", "").unwrap();
        let id = scalar(&c, "SELECT id FROM invoices LIMIT 1");
        return_sale_items(&mut c, id, "PEN | 1", "cash").unwrap();
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 9);
        assert!(void_invoice(&mut c, id).is_err());
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 9);
        assert_eq!(scalar(&c, "SELECT voided FROM invoices WHERE id=1"), 0);
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

    #[test]
    fn schema_migration_adds_returns_and_voided_atomically_and_is_idempotent() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE invoices(id INTEGER PRIMARY KEY,number INTEGER NOT NULL UNIQUE,kind TEXT NOT NULL,contact_id INTEGER,contact_name TEXT NOT NULL,invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),subtotal INTEGER NOT NULL,discount INTEGER NOT NULL DEFAULT 0,total INTEGER NOT NULL,paid INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '');
             CREATE TABLE products(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL UNIQUE);
             CREATE TABLE invoice_items(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),item_name TEXT NOT NULL);"
        ).unwrap();
        c.execute("INSERT INTO invoices(id,number,kind,contact_name,subtotal,total) VALUES(1,1,'sale','Legacy',100,100)", []).unwrap();
        migrate_database(&mut c).unwrap();
        assert_eq!(scalar(&c, "SELECT voided FROM invoices WHERE id=1"), 0);
        assert_eq!(scalar(&c, "PRAGMA user_version"), 1);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('sales_returns','sales_return_items','sales_refunds')"), 3);
        migrate_database(&mut c).unwrap();
        assert_eq!(scalar(&c, "PRAGMA user_version"), 1);
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 1);
    }

    #[test]
    fn customer_opening_balance_is_reduced_before_invoice_allocations() {
        let mut c = test_db();
        save_contact(&c, "Opening customer", "customer", "", "100").unwrap();
        save_payment(&mut c, "receipt", "Opening customer", "40", "cash", "").unwrap();
        assert_eq!(scalar(&c, "SELECT opening_balance FROM contacts WHERE name='Opening customer'"), 60);
        assert_eq!(scalar(&c, "SELECT amount FROM payments WHERE contact_name='Opening customer'"), 40);
    }

    #[test]
    fn supplier_opening_balance_is_reduced_by_payment() {
        let mut c = test_db();
        save_contact(&c, "Opening supplier", "supplier", "", "250").unwrap();
        save_payment(&mut c, "payment", "Opening supplier", "75", "bank", "").unwrap();
        assert_eq!(scalar(&c, "SELECT opening_balance FROM contacts WHERE name='Opening supplier'"), 175);
        assert_eq!(scalar(&c, "SELECT amount FROM payments WHERE kind='payment'"), 75);
    }

    #[test]
    fn both_role_contact_is_allowed_with_zero_opening_balance() {
        let c = test_db();
        save_contact(&c, "Dual role", "both", "", "0").unwrap();
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM contacts WHERE name='Dual role' AND kind='both'"), 1);
        assert!(save_contact(&c, "Dual role with balance", "both", "", "10").is_err());
    }

    #[test]
    fn receipt_and_payment_reject_opposite_contact_roles_without_writes() {
        let mut c = test_db();
        c.execute("INSERT INTO contacts(name,kind) VALUES('Vendor','supplier')", []).unwrap();
        assert!(save_payment(&mut c, "receipt", "Vendor", "1", "cash", "").is_err());
        assert!(save_payment(&mut c, "payment", "Ali", "1", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM payments"), 0);
    }

    #[test]
    fn invoice_discount_cannot_exceed_subtotal_and_rolls_back_stock() {
        let mut c = test_db();
        assert!(save_invoice(&mut c, "sale", "Ali", "PEN", "2", "50", "101", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
    }

    #[test]
    fn multi_row_invoice_rejects_integer_overflow_without_side_effects() {
        let mut c = test_db();
        assert!(save_invoice_multi(&mut c, "sale", "Ali", "PEN | 9223372036854775807 | 2", "0", "0", "cash", "").is_err());
        assert_eq!(scalar(&c, "SELECT COUNT(*) FROM invoices"), 0);
        assert_eq!(scalar(&c, "SELECT quantity FROM products WHERE code='PEN'"), 10);
    }

}