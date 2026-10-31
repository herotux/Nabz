//! Shared accounting write operations used by the Tauri and Slint frontends.
//! Keep validation and persistence rules here; UI layers only parse user input.

use rusqlite::{params, Connection, OptionalExtension};

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

/// Calculate perpetual moving weighted-average cost with checked integer arithmetic.
fn weighted_average_cost(existing_quantity:i64,existing_unit_cost:i64,incoming_quantity:i64,incoming_unit_cost:i64)->Result<i64,String>{
    if existing_quantity<0||incoming_quantity<0||existing_unit_cost<0||incoming_unit_cost<0{
        return Err("مقادیر محاسبهٔ میانگین موزون نمی‌توانند منفی باشند".into());
    }
    let total_quantity=existing_quantity.checked_add(incoming_quantity).ok_or("تعداد موجودی بیش از ظرفیت عددی است")?;
    if total_quantity==0{return Ok(0);}
    let existing_value=existing_quantity.checked_mul(existing_unit_cost).ok_or("ارزش موجودی قبلی بیش از ظرفیت عددی است")?;
    let incoming_value=incoming_quantity.checked_mul(incoming_unit_cost).ok_or("ارزش خرید جدید بیش از ظرفیت عددی است")?;
    let total_value=existing_value.checked_add(incoming_value).ok_or("ارزش کل موجودی بیش از ظرفیت عددی است")?;
    total_value.checked_add(total_quantity/2).ok_or("گرد کردن بهای میانگین موزون بیش از ظرفیت عددی است").map(|v|v/total_quantity)
}

/// Allocate an invoice-level purchase discount proportionally, assigning
/// fractional remainder units to the lines with the largest remainders.
fn allocate_purchase_discount(line_totals:&[i64],discount:i64,subtotal:i64)->Result<Vec<i64>,String>{
    if discount<0||subtotal<0||discount>subtotal{return Err("تخفیف خرید معتبر نیست".into());}
    if line_totals.is_empty(){return Ok(Vec::new());}
    if discount==0||subtotal==0{return Ok(vec![0;line_totals.len()]);}
    let mut allocations=vec![0_i64;line_totals.len()];
    let mut remainders=Vec::with_capacity(line_totals.len());
    let mut allocated=0_i64;
    for (index,line_total) in line_totals.iter().enumerate(){
        if *line_total<0{return Err("مبلغ ردیف خرید نامعتبر است".into());}
        let weighted=discount.checked_mul(*line_total).ok_or("تخصیص تخفیف خرید بیش از ظرفیت عددی است")?;
        let base=weighted/subtotal;
        allocations[index]=base;
        allocated=allocated.checked_add(base).ok_or("جمع تخفیف‌های تخصیص‌یافته بیش از ظرفیت عددی است")?;
        remainders.push((index,weighted%subtotal));
    }
    let remaining=discount.checked_sub(allocated).ok_or("تخصیص تخفیف خرید نامعتبر است")?;
    remainders.sort_by(|a,b|b.1.cmp(&a.1).then_with(||a.0.cmp(&b.0)));
    for (index,_) in remainders.into_iter().take(remaining as usize){
        allocations[index]=allocations[index].checked_add(1).ok_or("تخفیف ردیف بیش از ظرفیت عددی است")?;
    }
    Ok(allocations)
}

