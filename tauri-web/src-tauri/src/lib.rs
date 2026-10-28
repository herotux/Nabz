use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::{fs, sync::Mutex};
use tauri::{Manager, State};

struct Db(Mutex<Connection>);
#[derive(Serialize)]
struct Summary { sales_today:i64, receipts_today:i64, customer_balance:i64, inventory_value:i64, contacts:i64, products:i64, invoices:i64 }
#[derive(Serialize)]
struct Record { id:i64, name:String, detail:String, amount:i64, status:String, date:String }
#[derive(Serialize)]
struct ProductRecord { id:i64, name:String, code:String, quantity:i64, buy_price:i64, sell_price:i64, min_quantity:i64 }

fn text(v:&Value,k:&str)->String { v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned() }
fn num(v:&Value,k:&str)->i64 { v.get(k).and_then(|x|x.as_i64().or_else(||x.as_str().and_then(|s|s.replace(',',"").replace('٬',"").parse().ok()))).unwrap_or(0) }
fn init(c:&Connection)->Result<(),String>{ c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS contacts(id INTEGER PRIMARY KEY,name TEXT NOT NULL,kind TEXT NOT NULL DEFAULT 'customer',phone TEXT NOT NULL DEFAULT '',opening_balance INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL DEFAULT(datetime('now','localtime')));
CREATE TABLE IF NOT EXISTS products(id INTEGER PRIMARY KEY,name TEXT NOT NULL,code TEXT NOT NULL UNIQUE,barcode TEXT NOT NULL DEFAULT '',buy_price INTEGER NOT NULL DEFAULT 0,sell_price INTEGER NOT NULL DEFAULT 0,quantity INTEGER NOT NULL DEFAULT 0,min_quantity INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS invoices(id INTEGER PRIMARY KEY,number INTEGER NOT NULL UNIQUE,kind TEXT NOT NULL,contact_id INTEGER,contact_name TEXT NOT NULL,invoice_date TEXT NOT NULL DEFAULT(date('now','localtime')),subtotal INTEGER NOT NULL,discount INTEGER NOT NULL DEFAULT 0,total INTEGER NOT NULL,paid INTEGER NOT NULL DEFAULT 0,notes TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS invoice_items(id INTEGER PRIMARY KEY,invoice_id INTEGER NOT NULL REFERENCES invoices(id),product_id INTEGER,item_name TEXT NOT NULL,quantity INTEGER NOT NULL,unit_price INTEGER NOT NULL,line_total INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS payments(id INTEGER PRIMARY KEY,kind TEXT NOT NULL CHECK(kind IN('receipt','payment')),contact_id INTEGER,contact_name TEXT NOT NULL,amount INTEGER NOT NULL CHECK(amount>0),method TEXT NOT NULL DEFAULT 'cash',payment_date TEXT NOT NULL DEFAULT(date('now','localtime')),notes TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(|e|e.to_string()) }
#[tauri::command] fn app_version()-> &'static str { env!("CARGO_PKG_VERSION") }
#[tauri::command] fn db_status(db:State<'_,Db>)->Result<String,String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.query_row("PRAGMA database_list",[],|r|r.get(2)).map_err(|e|e.to_string())}
#[tauri::command] fn dashboard_summary(db:State<'_,Db>)->Result<Summary,String>{let c=db.0.lock().map_err(|e|e.to_string())?;Ok(Summary{sales_today:c.query_row("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?,receipts_today:c.query_row("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?,customer_balance:c.query_row("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale'),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts),0)",[],|r|r.get(0)).map_err(|e|e.to_string())?,inventory_value:c.query_row("SELECT COALESCE(SUM(quantity*buy_price),0) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,contacts:c.query_row("SELECT COUNT(*) FROM contacts",[],|r|r.get(0)).map_err(|e|e.to_string())?,products:c.query_row("SELECT COUNT(*) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,invoices:c.query_row("SELECT COUNT(*) FROM invoices",[],|r|r.get(0)).map_err(|e|e.to_string())?})}
#[tauri::command] fn list_records(kind:String,db:State<'_,Db>)->Result<Vec<Record>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let sql=match kind.as_str(){"contacts"=>"SELECT id,name,phone,opening_balance,kind,datetime(created_at) FROM contacts ORDER BY id DESC","products"=>"SELECT id,name,code,quantity,sell_price,'موجود', '' FROM products ORDER BY id DESC","invoices"=>"SELECT id,'فاکتور #'||number||' · '||contact_name,kind,total,CASE WHEN paid>=total THEN 'تسویه' ELSE 'بدهکار' END,invoice_date FROM invoices ORDER BY id DESC","payments"=>"SELECT id,contact_name,kind,amount,method,payment_date FROM payments ORDER BY id DESC",_=>return Err("فهرست نامعتبر است".into())};let mut s=c.prepare(sql).map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Record{id:r.get(0)?,name:r.get(1)?,detail:r.get(2)?,amount:r.get(3)?,status:r.get(4)?,date:r.get(5)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn list_products(db:State<'_,Db>)->Result<Vec<ProductRecord>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT id,name,code,quantity,buy_price,sell_price,min_quantity FROM products ORDER BY id DESC").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(ProductRecord{id:r.get(0)?,name:r.get(1)?,code:r.get(2)?,quantity:r.get(3)?,buy_price:r.get(4)?,sell_price:r.get(5)?,min_quantity:r.get(6)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command]
fn delete_record(kind:String,id:i64,db:State<'_,Db>)->Result<(),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 match kind.as_str(){
  "contacts"=>{let n:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM invoices WHERE contact_id=?1)+(SELECT COUNT(*) FROM payments WHERE contact_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این طرف‌حساب سابقه فاکتور یا دریافت/پرداخت دارد و حذف نمی‌شود.".into())}tx.execute("DELETE FROM contacts WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "products"=>{let n:i64=tx.query_row("SELECT COUNT(*) FROM invoice_items WHERE product_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این کالا در فاکتور ثبت شده و برای حفظ سابقه حذف نمی‌شود.".into())}tx.execute("DELETE FROM products WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "invoices"=>{let inv:Option<(String,i64)>=tx.query_row("SELECT kind,paid FROM invoices WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;let Some((ik,paid))=inv else{return Err("فاکتور پیدا نشد.".into())};if paid>0{return Err("فاکتور تسویه جزئی/کامل دارد؛ ابتدا گردش دریافت و پرداخت را اصلاح کنید.".into())}let mut st=tx.prepare("SELECT product_id,quantity FROM invoice_items WHERE invoice_id=?1").map_err(|e|e.to_string())?;let rows=st.query_map([id],|r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,i64>(1)?))).map_err(|e|e.to_string())?;let items=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;drop(st);for (pid,q) in items{if let Some(pid)=pid{if ik=="sale"{tx.execute("UPDATE products SET quantity=quantity+?1 WHERE id=?2",params![q,pid]).map_err(|e|e.to_string())?;}else{let stock:i64=tx.query_row("SELECT quantity FROM products WHERE id=?1",[pid],|r|r.get(0)).map_err(|e|e.to_string())?;if stock<q{return Err("حذف فاکتور خرید موجودی منفی ایجاد می‌کند؛ ابتدا فروش‌های بعدی را بررسی کنید.".into())}tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2",params![q,pid]).map_err(|e|e.to_string())?;}}}tx.execute("DELETE FROM invoice_items WHERE invoice_id=?1",[id]).map_err(|e|e.to_string())?;tx.execute("DELETE FROM invoices WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  _=>return Err("حذف این نوع رکورد برای جلوگیری از آسیب به گردش مالی مجاز نیست.".into())
 }
 tx.commit().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command] fn save_contact(payload:Value,db:State<'_,Db>)->Result<i64,String>{let n=text(&payload,"name");if n.is_empty(){return Err("نام طرف‌حساب الزامی است".into())}let c=db.0.lock().map_err(|e|e.to_string())?;let existing:i64=c.query_row("SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE",[&n],|r|r.get(0)).map_err(|e|e.to_string())?;if existing>0{return Err("طرف‌حسابی با این نام قبلاً ثبت شده است".into())}let kind=text(&payload,"kind");let kind=if kind.is_empty(){"customer".to_string()}else{kind};c.execute("INSERT INTO contacts(name,kind,phone,opening_balance,notes) VALUES(?1,?2,?3,?4,?5)",params![n,kind,text(&payload,"phone"),num(&payload,"balance"),text(&payload,"notes")]).map_err(|e|e.to_string())?;Ok(c.last_insert_rowid())}
#[tauri::command] fn save_product(payload:Value,db:State<'_,Db>)->Result<i64,String>{let n=text(&payload,"name");let code=text(&payload,"code");let qty=num(&payload,"qty");let buy=num(&payload,"buy");let sell=num(&payload,"sell");if n.is_empty()||code.is_empty(){return Err("نام و کد کالا الزامی است".into())}if qty<0||buy<0||sell<0{return Err("موجودی و قیمت نمی‌تواند منفی باشد".into())}let c=db.0.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO products(name,code,barcode,buy_price,sell_price,quantity,min_quantity) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![n,code,text(&payload,"barcode"),buy,sell,qty,num(&payload,"min")]).map_err(|e|if e.to_string().contains("UNIQUE"){"کد کالا تکراری است".into()}else{e.to_string()})?;Ok(c.last_insert_rowid())}
#[tauri::command]
fn save_invoice(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let kind=text(&payload,"kind"); let contact=text(&payload,"customer");
 if !["sale","purchase"].contains(&kind.as_str())||contact.is_empty(){return Err("نوع فاکتور و طرف‌حساب الزامی است".into())}
 let mut items:Vec<(String,i64,i64)>=Vec::new();
 if let Some(arr)=payload.get("items").and_then(Value::as_array){
  for x in arr {let name=x.get("item").and_then(Value::as_str).unwrap_or("").trim().to_owned();let q=x.get("qty").and_then(Value::as_i64).unwrap_or(0);let p=x.get("price").and_then(Value::as_i64).unwrap_or(0);if !name.is_empty(){items.push((name,q,p));}}
 } else {
  for i in 1..=5 {let suffix=if i==1{String::new()}else{i.to_string()};let name=text(&payload,&format!("item{suffix}"));let q=num(&payload,&format!("qty{suffix}"));let p=num(&payload,&format!("price{suffix}"));if !name.is_empty(){items.push((name,q,p));}}
 }
 if items.is_empty(){return Err("حداقل یک ردیف کالا یا خدمت وارد کنید".into())}
 if items.iter().any(|(_,q,p)|*q<=0||*p<0){return Err("تعداد هر ردیف باید مثبت و قیمت‌ها نامنفی باشند".into())}
 let subtotal=items.iter().try_fold(0i64,|sum,(_,q,p)|q.checked_mul(*p).and_then(|v|sum.checked_add(v))).ok_or("مبلغ فاکتور بیش از حد بزرگ است")?;
 let discount=num(&payload,"discount");if discount<0||discount>subtotal{return Err("تخفیف باید بین صفر و مبلغ کل باشد".into())}
 let total=subtotal-discount;let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 let cid:Option<i64>=tx.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1",[&contact],|r|r.get(0)).optional().map_err(|e|e.to_string())?;if cid.is_none(){return Err("طرف‌حساب ثبت‌شده‌ای با این نام پیدا نشد؛ ابتدا آن را در بخش طرف‌حساب‌ها ثبت کنید.".into())}
 let mut resolved:Vec<(Option<i64>,String,i64,i64,i64)>=Vec::new();
 for (name,qty,price) in items {
  let found:Option<(i64,i64)>=tx.query_row("SELECT id,quantity FROM products WHERE name=?1 COLLATE NOCASE OR code=?1 OR barcode=?1 LIMIT 1",[&name],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  let pid=if let Some((pid,stock))=found {
   if kind=="sale" {if stock<qty{return Err(format!("موجودی «{name}» کافی نیست؛ موجودی فعلی {stock}"))}tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2",params![qty,pid]).map_err(|e|e.to_string())?;}
   else {tx.execute("UPDATE products SET quantity=quantity+?1,buy_price=?2 WHERE id=?3",params![qty,price,pid]).map_err(|e|e.to_string())?;}
   Some(pid)
  } else if kind=="purchase" {
   let code=format!("AUTO-{}",tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM products",[],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?);
   tx.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity) VALUES(?1,?2,?3,?3,?4)",params![name,code,price,qty]).map_err(|e|e.to_string())?;
   Some(tx.last_insert_rowid())
  } else {None};
  let line=qty.checked_mul(price).ok_or("مبلغ ردیف بیش از حد بزرگ است")?;
  resolved.push((pid,name,qty,price,line));
 }
 let number:i64=tx.query_row("SELECT COALESCE(MAX(number),1000)+1 FROM invoices",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 tx.execute("INSERT INTO invoices(number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,notes) VALUES(?1,?2,?3,?4,COALESCE(NULLIF(?5,''),date('now','localtime')),?6,?7,?8,?9)",params![number,kind,cid,contact,text(&payload,"date"),subtotal,discount,total,text(&payload,"notes")]).map_err(|e|e.to_string())?;
 let id=tx.last_insert_rowid();for (pid,name,q,p,line) in resolved {tx.execute("INSERT INTO invoice_items(invoice_id,product_id,item_name,quantity,unit_price,line_total) VALUES(?1,?2,?3,?4,?5,?6)",params![id,pid,name,q,p,line]).map_err(|e|e.to_string())?;}
 tx.commit().map_err(|e|e.to_string())?;Ok(id)
}
#[tauri::command] fn save_payment(payload:Value,db:State<'_,Db>)->Result<i64,String>{let kind=text(&payload,"kind");let contact=text(&payload,"customer");let amount=num(&payload,"amount");if !["receipt","payment"].contains(&kind.as_str())||contact.is_empty()||amount<=0{return Err("طرف‌حساب و مبلغ مثبت الزامی است".into())}let method=text(&payload,"method");let date=text(&payload,"date");let notes=text(&payload,"notes");let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;let cid:Option<i64>=tx.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1",[&contact],|r|r.get(0)).optional().map_err(|e|e.to_string())?;if cid.is_none(){return Err("طرف‌حساب ثبت‌شده‌ای با این نام پیدا نشد؛ ابتدا آن را در بخش طرف‌حساب‌ها ثبت کنید.".into())}tx.execute("INSERT INTO payments(kind,contact_id,contact_name,amount,method,payment_date,notes) VALUES(?1,?2,?3,?4,COALESCE(NULLIF(?5,''),'cash'),COALESCE(NULLIF(?6,''),date('now','localtime')),?7)",params![kind,cid,contact,amount,method,date,notes]).map_err(|e|e.to_string())?;let id=tx.last_insert_rowid();let invoice_kind=if kind=="receipt"{"sale"}else{"purchase"};let mut remaining=amount;{let mut st=tx.prepare("SELECT id,total,paid FROM invoices WHERE contact_name=?1 COLLATE NOCASE AND kind=?2 AND paid<total ORDER BY invoice_date,id").map_err(|e|e.to_string())?;let rows=st.query_map(params![contact,invoice_kind],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?))).map_err(|e|e.to_string())?;let invoices=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;drop(st);for (iid,total,paid) in invoices{if remaining<=0{break}let applied=remaining.min(total-paid);tx.execute("UPDATE invoices SET paid=paid+?1 WHERE id=?2",params![applied,iid]).map_err(|e|e.to_string())?;remaining-=applied;}}tx.commit().map_err(|e|e.to_string())?;Ok(id)}

#[derive(Serialize)]
struct SalesTrendPoint { date:String, total:i64 }
#[tauri::command]
fn sales_trend(days:i64,db:State<'_,Db>)->Result<Vec<SalesTrendPoint>,String>{
 if !(7..=30).contains(&days){return Err("بازهٔ نمودار باید ۷ یا ۳۰ روز باشد".into())}
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut st=c.prepare("WITH RECURSIVE days(d,n) AS (SELECT date('now','localtime', '-' || (?1-1) || ' days'),0 UNION ALL SELECT date(d,'+1 day'),n+1 FROM days WHERE n<?1-1) SELECT days.d,COALESCE(SUM(invoices.total),0) FROM days LEFT JOIN invoices ON invoices.invoice_date=days.d AND invoices.kind='sale' GROUP BY days.d ORDER BY days.d").map_err(|e|e.to_string())?;
 let rows=st.query_map([days],|r|Ok(SalesTrendPoint{date:r.get(0)?,total:r.get(1)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn reports_summary(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let q=|sql:&str|->Result<i64,String>{c.query_row(sql,[],|r|r.get(0)).map_err(|e|e.to_string())};
 Ok(json!({
  "sales_today":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')")?,
  "receipts_today":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')")?,
  "sales_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale'")?,
  "purchases_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase'")?,
  "receipts_total":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt'")?,
  "payments_total":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='payment'")?,
  "customer_debt":q("SELECT COALESCE(SUM(total-paid),0) FROM invoices WHERE kind='sale'")?,
  "supplier_debt":q("SELECT COALESCE(SUM(total-paid),0) FROM invoices WHERE kind='purchase'")?,
  "inventory_value":q("SELECT COALESCE(SUM(quantity*buy_price),0) FROM products")?,
  "contacts":q("SELECT COUNT(*) FROM contacts")?,
  "products":q("SELECT COUNT(*) FROM products")?,
  "invoices":q("SELECT COUNT(*) FROM invoices")?
 }))
}
#[tauri::command]
fn export_data(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut contacts=Vec::new();{let mut s=c.prepare("SELECT id,name,kind,phone,opening_balance,notes,created_at FROM contacts").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"phone":r.get::<_,String>(3)?,"opening_balance":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?,"created_at":r.get::<_,String>(6)?}))).map_err(|e|e.to_string())?;for x in rows{contacts.push(x.map_err(|e|e.to_string())?);}}
 let mut products=Vec::new();{let mut s=c.prepare("SELECT id,name,code,barcode,buy_price,sell_price,quantity,min_quantity FROM products").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"code":r.get::<_,String>(2)?,"barcode":r.get::<_,String>(3)?,"buy_price":r.get::<_,i64>(4)?,"sell_price":r.get::<_,i64>(5)?,"quantity":r.get::<_,i64>(6)?,"min_quantity":r.get::<_,i64>(7)?}))).map_err(|e|e.to_string())?;for x in rows{products.push(x.map_err(|e|e.to_string())?);}}
 let mut invoices=Vec::new();{let mut s=c.prepare("SELECT id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes FROM invoices").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,"contact_id":r.get::<_,Option<i64>>(3)?,"contact_name":r.get::<_,String>(4)?,"invoice_date":r.get::<_,String>(5)?,"subtotal":r.get::<_,i64>(6)?,"discount":r.get::<_,i64>(7)?,"total":r.get::<_,i64>(8)?,"paid":r.get::<_,i64>(9)?,"notes":r.get::<_,String>(10)?}))).map_err(|e|e.to_string())?;for x in rows{invoices.push(x.map_err(|e|e.to_string())?);}}
 let mut items=Vec::new();{let mut s=c.prepare("SELECT id,invoice_id,product_id,item_name,quantity,unit_price,line_total FROM invoice_items").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"product_id":r.get::<_,Option<i64>>(2)?,"item_name":r.get::<_,String>(3)?,"quantity":r.get::<_,i64>(4)?,"unit_price":r.get::<_,i64>(5)?,"line_total":r.get::<_,i64>(6)?}))).map_err(|e|e.to_string())?;for x in rows{items.push(x.map_err(|e|e.to_string())?);}}
 let mut payments=Vec::new();{let mut s=c.prepare("SELECT id,kind,contact_id,contact_name,amount,method,payment_date,notes FROM payments").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"contact_id":r.get::<_,Option<i64>>(2)?,"contact_name":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"method":r.get::<_,String>(5)?,"payment_date":r.get::<_,String>(6)?,"notes":r.get::<_,String>(7)?}))).map_err(|e|e.to_string())?;for x in rows{payments.push(x.map_err(|e|e.to_string())?);}}
 let mut settings=Vec::new();{let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"key":r.get::<_,String>(0)?,"value":r.get::<_,String>(1)?}))).map_err(|e|e.to_string())?;for x in rows{settings.push(x.map_err(|e|e.to_string())?);}}
 Ok(json!({"format":"nabz-backup-v1","contacts":contacts,"products":products,"invoices":invoices,"invoice_items":items,"payments":payments,"settings":settings}))
}
#[tauri::command]
fn restore_data(data:Value,db:State<'_,Db>)->Result<(),String>{
 if data.get("format").and_then(Value::as_str)!=Some("nabz-backup-v1"){return Err("فایل پشتیبان نبض معتبر نیست".into())}
 for k in ["contacts","products","invoices","invoice_items","payments","settings"]{if !data.get(k).and_then(Value::as_array).is_some(){return Err(format!("ساختار پشتیبان ناقص است: {k}"))}}
 let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 tx.execute_batch("DELETE FROM invoice_items; DELETE FROM payments; DELETE FROM invoices; DELETE FROM products; DELETE FROM contacts; DELETE FROM settings;").map_err(|e|e.to_string())?;
 for x in data["contacts"].as_array().unwrap(){tx.execute("INSERT INTO contacts(id,name,kind,phone,opening_balance,notes,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["kind"].as_str().unwrap_or("customer"),x["phone"].as_str().unwrap_or(""),x["opening_balance"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or(""),x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 for x in data["products"].as_array().unwrap(){tx.execute("INSERT INTO products(id,name,code,barcode,buy_price,sell_price,quantity,min_quantity) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["code"].as_str().unwrap_or(""),x["barcode"].as_str().unwrap_or(""),x["buy_price"].as_i64().unwrap_or(0),x["sell_price"].as_i64().unwrap_or(0),x["quantity"].as_i64().unwrap_or(0),x["min_quantity"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;}
 for x in data["invoices"].as_array().unwrap(){tx.execute("INSERT INTO invoices(id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![x["id"].as_i64(),x["number"].as_i64(),x["kind"].as_str().unwrap_or("sale"),x["contact_id"].as_i64(),x["contact_name"].as_str().unwrap_or(""),x["invoice_date"].as_str().unwrap_or(""),x["subtotal"].as_i64().unwrap_or(0),x["discount"].as_i64().unwrap_or(0),x["total"].as_i64().unwrap_or(0),x["paid"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 for x in data["invoice_items"].as_array().unwrap(){tx.execute("INSERT INTO invoice_items(id,invoice_id,product_id,item_name,quantity,unit_price,line_total) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["product_id"].as_i64(),x["item_name"].as_str().unwrap_or(""),x["quantity"].as_i64().unwrap_or(0),x["unit_price"].as_i64().unwrap_or(0),x["line_total"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;}
 for x in data["payments"].as_array().unwrap(){tx.execute("INSERT INTO payments(id,kind,contact_id,contact_name,amount,method,payment_date,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![x["id"].as_i64(),x["kind"].as_str().unwrap_or("receipt"),x["contact_id"].as_i64(),x["contact_name"].as_str().unwrap_or(""),x["amount"].as_i64().unwrap_or(0),x["method"].as_str().unwrap_or("cash"),x["payment_date"].as_str().unwrap_or(""),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 for x in data["settings"].as_array().unwrap(){tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",params![x["key"].as_str().unwrap_or(""),x["value"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 tx.commit().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command] fn save_setting(key:String,value:String,db:State<'_,Db>)->Result<(),String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command] fn get_settings(db:State<'_,Db>)->Result<std::collections::HashMap<String,String>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;rows.collect::<Result<_,_>>().map_err(|e|e.to_string())}
#[cfg_attr(mobile,tauri::mobile_entry_point)] pub fn run(){tauri::Builder::default().setup(|app|{let dir=app.path().app_local_data_dir().map_err(std::io::Error::other)?;fs::create_dir_all(&dir)?;let c=Connection::open(dir.join("nabz.sqlite3")).map_err(std::io::Error::other)?;init(&c).map_err(std::io::Error::other)?;app.manage(Db(Mutex::new(c)));Ok(())}).invoke_handler(tauri::generate_handler![app_version,db_status,dashboard_summary,reports_summary,export_data,restore_data,list_records,list_products,delete_record,save_contact,save_product,save_invoice,save_payment,save_setting,get_settings,sales_trend]).run(tauri::generate_context!()).expect("error while running Nabz");}