pub fn create_product(connection: &Connection, input: &ProductInput) -> Result<i64, String> {
    validate_product(input)?;
    connection.execute(
        "INSERT INTO products(name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost) VALUES(?1,?2,?3,?4,?5,?6,?7,?4)",
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
pub struct InvoiceLineInput {
    /// Product name, internal code, or barcode.
    pub product_lookup: String,
    pub quantity: i64,
    pub unit_price: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvoiceInput {
    pub kind: String,
    pub contact_name: String,
    pub date: String,
    pub discount: i64,
    pub paid: i64,
    pub method: String,
    pub notes: String,
    pub lines: Vec<InvoiceLineInput>,
}

/// Create a sale or purchase, update inventory, and record an immediate sale
/// receipt atomically. This is the single invoice policy for both frontends.
pub fn create_invoice(connection: &mut Connection, input: &InvoiceInput) -> Result<i64, String> {
    if !["sale", "purchase"].contains(&input.kind.as_str()) {
        return Err("نوع فاکتور معتبر نیست".into());
    }
    if input.lines.is_empty() {
        return Err("حداقل یک ردیف کالا وارد کنید".into());
    }
    if input.discount < 0 || input.paid < 0 {
        return Err("تخفیف و مبلغ دریافتی نمی‌توانند منفی باشند".into());
    }

    let mut subtotal = 0_i64;
    let mut normalized = Vec::with_capacity(input.lines.len());
    for (index, line) in input.lines.iter().enumerate() {
        let lookup = line.product_lookup.trim();
        if lookup.is_empty() || line.quantity <= 0 || line.unit_price < 0 {
            return Err(format!("ردیف {}: کالا، تعداد مثبت و قیمت نامنفی الزامی است", index + 1));
        }
        let line_total = line.quantity.checked_mul(line.unit_price)
            .ok_or_else(|| format!("ردیف {}: مبلغ بیش از حد بزرگ است", index + 1))?;
        subtotal = subtotal.checked_add(line_total).ok_or("جمع فاکتور بیش از حد بزرگ است")?;
        normalized.push((lookup.to_owned(), line.quantity, line.unit_price, line_total));
    }
    if input.discount > subtotal {
        return Err("تخفیف از جمع فاکتور بیشتر است".into());
    }
    let total = subtotal.checked_sub(input.discount).ok_or("مبلغ نهایی نامعتبر است")?;
    let purchase_discounts = if input.kind=="purchase" {
        let line_totals=normalized.iter().map(|line|line.3).collect::<Vec<_>>();
        allocate_purchase_discount(&line_totals,input.discount,subtotal)?
    } else { vec![0;normalized.len()] };
    let paid = if input.kind == "sale" { input.paid } else { 0 };
    if paid > total {
        return Err("مبلغ دریافتی از مبلغ نهایی بیشتر است".into());
    }

    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let contact = input.contact_name.trim();
    let contact_id: Option<i64> = if contact.is_empty() {
        None
    } else {
        tx.query_row(
            "SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1",
            [contact], |row| row.get(0)
        ).optional().map_err(|e| e.to_string())?
    };
    if input.kind == "purchase" && contact_id.is_none() {
        return Err("برای خرید، تأمین‌کنندهٔ ثبت‌شده الزامی است".into());
    }
    if input.kind == "sale" && !contact.is_empty() && contact_id.is_none() {
        return Err("مشتری پیدا نشد؛ ابتدا طرف‌حساب را ثبت کنید".into());
    }
    if let Some(id) = contact_id {
        let role: String = tx.query_row("SELECT kind FROM contacts WHERE id=?1", [id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if input.kind == "sale" && role == "supplier" {
            return Err("برای فاکتور فروش، مشتری انتخاب کنید".into());
        }
        if input.kind == "purchase" && role == "customer" {
            return Err("برای فاکتور خرید، تأمین‌کننده انتخاب کنید".into());
        }
    }
    if input.kind == "sale" && paid < total && contact_id.is_none() {
        return Err("فروش گذری باید کامل تسویه شود".into());
    }

    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number),1000)+1 FROM invoices", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    let contact_label = if contact.is_empty() { "مشتری گذری" } else { contact };
    tx.execute(
        "INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes)
         VALUES(?1,?2,?3,?4,COALESCE(NULLIF(?5,''),date('now','localtime')),?6,?7,?8,?9,?10)",
        params![number, input.kind, contact_id, contact_label, input.date.trim(), subtotal,
            input.discount, total, paid, input.notes.trim()],
    ).map_err(|e| e.to_string())?;
    let invoice_id = tx.last_insert_rowid();

    for (line_index,(lookup, quantity, unit_price, line_total)) in normalized.into_iter().enumerate() {
        let line_discount=purchase_discounts[line_index];
        let net_line_cost=line_total.checked_sub(line_discount).ok_or("بهای خالص ردیف خرید نامعتبر است")?;
        let inventory_unit_cost=if input.kind=="purchase" {
            net_line_cost.checked_add(quantity/2).ok_or("گرد کردن بهای خرید بیش از ظرفیت عددی است")?/quantity
        } else { unit_price };
        let product: Option<(i64, i64, String, i64)> = tx.query_row(
            "SELECT id,quantity,name,average_cost FROM products
             WHERE name=?1 COLLATE NOCASE OR code=?1 OR barcode=?1 LIMIT 1",
            [&lookup], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        ).optional().map_err(|e| e.to_string())?;
        let (product_id, item_name, unit_cost) = if let Some((id, stock, name, average_cost)) = product {
            if input.kind == "sale" {
                if stock < quantity {
                    return Err(format!("موجودی «{lookup}» کافی نیست؛ موجودی فعلی: {stock}"));
                }
                let changed = tx.execute(
                    "UPDATE products SET quantity=quantity-?1 WHERE id=?2 AND quantity>=?1",
                    params![quantity, id],
                ).map_err(|e| e.to_string())?;
                if changed != 1 {
                    return Err(format!("موجودی «{lookup}» کافی نیست"));
                }
            } else {
                let next_quantity=stock.checked_add(quantity).ok_or_else(||format!("موجودی «{lookup}» بیش از ظرفیت عددی است"))?;
                let next_average=weighted_average_cost(stock,average_cost,quantity,inventory_unit_cost)?;
                tx.execute("UPDATE products SET quantity=?1,buy_price=?2,average_cost=?3 WHERE id=?4",
                    params![next_quantity,unit_price,next_average,id]).map_err(|e|e.to_string())?;
            }
            (id, name, if input.kind == "sale" { Some(average_cost) } else { None })
        } else if input.kind == "purchase" {
            let code = format!("AUTO-{}", tx.query_row(
                "SELECT COALESCE(MAX(id),0)+1 FROM products", [], |row| row.get::<_, i64>(0)
            ).map_err(|e| e.to_string())?);
            tx.execute(
                "INSERT INTO products(name,code,buy_price,sell_price,quantity,average_cost) VALUES(?1,?2,?3,?3,?4,?5)",
                params![lookup, code, unit_price, quantity, inventory_unit_cost],
            ).map_err(|e| e.to_string())?;
            (tx.last_insert_rowid(), lookup, None)
        } else {
            return Err(format!("کالای «{lookup}» در انبار پیدا نشد؛ ابتدا کالا را ثبت کنید"));
        };
        tx.execute(
            "INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![invoice_id, product_id, item_name, quantity, unit_price, line_total, unit_cost],
        ).map_err(|e| e.to_string())?;
    }

    if input.kind == "sale" && paid > 0 {
        tx.execute(
            "INSERT INTO payments(kind,contact_id,contact_name,amount,method,payment_date,notes)
             VALUES('receipt',?1,?2,?3,COALESCE(NULLIF(?4,''),'cash'),
                    COALESCE(NULLIF(?5,''),date('now','localtime')),?6)",
            params![contact_id, contact_label, paid, input.method.trim(), input.date.trim(),
                format!("دریافت بابت فاکتور فروش #{number}")],
        ).map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(invoice_id)
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnLineInput {
    pub product_lookup: String,
    pub quantity: i64,
}

/// Process a partial sale return, restore stock, and create any cash refund
/// atomically. The calculation and validation are shared by both UIs.
pub fn return_sale_items(
    connection: &mut Connection,
    invoice_id: i64,
    lines: &[ReturnLineInput],
    refund_method: &str,
) -> Result<(i64, i64), String> {
    if invoice_id <= 0 || lines.is_empty() {
        return Err("ابتدا فاکتور فروش و حداقل یک ردیف برگشت را انتخاب کنید".into());
    }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let invoice: (String, i64, i64, i64, i64) = tx.query_row(
        "SELECT kind,subtotal,discount,total,paid FROM invoices WHERE id=?1 AND voided=0",
        [invoice_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).optional().map_err(|e| e.to_string())?
        .ok_or("فاکتور پیدا نشد یا باطل شده است")?;
    if invoice.0 != "sale" {
        return Err("برگشت جزئی فقط برای فاکتور فروش مجاز است".into());
    }

    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();
    let mut gross_return = 0_i64;
    for (index, line) in lines.iter().enumerate() {
        let lookup = line.product_lookup.trim();
        if lookup.is_empty() || line.quantity <= 0 {
            return Err(format!("ردیف {}: کالا و تعداد برگشتی مثبت الزامی است", index + 1));
        }
        if !seen.insert(lookup.to_lowercase()) {
            return Err(format!("کالای «{lookup}» را فقط یک بار در فهرست برگشت وارد کنید"));
        }
        let matches = {
            let mut statement = tx.prepare(
                "SELECT ii.id,ii.product_id,ii.quantity,ii.unit_price,ii.item_name,ii.unit_cost
                 FROM invoice_items ii JOIN products p ON p.id=ii.product_id
                 WHERE ii.invoice_id=?1 AND
                 (ii.item_name=?2 COLLATE NOCASE OR p.code=?2 OR p.barcode=?2)
                 ORDER BY ii.id"
            ).map_err(|e| e.to_string())?;
            let rows = statement.query_map(params![invoice_id, lookup], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?))
            }).map_err(|e| e.to_string())?;
            let collected = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
            collected
        };
        if matches.len() != 1 {
            return Err(format!("کالای «{lookup}» در این فاکتور یکتا نیست یا پیدا نشد؛ از کد یکتا استفاده کنید"));
        }
        let (item_id, product_id, original_qty, unit_price, item_name, historical_cost) =
            matches.into_iter().next().expect("one match");
        if product_id <= 0 {
            return Err(format!("کالای «{item_name}» شناسهٔ انبار ندارد؛ برگشت ممکن نیست"));
        }
        let already: i64 = tx.query_row(
            "SELECT COALESCE(SUM(quantity),0) FROM sales_return_items WHERE invoice_item_id=?1",
            [item_id], |r| r.get(0)
        ).map_err(|e| e.to_string())?;
        let remaining = original_qty.checked_sub(already).ok_or("سابقهٔ برگشت نامعتبر است")?;
        if line.quantity > remaining {
            return Err(format!("برای «{item_name}» فقط {} عدد قابل برگشت است", remaining));
        }
        let amount = line.quantity.checked_mul(unit_price).ok_or("مبلغ برگشت بیش از حد بزرگ است")?;
        gross_return = gross_return.checked_add(amount).ok_or("جمع برگشت بیش از حد بزرگ است")?;
        normalized.push((item_id, product_id, line.quantity, unit_price, item_name, amount, historical_cost));
    }

    if invoice.1 <= 0 || gross_return > invoice.1 {
        return Err("مبلغ ردیف‌های برگشتی از جمع اولیهٔ فاکتور بیشتر است".into());
    }
    let credit = gross_return.checked_mul(invoice.3).ok_or("محاسبهٔ مبلغ برگشت بیش از حد بزرگ است")? / invoice.1;
    if credit <= 0 {
        return Err("مبلغ خالص برگشت صفر است؛ مقدار برگشت را بررسی کنید".into());
    }
    let new_subtotal = invoice.1 - gross_return;
    let new_total = invoice.3.checked_sub(credit).ok_or("مبلغ نهایی نامعتبر است")?;
    let refund = invoice.4.saturating_sub(new_total).max(0);
    let new_discount = new_subtotal - new_total;
    tx.execute(
        "INSERT INTO sales_returns(invoice_id,amount,notes) VALUES(?1,?2,?3)",
        params![invoice_id, credit, format!("برگشت جزئی فاکتور #{invoice_id}")],
    ).map_err(|e| e.to_string())?;
    let return_id = tx.last_insert_rowid();
    if refund > 0 {
        let method = if refund_method.trim().is_empty() { "cash" } else { refund_method.trim() };
        tx.execute(
            "INSERT INTO sales_refunds(invoice_id,return_id,amount,method,notes)
             VALUES(?1,?2,?3,?4,?5)",
            params![invoice_id, return_id, refund, method, format!("بازپرداخت بابت برگشت فاکتور #{invoice_id}")],
        ).map_err(|e| e.to_string())?;
    }
    for (item_id, product_id, quantity, unit_price, item_name, amount, historical_cost) in normalized {
        let (stock,average_cost):(i64,i64)=tx.query_row("SELECT quantity,average_cost FROM products WHERE id=?1",[product_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
        let next_quantity=stock.checked_add(quantity).ok_or("موجودی پس از برگشت بیش از ظرفیت عددی است")?;
        let next_average=weighted_average_cost(stock,average_cost,quantity,historical_cost.unwrap_or(average_cost))?;
        tx.execute("UPDATE products SET quantity=?1,average_cost=?2 WHERE id=?3",params![next_quantity,next_average,product_id]).map_err(|e|e.to_string())?;
        tx.execute(
            "INSERT INTO sales_return_items(return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![return_id, item_id, product_id, item_name, quantity, unit_price, amount],
        ).map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE invoices SET subtotal=?1,discount=?2,total=?3,paid=MIN(paid,?3) WHERE id=?4",
        params![new_subtotal, new_discount, new_total, invoice_id],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok((credit, refund))
}

/// Return purchased goods to a supplier, reduce the purchase invoice and
/// receive any amount already paid above the revised invoice total.
/// Inventory, invoice totals and the supplier-refund audit record are atomic.
pub fn return_purchase_items(
    connection: &mut Connection,
    invoice_id: i64,
    lines: &[ReturnLineInput],
    refund_method: &str,
) -> Result<(i64, i64), String> {
    if invoice_id <= 0 || lines.is_empty() {
        return Err("ابتدا فاکتور خرید و حداقل یک ردیف مرجوعی را انتخاب کنید".into());
    }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let invoice: (String, i64, i64, i64, i64) = tx.query_row(
        "SELECT kind,subtotal,discount,total,paid FROM invoices WHERE id=?1 AND voided=0",
        [invoice_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
    ).optional().map_err(|e| e.to_string())?.ok_or("فاکتور پیدا نشد یا باطل شده است")?;
    if invoice.0 != "purchase" { return Err("مرجوعی فقط برای فاکتور خرید مجاز است".into()); }

    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();
    let mut gross_return = 0_i64;
    for (index, line) in lines.iter().enumerate() {
        let lookup = line.product_lookup.trim();
        if lookup.is_empty() || line.quantity <= 0 {
            return Err(format!("ردیف {}: کالا و تعداد مرجوعی مثبت الزامی است", index + 1));
        }
        if !seen.insert(lookup.to_lowercase()) {
            return Err(format!("کالای «{lookup}» را فقط یک بار در فهرست مرجوعی وارد کنید"));
        }
        let matches = {
            let mut st = tx.prepare(
                "SELECT ii.id,ii.product_id,ii.quantity,ii.unit_price,ii.item_name
                 FROM invoice_items ii JOIN products p ON p.id=ii.product_id
                 WHERE ii.invoice_id=?1 AND (ii.item_name=?2 COLLATE NOCASE OR p.code=?2 OR p.barcode=?2)
                 ORDER BY ii.id"
            ).map_err(|e| e.to_string())?;
            let rows = st.query_map(params![invoice_id,lookup],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<i64>>(1)?.unwrap_or(0),r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?)))
                .map_err(|e|e.to_string())?;
            let collected = rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            collected
        };
        if matches.len()!=1 { return Err(format!("کالای «{lookup}» در این فاکتور یکتا نیست یا پیدا نشد؛ از کد یکتا استفاده کنید")); }
        let (item_id,product_id,original_qty,unit_price,item_name)=matches.into_iter().next().expect("one match");
        if product_id<=0 { return Err(format!("کالای «{item_name}» شناسهٔ انبار ندارد؛ مرجوعی ممکن نیست")); }
        let already:i64=tx.query_row("SELECT COALESCE(SUM(quantity),0) FROM purchase_return_items WHERE invoice_item_id=?1",[item_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let remaining=original_qty.checked_sub(already).ok_or("سابقهٔ مرجوعی نامعتبر است")?;
        if line.quantity>remaining { return Err(format!("برای «{item_name}» فقط {} عدد قابل مرجوعی است",remaining)); }
        let amount=line.quantity.checked_mul(unit_price).ok_or("مبلغ مرجوعی بیش از حد بزرگ است")?;
        gross_return=gross_return.checked_add(amount).ok_or("جمع مرجوعی بیش از حد بزرگ است")?;
        normalized.push((item_id,product_id,line.quantity,unit_price,item_name,amount));
    }
    if invoice.1<=0 || gross_return>invoice.1 { return Err("مبلغ ردیف‌های مرجوعی از جمع اولیهٔ فاکتور بیشتر است".into()); }
    let credit=gross_return.checked_mul(invoice.3).ok_or("محاسبهٔ مبلغ مرجوعی بیش از حد بزرگ است")?/invoice.1;
    if credit<=0 { return Err("مبلغ خالص مرجوعی صفر است؛ مقدار را بررسی کنید".into()); }
    let new_subtotal=invoice.1-gross_return;
    let new_total=invoice.3.checked_sub(credit).ok_or("مبلغ نهایی نامعتبر است")?;
    let supplier_refund=invoice.4.saturating_sub(new_total).max(0);
    let new_discount=new_subtotal-new_total;
    tx.execute("INSERT INTO purchase_returns(invoice_id,amount,notes) VALUES(?1,?2,?3)",params![invoice_id,credit,format!("مرجوعی فاکتور خرید #{invoice_id}")]).map_err(|e|e.to_string())?;
    let return_id=tx.last_insert_rowid();
    if supplier_refund>0 {
        let method=if refund_method.trim().is_empty(){"cash"}else{refund_method.trim()};
        tx.execute("INSERT INTO purchase_refunds(invoice_id,return_id,amount,method,notes) VALUES(?1,?2,?3,?4,?5)",params![invoice_id,return_id,supplier_refund,method,format!("دریافت وجه بابت مرجوعی خرید #{invoice_id}")]).map_err(|e|e.to_string())?;
    }
    for (item_id,product_id,quantity,unit_price,item_name,amount) in normalized {
        let changed=tx.execute("UPDATE products SET quantity=quantity-?1,average_cost=CASE WHEN quantity=?1 THEN 0 ELSE average_cost END WHERE id=?2 AND quantity>=?1",params![quantity,product_id]).map_err(|e|e.to_string())?;
        if changed==0 { return Err(format!("موجودی «{item_name}» برای مرجوعی کافی نیست؛ موجودی انبار تغییر نکرد")); }
        tx.execute("INSERT INTO purchase_return_items(return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![return_id,item_id,product_id,item_name,quantity,unit_price,amount]).map_err(|e|e.to_string())?;
    }
    tx.execute("UPDATE invoices SET subtotal=?1,discount=?2,total=?3,paid=MIN(paid,?3) WHERE id=?4",params![new_subtotal,new_discount,new_total,invoice_id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok((credit,supplier_refund))
}

/// Void an invoice only when linked returns and subsequent allocations cannot
/// be made inconsistent. Keep the invoice as an audit record.
pub fn void_invoice(connection: &mut Connection, invoice_id: i64) -> Result<(), String> {
    if invoice_id <= 0 { return Err("ابتدا یک فاکتور را انتخاب کنید".into()); }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let invoice: Option<(i64, String, i64, i64)> = tx.query_row(
        "SELECT number,kind,total,paid FROM invoices WHERE id=?1 AND voided=0",
        [invoice_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    ).optional().map_err(|e| e.to_string())?;
    let (number, kind, _total, paid) = invoice.ok_or("فاکتور پیدا نشد یا قبلاً باطل شده است")?;

    let returns: i64 = if kind == "sale" {
        tx.query_row("SELECT COUNT(*) FROM sales_returns WHERE invoice_id=?1", [invoice_id], |r| r.get(0))
    } else {
        tx.query_row("SELECT COUNT(*) FROM purchase_returns WHERE invoice_id=?1", [invoice_id], |r| r.get(0))
    }.map_err(|e| e.to_string())?;
    if returns > 0 {
        return Err("این فاکتور سابقهٔ برگشت/مرجوعی یا بازپرداخت دارد؛ ابطال مستقیم مجاز نیست".into());
    }
    let initial_receipt: i64 = if kind == "sale" {
        tx.query_row(
            "SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND notes=?1",
            [format!("دریافت بابت فاکتور فروش #{number}")], |r| r.get(0)
        ).map_err(|e| e.to_string())?
    } else { 0 };
    if kind == "sale" && paid != initial_receipt {
        return Err("این فاکتور پرداخت‌های تخصیص‌یافتهٔ بعدی دارد؛ ابتدا گردش‌های مرتبط را اصلاح کنید".into());
    }
    if kind == "purchase" && paid != 0 {
        return Err("برای این فاکتور خرید پرداخت تخصیص یافته است؛ ابتدا گردش مالی آن را اصلاح کنید".into());
    }
    if kind != "sale" && kind != "purchase" {
        return Err("نوع فاکتور ناشناخته است".into());
    }

    let items = {
        let mut statement = tx.prepare(
            "SELECT product_id,item_name,quantity,unit_cost FROM invoice_items WHERE invoice_id=?1 ORDER BY id"
        ).map_err(|e| e.to_string())?;
        let rows = statement.query_map([invoice_id], |r| Ok((
            r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?,
            r.get::<_, Option<i64>>(3)?
        ))).map_err(|e| e.to_string())?;
        let collected = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        collected
    };
    if items.is_empty() { return Err("فاکتور ردیف کالا ندارد؛ ابطال خودکار ممکن نیست".into()); }
    for (product_id, item_name, quantity, historical_cost) in items {
        if quantity <= 0 { return Err(format!("تعداد نامعتبر برای کالای «{item_name}»؛ ابطال لغو شد")); }
        if let Some(product_id) = product_id {
            let (stock,average_cost):(i64,i64)=tx.query_row("SELECT quantity,average_cost FROM products WHERE id=?1",[product_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?.ok_or(format!("کالای «{item_name}» حذف شده؛ ابطال امن ممکن نیست"))?;
            if kind=="sale" {
                let next=stock.checked_add(quantity).ok_or("موجودی پس از برگشت بیش از حد بزرگ می‌شود")?;
                let next_average=weighted_average_cost(stock,average_cost,quantity,historical_cost.unwrap_or(average_cost))?;
                tx.execute("UPDATE products SET quantity=?1,average_cost=?2 WHERE id=?3",params![next,next_average,product_id]).map_err(|e|e.to_string())?;
            } else {
                if stock<quantity{return Err(format!("موجودی «{item_name}» برای برگشت خرید کافی نیست؛ موجودی فعلی {} و مقدار برگشت {}",stock,quantity));}
                let next=stock-quantity;
                tx.execute("UPDATE products SET quantity=?1,average_cost=CASE WHEN ?1=0 THEN 0 ELSE average_cost END WHERE id=?2",params![next,product_id]).map_err(|e|e.to_string())?;
            }
        } else if kind == "purchase" {
            return Err(format!("کالای «{item_name}» شناسهٔ انبار ندارد؛ ابطال امن ممکن نیست"));
        }
    }
    if initial_receipt > 0 {
        tx.execute("DELETE FROM payments WHERE kind='receipt' AND notes=?1",
            [format!("دریافت بابت فاکتور فروش #{number}")]).map_err(|e| e.to_string())?;
    }
    tx.execute("UPDATE invoices SET voided=1 WHERE id=?1 AND voided=0", [invoice_id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryAdjustment {
    pub product_id: i64,
    pub previous_quantity: i64,
    pub new_quantity: i64,
    pub delta: i64,
    pub reason: String,
    pub created_at: String,
}

pub fn adjust_inventory(
    connection: &mut Connection,
    product_id: i64,
    new_quantity: i64,
    reason: &str,
) -> Result<InventoryAdjustment, String> {
    let reason = reason.trim();
    if product_id <= 0 || new_quantity < 0 || reason.chars().count() < 3 {
        return Err("کالا، موجودی جدید نامنفی و علت حداقل سه‌حرفی الزامی است".into());
    }
    let tx = connection.transaction().map_err(|e| e.to_string())?;
    let (name, previous): (String, i64) = tx.query_row(
        "SELECT name,quantity FROM products WHERE id=?1",
        [product_id], |r| Ok((r.get(0)?,r.get(1)?))
    ).optional().map_err(|e| e.to_string())?.ok_or("کالای انتخاب‌شده پیدا نشد")?;
    let delta = new_quantity.checked_sub(previous).ok_or("اختلاف موجودی بیش از حد بزرگ است")?;
    if delta == 0 { return Err("موجودی جدید با موجودی فعلی برابر است؛ تغییری برای ثبت وجود ندارد".into()); }
    let changed = tx.execute("UPDATE products SET quantity=?1,average_cost=CASE WHEN ?1=0 THEN 0 ELSE average_cost END WHERE id=?2 AND quantity=?3",
        params![new_quantity,product_id,previous]).map_err(|e|e.to_string())?;
    if changed != 1 {
        return Err("موجودی کالا هم‌زمان تغییر کرده است؛ فهرست را تازه‌سازی و دوباره تلاش کنید".into());
    }
    tx.execute(
        "INSERT INTO inventory_adjustments(product_id,product_name,previous_quantity,new_quantity,delta,reason)
         VALUES(?1,?2,?3,?4,?5,?6)",
        params![product_id,name,previous,new_quantity,delta,reason]
    ).map_err(|e|e.to_string())?;
    let created_at:String=tx.query_row(
        "SELECT created_at FROM inventory_adjustments WHERE id=last_insert_rowid()",[],|r|r.get(0)
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(InventoryAdjustment{product_id,previous_quantity:previous,new_quantity,delta,reason:reason.into(),created_at})
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryValuation {
    pub product_id: i64,
    pub product_name: String,
    pub code: String,
    pub quantity: i64,
    /// Most recent purchase price.
    pub buy_price: i64,
    /// Perpetual moving weighted-average cost used for valuation.
    pub average_cost: i64,
    pub stock_value: i64,
    pub min_quantity: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryMovement {
    pub product_id: i64,
    pub product_name: String,
    pub movement_date: String,
    pub movement_type: String,
    pub reference: String,
    pub quantity_delta: i64,
    pub balance_after: i64,
}

/// Current stock valuation for every product. Stock value uses the perpetual
/// moving weighted-average cost, with checked arithmetic to expose overflow.
pub fn inventory_valuation(connection: &Connection) -> Result<Vec<InventoryValuation>, String> {
    let mut statement = connection.prepare(
        "SELECT id,name,code,quantity,buy_price,average_cost,min_quantity
         FROM products ORDER BY name COLLATE NOCASE,id"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| {
        let quantity: i64 = row.get(3)?;
        let buy_price: i64 = row.get(4)?;
        let average_cost: i64 = row.get(5)?;
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?,
            row.get::<_, String>(2)?, quantity, buy_price, average_cost, row.get::<_, i64>(6)?))
    }).map_err(|e| e.to_string())?;
    let products = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    products.into_iter().map(|(product_id, product_name, code, quantity, buy_price, average_cost, min_quantity)| {
        let stock_value = quantity.checked_mul(average_cost)
            .ok_or_else(|| format!("ارزش موجودی کالای «{product_name}» بیش از ظرفیت عددی است"))?;
        Ok(InventoryValuation { product_id, product_name, code, quantity, buy_price, average_cost, stock_value, min_quantity })
    }).collect()
}

/// Build a chronological stock ledger from purchases, sales, both return types,
/// and manual inventory adjustments. Opening stock is derived from the current
/// quantity minus the complete movement sum, so legacy opening balances remain valid.
pub fn inventory_movements(connection: &Connection) -> Result<Vec<InventoryMovement>, String> {
    use std::collections::HashMap;
    let mut current = HashMap::<i64, i64>::new();
    {
        let mut statement = connection.prepare("SELECT id,quantity FROM products")
            .map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, quantity) = row.map_err(|e| e.to_string())?;
            current.insert(id, quantity);
        }
    }

    let sql = "
      SELECT movement_date,product_id,product_name,movement_type,reference,quantity_delta,sort_id
      FROM (
        SELECT i.invoice_date AS movement_date,ii.product_id AS product_id,ii.item_name AS product_name,
          CASE i.kind WHEN 'purchase' THEN 'خرید' ELSE 'فروش' END AS movement_type,
          'فاکتور #'||i.number AS reference,
          CASE i.kind WHEN 'purchase' THEN ii.quantity ELSE -ii.quantity END AS quantity_delta,
          i.id AS sort_id
        FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id
        WHERE i.voided=0 AND ii.product_id IS NOT NULL
        UNION ALL
        SELECT sr.return_date,sri.product_id,sri.item_name,'برگشت فروش',
          'برگشت فاکتور #'||sr.invoice_id,sri.quantity,sr.id
        FROM sales_return_items sri JOIN sales_returns sr ON sr.id=sri.return_id
        UNION ALL
        SELECT pr.return_date,pri.product_id,pri.item_name,'مرجوعی خرید',
          'مرجوعی فاکتور #'||pr.invoice_id,-pri.quantity,pr.id
        FROM purchase_return_items pri JOIN purchase_returns pr ON pr.id=pri.return_id
        UNION ALL
        SELECT created_at,product_id,product_name,'اصلاح موجودی',reason,delta,id
        FROM inventory_adjustments
      )
      ORDER BY movement_date,sort_id,product_id";
    let mut statement = connection.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok((
        row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?,
        row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?
    ))).map_err(|e| e.to_string())?;
    let raw = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let mut movement_totals = HashMap::<i64, i64>::new();
    for (_, product_id, _, _, _, delta) in &raw {
        let total = movement_totals.entry(*product_id).or_default();
        *total = total.checked_add(*delta).ok_or("جمع گردش موجودی بیش از ظرفیت عددی است")?;
    }
    let mut balances = HashMap::<i64, i64>::new();
    for (id, quantity) in &current {
        let moved = movement_totals.get(id).copied().unwrap_or(0);
        balances.insert(*id, quantity.checked_sub(moved).ok_or("محاسبه موجودی اول دوره نامعتبر است")?);
    }

    let mut result = Vec::with_capacity(raw.len());
    for (movement_date, product_id, product_name, movement_type, reference, quantity_delta) in raw {
        let balance = balances.entry(product_id).or_default();
        *balance = balance.checked_add(quantity_delta).ok_or("محاسبه مانده گردش موجودی بیش از ظرفیت عددی است")?;
        result.push(InventoryMovement {
            product_id, product_name, movement_date, movement_type, reference,
            quantity_delta, balance_after: *balance,
        });
    }
    result.reverse();
    Ok(result)
}

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
        "SELECT opening_balance FROM contacts WHERE id=?1", [contact_id], |row| row.get::<_, i64>(0)
    ).map_err(|e| e.to_string())?;
    let opening = opening.max(0);
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


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpenseInput {
    pub category: String,
    pub description: String,
    pub amount: i64,
    pub date: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpenseRecord {
    pub id: i64,
    pub category: String,
    pub description: String,
    pub amount: i64,
    pub date: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfitLossReport {
    pub sales: i64,
    pub discounts: i64,
    pub net_sales: i64,
    pub cost_of_goods_sold: i64,
    pub gross_profit: i64,
    pub expenses: i64,
    pub net_profit: i64,
    pub uncosted_sales: i64,
    pub expense_rows: Vec<ExpenseRecord>,
}

/// Save an operating expense in the shared ledger.
pub fn create_expense(connection: &Connection, input: &ExpenseInput) -> Result<i64, String> {
    if input.category.trim().is_empty() || input.description.trim().is_empty() || input.amount <= 0 {
        return Err("دسته‌بندی، شرح و مبلغ مثبت هزینه الزامی است".into());
    }
    connection.execute(
        "INSERT INTO expenses(category,description,amount,expense_date) VALUES(?1,?2,?3,COALESCE(NULLIF(?4,''),date('now','localtime')))",
        params![input.category.trim(), input.description.trim(), input.amount, input.date.trim()],
    ).map_err(|e| e.to_string())?;
    Ok(connection.last_insert_rowid())
}

pub fn list_expenses(connection: &Connection, from: &str, to: &str) -> Result<Vec<ExpenseRecord>, String> {
    let mut statement = connection.prepare(
        "SELECT id,category,description,amount,expense_date FROM expenses
         WHERE (?1='' OR expense_date>=?1) AND (?2='' OR expense_date<=?2)
         ORDER BY expense_date DESC,id DESC LIMIT 500"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map(params![from, to], |row| Ok(ExpenseRecord {
        id: row.get(0)?, category: row.get(1)?, description: row.get(2)?,
        amount: row.get(3)?, date: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())
}

/// Profit report. Legacy sale rows without a saved unit_cost are deliberately
/// excluded from COGS and exposed as uncosted sales; current product buy_price
/// is never substituted for historical cost.
pub fn profit_loss_report(connection: &Connection, from: &str, to: &str) -> Result<ProfitLossReport, String> {
    let sum = |sql: &str| -> Result<i64,String> {
        connection.query_row(sql, params![from, to], |row| row.get(0)).map_err(|e| e.to_string())
    };
    let sales = sum("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND (?1='' OR invoice_date>=?1) AND (?2='' OR invoice_date<=?2)")?;
    let discounts = sum("SELECT COALESCE(SUM(discount),0) FROM invoices WHERE kind='sale' AND voided=0 AND (?1='' OR invoice_date>=?1) AND (?2='' OR invoice_date<=?2)")?;
    let net_sales = sales;
    let cost_of_goods_sold = sum(
        "SELECT COALESCE(SUM(ii.unit_cost * (ii.quantity-COALESCE((SELECT SUM(sri.quantity) FROM sales_return_items sri WHERE sri.invoice_item_id=ii.id),0))),0)
         FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id
         WHERE i.kind='sale' AND i.voided=0 AND ii.unit_cost IS NOT NULL
         AND (?1='' OR i.invoice_date>=?1) AND (?2='' OR i.invoice_date<=?2)"
    )?;
    let uncosted_sales = sum(
        "SELECT COALESCE(SUM(ii.line_total),0) FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id
         WHERE i.kind='sale' AND i.voided=0 AND ii.unit_cost IS NULL
         AND (?1='' OR i.invoice_date>=?1) AND (?2='' OR i.invoice_date<=?2)"
    )?;
    let expenses = sum("SELECT COALESCE(SUM(amount),0) FROM expenses WHERE (?1='' OR expense_date>=?1) AND (?2='' OR expense_date<=?2)")?;
    let gross_profit = net_sales.checked_sub(cost_of_goods_sold).ok_or("محاسبه سود ناخالص از ظرفیت عددی خارج شد")?;
    let net_profit = gross_profit.checked_sub(expenses).ok_or("محاسبه سود خالص از ظرفیت عددی خارج شد")?;
    let expense_rows = list_expenses(connection, from, to)?;
    Ok(ProfitLossReport { sales, discounts, net_sales, cost_of_goods_sold, gross_profit, expenses, net_profit, uncosted_sales, expense_rows })
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
    fn inventory_adjustment_updates_stock_and_writes_audit_atomically() {
        let mut c=database();
        let result=adjust_inventory(&mut c,1,17,"شمارش دستی انبار").unwrap();
        assert_eq!((result.previous_quantity,result.new_quantity,result.delta),(10,17,7));
        let audit:(String,i64,i64,i64,String)=c.query_row(
            "SELECT product_name,previous_quantity,new_quantity,delta,reason FROM inventory_adjustments WHERE product_id=1",
            [],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        ).unwrap();
        assert_eq!(audit,("Pen".into(),10,17,7,"شمارش دستی انبار".into()));
        assert_eq!(c.query_row("SELECT quantity FROM products WHERE id=1",[],|r|r.get::<_,i64>(0)).unwrap(),17);
    }

    #[test]
    fn inventory_adjustment_rejects_invalid_input_without_mutation() {
        let mut c=database();
        assert!(adjust_inventory(&mut c,1,-1,"شمارش دستی").is_err());
        assert!(adjust_inventory(&mut c,1,10,"").is_err());
        assert!(adjust_inventory(&mut c,1,10,"تغییر").is_err());
        assert!(adjust_inventory(&mut c,999,3,"شمارش دستی").is_err());
        assert_eq!(c.query_row("SELECT quantity FROM products WHERE id=1",[],|r|r.get::<_,i64>(0)).unwrap(),10);
        assert_eq!(c.query_row("SELECT COUNT(*) FROM inventory_adjustments",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }

    #[test]
    fn shared_invoice_service_records_sale_and_decrements_stock_atomically() {
        let mut connection = database();
        let contact_id = create_contact(&connection, &ContactInput {
            name: "مشتری فاکتور".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 0, notes: String::new(),
        }).unwrap();
        let product_id = create_product(&connection, &ProductInput {
            name: "کالای فاکتور".into(), code: "INV-1".into(), barcode: "991".into(),
            buy_price: 40, sell_price: 100, quantity: 5, min_quantity: 1,
        }).unwrap();
        let invoice_id = create_invoice(&mut connection, &InvoiceInput {
            kind: "sale".into(), contact_name: "مشتری فاکتور".into(),
            date: "2026-01-05".into(), discount: 10, paid: 50,
            method: "cash".into(), notes: String::new(),
            lines: vec![InvoiceLineInput { product_lookup: "INV-1".into(), quantity: 1, unit_price: 100 }],
        }).unwrap();
        let (total, paid): (i64, i64) = connection.query_row(
            "SELECT total,paid FROM invoices WHERE id=?1", [invoice_id],
            |r| Ok((r.get(0)?,r.get(1)?))
        ).unwrap();
        let stock: i64 = connection.query_row(
            "SELECT quantity FROM products WHERE id=?1", [product_id], |r| r.get(0)
        ).unwrap();
        let receipts: i64 = connection.query_row(
            "SELECT COUNT(*) FROM payments WHERE kind='receipt' AND amount=50", [], |r| r.get(0)
        ).unwrap();
        assert_eq!((total, paid, stock, receipts), (90, 50, 4, 1));
        assert_eq!(contact_id > 0, true);
    }

    #[test]
    fn shared_invoice_service_rolls_back_all_lines_when_later_line_has_insufficient_stock() {
        let mut connection = database();
        let first = create_product(&connection, &ProductInput {
            name: "موجود".into(), code: "ROLL-1".into(), barcode: String::new(),
            buy_price: 10, sell_price: 20, quantity: 5, min_quantity: 0,
        }).unwrap();
        create_product(&connection, &ProductInput {
            name: "ناموجود".into(), code: "ROLL-2".into(), barcode: String::new(),
            buy_price: 10, sell_price: 20, quantity: 1, min_quantity: 0,
        }).unwrap();
        let input = InvoiceInput {
            kind: "sale".into(), contact_name: String::new(), date: String::new(),
            discount: 0, paid: 40, method: "cash".into(), notes: String::new(),
            lines: vec![
                InvoiceLineInput { product_lookup: "ROLL-1".into(), quantity: 2, unit_price: 20 },
                InvoiceLineInput { product_lookup: "ROLL-2".into(), quantity: 2, unit_price: 20 },
            ],
        };
        assert!(create_invoice(&mut connection, &input).is_err());
        let (stock, invoices): (i64, i64) = connection.query_row(
            "SELECT quantity,(SELECT COUNT(*) FROM invoices) FROM products WHERE id=?1",
            [first], |r| Ok((r.get(0)?,r.get(1)?))
        ).unwrap();
        assert_eq!((stock, invoices), (5, 0));
    }

    #[test]
    fn shared_return_service_restores_stock_and_reduces_invoice_atomically() {
        let mut connection = database();
        create_contact(&connection, &ContactInput {
            name: "مشتری برگشت".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 0, notes: String::new(),
        }).unwrap();
        let product_id = create_product(&connection, &ProductInput {
            name: "کالای برگشتی".into(), code: "RET-1".into(), barcode: "RET123".into(),
            buy_price: 40, sell_price: 100, quantity: 10, min_quantity: 0,
        }).unwrap();
        let invoice_id = create_invoice(&mut connection, &InvoiceInput {
            kind: "sale".into(), contact_name: "مشتری برگشت".into(), date: "2026-10-30".into(),
            discount: 20, paid: 0, method: "cash".into(), notes: String::new(),
            lines: vec![InvoiceLineInput { product_lookup: "RET-1".into(), quantity: 3, unit_price: 100 }],
        }).unwrap();
        let result = return_sale_items(&mut connection, invoice_id, &[ReturnLineInput {
            product_lookup: "RET123".into(), quantity: 1,
        }], "cash").unwrap();
        let (stock, total, returned): (i64, i64, i64) = connection.query_row(
            "SELECT p.quantity,i.total,(SELECT COALESCE(SUM(quantity),0) FROM sales_return_items WHERE invoice_item_id=1)
             FROM products p JOIN invoices i ON i.id=?1 WHERE p.id=?2",
            params![invoice_id, product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        ).unwrap();
        assert_eq!(result, (93, 0));
        assert_eq!((stock, total, returned), (8, 187, 1));
        let over_return = return_sale_items(&mut connection, invoice_id, &[ReturnLineInput {
            product_lookup: "RET-1".into(), quantity: 3,
        }], "cash");
        assert!(over_return.is_err());
        let stock_after: i64 = connection.query_row(
            "SELECT quantity FROM products WHERE id=?1", [product_id], |r| r.get(0)
        ).unwrap();
        assert_eq!(stock_after, 8);
    }

    #[test]
    fn shared_void_service_preserves_invoice_audit_and_restores_sale_stock() {
        let mut connection = database();
        create_contact(&connection, &ContactInput {
            name: "مشتری ابطال".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 0, notes: String::new(),
        }).unwrap();
        let product_id = create_product(&connection, &ProductInput {
            name: "کالای ابطال".into(), code: "VOID-1".into(), barcode: String::new(),
            buy_price: 30, sell_price: 60, quantity: 5, min_quantity: 0,
        }).unwrap();
        let invoice_id = create_invoice(&mut connection, &InvoiceInput {
            kind: "sale".into(), contact_name: "مشتری ابطال".into(), date: String::new(),
            discount: 0, paid: 0, method: "cash".into(), notes: String::new(),
            lines: vec![InvoiceLineInput { product_lookup: "VOID-1".into(), quantity: 2, unit_price: 60 }],
        }).unwrap();
        void_invoice(&mut connection, invoice_id).unwrap();
        let (stock, voided, items): (i64, i64, i64) = connection.query_row(
            "SELECT p.quantity,i.voided,(SELECT COUNT(*) FROM invoice_items WHERE invoice_id=i.id)
             FROM products p JOIN invoices i ON i.id=?1 WHERE p.id=?2",
            params![invoice_id, product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        ).unwrap();
        assert_eq!((stock, voided, items), (5, 1, 1));
        assert!(void_invoice(&mut connection, invoice_id).is_err());
    }

    #[test]
    fn purchase_return_reduces_stock_and_invoice_and_records_supplier_refund() {
        let mut connection=database();
        create_contact(&connection,&ContactInput{name:"تأمین‌کننده مرجوعی".into(),kind:"supplier".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        create_product(&connection,&ProductInput{name:"کالای خرید".into(),code:"BUY-RET".into(),barcode:String::new(),buy_price:10,sell_price:20,quantity:0,min_quantity:0}).unwrap();
        let invoice_id=create_invoice(&mut connection,&InvoiceInput{kind:"purchase".into(),contact_name:"تأمین‌کننده مرجوعی".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"BUY-RET".into(),quantity:5,unit_price:20}]}).unwrap();
        record_payment(&mut connection,&PaymentInput{kind:"payment".into(),contact_name:"تأمین‌کننده مرجوعی".into(),amount:100,method:"cash".into(),date:String::new(),notes:String::new()}).unwrap();
        let result=return_purchase_items(&mut connection,invoice_id,&[ReturnLineInput{product_lookup:"BUY-RET".into(),quantity:2}],"cash").unwrap();
        assert_eq!(result,(40,40));
        let (stock,total,paid,returned):(i64,i64,i64,i64)=connection.query_row("SELECT p.quantity,i.total,i.paid,(SELECT SUM(quantity) FROM purchase_return_items pri JOIN purchase_returns pr ON pr.id=pri.return_id WHERE pr.invoice_id=i.id) FROM products p JOIN invoices i ON i.id=?1 WHERE p.code='BUY-RET'",[invoice_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!((stock,total,paid,returned),(3,60,60,2));
        assert!(void_invoice(&mut connection,invoice_id).is_err());
    }

    #[test]
    fn purchase_return_rejects_over_return_without_stock_or_invoice_mutation() {
        let mut connection=database();
        create_contact(&connection,&ContactInput{name:"تأمین‌کننده دوم".into(),kind:"supplier".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        create_product(&connection,&ProductInput{name:"قلم خرید".into(),code:"BUY-OVER".into(),barcode:String::new(),buy_price:10,sell_price:20,quantity:0,min_quantity:0}).unwrap();
        let invoice_id=create_invoice(&mut connection,&InvoiceInput{kind:"purchase".into(),contact_name:"تأمین‌کننده دوم".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"BUY-OVER".into(),quantity:2,unit_price:20}]}).unwrap();
        assert!(return_purchase_items(&mut connection,invoice_id,&[ReturnLineInput{product_lookup:"BUY-OVER".into(),quantity:3}],"cash").is_err());
        let (stock,total,returns):(i64,i64,i64)=connection.query_row("SELECT p.quantity,i.total,(SELECT COUNT(*) FROM purchase_returns WHERE invoice_id=i.id) FROM products p JOIN invoices i ON i.id=?1 WHERE p.code='BUY-OVER'",[invoice_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!((stock,total,returns),(2,40,0));
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
            let mut statement = connection.prepare("SELECT paid FROM invoices ORDER BY id").unwrap();
            let rows = statement.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<Result<_,_>>().unwrap()
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

    #[test]
    fn inventory_ledger_reconciles_purchases_sales_returns_and_adjustments() {
        let mut connection = database();
        create_contact(&connection, &ContactInput {
            name: "مشتری گزارش".into(), kind: "customer".into(),
            phone: String::new(), opening_balance: 0, notes: String::new(),
        }).unwrap();
        create_contact(&connection, &ContactInput {
            name: "تأمین‌کننده گزارش".into(), kind: "supplier".into(),
            phone: String::new(), opening_balance: 0, notes: String::new(),
        }).unwrap();
        let product_id = create_product(&connection, &ProductInput {
            name: "کالای گزارش".into(), code: "LEDGER-1".into(), barcode: String::new(),
            buy_price: 10, sell_price: 30, quantity: 10, min_quantity: 2,
        }).unwrap();

        let purchase_id = create_invoice(&mut connection, &InvoiceInput {
            kind: "purchase".into(), contact_name: "تأمین‌کننده گزارش".into(),
            date: "2026-10-29".into(), discount: 0, paid: 0, method: "cash".into(),
            notes: String::new(), lines: vec![InvoiceLineInput {
                product_lookup: "LEDGER-1".into(), quantity: 5, unit_price: 20,
            }],
        }).unwrap();
        let sale_id = create_invoice(&mut connection, &InvoiceInput {
            kind: "sale".into(), contact_name: "مشتری گزارش".into(),
            date: "2026-10-30".into(), discount: 0, paid: 0, method: "cash".into(),
            notes: String::new(), lines: vec![InvoiceLineInput {
                product_lookup: "LEDGER-1".into(), quantity: 3, unit_price: 30,
            }],
        }).unwrap();
        return_sale_items(&mut connection, sale_id, &[ReturnLineInput {
            product_lookup: "LEDGER-1".into(), quantity: 1,
        }], "cash").unwrap();
        return_purchase_items(&mut connection, purchase_id, &[ReturnLineInput {
            product_lookup: "LEDGER-1".into(), quantity: 1,
        }], "cash").unwrap();
        adjust_inventory(&mut connection, product_id, 11, "انبارگردانی دوره‌ای").unwrap();

        let valuation = inventory_valuation(&connection).unwrap();
        assert_eq!(valuation.len(), 1);
        assert_eq!((valuation[0].quantity, valuation[0].buy_price, valuation[0].average_cost, valuation[0].stock_value), (11, 20, 13, 143));

        let movements = inventory_movements(&connection).unwrap();
        assert_eq!(movements.len(), 5);
        let delta_sum: i64 = movements.iter().map(|m| m.quantity_delta).sum();
        assert_eq!(delta_sum, 1);
        assert_eq!(movements[0].movement_type, "اصلاح موجودی");
        assert_eq!(movements[0].balance_after, 11);
        let opening = movements.last().unwrap().balance_after - movements.last().unwrap().quantity_delta;
        assert_eq!(opening, 10);
    }

    #[test]
    fn inventory_valuation_detects_integer_overflow() {
        let connection = database();
        connection.execute(
            "INSERT INTO products(name,code,quantity,buy_price,average_cost) VALUES('کالای بزرگ','BIG-1',9223372036854775807,2,2)",
            []
        ).unwrap();
        assert!(inventory_valuation(&connection).unwrap_err().contains("ظرفیت عددی"));
    }

    #[test]
    fn sale_cost_is_snapshotted_and_expenses_reduce_net_profit() {
        let mut c = database();
        c.execute("INSERT INTO contacts(name,kind) VALUES('Ali','customer')", []).unwrap();
        c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,average_cost) VALUES('Pen','PEN',10,20,10,10)", []).unwrap();
        let id = create_invoice(&mut c, &InvoiceInput {
            kind: "sale".into(), contact_name: "Ali".into(), date: "2026-10-31".into(),
            discount: 0, paid: 0, method: "cash".into(), notes: String::new(),
            lines: vec![InvoiceLineInput { product_lookup: "PEN".into(), quantity: 2, unit_price: 20 }],
        }).unwrap();
        let saved_cost: i64 = c.query_row("SELECT unit_cost FROM invoice_items WHERE invoice_id=?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(saved_cost, 10);
        c.execute("UPDATE products SET buy_price=15 WHERE code='PEN'", []).unwrap();
        create_expense(&c, &ExpenseInput { category:"اجاره".into(), description:"اجاره مغازه".into(), amount:5, date:"2026-10-31".into() }).unwrap();
        let report = profit_loss_report(&c, "2026-10-31", "2026-10-31").unwrap();
        assert_eq!(report.net_sales, 40);
        assert_eq!(report.cost_of_goods_sold, 20);
        assert_eq!(report.gross_profit, 20);
        assert_eq!(report.expenses, 5);
        assert_eq!(report.net_profit, 15);
        assert_eq!(report.uncosted_sales, 0);
    }

    #[test]
    fn legacy_sale_without_cost_is_reported_as_uncosted_not_guessed() {
        let c = database();
        c.execute("INSERT INTO contacts(name,kind) VALUES('Ali','customer')", []).unwrap();
        c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity) VALUES('Pen','PEN',10,20,10)", []).unwrap();
        c.execute("INSERT INTO invoices(number,kind,contact_name,invoice_date,subtotal,total) VALUES(1001,'sale','Ali','2026-10-31',100,100)", []).unwrap();
        c.execute("INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total) VALUES(1,1,'Pen',2,50,100)", []).unwrap();
        let report = profit_loss_report(&c, "2026-10-31", "2026-10-31").unwrap();
        assert_eq!(report.cost_of_goods_sold, 0);
        assert_eq!(report.uncosted_sales, 100);
    }


    #[test]
    fn moving_average_costs_purchases_and_preserves_historical_sale_cost() {
        let mut c=database();
        create_contact(&c,&ContactInput{name:"تامین‌کننده میانگین".into(),kind:"supplier".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        create_contact(&c,&ContactInput{name:"مشتری میانگین".into(),kind:"customer".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        let pid=create_product(&c,&ProductInput{name:"کالا".into(),code:"AVG-1".into(),barcode:String::new(),buy_price:10,sell_price:50,quantity:1,min_quantity:0}).unwrap();
        create_invoice(&mut c,&InvoiceInput{kind:"purchase".into(),contact_name:"تامین‌کننده میانگین".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-1".into(),quantity:1,unit_price:30}]}).unwrap();
        assert_eq!(c.query_row("SELECT average_cost FROM products WHERE id=?1",[pid],|r|r.get::<_,i64>(0)).unwrap(),20);
        let sale=create_invoice(&mut c,&InvoiceInput{kind:"sale".into(),contact_name:"مشتری میانگین".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-1".into(),quantity:1,unit_price:50}]}).unwrap();
        assert_eq!(c.query_row("SELECT unit_cost FROM invoice_items WHERE invoice_id=?1",[sale],|r|r.get::<_,i64>(0)).unwrap(),20);
        create_invoice(&mut c,&InvoiceInput{kind:"purchase".into(),contact_name:"تامین‌کننده میانگین".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-1".into(),quantity:1,unit_price:40}]}).unwrap();
        let prices:(i64,i64)=c.query_row("SELECT buy_price,average_cost FROM products WHERE id=?1",[pid],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(prices,(40,30));
        assert_eq!(c.query_row("SELECT unit_cost FROM invoice_items WHERE invoice_id=?1",[sale],|r|r.get::<_,i64>(0)).unwrap(),20);
    }

    #[test]
    fn sale_return_restores_inventory_at_original_saved_average_cost() {
        let mut c=database();
        create_contact(&c,&ContactInput{name:"تامین‌کننده برگشت".into(),kind:"supplier".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        create_contact(&c,&ContactInput{name:"مشتری برگشت هزینه".into(),kind:"customer".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        let pid=create_product(&c,&ProductInput{name:"کالای برگشت".into(),code:"AVG-RET".into(),barcode:String::new(),buy_price:10,sell_price:50,quantity:1,min_quantity:0}).unwrap();
        create_invoice(&mut c,&InvoiceInput{kind:"purchase".into(),contact_name:"تامین‌کننده برگشت".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-RET".into(),quantity:1,unit_price:30}]}).unwrap();
        let sale=create_invoice(&mut c,&InvoiceInput{kind:"sale".into(),contact_name:"مشتری برگشت هزینه".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-RET".into(),quantity:1,unit_price:50}]}).unwrap();
        create_invoice(&mut c,&InvoiceInput{kind:"purchase".into(),contact_name:"تامین‌کننده برگشت".into(),date:String::new(),discount:0,paid:0,method:"cash".into(),notes:String::new(),lines:vec![InvoiceLineInput{product_lookup:"AVG-RET".into(),quantity:1,unit_price:40}]}).unwrap();
        return_sale_items(&mut c,sale,&[ReturnLineInput{product_lookup:"AVG-RET".into(),quantity:1}],"cash").unwrap();
        let result:(i64,i64)=c.query_row("SELECT quantity,average_cost FROM products WHERE id=?1",[pid],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(result,(3,27));
    }

    #[test]
    fn purchase_invoice_discount_is_allocated_into_inventory_average_cost() {
        let mut c=database();
        create_contact(&c,&ContactInput{name:"تامین‌کننده تخفیف".into(),kind:"supplier".into(),phone:String::new(),opening_balance:0,notes:String::new()}).unwrap();
        let pid=create_product(&c,&ProductInput{name:"کالای تخفیف".into(),code:"AVG-DISC".into(),barcode:String::new(),buy_price:10,sell_price:50,quantity:1,min_quantity:0}).unwrap();
        create_invoice(&mut c,&InvoiceInput{
            kind:"purchase".into(),contact_name:"تامین‌کننده تخفیف".into(),date:String::new(),
            discount:10,paid:0,method:"cash".into(),notes:String::new(),
            lines:vec![InvoiceLineInput{product_lookup:"AVG-DISC".into(),quantity:1,unit_price:30}],
        }).unwrap();
        let (latest_price,average):(i64,i64)=c.query_row(
            "SELECT buy_price,average_cost FROM products WHERE id=?1",[pid],|r|Ok((r.get(0)?,r.get(1)?))
        ).unwrap();
        assert_eq!((latest_price,average),(30,15));
    }

}
