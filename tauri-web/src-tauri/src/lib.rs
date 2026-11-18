use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};
use tauri::{Manager, State};
use nabz_core::accounting::{
    adjust_inventory, create_expense, create_invoice_with_payment_split_idempotent, inventory_valuation,
    list_expenses, profit_loss_report, record_payment, record_invoice_payment, return_purchase_items, return_sale_items,
    save_contact as core_save_contact, save_product as core_save_product,
    void_invoice as core_void_invoice, ContactInput, ExpenseInput, InvoiceInput, InvoiceLineInput,
    PaymentInput, ProductInput, ReturnLineInput,
};

struct Db(Mutex<Connection>);
#[derive(Serialize)]
struct Summary {
 sales_today:i64, receipts_today:i64, customer_balance:i64, supplier_balance:i64,
 inventory_value:i64, contacts:i64, products:i64, invoices:i64,
 sales_month:i64, purchases_month:i64, gross_profit_month:i64, net_profit_month:i64,
 low_stock_count:i64,
}
#[derive(Serialize)]
struct Record { id:i64, name:String, detail:String, amount:i64, status:String, date:String }
#[derive(Serialize)]
struct ProductRecord { id:i64, name:String, code:String, barcode:String, quantity:i64, buy_price:i64, average_cost:i64, sell_price:i64, min_quantity:i64, category:String, unit:String, wholesale_price:i64, is_service:bool, is_active:bool }

fn text(v:&Value,k:&str)->String { v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned() }
fn num(v:&Value,k:&str)->i64 { v.get(k).and_then(|x|x.as_i64().or_else(||x.as_str().and_then(|s|s.replace(',',"").replace('٬',"").parse().ok()))).unwrap_or(0) }
fn normalize_contact_kind(raw:&str)->Option<&'static str>{ nabz_core::normalize_contact_kind(raw) }
fn init(c:&Connection)->Result<(),String>{ nabz_core::initialize(c) }
#[tauri::command] fn app_version()-> &'static str { env!("CARGO_PKG_VERSION") }
#[tauri::command] fn db_status(db:State<'_,Db>)->Result<String,String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.query_row("PRAGMA database_list",[],|r|r.get(2)).map_err(|e|e.to_string())}
#[tauri::command]
fn dashboard_summary(db:State<'_,Db>)->Result<Summary,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let today:String=c.query_row("SELECT date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let month_start:String=c.query_row("SELECT strftime('%Y-%m-01',date('now','localtime'))",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let month=reports_period_with_connection(&c,&month_start,&today)?;
 let value=|key:&str|month.get(key).and_then(Value::as_i64).unwrap_or(0);
 let q=|sql:&str|->Result<i64,String>{c.query_row(sql,[],|r|r.get(0)).map_err(|e|e.to_string())};
 Ok(Summary{
  sales_today:q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date=date('now','localtime')")?,
  receipts_today:q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date=date('now','localtime')),0)-COALESCE((SELECT SUM(cash_refund) FROM invoice_voids WHERE kind='sale' AND void_date=date('now','localtime')),0)")?,
  customer_balance:q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)-COALESCE((SELECT SUM(customer_credit_balance) FROM contacts WHERE kind IN ('customer','both')),0)")?,
  supplier_balance:q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='purchase' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('supplier','both')),0)-COALESCE((SELECT SUM(supplier_credit_balance) FROM contacts WHERE kind IN ('supplier','both')),0)")?,
  inventory_value:q("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products WHERE is_service=0")?,
  contacts:q("SELECT COUNT(*) FROM contacts")?,
  products:q("SELECT COUNT(*) FROM products WHERE is_active=1")?,
  invoices:q("SELECT COUNT(*) FROM invoices")?,
  sales_month:value("sales_total"), purchases_month:value("purchases_total"),
  gross_profit_month:value("gross_profit"), net_profit_month:value("net_profit"),
  low_stock_count:q("SELECT COUNT(*) FROM products WHERE is_active=1 AND is_service=0 AND quantity<=min_quantity")?,
 })
}
#[tauri::command] fn list_records(kind:String,db:State<'_,Db>)->Result<Vec<Record>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let sql=match kind.as_str(){"contacts"=>"SELECT id,name,phone,opening_balance,kind,datetime(created_at) FROM contacts ORDER BY id DESC","products"=>"SELECT id,name,code,quantity,sell_price,'موجود', '' FROM products ORDER BY id DESC","invoices"=>"SELECT id,'فاکتور #'||number||' · '||contact_name,kind,total,CASE WHEN voided=1 THEN 'باطل‌شده' WHEN paid>=total THEN 'تسویه' WHEN paid>0 THEN 'پرداخت جزئی' ELSE 'پرداخت‌نشده' END,invoice_date FROM invoices ORDER BY invoice_date DESC,number DESC","payments"=>"SELECT id,contact_name,kind,amount,method,payment_date FROM payments ORDER BY id DESC",_=>return Err("فهرست نامعتبر است".into())};let mut s=c.prepare(sql).map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Record{id:r.get(0)?,name:r.get(1)?,detail:r.get(2)?,amount:r.get(3)?,status:r.get(4)?,date:r.get(5)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn list_products(db:State<'_,Db>)->Result<Vec<ProductRecord>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut s=c.prepare("SELECT id,name,code,barcode,quantity,buy_price,average_cost,sell_price,min_quantity,category,unit,wholesale_price,is_service,is_active FROM products ORDER BY is_active DESC,name COLLATE NOCASE,id DESC").map_err(|e|e.to_string())?;
 let rows=s.query_map([],|r|Ok(ProductRecord{id:r.get(0)?,name:r.get(1)?,code:r.get(2)?,barcode:r.get(3)?,quantity:r.get(4)?,buy_price:r.get(5)?,average_cost:r.get(6)?,sell_price:r.get(7)?,min_quantity:r.get(8)?,category:r.get(9)?,unit:r.get(10)?,wholesale_price:r.get(11)?,is_service:r.get::<_,i64>(12)?!=0,is_active:r.get::<_,i64>(13)?!=0})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn delete_record(kind:String,id:i64,db:State<'_,Db>)->Result<(),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 match kind.as_str(){
  "contacts"=>{let n:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM invoices WHERE contact_id=?1)+(SELECT COUNT(*) FROM payments WHERE contact_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این طرف‌حساب سابقه فاکتور یا دریافت/پرداخت دارد و حذف نمی‌شود.".into())}tx.execute("DELETE FROM contacts WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "products"=>{let quantity:Option<i64>=tx.query_row("SELECT quantity FROM products WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;let quantity=quantity.ok_or("کالا پیدا نشد")?;if quantity!=0{return Err("برای غیرفعال‌سازی، ابتدا موجودی کالا را به صفر برسانید؛ سوابق کالا حذف نمی‌شوند.".into())}tx.execute("UPDATE products SET is_active=0 WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "invoices"=>return Err("فاکتور حذف نمی‌شود تا سابقهٔ مالی و انبار باقی بماند؛ از عملیات ابطال امن استفاده کنید.".into()),
  _=>return Err("حذف این نوع رکورد برای جلوگیری از آسیب به گردش مالی مجاز نیست.".into())
 }
 tx.commit().map_err(|e|e.to_string())?;Ok(())
}
fn validate_opening_balance(kind:&str,balance:i64)->Result<(),String>{
 if balance<0{return Err("ماندهٔ افتتاحیهٔ منفی فعلاً پشتیبانی نمی‌شود؛ مبلغ را مثبت وارد کنید یا تا تعیین جهت مانده صفر بگذارید.".into())}
 if kind=="both"&&balance!=0{return Err("برای طرف‌حساب دوگانه، تا زمان پشتیبانی از دو ماندهٔ جداگانه، ماندهٔ اولیه را صفر بگذارید.".into())}
 Ok(())
}
#[tauri::command]
fn save_contact(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 save_contact_with_connection(payload,&c)
}
fn save_contact_with_connection(payload:Value,c:&Connection)->Result<i64,String>{
 let raw_kind=text(&payload,"kind");
 let kind=normalize_contact_kind(&raw_kind).ok_or("نوع طرف‌حساب را از گزینه‌های موجود انتخاب کنید")?;
 validate_opening_balance(kind,num(&payload,"balance"))?;
 let input=ContactInput{
  name:text(&payload,"name"),
  kind:kind.to_string(),
  phone:text(&payload,"phone"),
  opening_balance:num(&payload,"balance"),
  notes:text(&payload,"notes"),
 };
 core_save_contact(c,payload.get("id").and_then(Value::as_i64),&input)
}
#[tauri::command]
fn save_product(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 save_product_with_connection(payload,&c)
}
fn save_product_with_connection(payload:Value,c:&Connection)->Result<i64,String>{
 let input=ProductInput{
  name:text(&payload,"name"),
  code:text(&payload,"code"),
  barcode:text(&payload,"barcode"),
  buy_price:num(&payload,"buy"),
  sell_price:num(&payload,"sell"),
  quantity:num(&payload,"qty"),
  min_quantity:num(&payload,"min"),
 };
 let category=text(&payload,"category");
 let unit=text(&payload,"unit");
 let unit=if unit.is_empty(){"عدد".to_string()}else{unit};
 let wholesale=num(&payload,"wholesale");
 let is_service=payload.get("is_service").and_then(Value::as_bool).unwrap_or(false);
 if wholesale<0{return Err("قیمت عمده نمی‌تواند منفی باشد".into())}
 if !["عدد","متر","کیلوگرم","گرم","لیتر","بسته","حلقه","شاخه","جعبه","دستگاه","سرویس"].contains(&unit.as_str()) {
  return Err("واحد شمارش را از واحدهای معتبر انتخاب کنید".into())
 }
 if is_service && input.quantity!=0 {
  return Err("خدمت موجودی انبار ندارد؛ موجودی آن باید صفر باشد".into())
 }
 let barcode_changed=if let Some(id)=payload.get("id").and_then(Value::as_i64) {
  let current:String=c.query_row("SELECT barcode FROM products WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  current!=input.barcode
 } else { true };
 if barcode_changed && !input.barcode.is_empty() {
  let duplicate:i64=c.query_row("SELECT COUNT(*) FROM products WHERE barcode=?1 AND is_active=1 AND (?2 IS NULL OR id<>?2)",rusqlite::params![input.barcode,payload.get("id").and_then(Value::as_i64)],|r|r.get(0)).map_err(|e|e.to_string())?;
  if duplicate>0{return Err("این بارکد قبلاً برای کالای دیگری ثبت شده است".into())}
 }
 if let Some(id)=payload.get("id").and_then(Value::as_i64) {
  let old_service:i64=c.query_row("SELECT is_service FROM products WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if (old_service!=0)!=is_service {
   let history:i64=c.query_row("SELECT COUNT(*) FROM invoice_items WHERE product_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
   if history>0{return Err("نوع کالا/خدمت پس از ثبت سابقه فاکتور قابل تغییر نیست".into())}
  }
 }
 let id=core_save_product(c,payload.get("id").and_then(Value::as_i64),&input)?;
 c.execute("UPDATE products SET category=?1,unit=?2,wholesale_price=?3,is_service=?4,quantity=CASE WHEN ?4=1 THEN 0 ELSE quantity END,average_cost=CASE WHEN ?4=1 THEN 0 ELSE average_cost END WHERE id=?5",
  rusqlite::params![category,unit,wholesale,is_service as i64,id]).map_err(|e|e.to_string())?;
 Ok(id)
}
#[tauri::command]
fn save_invoice(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 save_invoice_with_connection(payload,&mut c)
}
fn save_invoice_with_connection(payload:Value,c:&mut Connection)->Result<i64,String>{
 let kind=text(&payload,"kind");
 let contact=text(&payload,"customer");
 if !["sale","purchase"].contains(&kind.as_str()){return Err("نوع فاکتور نامعتبر است".into())}
 let mut lines:Vec<InvoiceLineInput>=Vec::new();
 if let Some(arr)=payload.get("items").and_then(Value::as_array){
  for x in arr {
   let name=x.get("item").and_then(Value::as_str).unwrap_or("").trim().to_owned();
   let qty=x.get("qty").and_then(|v|v.as_i64().or_else(||v.as_str().and_then(|n|n.parse().ok()))).unwrap_or(0);
   let price=x.get("price").and_then(|v|v.as_i64().or_else(||v.as_str().and_then(|n|n.parse().ok()))).unwrap_or(0);
   if !name.is_empty(){lines.push(InvoiceLineInput{product_lookup:name,quantity:qty,unit_price:price});}
  }
 }else{
  for i in 1..=5{
   let suffix=if i==1{String::new()}else{i.to_string()};
   let name=text(&payload,&format!("item{suffix}"));
   let qty=num(&payload,&format!("qty{suffix}"));
   let price=num(&payload,&format!("price{suffix}"));
   if !name.is_empty(){lines.push(InvoiceLineInput{product_lookup:name,quantity:qty,unit_price:price});}
  }
 }
 if lines.is_empty(){return Err("حداقل یک ردیف کالا یا خدمت وارد کنید".into())}
 if lines.iter().any(|x|x.quantity<=0||x.unit_price<0){return Err("تعداد هر ردیف باید مثبت و قیمت‌ها نامنفی باشند".into())}
 let subtotal=lines.iter().try_fold(0i64,|sum,line|line.quantity.checked_mul(line.unit_price).and_then(|v|sum.checked_add(v))).ok_or("مبلغ فاکتور بیش از حد بزرگ است")?;
 let discount=num(&payload,"discount");
 if discount<0||discount>subtotal{return Err("تخفیف باید بین صفر و مبلغ کل باشد".into())}
 let total=subtotal-discount;

 let contact_id:Option<i64>=if contact.is_empty(){None}else{
  c.query_row("SELECT id FROM contacts WHERE name=?1 COLLATE NOCASE LIMIT 1",[&contact],|r|r.get(0)).optional().map_err(|e|e.to_string())?
 };
 if kind=="purchase"&&contact_id.is_none(){return Err("انتخاب تأمین‌کنندهٔ ثبت‌شده برای فاکتور خرید الزامی است".into())}
 if kind=="sale"&&!contact.is_empty()&&contact_id.is_none(){return Err("مشتری واردشده در فهرست طرف‌حساب‌ها پیدا نشد؛ نام را انتخاب کنید یا برای فروش گذری فیلد مشتری را خالی بگذارید.".into())}

 let credit_column=if kind=="sale"{"customer_credit_balance"}else{"supplier_credit_balance"};
 let credit_available:i64=if let Some(id)=contact_id{
  c.query_row(&format!("SELECT {credit_column} FROM contacts WHERE id=?1"),[id],|r|r.get(0)).map_err(|e|e.to_string())?
 }else{0};

 let (cash_received,credit_used)=if kind=="sale"{
  let status=text(&payload,"payment_status");
  let status=if status.is_empty(){"credit"}else{status.as_str()};
  if !["paid","partial","credit"].contains(&status){return Err("وضعیت پرداخت نامعتبر است".into())}
  let requested=match status{
   "paid"=>total,
   "partial"=>num(&payload,"paid_amount"),
   _=>0
  };
  if requested<0||requested>total{return Err("مبلغ دریافتی باید بین صفر و مبلغ نهایی فاکتور باشد".into())}
  if status=="partial"&&(requested<=0||requested>=total){return Err("مبلغ دریافت‌شده در پرداخت جزئی باید بیشتر از صفر و کمتر از مبلغ فاکتور باشد".into())}
  if requested<total&&contact_id.is_none(){return Err("برای فروش نسیه یا پرداخت ناقص، مشتری ثبت‌شده انتخاب کنید؛ فروش گذری باید تسویه کامل باشد.".into())}
  match status{
   "paid"=>{let credit=credit_available.min(total);(total-credit,credit)},
   "partial"=>{let credit=credit_available.min(total-requested);(requested,credit)},
   _=>{let credit=credit_available.min(total);(0,credit)}
  }
 }else{
  (0,credit_available.min(total))
 };
 let paid=cash_received.checked_add(credit_used).ok_or("مبلغ تسویه بیش از حد بزرگ است")?;
 let input=InvoiceInput{
  kind:kind.clone(),
  contact_name:contact,
  date:text(&payload,"date"),
  discount,
  paid,
  method:text(&payload,"method"),
  notes:text(&payload,"notes"),
  lines,
 };
 let request_key=text(&payload,"idempotency_key");
 create_invoice_with_payment_split_idempotent(c,&input,cash_received,credit_used,if request_key.is_empty(){None}else{Some(request_key.as_str())})
}
#[tauri::command]
fn save_invoice_payment(invoice_id:i64,payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let input=PaymentInput{
  kind:text(&payload,"kind"),
  contact_name:text(&payload,"customer"),
  amount:num(&payload,"amount"),
  method:text(&payload,"method"),
  date:text(&payload,"date"),
  notes:text(&payload,"notes"),
 };
 record_invoice_payment(&mut c,invoice_id,&input)
}

#[tauri::command]
fn save_invoice_draft(payload:Value,draft_id:Option<i64>,db:State<'_,Db>)->Result<i64,String>{
 let kind=text(&payload,"kind");
 if !["sale","purchase"].contains(&kind.as_str()){return Err("نوع پیش‌نویس فاکتور نامعتبر است".into())}
 let json=serde_json::to_string(&payload).map_err(|e|e.to_string())?;
 if json.len()>1_000_000{return Err("حجم پیش‌نویس بیش از حد مجاز است".into())}
 let title=text(&payload,"customer");
 let c=db.0.lock().map_err(|e|e.to_string())?;
 if let Some(id)=draft_id{
  let changed=c.execute("UPDATE invoice_drafts SET kind=?1,title=?2,payload_json=?3,updated_at=datetime('now','localtime') WHERE id=?4",params![kind,title,json,id]).map_err(|e|e.to_string())?;
  if changed==0{return Err("پیش‌نویس پیدا نشد؛ آن را دوباره ذخیره کنید".into())}
  Ok(id)
 }else{
  c.execute("INSERT INTO invoice_drafts(kind,title,payload_json) VALUES(?1,?2,?3)",params![kind,title,json]).map_err(|e|e.to_string())?;
  Ok(c.last_insert_rowid())
 }
}
#[tauri::command]
fn list_invoice_drafts(db:State<'_,Db>)->Result<Vec<Value>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut st=c.prepare("SELECT id,kind,title,updated_at FROM invoice_drafts ORDER BY updated_at DESC,id DESC").map_err(|e|e.to_string())?;
 let rows=st.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,"updated_at":r.get::<_,String>(3)?}))).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn get_invoice_draft(id:i64,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let raw:String=c.query_row("SELECT payload_json FROM invoice_drafts WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or("پیش‌نویس پیدا نشد")?;
 serde_json::from_str(&raw).map_err(|e|format!("محتوای پیش‌نویس معتبر نیست: {e}"))
}
#[tauri::command]
fn delete_invoice_draft(id:i64,db:State<'_,Db>)->Result<(),String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let n=c.execute("DELETE FROM invoice_drafts WHERE id=?1",[id]).map_err(|e|e.to_string())?;
 if n==0{return Err("پیش‌نویس پیدا نشد".into())} Ok(())
}

#[tauri::command]
fn save_payment(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 save_payment_with_connection(payload,&mut c)
}
fn save_payment_with_connection(payload:Value,c:&mut Connection)->Result<i64,String>{
 let input=PaymentInput{
  kind:text(&payload,"kind"),
  contact_name:text(&payload,"customer"),
  amount:num(&payload,"amount"),
  method:text(&payload,"method"),
  date:text(&payload,"date"),
  notes:text(&payload,"notes"),
 };
 record_payment(c,&input)
}


#[derive(Serialize)]
struct SalesTrendPoint { date:String, total:i64 }
#[tauri::command]
fn sales_trend(days:i64,db:State<'_,Db>)->Result<Vec<SalesTrendPoint>,String>{
 if !(7..=30).contains(&days){return Err("بازهٔ نمودار باید ۷ یا ۳۰ روز باشد".into())}
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut st=c.prepare("WITH RECURSIVE days(d,n) AS (SELECT date('now','localtime', '-' || (?1-1) || ' days'),0 UNION ALL SELECT date(d,'+1 day'),n+1 FROM days WHERE n<?1-1) SELECT days.d,COALESCE(SUM(invoices.total),0) FROM days LEFT JOIN invoices ON invoices.invoice_date=days.d AND invoices.kind='sale' AND invoices.voided=0 GROUP BY days.d ORDER BY days.d").map_err(|e|e.to_string())?;
 let rows=st.query_map([days],|r|Ok(SalesTrendPoint{date:r.get(0)?,total:r.get(1)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn reports_summary(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 reports_summary_with_connection(&c)
}
fn reports_summary_with_connection(c:&Connection)->Result<Value,String>{
 let q=|sql:&str|->Result<i64,String>{c.query_row(sql,[],|r|r.get(0)).map_err(|e|e.to_string())};
 Ok(json!({
  "sales_today":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date=date('now','localtime')")?,
  "receipts_today":q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date=date('now','localtime')),0)-COALESCE((SELECT SUM(cash_refund) FROM invoice_voids WHERE kind='sale' AND void_date=date('now','localtime')),0)")?,
  "sales_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0")?,
  "purchases_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase' AND voided=0")?,
  "receipts_total":q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt'),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds),0)-COALESCE((SELECT SUM(cash_refund) FROM invoice_voids WHERE kind='sale'),0)")?,
  "payments_total":q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='payment'),0)-COALESCE((SELECT SUM(amount) FROM purchase_refunds),0)")?,
  "customer_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)-COALESCE((SELECT SUM(customer_credit_balance) FROM contacts WHERE kind IN ('customer','both')),0)")?,
  "supplier_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='purchase' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('supplier','both')),0)-COALESCE((SELECT SUM(supplier_credit_balance) FROM contacts WHERE kind IN ('supplier','both')),0)")?,
  "inventory_value":q("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products")?,
  "contacts":q("SELECT COUNT(*) FROM contacts")?,
  "products":q("SELECT COUNT(*) FROM products")?,
  "invoices":q("SELECT COUNT(*) FROM invoices")?
 }))
}
fn valid_iso_date(value:&str)->bool{
 let bytes=value.as_bytes();
 if bytes.len()!=10||bytes[4]!=b'-'||bytes[7]!=b'-'||!bytes.iter().enumerate().all(|(i,b)|i==4||i==7||b.is_ascii_digit()){return false}
 let year=value[0..4].parse::<i32>().ok();
 let month=value[5..7].parse::<u32>().ok();
 let day=value[8..10].parse::<u32>().ok();
 let (Some(year),Some(month),Some(day))=(year,month,day) else{return false};
 let leap=year%4==0&&(year%100!=0||year%400==0);
 let max=match month{1|3|5|7|8|10|12=>31,4|6|9|11=>30,2=>if leap{29}else{28},_=>return false};
 day>=1&&day<=max
}
#[tauri::command]
fn reports_period(start_date:String,end_date:String,contact_id:Option<i64>,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 reports_period_filtered_with_connection(&c,&start_date,&end_date,contact_id)
}
fn reports_period_with_connection(c:&Connection,start:&str,end:&str)->Result<Value,String>{
 reports_period_filtered_with_connection(c,start,end,None)
}
fn reports_period_filtered_with_connection(c:&Connection,start:&str,end:&str,contact_id:Option<i64>)->Result<Value,String>{
 if !valid_iso_date(start)||!valid_iso_date(end){return Err("تاریخ شروع و پایان باید تاریخ میلادی معتبر با قالب YYYY-MM-DD باشند.".into())}
 if start>end{return Err("تاریخ شروع باید قبل از تاریخ پایان باشد.".into())}
 let span:i64=c.query_row("SELECT CAST(julianday(?2)-julianday(?1) AS INTEGER)",params![start,end],|r|r.get(0)).map_err(|e|e.to_string())?;
 if span>365{return Err("بازهٔ گزارش حداکثر می‌تواند ۳۶۶ روز باشد.".into())}
 if let Some(id)=contact_id{
  if id<=0{return Err("طرف‌حساب انتخاب‌شده معتبر نیست.".into())}
  let exists:i64=c.query_row("SELECT COUNT(*) FROM contacts WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if exists==0{return Err("طرف‌حساب انتخاب‌شده پیدا نشد.".into())}
 }
 let sum=|sql:&str|->Result<i64,String>{c.query_row(sql,params![start,end,contact_id],|r|r.get(0)).map_err(|e|e.to_string())};
 let sales_total=sum("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR contact_id=?3)")?;
 let purchases_total=sum("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR contact_id=?3)")?;
 let receipts_total=sum("SELECT COALESCE((SELECT SUM(p.amount) FROM payments p WHERE p.kind='receipt' AND p.payment_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR p.contact_id=?3)),0)-COALESCE((SELECT SUM(r.amount) FROM sales_refunds r JOIN invoices i ON i.id=r.invoice_id WHERE r.refund_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)),0)-COALESCE((SELECT SUM(v.cash_refund) FROM invoice_voids v JOIN invoices i ON i.id=v.invoice_id WHERE v.kind='sale' AND v.void_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)),0)")?;
 let payments_total=sum("SELECT COALESCE((SELECT SUM(p.amount) FROM payments p WHERE p.kind='payment' AND p.payment_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR p.contact_id=?3)),0)-COALESCE((SELECT SUM(r.amount) FROM purchase_refunds r JOIN invoices i ON i.id=r.invoice_id WHERE r.refund_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)),0)")?;
 let sales_returns_total=sum("SELECT COALESCE(SUM(r.amount),0) FROM sales_returns r JOIN invoices i ON i.id=r.invoice_id WHERE r.return_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)")?;
 let purchase_returns_total=sum("SELECT COALESCE(SUM(r.amount),0) FROM purchase_returns r JOIN invoices i ON i.id=r.invoice_id WHERE r.return_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)")?;
 let sales_count=sum("SELECT COUNT(*) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR contact_id=?3)")?;
 let purchases_count=sum("SELECT COUNT(*) FROM invoices WHERE kind='purchase' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR contact_id=?3)")?;
 let cost_of_goods_sold=sum("SELECT COALESCE(SUM(ii.unit_cost * (ii.quantity-COALESCE((SELECT SUM(sri.quantity) FROM sales_return_items sri WHERE sri.invoice_item_id=ii.id),0))),0) FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id WHERE i.kind='sale' AND i.voided=0 AND ii.unit_cost IS NOT NULL AND i.invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)")?;
 let uncosted_sales=sum("SELECT COALESCE(SUM(ii.line_total-COALESCE((SELECT SUM(sri.amount) FROM sales_return_items sri WHERE sri.invoice_item_id=ii.id),0)),0) FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id WHERE i.kind='sale' AND i.voided=0 AND ii.unit_cost IS NULL AND ii.item_name NOT LIKE 'خدمت:%' AND i.invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3)")?;
 let expenses_total=sum("SELECT COALESCE(SUM(amount),0) FROM expenses WHERE expense_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR 1=1)")?;
 let inventory_value=sum("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products WHERE is_service=0 AND (?3 IS NULL OR 1=1)")?;
 let customer_balance=sum("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0 AND (?3 IS NULL OR contact_id=?3)),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both') AND (?3 IS NULL OR id=?3)),0)-COALESCE((SELECT SUM(customer_credit_balance) FROM contacts WHERE kind IN ('customer','both') AND (?3 IS NULL OR id=?3)),0)")?;
 let supplier_balance=sum("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='purchase' AND voided=0 AND (?3 IS NULL OR contact_id=?3)),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('supplier','both') AND (?3 IS NULL OR id=?3)),0)-COALESCE((SELECT SUM(supplier_credit_balance) FROM contacts WHERE kind IN ('supplier','both') AND (?3 IS NULL OR id=?3)),0)")?;
 let low_stock_count=sum("SELECT COUNT(*) FROM products WHERE is_active=1 AND is_service=0 AND quantity<=min_quantity AND (?3 IS NULL OR 1=1)")?;
 let gross_profit=sales_total.checked_sub(cost_of_goods_sold).ok_or("محاسبه سود ناخالص از ظرفیت عددی خارج شد")?;
 let net_profit=gross_profit.checked_sub(expenses_total).ok_or("محاسبه سود خالص از ظرفیت عددی خارج شد")?;
 let daily_sales={
  let mut st=c.prepare("WITH RECURSIVE days(d) AS (SELECT date(?1) UNION ALL SELECT date(d,'+1 day') FROM days WHERE d<date(?2)) SELECT days.d,COALESCE(SUM(i.total),0) FROM days LEFT JOIN invoices i ON i.invoice_date=days.d AND i.kind='sale' AND i.voided=0 AND (?3 IS NULL OR i.contact_id=?3) GROUP BY days.d ORDER BY days.d").map_err(|e|e.to_string())?;
  let rows=st.query_map(params![start,end,contact_id],|r|Ok(json!({"date":r.get::<_,String>(0)?,"total":r.get::<_,i64>(1)?}))).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let top_products={
  let mut st=c.prepare("SELECT ii.item_name,COALESCE(p.code,''),SUM(ii.quantity-COALESCE((SELECT SUM(sri.quantity) FROM sales_return_items sri WHERE sri.invoice_item_id=ii.id),0)) AS sold_quantity FROM invoice_items ii JOIN invoices i ON i.id=ii.invoice_id LEFT JOIN products p ON p.id=ii.product_id WHERE i.kind='sale' AND i.voided=0 AND i.invoice_date BETWEEN ?1 AND ?2 AND (?3 IS NULL OR i.contact_id=?3) GROUP BY ii.item_name,p.code HAVING sold_quantity>0 ORDER BY sold_quantity DESC,ii.item_name COLLATE NOCASE LIMIT 10").map_err(|e|e.to_string())?;
  let rows=st.query_map(params![start,end,contact_id],|r|Ok(json!({"name":r.get::<_,String>(0)?,"code":r.get::<_,String>(1)?,"quantity":r.get::<_,i64>(2)?}))).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 Ok(json!({
  "start_date":start,"end_date":end,"contact_id":contact_id,
  "sales_total":sales_total,"purchases_total":purchases_total,
  "receipts_total":receipts_total,"payments_total":payments_total,
  "sales_returns_total":sales_returns_total,"purchase_returns_total":purchase_returns_total,
  "sales_count":sales_count,"purchases_count":purchases_count,
  "cost_of_goods_sold":cost_of_goods_sold,"gross_profit":gross_profit,
  "expenses_total":expenses_total,"net_profit":net_profit,"uncosted_sales":uncosted_sales,
  "inventory_value":inventory_value,"customer_balance":customer_balance,
  "supplier_balance":supplier_balance,"low_stock_count":low_stock_count,
  "daily_sales":daily_sales,"top_products":top_products,
  "expenses_are_store_wide":true
 }))
}

fn collect_json_rows<F>(c:&Connection,sql:&str,mapper:F)->Result<Vec<Value>,String>
where F:FnMut(&rusqlite::Row<'_>)->rusqlite::Result<Value>{
 let mut statement=c.prepare(sql).map_err(|e|e.to_string())?;
 let rows=statement.query_map([],mapper).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn export_data(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let contacts=collect_json_rows(&c,"SELECT id,name,kind,phone,opening_balance,notes,created_at,customer_credit_balance,supplier_credit_balance FROM contacts",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,
  "phone":r.get::<_,String>(3)?,"opening_balance":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?,
  "created_at":r.get::<_,String>(6)?,"customer_credit_balance":r.get::<_,i64>(7)?,"supplier_credit_balance":r.get::<_,i64>(8)?
})))?;
 let products=collect_json_rows(&c,"SELECT id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost,category,unit,wholesale_price,is_service,is_active FROM products",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"code":r.get::<_,String>(2)?,
  "barcode":r.get::<_,String>(3)?,"buy_price":r.get::<_,i64>(4)?,"sell_price":r.get::<_,i64>(5)?,
  "quantity":r.get::<_,i64>(6)?,"min_quantity":r.get::<_,i64>(7)?,"average_cost":r.get::<_,i64>(8)?,
  "category":r.get::<_,String>(9)?,"unit":r.get::<_,String>(10)?,"wholesale_price":r.get::<_,i64>(11)?,
  "is_service":r.get::<_,i64>(12)?,"is_active":r.get::<_,i64>(13)?
})))?;
 let invoices=collect_json_rows(&c,"SELECT id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied,idempotency_key FROM invoices",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,
  "contact_id":r.get::<_,Option<i64>>(3)?,"contact_name":r.get::<_,String>(4)?,"invoice_date":r.get::<_,String>(5)?,
  "subtotal":r.get::<_,i64>(6)?,"discount":r.get::<_,i64>(7)?,"total":r.get::<_,i64>(8)?,
  "paid":r.get::<_,i64>(9)?,"notes":r.get::<_,String>(10)?,"voided":r.get::<_,i64>(11)?,
  "credit_applied":r.get::<_,i64>(12)?
})))?;
 let invoice_drafts=collect_json_rows(&c,"SELECT id,kind,title,payload_json,created_at,updated_at FROM invoice_drafts",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,
  "payload_json":r.get::<_,String>(3)?,"created_at":r.get::<_,String>(4)?,"updated_at":r.get::<_,String>(5)?
 })))?;
 let invoice_items=collect_json_rows(&c,"SELECT id,invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost,quantity_before,average_cost_before,buy_price_before FROM invoice_items",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"product_id":r.get::<_,Option<i64>>(2)?,
  "item_name":r.get::<_,String>(3)?,"quantity":r.get::<_,i64>(4)?,"unit_price":r.get::<_,i64>(5)?,
  "line_total":r.get::<_,i64>(6)?,"unit_cost":r.get::<_,Option<i64>>(7)?,
  "quantity_before":r.get::<_,Option<i64>>(8)?,"average_cost_before":r.get::<_,Option<i64>>(9)?,"buy_price_before":r.get::<_,Option<i64>>(10)?
})))?;
 let payments=collect_json_rows(&c,"SELECT id,kind,contact_id,contact_name,amount,method,payment_date,notes FROM payments",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"contact_id":r.get::<_,Option<i64>>(2)?,
  "contact_name":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"method":r.get::<_,String>(5)?,
  "payment_date":r.get::<_,String>(6)?,"notes":r.get::<_,String>(7)?
})))?;
 let invoice_voids=collect_json_rows(&c,"SELECT id,invoice_id,kind,void_date,cash_refund,credit_refund,notes FROM invoice_voids",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,
  "void_date":r.get::<_,String>(3)?,"cash_refund":r.get::<_,i64>(4)?,
  "credit_refund":r.get::<_,i64>(5)?,"notes":r.get::<_,String>(6)?
})))?;
 let payment_allocations=collect_json_rows(&c,"SELECT id,payment_id,invoice_id,contact_id,allocation_type,amount,created_at FROM payment_allocations",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"payment_id":r.get::<_,Option<i64>>(1)?,"invoice_id":r.get::<_,Option<i64>>(2)?,
  "contact_id":r.get::<_,Option<i64>>(3)?,"allocation_type":r.get::<_,String>(4)?,
  "amount":r.get::<_,i64>(5)?,"created_at":r.get::<_,String>(6)?
})))?;
 let settings=collect_json_rows(&c,"SELECT key,value FROM settings",|r|Ok(json!({"key":r.get::<_,String>(0)?,"value":r.get::<_,String>(1)?})))?;
 let sales_returns=collect_json_rows(&c,"SELECT id,invoice_id,return_date,amount,notes,credit_refund FROM sales_returns",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"return_date":r.get::<_,String>(2)?,
  "amount":r.get::<_,i64>(3)?,"notes":r.get::<_,String>(4)?,"credit_refund":r.get::<_,i64>(5)?
})))?;
 let sales_return_items=collect_json_rows(&c,"SELECT id,return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount FROM sales_return_items",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"return_id":r.get::<_,i64>(1)?,"invoice_item_id":r.get::<_,i64>(2)?,
  "product_id":r.get::<_,i64>(3)?,"item_name":r.get::<_,String>(4)?,"quantity":r.get::<_,i64>(5)?,
  "unit_price":r.get::<_,i64>(6)?,"amount":r.get::<_,i64>(7)?
})))?;
 let sales_refunds=collect_json_rows(&c,"SELECT id,invoice_id,return_id,refund_date,amount,method,notes FROM sales_refunds",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"return_id":r.get::<_,i64>(2)?,
  "refund_date":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"method":r.get::<_,String>(5)?,"notes":r.get::<_,String>(6)?
})))?;
 let purchase_returns=collect_json_rows(&c,"SELECT id,invoice_id,return_date,amount,notes,credit_refund FROM purchase_returns",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"return_date":r.get::<_,String>(2)?,
  "amount":r.get::<_,i64>(3)?,"notes":r.get::<_,String>(4)?,"credit_refund":r.get::<_,i64>(5)?
})))?;
 let purchase_return_items=collect_json_rows(&c,"SELECT id,return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount FROM purchase_return_items",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"return_id":r.get::<_,i64>(1)?,"invoice_item_id":r.get::<_,i64>(2)?,
  "product_id":r.get::<_,i64>(3)?,"item_name":r.get::<_,String>(4)?,"quantity":r.get::<_,i64>(5)?,
  "unit_price":r.get::<_,i64>(6)?,"amount":r.get::<_,i64>(7)?
})))?;
 let purchase_refunds=collect_json_rows(&c,"SELECT id,invoice_id,return_id,refund_date,amount,method,notes FROM purchase_refunds",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"return_id":r.get::<_,i64>(2)?,
  "refund_date":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"method":r.get::<_,String>(5)?,"notes":r.get::<_,String>(6)?
})))?;
 let inventory_adjustments=collect_json_rows(&c,"SELECT id,product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at FROM inventory_adjustments",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"product_id":r.get::<_,i64>(1)?,"product_name":r.get::<_,String>(2)?,
  "previous_quantity":r.get::<_,i64>(3)?,"new_quantity":r.get::<_,i64>(4)?,"delta":r.get::<_,i64>(5)?,
  "reason":r.get::<_,String>(6)?,"created_at":r.get::<_,String>(7)?
})))?;
 let expenses=collect_json_rows(&c,"SELECT id,category,description,amount,expense_date,created_at FROM expenses",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"category":r.get::<_,String>(1)?,"description":r.get::<_,String>(2)?,
  "amount":r.get::<_,i64>(3)?,"expense_date":r.get::<_,String>(4)?,"created_at":r.get::<_,String>(5)?
})))?;
 Ok(json!({
  "format":"nabz-backup-v2","contacts":contacts,"products":products,"invoices":invoices,
  "invoice_items":invoice_items,"payments":payments,"invoice_voids":invoice_voids,
  "payment_allocations":payment_allocations,"settings":settings,
  "sales_returns":sales_returns,"sales_return_items":sales_return_items,"sales_refunds":sales_refunds,
  "purchase_returns":purchase_returns,"purchase_return_items":purchase_return_items,"purchase_refunds":purchase_refunds,
  "inventory_adjustments":inventory_adjustments,"expenses":expenses,"invoice_drafts":invoice_drafts
 }))
}
fn backup_has_int(x:&Value,key:&str)->bool{x.get(key).and_then(Value::as_i64).is_some()}
fn backup_has_string(x:&Value,key:&str)->bool{x.get(key).and_then(Value::as_str).is_some()}
fn backup_has_optional_int(x:&Value,key:&str)->bool{x.get(key).is_some_and(|v|v.is_null()||v.as_i64().is_some())}
fn backup_optional_array(data:&Value,key:&str)->Vec<Value>{data.get(key).and_then(Value::as_array).cloned().unwrap_or_default()}
fn validate_backup(data:&Value)->Result<(),String>{
 let format=data.get("format").and_then(Value::as_str).unwrap_or("");
 if !["nabz-backup-v1","nabz-backup-v2"].contains(&format){return Err("نسخهٔ فایل پشتیبان نبض پشتیبانی نمی‌شود".into())}
 for key in ["contacts","products","invoices","invoice_items","payments","settings"]{
  if !data.get(key).and_then(Value::as_array).is_some(){return Err(format!("ساختار پشتیبان ناقص است: {key}"))}
 }
 let optional_keys=["sales_returns","sales_return_items","sales_refunds","purchase_returns","purchase_return_items","purchase_refunds","inventory_adjustments","expenses","invoice_voids","payment_allocations","invoice_drafts"];
 for key in optional_keys{
  if data.get(key).is_some()&&!data[key].is_array(){return Err(format!("ساختار بخش {key} در پشتیبان معتبر نیست"))}
 }
 for key in ["contacts","products","invoices","invoice_items","payments","settings"]{
  for (i,x) in data[key].as_array().unwrap().iter().enumerate(){
   let bad=match key{
    "contacts"=>!(backup_has_int(x,"id")&&backup_has_string(x,"name")&&!x["name"].as_str().unwrap_or("").trim().is_empty()
      &&backup_has_string(x,"kind")&&backup_has_string(x,"phone")&&backup_has_int(x,"opening_balance")&&backup_has_string(x,"notes")
      &&backup_has_string(x,"created_at")&&(x.get("customer_credit_balance").is_none()||x["customer_credit_balance"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("supplier_credit_balance").is_none()||x["supplier_credit_balance"].as_i64().unwrap_or(-1)>=0)),
    "products"=>!(backup_has_int(x,"id")&&backup_has_string(x,"name")&&!x["name"].as_str().unwrap_or("").trim().is_empty()
      &&backup_has_string(x,"code")&&!x["code"].as_str().unwrap_or("").trim().is_empty()&&backup_has_string(x,"barcode")
      &&backup_has_int(x,"buy_price")&&backup_has_int(x,"sell_price")&&backup_has_int(x,"quantity")&&backup_has_int(x,"min_quantity")
      &&x["buy_price"].as_i64().unwrap_or(-1)>=0&&x["sell_price"].as_i64().unwrap_or(-1)>=0
      &&x["quantity"].as_i64().unwrap_or(-1)>=0&&x["min_quantity"].as_i64().unwrap_or(-1)>=0
      &&(x.get("average_cost").is_none()||x["average_cost"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("wholesale_price").is_none()||x["wholesale_price"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("is_service").is_none()||[0,1].contains(&x["is_service"].as_i64().unwrap_or(-1)))
      &&(x.get("is_active").is_none()||[0,1].contains(&x["is_active"].as_i64().unwrap_or(-1)))
      &&(x.get("category").is_none()||x["category"].is_string())
      &&(x.get("unit").is_none()||x["unit"].is_string())),
    "invoices"=>!(backup_has_int(x,"id")&&backup_has_int(x,"number")&&backup_has_string(x,"kind")
      &&["sale","purchase"].contains(&x["kind"].as_str().unwrap_or(""))&&backup_has_optional_int(x,"contact_id")
      &&backup_has_string(x,"contact_name")&&backup_has_string(x,"invoice_date")&&backup_has_int(x,"subtotal")
      &&backup_has_int(x,"discount")&&backup_has_int(x,"total")&&backup_has_int(x,"paid")&&backup_has_string(x,"notes")
      &&x["subtotal"].as_i64().unwrap_or(-1)>=0&&x["discount"].as_i64().unwrap_or(-1)>=0
      &&x["total"].as_i64().unwrap_or(-1)>=0&&x["paid"].as_i64().unwrap_or(-1)>=0
      &&x["paid"].as_i64().unwrap_or(i64::MAX)<=x["total"].as_i64().unwrap_or(-1)
      &&(x.get("voided").is_none()||[0,1].contains(&x["voided"].as_i64().unwrap_or(-1)))
      &&(x.get("credit_applied").is_none()||(x["credit_applied"].as_i64().unwrap_or(-1)>=0&&x["credit_applied"].as_i64().unwrap_or(i64::MAX)<=x["paid"].as_i64().unwrap_or(-1)))
      &&(x.get("idempotency_key").is_none()||x["idempotency_key"].is_null()||x["idempotency_key"].as_str().is_some_and(|v|v.len()<=128))),
    "invoice_items"=>!(backup_has_int(x,"id")&&backup_has_int(x,"invoice_id")&&backup_has_optional_int(x,"product_id")
      &&backup_has_string(x,"item_name")&&!x["item_name"].as_str().unwrap_or("").trim().is_empty()
      &&backup_has_int(x,"quantity")&&x["quantity"].as_i64().unwrap_or(0)>0&&backup_has_int(x,"unit_price")
      &&x["unit_price"].as_i64().unwrap_or(-1)>=0&&backup_has_int(x,"line_total")&&x["line_total"].as_i64().unwrap_or(-1)>=0
      &&(x.get("unit_cost").is_none()||x["unit_cost"].is_null()||x["unit_cost"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("quantity_before").is_none()||x["quantity_before"].is_null()||x["quantity_before"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("average_cost_before").is_none()||x["average_cost_before"].is_null()||x["average_cost_before"].as_i64().unwrap_or(-1)>=0)
      &&(x.get("buy_price_before").is_none()||x["buy_price_before"].is_null()||x["buy_price_before"].as_i64().unwrap_or(-1)>=0)),
    "payments"=>!(backup_has_int(x,"id")&&backup_has_string(x,"kind")&&["receipt","payment"].contains(&x["kind"].as_str().unwrap_or(""))
      &&backup_has_optional_int(x,"contact_id")&&backup_has_string(x,"contact_name")&&backup_has_int(x,"amount")
      &&x["amount"].as_i64().unwrap_or(0)>0&&backup_has_string(x,"method")&&backup_has_string(x,"payment_date")&&backup_has_string(x,"notes")),
    "settings"=>!(backup_has_string(x,"key")&&!x["key"].as_str().unwrap_or("").is_empty()&&backup_has_string(x,"value")),
    _=>true
   };
   if bad{return Err(format!("رکورد نامعتبر در بخش {key}، ردیف {}؛ داده‌های فعلی دست‌نخورده باقی می‌مانند.",i+1))}
  }
 }
 let ids=|key:&str|->Result<HashSet<i64>,String>{
  let mut out=HashSet::new();
  for x in data[key].as_array().unwrap(){let id=x["id"].as_i64().ok_or_else(||format!("شناسهٔ نامعتبر در {key}"))?;if !out.insert(id){return Err(format!("شناسهٔ تکراری در بخش {key}: {id}"))}}
  Ok(out)
 };
 let contact_ids=ids("contacts")?;let product_ids=ids("products")?;let invoice_ids=ids("invoices")?;
 let item_ids=ids("invoice_items")?;let payment_ids=ids("payments")?;
 let invoice_kinds:HashMap<i64,String>=data["invoices"].as_array().unwrap().iter()
  .map(|row|(row["id"].as_i64().unwrap(),row["kind"].as_str().unwrap_or("").to_owned())).collect();
 let voided_invoices:HashSet<i64>=data["invoices"].as_array().unwrap().iter()
  .filter(|row|row.get("voided").and_then(Value::as_i64).unwrap_or(0)==1)
  .filter_map(|row|row["id"].as_i64()).collect();
 let mut codes=HashSet::new();
 for p in data["products"].as_array().unwrap(){if !codes.insert(p["code"].as_str().unwrap_or("")){return Err("کد کالا در فایل پشتیبان تکراری است".into())}}
 let mut numbers=HashSet::new();let mut subtotals:HashMap<i64,i64>=HashMap::new();
 for inv in data["invoices"].as_array().unwrap(){
  let id=inv["id"].as_i64().unwrap();let number=inv["number"].as_i64().unwrap();
  if !numbers.insert(number){return Err("شمارهٔ فاکتور تکراری است".into())}
  if let Some(cid)=inv["contact_id"].as_i64(){if !contact_ids.contains(&cid){return Err(format!("فاکتور {number} به طرف‌حساب ناموجود اشاره می‌کند"))}}
  let sub=inv["subtotal"].as_i64().unwrap();let disc=inv["discount"].as_i64().unwrap();let total=inv["total"].as_i64().unwrap();
  if disc>sub||sub-disc!=total{return Err(format!("مبلغ نهایی فاکتور {number} با تخفیف سازگار نیست"))}
  subtotals.insert(id,0);
 }
 for item in data["invoice_items"].as_array().unwrap(){
  let iid=item["invoice_id"].as_i64().unwrap();
  if !invoice_ids.contains(&iid){return Err("ردیف فاکتور به فاکتور ناموجود اشاره می‌کند".into())}
  if let Some(pid)=item["product_id"].as_i64(){if !product_ids.contains(&pid){return Err("ردیف فاکتور به کالای ناموجود اشاره می‌کند".into())}}
  let q=item["quantity"].as_i64().unwrap();let price=item["unit_price"].as_i64().unwrap();let line=item["line_total"].as_i64().unwrap();
  if q.checked_mul(price)!=Some(line){return Err("مبلغ ردیف با تعداد و قیمت واحد سازگار نیست".into())}
  let next=subtotals.get(&iid).copied().unwrap_or(0).checked_add(line).ok_or("جمع ردیف‌ها بیش از حد بزرگ است")?;
  subtotals.insert(iid,next);
 }
 for inv in data["invoices"].as_array().unwrap(){let id=inv["id"].as_i64().unwrap();if subtotals.get(&id).copied().unwrap_or(0)!=inv["subtotal"].as_i64().unwrap(){return Err(format!("جمع ردیف‌های فاکتور {} ناسازگار است",inv["number"].as_i64().unwrap()))}}
 for p in data["payments"].as_array().unwrap(){if let Some(cid)=p["contact_id"].as_i64(){if !contact_ids.contains(&cid){return Err("دریافت/پرداخت به طرف‌حساب ناموجود اشاره می‌کند".into())}}}
 for contact in data["contacts"].as_array().unwrap(){
  let kind=contact["kind"].as_str().unwrap_or("");let bal=contact["opening_balance"].as_i64().unwrap_or(-1);
  let cc=contact.get("customer_credit_balance").and_then(Value::as_i64).unwrap_or(0);
  let sc=contact.get("supplier_credit_balance").and_then(Value::as_i64).unwrap_or(0);
  if normalize_contact_kind(kind).is_none()||bal<0||cc<0||sc<0||(normalize_contact_kind(kind)==Some("both")&&bal!=0){return Err("نوع یا ماندهٔ افتتاحیهٔ طرف‌حساب معتبر نیست".into())}
 }
 let mut void_invoice_ids=HashSet::new();
 let mut void_row_ids=HashSet::new();
 for row in backup_optional_array(data,"invoice_voids"){
  let id=row.get("id").and_then(Value::as_i64).unwrap_or(0);
  let invoice_id=row.get("invoice_id").and_then(Value::as_i64).unwrap_or(0);
  let kind=row.get("kind").and_then(Value::as_str).unwrap_or("");
  let cash_refund=row.get("cash_refund").and_then(Value::as_i64).unwrap_or(-1);
  let credit_refund=row.get("credit_refund").and_then(Value::as_i64).unwrap_or(-1);
  if id<=0||!void_row_ids.insert(id)||!invoice_ids.contains(&invoice_id)
   ||!voided_invoices.contains(&invoice_id)||invoice_kinds.get(&invoice_id).map(String::as_str)!=Some(kind)
   ||!void_invoice_ids.insert(invoice_id)||!["sale","purchase"].contains(&kind)
   ||cash_refund<0||credit_refund<0||(kind=="purchase"&&cash_refund!=0){
    return Err("رکورد ابطال فاکتور در پشتیبان معتبر نیست".into());
  }
 }
 let mut allocation_ids=HashSet::new();
 for row in backup_optional_array(data,"payment_allocations"){
  let id=row.get("id").and_then(Value::as_i64).unwrap_or(0);
  let payment_id=row.get("payment_id").and_then(Value::as_i64);
  let invoice_id=row.get("invoice_id").and_then(Value::as_i64);
  let contact_id=row.get("contact_id").and_then(Value::as_i64);
  let kind=row.get("allocation_type").and_then(Value::as_str).unwrap_or("");
  let valid=match kind{
   "invoice"=>payment_id.is_some()&&invoice_id.is_some(),
   "opening_balance"|"advance"=>payment_id.is_some()&&invoice_id.is_none()&&contact_id.is_some(),
   "advance_application"=>payment_id.is_none()&&invoice_id.is_some()&&contact_id.is_some(),
   _=>false
  };
  if id<=0||!allocation_ids.insert(id)||!valid
    ||payment_id.is_some_and(|value|!payment_ids.contains(&value))
    ||invoice_id.is_some_and(|value|!invoice_ids.contains(&value))
    ||contact_id.is_some_and(|value|!contact_ids.contains(&value))
    ||row.get("amount").and_then(Value::as_i64).unwrap_or(0)<=0{
    return Err("رکورد تخصیص دریافت/پرداخت در پشتیبان معتبر نیست".into());
  }
 }
 for key in ["sales_returns","purchase_returns"]{
  for row in backup_optional_array(data,key){
   let invoice_id=row.get("invoice_id").and_then(Value::as_i64).unwrap_or(0);
   if !invoice_ids.contains(&invoice_id)||row.get("amount").and_then(Value::as_i64).unwrap_or(0)<=0
      ||row.get("credit_refund").and_then(Value::as_i64).unwrap_or(0)<0{return Err(format!("رکورد نامعتبر در بخش {key}"))}
  }
 }
 for key in ["sales_return_items","purchase_return_items"]{
  for row in backup_optional_array(data,key){
   if !item_ids.contains(&row.get("invoice_item_id").and_then(Value::as_i64).unwrap_or(0))
      ||!product_ids.contains(&row.get("product_id").and_then(Value::as_i64).unwrap_or(0))
      ||row.get("quantity").and_then(Value::as_i64).unwrap_or(0)<=0
      ||row.get("amount").and_then(Value::as_i64).unwrap_or(0)<=0{return Err(format!("رکورد نامعتبر در بخش {key}"))}
  }
 }
 for row in backup_optional_array(data,"inventory_adjustments"){
  if !product_ids.contains(&row.get("product_id").and_then(Value::as_i64).unwrap_or(0))
   ||row.get("delta").and_then(Value::as_i64).unwrap_or(0)==0
   ||row.get("reason").and_then(Value::as_str).unwrap_or("").trim().chars().count()<3{return Err("رکورد اصلاح موجودی در پشتیبان معتبر نیست".into())}
 }
 for row in backup_optional_array(data,"expenses"){
  if row.get("amount").and_then(Value::as_i64).unwrap_or(0)<=0
   ||row.get("category").and_then(Value::as_str).unwrap_or("").trim().is_empty()
   ||row.get("description").and_then(Value::as_str).unwrap_or("").trim().is_empty(){return Err("رکورد هزینه در پشتیبان معتبر نیست".into())}
 }
 let mut draft_ids=HashSet::new();
 for row in backup_optional_array(data,"invoice_drafts"){
  let id=row.get("id").and_then(Value::as_i64).unwrap_or(0);
  let kind=row.get("kind").and_then(Value::as_str).unwrap_or("");
  let raw=row.get("payload_json").and_then(Value::as_str).unwrap_or("");
  let parsed=serde_json::from_str::<Value>(raw);
  if id<=0||!draft_ids.insert(id)||!["sale","purchase"].contains(&kind)||raw.len()>1_000_000
   ||!row.get("title").and_then(Value::as_str).is_some()
   ||parsed.as_ref().ok().and_then(Value::as_object).is_none(){
   return Err("پیش‌نویس فاکتور در فایل پشتیبان معتبر نیست".into())
  }
 }
 Ok(())
}
fn create_restore_safety_backup(c:&Connection)->Result<PathBuf,String>{
 let filename:String=c.query_row("PRAGMA database_list",[],|r|r.get(2)).map_err(|e|e.to_string())?;
 let directory=if filename.trim().is_empty(){
  std::env::temp_dir().join("nabz-json-restore-backups")
 }else{
  PathBuf::from(filename).parent().ok_or("مسیر دیتابیس معتبر نیست")?.join("backups")
 };
 std::fs::create_dir_all(&directory).map_err(|e|format!("ساخت پوشهٔ پشتیبان پیش از بازیابی ناموفق بود: {e}"))?;
 let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
 let path=directory.join(format!("pre-json-restore-{stamp}.sqlite3"));
 nabz_core::backup_database_to(c,&path).map_err(|e|format!("بازیابی متوقف شد؛ پشتیبان ایمنی ساخته نشد و دیتابیس فعلی تغییر نکرد: {e}"))?;
 Ok(path)
}
#[tauri::command]
fn restore_data(data:Value,db:State<'_,Db>)->Result<(),String>{
 validate_backup(&data)?;
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let safety_path=create_restore_safety_backup(&c)?;
 let tx=c.transaction().map_err(|e|e.to_string())?;
 tx.execute_batch(
  "DROP TRIGGER IF EXISTS inventory_adjustments_no_update;
   DROP TRIGGER IF EXISTS inventory_adjustments_no_delete;
   DROP TRIGGER IF EXISTS trg_products_active_barcode_unique_insert;
   DROP TRIGGER IF EXISTS trg_products_active_barcode_unique_update;
   DELETE FROM payment_allocations; DELETE FROM invoice_voids;
   DELETE FROM sales_return_items; DELETE FROM sales_refunds; DELETE FROM sales_returns;
   DELETE FROM purchase_return_items; DELETE FROM purchase_refunds; DELETE FROM purchase_returns;
   DELETE FROM inventory_adjustments; DELETE FROM expenses;
   DELETE FROM invoice_items; DELETE FROM payments; DELETE FROM invoices;
   DELETE FROM products; DELETE FROM contacts; DELETE FROM settings;"
 ).map_err(|e|e.to_string())?;
 for x in data["contacts"].as_array().unwrap(){
  tx.execute("INSERT INTO contacts(id,name,kind,phone,opening_balance,notes,created_at,customer_credit_balance,supplier_credit_balance)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
   params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["kind"].as_str().unwrap_or("customer"),
    x["phone"].as_str().unwrap_or(""),x["opening_balance"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or(""),
    x["created_at"].as_str().unwrap_or(""),x.get("customer_credit_balance").and_then(Value::as_i64).unwrap_or(0),
    x.get("supplier_credit_balance").and_then(Value::as_i64).unwrap_or(0)]).map_err(|e|e.to_string())?;
 }
 for x in data["products"].as_array().unwrap(){
  let quantity=x["quantity"].as_i64().unwrap_or(0);let buy=x["buy_price"].as_i64().unwrap_or(0);
  let average=x.get("average_cost").and_then(Value::as_i64).unwrap_or(if quantity>0{buy}else{0});
  tx.execute("INSERT INTO products(id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost,category,unit,wholesale_price,is_service,is_active)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
   params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["code"].as_str().unwrap_or(""),
    x["barcode"].as_str().unwrap_or(""),buy,x["sell_price"].as_i64().unwrap_or(0),quantity,
    x["min_quantity"].as_i64().unwrap_or(0),average,x.get("category").and_then(Value::as_str).unwrap_or(""),
    x.get("unit").and_then(Value::as_str).unwrap_or("عدد"),x.get("wholesale_price").and_then(Value::as_i64).unwrap_or(0),
    x.get("is_service").and_then(Value::as_i64).unwrap_or(0),x.get("is_active").and_then(Value::as_i64).unwrap_or(1)]).map_err(|e|e.to_string())?;
 }
 for x in data["invoices"].as_array().unwrap(){
  tx.execute("INSERT INTO invoices(id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied,idempotency_key)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
   params![x["id"].as_i64(),x["number"].as_i64(),x["kind"].as_str().unwrap_or("sale"),x["contact_id"].as_i64(),
    x["contact_name"].as_str().unwrap_or(""),x["invoice_date"].as_str().unwrap_or(""),x["subtotal"].as_i64().unwrap_or(0),
    x["discount"].as_i64().unwrap_or(0),x["total"].as_i64().unwrap_or(0),x["paid"].as_i64().unwrap_or(0),
    x["notes"].as_str().unwrap_or(""),x.get("voided").and_then(Value::as_i64).unwrap_or(0),
    x.get("credit_applied").and_then(Value::as_i64).unwrap_or(0),x.get("idempotency_key").and_then(Value::as_str)]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"invoice_drafts"){
  tx.execute("INSERT INTO invoice_drafts(id,kind,title,payload_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6)",
   params![x["id"].as_i64(),x["kind"].as_str().unwrap_or("sale"),x["title"].as_str().unwrap_or(""),
    x["payload_json"].as_str().unwrap_or("{}"),x["created_at"].as_str().unwrap_or(""),x["updated_at"].as_str().unwrap_or("")])
   .map_err(|e|e.to_string())?;
 }
 for x in data["invoice_items"].as_array().unwrap(){
  tx.execute("INSERT INTO invoice_items(id,invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost,quantity_before,average_cost_before,buy_price_before)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["product_id"].as_i64(),x["item_name"].as_str().unwrap_or(""),
    x["quantity"].as_i64().unwrap_or(0),x["unit_price"].as_i64().unwrap_or(0),x["line_total"].as_i64().unwrap_or(0),
    x.get("unit_cost").and_then(Value::as_i64),x.get("quantity_before").and_then(Value::as_i64),
    x.get("average_cost_before").and_then(Value::as_i64),x.get("buy_price_before").and_then(Value::as_i64)]).map_err(|e|e.to_string())?;
 }
 for x in data["payments"].as_array().unwrap(){
  tx.execute("INSERT INTO payments(id,kind,contact_id,contact_name,amount,method,payment_date,notes)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
   params![x["id"].as_i64(),x["kind"].as_str().unwrap_or("receipt"),x["contact_id"].as_i64(),
    x["contact_name"].as_str().unwrap_or(""),x["amount"].as_i64().unwrap_or(0),x["method"].as_str().unwrap_or("cash"),
    x["payment_date"].as_str().unwrap_or(""),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"invoice_voids"){
  tx.execute("INSERT INTO invoice_voids(id,invoice_id,kind,void_date,cash_refund,credit_refund,notes)
   VALUES(?1,?2,?3,?4,?5,?6,?7)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["kind"].as_str().unwrap_or("sale"),
    x["void_date"].as_str().unwrap_or(""),x["cash_refund"].as_i64().unwrap_or(0),
    x["credit_refund"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"payment_allocations"){
  tx.execute("INSERT INTO payment_allocations(id,payment_id,invoice_id,contact_id,allocation_type,amount,created_at)
   VALUES(?1,?2,?3,?4,?5,?6,?7)",
   params![x["id"].as_i64(),x["payment_id"].as_i64(),x["invoice_id"].as_i64(),x["contact_id"].as_i64(),
    x["allocation_type"].as_str().unwrap_or("invoice"),x["amount"].as_i64().unwrap_or(0),
    x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in data["settings"].as_array().unwrap(){
  tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",params![x["key"].as_str().unwrap_or(""),x["value"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"sales_returns"){
  tx.execute("INSERT INTO sales_returns(id,invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,?2,?3,?4,?5,?6)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["return_date"].as_str().unwrap_or(""),
    x["amount"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or(""),x["credit_refund"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"sales_return_items"){
  tx.execute("INSERT INTO sales_return_items(id,return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
   params![x["id"].as_i64(),x["return_id"].as_i64(),x["invoice_item_id"].as_i64(),x["product_id"].as_i64(),
    x["item_name"].as_str().unwrap_or(""),x["quantity"].as_i64().unwrap_or(0),x["unit_price"].as_i64().unwrap_or(0),
    x["amount"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"sales_refunds"){
  tx.execute("INSERT INTO sales_refunds(id,invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["return_id"].as_i64(),x["refund_date"].as_str().unwrap_or(""),
    x["amount"].as_i64().unwrap_or(0),x["method"].as_str().unwrap_or("cash"),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"purchase_returns"){
  tx.execute("INSERT INTO purchase_returns(id,invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,?2,?3,?4,?5,?6)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["return_date"].as_str().unwrap_or(""),
    x["amount"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or(""),x["credit_refund"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"purchase_return_items"){
  tx.execute("INSERT INTO purchase_return_items(id,return_id,invoice_item_id,product_id,item_name,quantity,unit_price,amount)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
   params![x["id"].as_i64(),x["return_id"].as_i64(),x["invoice_item_id"].as_i64(),x["product_id"].as_i64(),
    x["item_name"].as_str().unwrap_or(""),x["quantity"].as_i64().unwrap_or(0),x["unit_price"].as_i64().unwrap_or(0),
    x["amount"].as_i64().unwrap_or(0)]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"purchase_refunds"){
  tx.execute("INSERT INTO purchase_refunds(id,invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
   params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["return_id"].as_i64(),x["refund_date"].as_str().unwrap_or(""),
    x["amount"].as_i64().unwrap_or(0),x["method"].as_str().unwrap_or("cash"),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"inventory_adjustments"){
  tx.execute("INSERT INTO inventory_adjustments(id,product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
   params![x["id"].as_i64(),x["product_id"].as_i64(),x["product_name"].as_str().unwrap_or(""),
    x["previous_quantity"].as_i64().unwrap_or(0),x["new_quantity"].as_i64().unwrap_or(0),
    x["delta"].as_i64().unwrap_or(0),x["reason"].as_str().unwrap_or(""),x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 for x in backup_optional_array(&data,"expenses"){
  tx.execute("INSERT INTO expenses(id,category,description,amount,expense_date,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
   params![x["id"].as_i64(),x["category"].as_str().unwrap_or(""),x["description"].as_str().unwrap_or(""),
    x["amount"].as_i64().unwrap_or(0),x["expense_date"].as_str().unwrap_or(""),x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
 }
 tx.execute_batch(
  "CREATE TRIGGER IF NOT EXISTS trg_products_active_barcode_unique_insert
   BEFORE INSERT ON products WHEN NEW.is_active=1 AND NEW.barcode<>'' AND EXISTS(
    SELECT 1 FROM products WHERE barcode=NEW.barcode AND is_active=1
   ) BEGIN SELECT RAISE(ABORT,'بارکد کالای فعال تکراری است'); END;
   CREATE TRIGGER IF NOT EXISTS trg_products_active_barcode_unique_update
   BEFORE UPDATE OF barcode,is_active ON products WHEN NEW.is_active=1 AND NEW.barcode<>'' AND (NEW.barcode<>OLD.barcode OR NEW.is_active<>OLD.is_active) AND EXISTS(
    SELECT 1 FROM products WHERE barcode=NEW.barcode AND is_active=1 AND id<>NEW.id
   ) BEGIN SELECT RAISE(ABORT,'بارکد کالای فعال تکراری است'); END;
   CREATE TRIGGER IF NOT EXISTS inventory_adjustments_no_update BEFORE UPDATE ON inventory_adjustments
   BEGIN SELECT RAISE(ABORT,'inventory adjustment history is immutable'); END;
   CREATE TRIGGER IF NOT EXISTS inventory_adjustments_no_delete BEFORE DELETE ON inventory_adjustments
   BEGIN SELECT RAISE(ABORT,'inventory adjustment history is immutable'); END;"
 ).map_err(|e|e.to_string())?;
 let foreign_key_errors:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if foreign_key_errors!=0{return Err("اعتبارسنجی ارتباط رکوردها پس از بازیابی ناموفق بود؛ داده‌های قبلی حفظ شدند".into())}
 let integrity:String=tx.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if integrity!="ok"{return Err(format!("بازیابی متوقف شد؛ بررسی سلامت دیتابیس ناموفق بود و تراکنش rollback شد. پشتیبان ایمنی: {}؛ نتیجه: {integrity}",safety_path.display()))}
 tx.commit().map_err(|e|e.to_string())?;
 Ok(())
}
fn parse_return_lines(lines:Vec<Value>)->Result<Vec<ReturnLineInput>,String>{
 let parsed=lines.into_iter().map(|x|{
  let lookup=x.get("item").or_else(||x.get("product_lookup")).and_then(Value::as_str).unwrap_or("").trim().to_owned();
  let invoice_item_id=x.get("invoice_item_id").and_then(Value::as_i64);
  let quantity=x.get("qty").or_else(||x.get("quantity")).and_then(|v|v.as_i64().or_else(||v.as_str().and_then(|n|n.parse().ok()))).unwrap_or(0);
  ReturnLineInput{invoice_item_id,product_lookup:lookup,quantity}
 }).collect::<Vec<_>>();
 if parsed.is_empty()||parsed.iter().any(|x|(x.invoice_item_id.is_none()&&x.product_lookup.is_empty())||x.invoice_item_id.is_some_and(|id|id<=0)||x.quantity<=0){return Err("ردیف‌های برگشت باید شناسهٔ فاکتور یا کد کالا و تعداد مثبت داشته باشند".into())}
 Ok(parsed)
}
#[tauri::command]
fn return_sale_invoice(invoice_id:i64,lines:Vec<Value>,method:String,db:State<'_,Db>)->Result<Value,String>{
 let parsed=parse_return_lines(lines)?;
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let (credit,refund)=return_sale_items(&mut c,invoice_id,&parsed,&method)?;
 Ok(json!({"credit":credit,"cash_refund":refund}))
}
#[tauri::command]
fn return_purchase_invoice(invoice_id:i64,lines:Vec<Value>,method:String,db:State<'_,Db>)->Result<Value,String>{
 let parsed=parse_return_lines(lines)?;
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let (credit,refund)=return_purchase_items(&mut c,invoice_id,&parsed,&method)?;
 Ok(json!({"credit":credit,"cash_refund":refund}))
}
#[tauri::command]
fn void_invoice(invoice_id:i64,db:State<'_,Db>)->Result<(),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 core_void_invoice(&mut c,invoice_id)
}
#[tauri::command]
fn adjust_inventory_command(product_id:i64,new_quantity:i64,reason:String,db:State<'_,Db>)->Result<Value,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let result=adjust_inventory(&mut c,product_id,new_quantity,&reason)?;
 Ok(json!({"product_id":result.product_id,"previous_quantity":result.previous_quantity,"new_quantity":result.new_quantity,"delta":result.delta,"reason":result.reason,"created_at":result.created_at}))
}
fn set_product_active_with_connection(c:&Connection,product_id:i64,active:bool)->Result<(),String>{
 if !active {
  let quantity:Option<i64>=c.query_row("SELECT quantity FROM products WHERE id=?1",[product_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  let quantity=quantity.ok_or("کالا پیدا نشد")?;
  if quantity!=0{return Err("برای غیرفعال‌سازی، ابتدا موجودی کالا را به صفر برسانید؛ سابقه فاکتورها حفظ می‌شود.".into())}
 }
 let changed=c.execute("UPDATE products SET is_active=?1 WHERE id=?2",rusqlite::params![active as i64,product_id]).map_err(|e|e.to_string())?;
 if changed==0{return Err("کالا پیدا نشد".into())}
 Ok(())
}
#[tauri::command]
fn set_product_active(product_id:i64,active:bool,db:State<'_,Db>)->Result<(),String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 set_product_active_with_connection(&c,product_id,active)
}
#[tauri::command]
fn inventory_movements_command(db:State<'_,Db>)->Result<Vec<Value>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let rows=nabz_core::accounting::inventory_movements(&c)?;
 Ok(rows.into_iter().map(|x|json!({"product_id":x.product_id,"product_name":x.product_name,"movement_date":x.movement_date,"movement_type":x.movement_type,"reference":x.reference,"quantity_delta":x.quantity_delta,"balance_after":x.balance_after})).collect())
}
#[tauri::command]
fn inventory_valuation_command(db:State<'_,Db>)->Result<Vec<Value>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let rows=inventory_valuation(&c)?;
 Ok(rows.into_iter().map(|x|json!({"product_id":x.product_id,"product_name":x.product_name,"code":x.code,"quantity":x.quantity,"buy_price":x.buy_price,"average_cost":x.average_cost,"stock_value":x.stock_value,"min_quantity":x.min_quantity})).collect())
}
#[tauri::command]
fn create_expense_command(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let input=ExpenseInput{category:text(&payload,"category"),description:text(&payload,"description"),amount:num(&payload,"amount"),date:text(&payload,"date")};
 let c=db.0.lock().map_err(|e|e.to_string())?;
 create_expense(&c,&input)
}
#[tauri::command]
fn list_expenses_command(from:String,to:String,db:State<'_,Db>)->Result<Vec<Value>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let rows=list_expenses(&c,&from,&to)?;
 Ok(rows.into_iter().map(|x|json!({"id":x.id,"category":x.category,"description":x.description,"amount":x.amount,"date":x.date})).collect())
}
#[tauri::command]
fn profit_loss_report_command(from:String,to:String,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let x=profit_loss_report(&c,&from,&to)?;
 Ok(json!({"sales":x.sales,"discounts":x.discounts,"net_sales":x.net_sales,"cost_of_goods_sold":x.cost_of_goods_sold,"gross_profit":x.gross_profit,"expenses":x.expenses,"net_profit":x.net_profit,"uncosted_sales":x.uncosted_sales,"expense_rows":x.expense_rows.into_iter().map(|r|json!({"id":r.id,"category":r.category,"description":r.description,"amount":r.amount,"date":r.date})).collect::<Vec<_>>()}))
}
#[tauri::command]
fn contact_ledger_with_connection(id:i64,c:&Connection)->Result<Value,String>{
 let contact:Value=c.query_row("SELECT id,name,kind,phone,opening_balance,customer_credit_balance,supplier_credit_balance FROM contacts WHERE id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"phone":r.get::<_,String>(3)?,"opening_balance":r.get::<_,i64>(4)?,"customer_credit_balance":r.get::<_,i64>(5)?,"supplier_credit_balance":r.get::<_,i64>(6)?}))).optional().map_err(|e|e.to_string())?.ok_or("طرف‌حساب پیدا نشد")?;
 let mut entries:Vec<Value>=Vec::new();
 {
  let mut st=c.prepare("SELECT i.id,i.number,i.kind,i.invoice_date,i.total,i.paid,i.notes,i.voided,i.credit_applied,
   CASE WHEN i.kind='sale' THEN i.total+COALESCE((SELECT SUM(amount) FROM sales_returns WHERE invoice_id=i.id),0)
        ELSE i.total+COALESCE((SELECT SUM(amount) FROM purchase_returns WHERE invoice_id=i.id),0) END
   FROM invoices i WHERE i.contact_id=?1 ORDER BY i.invoice_date,i.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let voided=r.get::<_,i64>(7)?;
   let total=r.get::<_,i64>(4)?;
   let paid=r.get::<_,i64>(5)?;
   let kind=r.get::<_,String>(2)?;
   let original_total=r.get::<_,i64>(9)?;
   let debit=if kind=="sale"{original_total}else{0};
   let credit=if kind=="purchase"{original_total}else{0};
   Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"entry_type":"invoice","kind":kind,"date":r.get::<_,String>(3)?,"amount":total,"paid":paid,"debit":debit,"credit":credit,"balance_delta":if voided==1{0}else{total-paid},"voided":voided==1,"credit_applied":r.get::<_,i64>(8)?,"notes":r.get::<_,String>(6)?}))
  }).map_err(|e|e.to_string())?;
  for row in rows{entries.push(row.map_err(|e|e.to_string())?);}
 }
 {
  let mut st=c.prepare("SELECT id,kind,payment_date,amount,method,notes FROM payments WHERE contact_id=?1 ORDER BY payment_date,id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let kind=r.get::<_,String>(1)?;
   let amount=r.get::<_,i64>(3)?;
   let debit=if kind=="payment"{amount}else{0};
   let credit=if kind=="receipt"{amount}else{0};
   Ok(json!({"id":r.get::<_,i64>(0)?,"entry_type":"payment","kind":kind,"date":r.get::<_,String>(2)?,"amount":amount,"debit":debit,"credit":credit,"method":r.get::<_,String>(4)?,"notes":r.get::<_,String>(5)?}))
  }).map_err(|e|e.to_string())?;
  for row in rows{entries.push(row.map_err(|e|e.to_string())?);}
 }
 entries.sort_by(|a,b|a["date"].as_str().unwrap_or("").cmp(b["date"].as_str().unwrap_or("")));
 let payment_allocations={
  let mut st=c.prepare("SELECT id,payment_id,invoice_id,allocation_type,amount,created_at FROM payment_allocations WHERE contact_id=?1 ORDER BY created_at,id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({
   "id":r.get::<_,i64>(0)?,"payment_id":r.get::<_,Option<i64>>(1)?,"invoice_id":r.get::<_,Option<i64>>(2)?,
   "allocation_type":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"date":r.get::<_,String>(5)?
  }))).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let sales_returns={
  let mut st=c.prepare("SELECT sr.id,sr.invoice_id,sr.return_date,sr.amount,sr.credit_refund,sr.notes FROM sales_returns sr JOIN invoices i ON i.id=sr.invoice_id WHERE i.contact_id=?1 ORDER BY sr.return_date,sr.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let amount=r.get::<_,i64>(3)?;
   Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"date":r.get::<_,String>(2)?,"amount":amount,"debit":0,"credit":amount,"credit_refund":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?}))
  }).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let purchase_returns={
  let mut st=c.prepare("SELECT pr.id,pr.invoice_id,pr.return_date,pr.amount,pr.credit_refund,pr.notes FROM purchase_returns pr JOIN invoices i ON i.id=pr.invoice_id WHERE i.contact_id=?1 ORDER BY pr.return_date,pr.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let amount=r.get::<_,i64>(3)?;
   Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"date":r.get::<_,String>(2)?,"amount":amount,"debit":amount,"credit":0,"credit_refund":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?}))
  }).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let refunds={
  let mut st=c.prepare("SELECT id,invoice_id,refund_date,amount,method,notes,'sale' AS kind FROM sales_refunds WHERE invoice_id IN (SELECT id FROM invoices WHERE contact_id=?1)
   UNION ALL
   SELECT id,invoice_id,refund_date,amount,method,notes,'purchase' AS kind FROM purchase_refunds WHERE invoice_id IN (SELECT id FROM invoices WHERE contact_id=?1)
   ORDER BY 3,1").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let amount=r.get::<_,i64>(3)?;
   let kind=r.get::<_,String>(6)?;
   let debit=if kind=="sale"{amount}else{0};
   let credit=if kind=="purchase"{amount}else{0};
   Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"date":r.get::<_,String>(2)?,"amount":amount,"debit":debit,"credit":credit,"method":r.get::<_,String>(4)?,"notes":r.get::<_,String>(5)?,"kind":kind}))
  }).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let invoice_voids={
  let mut st=c.prepare("SELECT v.id,v.invoice_id,v.kind,v.void_date,v.cash_refund,v.credit_refund,v.notes,i.total FROM invoice_voids v JOIN invoices i ON i.id=v.invoice_id WHERE i.contact_id=?1 ORDER BY v.void_date,v.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let kind=r.get::<_,String>(2)?;
   let amount=r.get::<_,i64>(7)?;
   let debit=if kind=="purchase"{amount}else{0};
   let credit=if kind=="sale"{amount}else{0};
   Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"kind":kind,"date":r.get::<_,String>(3)?,"amount":amount,"debit":debit,"credit":credit,"cash_refund":r.get::<_,i64>(4)?,"credit_refund":r.get::<_,i64>(5)?,"notes":r.get::<_,String>(6)?}))
  }).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let kind=contact["kind"].as_str().unwrap_or("");
 let opening=contact["opening_balance"].as_i64().unwrap_or(0);
 let customer_advance=contact["customer_credit_balance"].as_i64().unwrap_or(0);
 let supplier_advance=contact["supplier_credit_balance"].as_i64().unwrap_or(0);
 let ledger_balances=json!({
  "opening_debit":if kind=="customer"{opening}else{0},
  "opening_credit":if kind=="supplier"{opening}else{0},
  "customer_advance_credit":customer_advance,
  "supplier_advance_debit":supplier_advance
 });
 Ok(json!({"contact":contact,"entries":entries,"ledger_balances":ledger_balances,
  "payment_allocations":payment_allocations,"sales_returns":sales_returns,
  "purchase_returns":purchase_returns,"refunds":refunds,"invoice_voids":invoice_voids}))
}
#[tauri::command]
fn get_contact_ledger(id:i64,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 contact_ledger_with_connection(id,&c)
}
#[tauri::command]
fn get_invoice_print(id:i64,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let invoice:Value=c.query_row("SELECT id,number,kind,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied FROM invoices WHERE id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,"contact_name":r.get::<_,String>(3)?,"invoice_date":r.get::<_,String>(4)?,"subtotal":r.get::<_,i64>(5)?,"discount":r.get::<_,i64>(6)?,"total":r.get::<_,i64>(7)?,"paid":r.get::<_,i64>(8)?,"voided":r.get::<_,i64>(10)?==1,"credit_applied":r.get::<_,i64>(11)?,"notes":r.get::<_,String>(9)?}))).optional().map_err(|e|e.to_string())?.ok_or("فاکتور پیدا نشد")?;
 let mut items=Vec::new();
 {let mut st=c.prepare("SELECT item_name,quantity,unit_price,line_total FROM invoice_items WHERE invoice_id=?1 ORDER BY id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"item_name":r.get::<_,String>(0)?,"quantity":r.get::<_,i64>(1)?,"unit_price":r.get::<_,i64>(2)?,"line_total":r.get::<_,i64>(3)?}))).map_err(|e|e.to_string())?;
  for row in rows{items.push(row.map_err(|e|e.to_string())?);}
 }
 let mut shop=std::collections::HashMap::new();
 {let mut st=c.prepare("SELECT key,value FROM settings WHERE key IN ('shop_name','shop_phone','shop_address','print_paper')").map_err(|e|e.to_string())?;
  let rows=st.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
  for row in rows{let (k,v)=row.map_err(|e|e.to_string())?;shop.insert(k,v);}
 }
 Ok(json!({"invoice":invoice,"items":items,"shop":shop}))
}
#[tauri::command]
fn get_invoice_detail(id:i64,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let invoice:Value=c.query_row(
  "SELECT id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied
   FROM invoices WHERE id=?1",[id],
  |r|Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,
   "contact_id":r.get::<_,Option<i64>>(3)?,"contact_name":r.get::<_,String>(4)?,"invoice_date":r.get::<_,String>(5)?,
   "subtotal":r.get::<_,i64>(6)?,"discount":r.get::<_,i64>(7)?,"total":r.get::<_,i64>(8)?,
   "paid":r.get::<_,i64>(9)?,"notes":r.get::<_,String>(10)?,"voided":r.get::<_,i64>(11)?!=0,
   "credit_applied":r.get::<_,i64>(12)?})))
  .optional().map_err(|e|e.to_string())?.ok_or("فاکتور پیدا نشد")?;
 let mut items=Vec::new();
 {
  let mut st=c.prepare("SELECT ii.id,ii.product_id,ii.item_name,ii.quantity,ii.unit_price,ii.line_total,
    ii.unit_cost,COALESCE((SELECT SUM(sri.quantity) FROM sales_return_items sri WHERE sri.invoice_item_id=ii.id),0),
    COALESCE((SELECT SUM(pri.quantity) FROM purchase_return_items pri WHERE pri.invoice_item_id=ii.id),0),
    COALESCE(p.code,''),COALESCE(p.barcode,'')
    FROM invoice_items ii LEFT JOIN products p ON p.id=ii.product_id WHERE ii.invoice_id=?1 ORDER BY ii.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"product_id":r.get::<_,Option<i64>>(1)?,
    "item_name":r.get::<_,String>(2)?,"quantity":r.get::<_,i64>(3)?,"unit_price":r.get::<_,i64>(4)?,
    "line_total":r.get::<_,i64>(5)?,"unit_cost":r.get::<_,Option<i64>>(6)?,
    "sales_returned":r.get::<_,i64>(7)?,"purchase_returned":r.get::<_,i64>(8)?,
    "code":r.get::<_,String>(9)?,"barcode":r.get::<_,String>(10)?}))).map_err(|e|e.to_string())?;
  items=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 }
 let mut payments=Vec::new();
 {
  let mut st=c.prepare("SELECT p.id,p.amount,p.method,p.payment_date,p.notes,pa.amount
   FROM payment_allocations pa JOIN payments p ON p.id=pa.payment_id
   WHERE pa.invoice_id=?1 AND pa.allocation_type='invoice' ORDER BY p.payment_date,p.id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"amount":r.get::<_,i64>(1)?,
   "method":r.get::<_,String>(2)?,"date":r.get::<_,String>(3)?,"notes":r.get::<_,String>(4)?,
   "allocated":r.get::<_,i64>(5)?}))).map_err(|e|e.to_string())?;
  payments=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 }
 let returns={
  let mut st=c.prepare("SELECT id,return_date,amount,credit_refund,notes,'sale' FROM sales_returns WHERE invoice_id=?1
   UNION ALL SELECT id,return_date,amount,credit_refund,notes,'purchase' FROM purchase_returns WHERE invoice_id=?1
   ORDER BY 2,1").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"date":r.get::<_,String>(1)?,
   "amount":r.get::<_,i64>(2)?,"credit_refund":r.get::<_,i64>(3)?,"notes":r.get::<_,String>(4)?,
   "kind":r.get::<_,String>(5)?}))).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let refunds={
  let mut st=c.prepare("SELECT id,refund_date,amount,method,notes,'sale' FROM sales_refunds WHERE invoice_id=?1
   UNION ALL SELECT id,refund_date,amount,method,notes,'purchase' FROM purchase_refunds WHERE invoice_id=?1
   ORDER BY 2,1").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"date":r.get::<_,String>(1)?,
   "amount":r.get::<_,i64>(2)?,"method":r.get::<_,String>(3)?,"notes":r.get::<_,String>(4)?,
   "kind":r.get::<_,String>(5)?}))).map_err(|e|e.to_string())?;
  rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
 };
 let void_record:Option<Value>=c.query_row("SELECT id,void_date,cash_refund,credit_refund,notes FROM invoice_voids WHERE invoice_id=?1",[id],
  |r|Ok(json!({"id":r.get::<_,i64>(0)?,"date":r.get::<_,String>(1)?,"cash_refund":r.get::<_,i64>(2)?,
   "credit_refund":r.get::<_,i64>(3)?,"notes":r.get::<_,String>(4)?}))).optional().map_err(|e|e.to_string())?;
 Ok(json!({"invoice":invoice,"items":items,"payments":payments,"returns":returns,"refunds":refunds,"void":void_record}))
}

#[tauri::command] fn save_setting(key:String,value:String,db:State<'_,Db>)->Result<(),String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command] fn get_settings(db:State<'_,Db>)->Result<std::collections::HashMap<String,String>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;rows.collect::<Result<_,_>>().map_err(|e|e.to_string())}
#[cfg_attr(mobile,tauri::mobile_entry_point)] pub fn run(){tauri::Builder::default().setup(|app|{let c=nabz_core::open_database().map_err(std::io::Error::other)?;app.manage(Db(Mutex::new(c)));Ok(())}).invoke_handler(tauri::generate_handler![app_version,db_status,dashboard_summary,reports_summary,export_data,restore_data,list_records,list_products,delete_record,save_contact,save_product,save_invoice,save_invoice_payment,save_invoice_draft,list_invoice_drafts,get_invoice_draft,delete_invoice_draft,save_payment,return_sale_invoice,return_purchase_invoice,void_invoice,get_invoice_detail,adjust_inventory_command,set_product_active,inventory_movements_command,inventory_valuation_command,create_expense_command,list_expenses_command,profit_loss_report_command,get_invoice_print,get_contact_ledger,save_setting,get_settings,sales_trend,reports_period]).run(tauri::generate_context!()).expect("error while running Nabz");}


#[cfg(test)]
mod accounting_tests {
 use super::*;
 use serde_json::json;

 fn database()->Connection {
  let c=Connection::open_in_memory().expect("in-memory SQLite");
  init(&c).expect("schema");
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Ali','customer',0)",[]).unwrap();
  c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity,average_cost) VALUES('Pen','PEN',10,20,10,2,10)",[]).unwrap();
  c
 }
 fn sale(customer:&str,status:&str,paid:i64,qty:i64,price:i64,discount:i64)->Value {
  json!({"kind":"sale","customer":customer,"payment_status":status,"paid_amount":paid,"method":"cash","items":[{"item":"Pen","qty":qty,"price":price}],"discount":discount,"date":"2026-10-28"})
 }
 fn scalar(c:&Connection,sql:&str)->i64 { c.query_row(sql,[],|r|r.get(0)).unwrap() }


 #[test]
 fn period_reports_only_include_records_inside_the_selected_date_range() {
  let mut c=database();
  save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"Pen","qty":2,"price":20}],"discount":0,"date":"2026-10-28"}),&mut c).unwrap();
  save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"Pen","qty":1,"price":20}],"discount":0,"date":"2026-10-30"}),&mut c).unwrap();
  let report=reports_period_with_connection(&c,"2026-10-29","2026-10-31").unwrap();
  assert_eq!(report["sales_total"].as_i64(),Some(20));
  assert_eq!(report["sales_count"].as_i64(),Some(1));
  assert_eq!(report["daily_sales"].as_array().unwrap().len(),3);
 }

 #[test]
 fn reports_filter_by_contact_and_reconcile_returns_costs_and_expenses() {
  let mut c=database();
  let invoice_id=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"Pen","qty":2,"price":20}],"discount":0,"date":"2026-10-28"}),&mut c).unwrap();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Sara','customer',0)",[]).unwrap();
  save_invoice_with_connection(json!({"kind":"sale","customer":"Sara","payment_status":"credit","items":[{"item":"Pen","qty":1,"price":20}],"discount":0,"date":"2026-10-28"}),&mut c).unwrap();
  let ali_id=scalar(&c,"SELECT id FROM contacts WHERE name='Ali'");
  let item_id=scalar(&c,&format!("SELECT id FROM invoice_items WHERE invoice_id={invoice_id}"));
  return_sale_items(&mut c,invoice_id,&[ReturnLineInput{invoice_item_id:Some(item_id),product_lookup:String::new(),quantity:1}],"cash").unwrap();
  c.execute("UPDATE sales_returns SET return_date='2026-10-29' WHERE invoice_id=?1",[invoice_id]).unwrap();
  create_expense(&c,&ExpenseInput{category:"اجاره".into(),description:"هزینه آزمایشی".into(),amount:5,date:"2026-10-29".into()}).unwrap();
  let report=reports_period_filtered_with_connection(&c,"2026-10-28","2026-10-30",Some(ali_id)).unwrap();
  assert_eq!(report["sales_total"].as_i64(),Some(20));
  assert_eq!(report["sales_count"].as_i64(),Some(1));
  assert_eq!(report["sales_returns_total"].as_i64(),Some(20));
  assert_eq!(report["cost_of_goods_sold"].as_i64(),Some(10));
  assert_eq!(report["gross_profit"].as_i64(),Some(10));
  assert_eq!(report["expenses_total"].as_i64(),Some(5));
  assert_eq!(report["net_profit"].as_i64(),Some(5));
  assert_eq!(report["top_products"][0]["quantity"].as_i64(),Some(1));
  let all=reports_period_with_connection(&c,"2026-10-28","2026-10-30").unwrap();
  assert_eq!(all["sales_total"].as_i64(),Some(40));
  assert_eq!(all["cost_of_goods_sold"].as_i64(),Some(20));
  assert_eq!(all["gross_profit"].as_i64(),Some(20));
 }

 #[test]
 fn reports_return_zeroes_and_complete_daily_series_for_an_empty_period() {
  let c=database();
  let report=reports_period_with_connection(&c,"2026-11-01","2026-11-03").unwrap();
  assert_eq!(report["sales_total"].as_i64(),Some(0));
  assert_eq!(report["purchases_total"].as_i64(),Some(0));
  assert_eq!(report["cost_of_goods_sold"].as_i64(),Some(0));
  assert_eq!(report["gross_profit"].as_i64(),Some(0));
  assert_eq!(report["net_profit"].as_i64(),Some(0));
  let days=report["daily_sales"].as_array().unwrap();
  assert_eq!(days.len(),3);
  assert!(days.iter().all(|day|day["total"].as_i64()==Some(0)));
  assert!(report["top_products"].as_array().unwrap().is_empty());
 }

 #[test]
 fn reports_reject_invalid_dates_and_ranges_over_one_year() {
  let c=database();
  assert!(reports_period_with_connection(&c,"2026-02-30","2026-03-01").is_err());
  assert!(reports_period_with_connection(&c,"2025-01-01","2026-01-02").is_err());
  assert!(reports_period_with_connection(&c,"2026-11-02","2026-11-01").is_err());
 }

 #[test]
 fn editing_product_preserves_stock_when_historical_invoice_lines_exist() {
  let mut c=database();
  let first_sale=json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"Pen","qty":1,"price":20}],"discount":0,"date":"2026-10-28"});
  save_invoice_with_connection(first_sale,&mut c).unwrap();
  let id=scalar(&c,"SELECT id FROM products WHERE code='PEN'");
  let updated=json!({"id":id,"name":"Pen Pro","code":"PEN-PRO","barcode":"123456","qty":999,"buy":12,"sell":25,"min":3});
  save_product_with_connection(updated,&c).unwrap();
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE id=1"),9);
  assert_eq!(scalar(&c,"SELECT buy_price FROM products WHERE id=1"),12);
  let second_sale=json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"Pen Pro","qty":1,"price":25}],"discount":0,"date":"2026-10-29"});
  save_invoice_with_connection(second_sale,&mut c).unwrap();
  let id=scalar(&c,"SELECT id FROM products WHERE code='PEN-PRO'");
  save_product_with_connection(json!({"id":id,"name":"Pen Pro Plus","code":"PEN-PRO","barcode":"123456","qty":0,"buy":13,"sell":27,"min":4}),&c).unwrap();
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN-PRO'"),8);
 }

 #[test]
 fn contact_edit_preserves_financial_role_and_opening_balance_after_activity() {
  let mut c=database();
  let id=scalar(&c,"SELECT id FROM contacts WHERE name='Ali'");
  save_invoice_with_connection(sale("Ali","credit",0,1,20,0),&mut c).unwrap();
  let rename=json!({"id":id,"name":"Ali Updated","kind":"customer","phone":"09120000000","balance":0});
  assert_eq!(save_contact_with_connection(rename,&c).unwrap(),id);
  assert_eq!(c.query_row("SELECT name FROM contacts WHERE id=?1",[id],|r|r.get::<_,String>(0)).unwrap(),"Ali Updated");
  let role_change=json!({"id":id,"name":"Ali Updated","kind":"supplier","phone":"09120000000","balance":0});
  assert!(save_contact_with_connection(role_change,&c).is_err());
  let balance_change=json!({"id":id,"name":"Ali Updated","kind":"customer","phone":"09120000000","balance":50});
  assert!(save_contact_with_connection(balance_change,&c).is_err());
 }
 #[test]
 fn backup_validation_rejects_malformed_records_before_restore_can_start() {
  let missing_fields=json!({"format":"nabz-backup-v1","contacts":[],"products":[{"id":1,"name":"Bad"}],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&missing_fields).is_err());
  let empty=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&empty).is_ok());
  let wrong_kind=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[{"id":1,"kind":"cash","contact_id":null,"contact_name":"A","amount":5,"method":"cash","payment_date":"2026-10-28","notes":""}],"settings":[]});
  assert!(validate_backup(&wrong_kind).is_err());
 }

 #[test]
 fn backup_validation_rejects_missing_foreign_references() {
  let backup=json!({
   "format":"nabz-backup-v1",
   "contacts":[{"id":1,"name":"Ali","kind":"customer","phone":"","opening_balance":0,"notes":"","created_at":"2026-10-28"}],
   "products":[{"id":1,"name":"Pen","code":"PEN","barcode":"","buy_price":10,"sell_price":20,"quantity":10,"min_quantity":2}],
   "invoices":[{"id":1,"number":1001,"kind":"sale","contact_id":1,"contact_name":"Ali","invoice_date":"2026-10-28","subtotal":20,"discount":0,"total":20,"paid":0,"notes":""}],
   "invoice_items":[{"id":1,"invoice_id":1,"product_id":999,"item_name":"Pen","quantity":1,"unit_price":20,"line_total":20}],
   "payments":[],"settings":[]
  });
  assert!(validate_backup(&backup).is_err());
 }

 #[test]
 fn service_invoice_line_does_not_change_inventory() {
  let mut c=database();
  let payload=json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"خدمت: نصب و راه‌اندازی","qty":1,"price":250},{"item":"Pen","qty":2,"price":20}],"discount":0,"date":"2026-10-28"});
  save_invoice_with_connection(payload,&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT total FROM invoices WHERE kind='sale'"),290);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),8);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoice_items WHERE product_id IS NULL AND item_name LIKE 'خدمت:%'"),1);
 }

 #[test]
 fn unsupported_negative_and_dual_contact_opening_balances_are_rejected() {
  assert!(validate_opening_balance("customer",-1).is_err());
  assert!(validate_opening_balance("supplier",-100).is_err());
  assert!(validate_opening_balance("both",1).is_err());
  assert!(validate_opening_balance("customer",0).is_ok());
  assert!(validate_opening_balance("supplier",100).is_ok());
  assert!(validate_opening_balance("both",0).is_ok());
 }
 
 #[test]
 fn sale_of_unknown_stock_item_is_rejected_without_creating_an_invoice() {
  let mut c=database();
  let payload=json!({"kind":"sale","customer":"Ali","payment_status":"paid","method":"cash","items":[{"item":"Pen","qty":1,"price":20},{"item":"Unknown item","qty":1,"price":50}],"discount":0,"date":"2026-10-28"});
  assert!(save_invoice_with_connection(payload,&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
 }
 
 #[test]
 fn paid_walk_in_sale_records_one_receipt_and_updates_stock_atomically() {
  let mut c=database();
  save_invoice_with_connection(sale("","paid",0,2,20,0),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT total FROM invoices WHERE kind='sale'"),40);
  assert_eq!(scalar(&c,"SELECT paid FROM invoices WHERE kind='sale'"),40);
  assert_eq!(scalar(&c,"SELECT SUM(amount) FROM payments WHERE kind='receipt'"),40);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),8);
  // Receipts are reported from the payments ledger, not added to invoice.paid a second time.
  assert_eq!(scalar(&c,"SELECT SUM(amount) FROM payments WHERE kind='receipt'"),scalar(&c,"SELECT paid FROM invoices WHERE kind='sale'"));
 }

 #[test]
 fn partial_payment_and_later_receipt_keep_invoice_and_cash_ledgers_reconciled() {
  let mut c=database();
  save_invoice_with_connection(sale("Ali","partial",30,2,50,0),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT total-paid FROM invoices WHERE kind='sale'"),70);
  save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":20,"method":"cash","date":"2026-10-28"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT paid FROM invoices WHERE kind='sale'"),50);
  assert_eq!(scalar(&c,"SELECT SUM(amount) FROM payments WHERE kind='receipt'"),50);
  assert_eq!(scalar(&c,"SELECT total-paid FROM invoices WHERE kind='sale'"),50);
 }

 #[test]
 fn anonymous_unpaid_sale_is_rejected_without_stock_or_invoice_changes() {
  let mut c=database();
  assert!(save_invoice_with_connection(sale("","credit",0,2,20,0),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM payments"),0);
 }

 #[test]
 fn discount_is_applied_before_receivable_is_calculated() {
  let mut c=database();
  save_invoice_with_connection(sale("Ali","credit",0,2,50,20),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT subtotal FROM invoices"),100);
  assert_eq!(scalar(&c,"SELECT discount FROM invoices"),20);
  assert_eq!(scalar(&c,"SELECT total FROM invoices"),80);
  assert_eq!(scalar(&c,"SELECT total-paid FROM invoices"),80);
 }

 #[test]
 fn insufficient_stock_rolls_back_invoice_and_all_stock_mutations() {
  let mut c=database();
  assert!(save_invoice_with_connection(sale("Ali","credit",0,11,20,0),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
 }

 #[test]
 fn receipt_is_allocated_fifo_and_overpayment_becomes_customer_advance() {
  let mut c=database();
  save_invoice_with_connection(sale("Ali","credit",0,2,50,0),&mut c).unwrap();
  save_invoice_with_connection(sale("Ali","credit",0,1,50,0),&mut c).unwrap();
  save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":120,"method":"cash","date":"2026-10-28"}),&mut c).unwrap();
  let vals={
   let mut st=c.prepare("SELECT paid FROM invoices ORDER BY id").unwrap();
   st.query_map([],|r|r.get::<_,i64>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
  };
  assert_eq!(vals,vec![100,20]);
  let before=scalar(&c,"SELECT COUNT(*) FROM payments");
  save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":31,"method":"cash"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM payments"),before+1);
  assert_eq!(scalar(&c,"SELECT customer_credit_balance FROM contacts WHERE name='Ali'"),1);
  assert_eq!(scalar(&c,"SELECT SUM(total-paid) FROM invoices WHERE kind='sale'"),0);
  save_invoice_with_connection(sale("Ali","credit",0,1,20,0),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT customer_credit_balance FROM contacts WHERE name='Ali'"),0);
  assert_eq!(scalar(&c,"SELECT paid FROM invoices ORDER BY id DESC LIMIT 1"),1);
  assert_eq!(scalar(&c,"SELECT total-paid FROM invoices ORDER BY id DESC LIMIT 1"),19);
 }

 #[test]
 fn supplier_overpayment_is_saved_and_applied_to_future_purchase() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind) VALUES('Vendor','supplier')",[]).unwrap();
  save_payment_with_connection(json!({"kind":"payment","customer":"Vendor","amount":75,"method":"cash"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT supplier_credit_balance FROM contacts WHERE name='Vendor'"),75);
  save_invoice_with_connection(json!({"kind":"purchase","customer":"Vendor","items":[{"item":"Pen","qty":2,"price":10}],"discount":0,"date":"2026-10-28"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT supplier_credit_balance FROM contacts WHERE name='Vendor'"),55);
  assert_eq!(scalar(&c,"SELECT paid FROM invoices WHERE kind='purchase'"),20);
 }

 #[test]
 fn purchase_increases_stock_and_records_supplier_debt_without_receipt() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind) VALUES('Vendor','supplier')",[]).unwrap();
  let payload=json!({"kind":"purchase","customer":"Vendor","items":[{"item":"Pen","qty":5,"price":12}],"discount":0,"date":"2026-10-28"});
  save_invoice_with_connection(payload,&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),15);
  assert_eq!(scalar(&c,"SELECT buy_price FROM products WHERE code='PEN'"),12);
  assert_eq!(scalar(&c,"SELECT total-paid FROM invoices WHERE kind='purchase'"),60);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM payments"),0);
 }

 #[test]
 fn receipt_or_supplier_payment_reduces_positive_opening_balance_before_invoice_fifo() {
  let mut c=database();
  c.execute("UPDATE contacts SET opening_balance=50 WHERE name='Ali'",[]).unwrap();
  save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":20,"method":"cash","date":"2026-10-28"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT opening_balance FROM contacts WHERE name='Ali'"),30);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT SUM(amount) FROM payments WHERE kind='receipt'"),20);
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor','supplier',40)",[]).unwrap();
  save_payment_with_connection(json!({"kind":"payment","customer":"Vendor","amount":15,"method":"cash","date":"2026-10-28"}),&mut c).unwrap();
  assert_eq!(scalar(&c,"SELECT opening_balance FROM contacts WHERE name='Vendor'"),25);
  assert_eq!(scalar(&c,"SELECT SUM(amount) FROM payments WHERE kind='payment'"),15);
 }

 #[test]
 fn reports_reconcile_sales_purchases_receipts_and_role_specific_balances() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor','supplier',40)",[]).unwrap();
  save_invoice_with_connection(sale("Ali","credit",0,2,20,0),&mut c).unwrap();
  save_invoice_with_connection(json!({"kind":"purchase","customer":"Vendor","items":[{"item":"Pen","qty":2,"price":10}],"discount":0,"date":"2026-10-28"}),&mut c).unwrap();
  let before=reports_summary_with_connection(&c).unwrap();
  assert_eq!(before["sales_total"].as_i64(),Some(40));
  assert_eq!(before["purchases_total"].as_i64(),Some(20));
  assert_eq!(before["receipts_total"].as_i64(),Some(0));
  assert_eq!(before["payments_total"].as_i64(),Some(0));
  assert_eq!(before["customer_debt"].as_i64(),Some(40));
  assert_eq!(before["supplier_debt"].as_i64(),Some(60));
  save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":10,"method":"cash"}),&mut c).unwrap();
  save_payment_with_connection(json!({"kind":"payment","customer":"Vendor","amount":15,"method":"cash"}),&mut c).unwrap();
  let after=reports_summary_with_connection(&c).unwrap();
  assert_eq!(after["receipts_total"].as_i64(),Some(10));
  assert_eq!(after["payments_total"].as_i64(),Some(15));
  assert_eq!(after["customer_debt"].as_i64(),Some(30));
  assert_eq!(after["supplier_debt"].as_i64(),Some(45));
 }

 #[test]
 fn net_cash_reports_subtract_sale_refunds_purchase_refunds_and_invoice_voids() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor','supplier',0)",[]).unwrap();
  let sale=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"paid","method":"cash","items":[{"item":"Pen","qty":2,"price":50}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  let purchase=save_invoice_with_connection(json!({"kind":"purchase","customer":"Vendor","items":[{"item":"Pen","qty":2,"price":40}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  save_payment_with_connection(json!({"kind":"payment","customer":"Vendor","amount":80,"method":"cash","date":"2026-11-17"}),&mut c).unwrap();

  c.execute("INSERT INTO sales_returns(invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,'2026-11-17',30,'برگشت آزمایشی',0)",[sale]).unwrap();
  let sales_return=c.last_insert_rowid();
  c.execute("INSERT INTO sales_refunds(invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,'2026-11-17',30,'cash','بازپرداخت آزمایشی')",params![sale,sales_return]).unwrap();

  c.execute("INSERT INTO purchase_returns(invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,'2026-11-17',20,'مرجوعی آزمایشی',0)",[purchase]).unwrap();
  let purchase_return=c.last_insert_rowid();
  c.execute("INSERT INTO purchase_refunds(invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,'2026-11-17',20,'cash','دریافت از تأمین‌کننده')",params![purchase,purchase_return]).unwrap();

  let void_sale=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"paid","method":"cash","items":[{"item":"Pen","qty":1,"price":10}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  c.execute("UPDATE invoices SET voided=1 WHERE id=?1",[void_sale]).unwrap();
  c.execute("INSERT INTO invoice_voids(invoice_id,kind,void_date,cash_refund,credit_refund,notes) VALUES(?1,'sale','2026-11-17',10,0,'ابطال آزمایشی')",[void_sale]).unwrap();
  let summary=reports_summary_with_connection(&c).unwrap();
  assert_eq!(summary["receipts_total"].as_i64(),Some(70));
  assert_eq!(summary["payments_total"].as_i64(),Some(60));
  let period=reports_period_with_connection(&c,"2026-11-17","2026-11-17").unwrap();
  assert_eq!(period["receipts_total"].as_i64(),Some(70));
  assert_eq!(period["payments_total"].as_i64(),Some(60));
 }

 #[test]
 fn contact_ledger_exposes_debit_credit_returns_and_payment_allocations() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor Ledger','supplier',0)",[]).unwrap();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor Opening','supplier',15)",[]).unwrap();
  let sale=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"paid","method":"cash","items":[{"item":"Pen","qty":1,"price":50}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  c.execute("UPDATE invoices SET total=40,paid=45 WHERE id=?1",[sale]).unwrap();
  c.execute("INSERT INTO sales_returns(invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,'2026-11-17',10,'برگشت مشتری',0)",[sale]).unwrap();
  let sale_return=c.last_insert_rowid();
  c.execute("INSERT INTO sales_refunds(invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,'2026-11-17',5,'cash','بازپرداخت مشتری')",params![sale,sale_return]).unwrap();

  let purchase=save_invoice_with_connection(json!({"kind":"purchase","customer":"Vendor Ledger","items":[{"item":"Pen","qty":1,"price":80}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  save_payment_with_connection(json!({"kind":"payment","customer":"Vendor Ledger","amount":80,"method":"cash","date":"2026-11-17"}),&mut c).unwrap();
  c.execute("UPDATE invoices SET total=60,paid=70 WHERE id=?1",[purchase]).unwrap();
  c.execute("INSERT INTO purchase_returns(invoice_id,return_date,amount,notes,credit_refund) VALUES(?1,'2026-11-17',20,'مرجوعی تأمین‌کننده',0)",[purchase]).unwrap();
  let purchase_return=c.last_insert_rowid();
  c.execute("INSERT INTO purchase_refunds(invoice_id,return_id,refund_date,amount,method,notes) VALUES(?1,?2,'2026-11-17',10,'cash','بازپرداخت تأمین‌کننده')",params![purchase,purchase_return]).unwrap();

  let customer_id:i64=c.query_row("SELECT id FROM contacts WHERE name='Ali'",[],|r|r.get(0)).unwrap();
  let customer=contact_ledger_with_connection(customer_id,&c).unwrap();
  let customer_invoice=customer["entries"].as_array().unwrap().iter().find(|x|x["entry_type"]=="invoice").unwrap();
  let customer_payment=customer["entries"].as_array().unwrap().iter().find(|x|x["entry_type"]=="payment").unwrap();
  assert_eq!((customer_invoice["debit"].as_i64(),customer_invoice["credit"].as_i64()),(Some(50),Some(0)));
  assert_eq!((customer_payment["debit"].as_i64(),customer_payment["credit"].as_i64()),(Some(0),Some(50)));
  assert_eq!(customer["sales_returns"][0]["credit"].as_i64(),Some(10));
  assert_eq!(customer["refunds"][0]["debit"].as_i64(),Some(5));
  assert_eq!(customer["payment_allocations"][0]["allocation_type"].as_str(),Some("invoice"));

  let supplier_id:i64=c.query_row("SELECT id FROM contacts WHERE name='Vendor Ledger'",[],|r|r.get(0)).unwrap();
  let supplier=contact_ledger_with_connection(supplier_id,&c).unwrap();
  let supplier_invoice=supplier["entries"].as_array().unwrap().iter().find(|x|x["entry_type"]=="invoice").unwrap();
  let supplier_payment=supplier["entries"].as_array().unwrap().iter().find(|x|x["entry_type"]=="payment").unwrap();
  assert_eq!((supplier_invoice["debit"].as_i64(),supplier_invoice["credit"].as_i64()),(Some(0),Some(80)));
  assert_eq!((supplier_payment["debit"].as_i64(),supplier_payment["credit"].as_i64()),(Some(80),Some(0)));
  assert_eq!(supplier["purchase_returns"][0]["debit"].as_i64(),Some(20));
  assert_eq!(supplier["refunds"][0]["credit"].as_i64(),Some(10));
  assert_eq!(supplier["ledger_balances"]["opening_credit"].as_i64(),Some(0));
  let opening_id:i64=c.query_row("SELECT id FROM contacts WHERE name='Vendor Opening'",[],|r|r.get(0)).unwrap();
  let opening=contact_ledger_with_connection(opening_id,&c).unwrap();
  assert_eq!(opening["ledger_balances"]["opening_credit"].as_i64(),Some(15));
 }

 #[test]
 fn wrong_contact_role_is_rejected_without_financial_or_stock_side_effects() {
  let mut c=database();
  let purchase=json!({"kind":"purchase","customer":"Ali","items":[{"item":"Pen","qty":2,"price":10}],"discount":0});
  assert!(save_invoice_with_connection(purchase,&mut c).is_err());
  assert!(save_payment_with_connection(json!({"kind":"payment","customer":"Ali","amount":10}),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM payments"),0);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
 }

 #[test]
 fn negative_minimum_stock_is_rejected_without_creating_product() {
  let mut c=database();
  let db_before=scalar(&c,"SELECT COUNT(*) FROM products");
  let payload=json!({"name":"Bad product","code":"BAD","qty":0,"buy":1,"sell":2,"min":-1});
  assert!(save_product_with_connection(payload,&c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM products"),db_before);
 }

 #[test]
 fn opening_balances_are_separated_by_contact_role() {
  let mut c=database();
  c.execute("UPDATE contacts SET opening_balance=25 WHERE name='Ali'",[]).unwrap();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor','supplier',40)",[]).unwrap();
  assert_eq!(scalar(&c,"SELECT SUM(opening_balance) FROM contacts WHERE kind='customer'"),25);
  assert_eq!(scalar(&c,"SELECT SUM(opening_balance) FROM contacts WHERE kind='supplier'"),40);
 }

 #[test]
 fn unknown_customer_and_invalid_discount_are_rejected_without_side_effects() {
  let mut c=database();
  assert!(save_invoice_with_connection(sale("Ghost","credit",0,1,20,0),&mut c).is_err());
  assert!(save_invoice_with_connection(sale("Ali","credit",0,1,20,21),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM invoices"),0);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
 }
 #[test]
 fn tauri_invoice_adapter_uses_shared_cost_snapshot_and_consumes_customer_credit(){
  let mut c=database();
  c.execute("UPDATE contacts SET customer_credit_balance=30 WHERE name='Ali'",[]).unwrap();
  let invoice=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"paid","items":[{"item":"Pen","qty":1,"price":20}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  let (paid,credit,cash):(i64,i64,i64)=c.query_row(
   "SELECT i.paid,i.credit_applied,(SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND notes LIKE '%فاکتور فروش%') FROM invoices i WHERE i.id=?1",
   [invoice],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
  ).unwrap();
  let (unit_cost,qty_before,avg_before):(Option<i64>,Option<i64>,Option<i64>)=c.query_row(
   "SELECT unit_cost,quantity_before,average_cost_before FROM invoice_items WHERE invoice_id=?1",[invoice],
   |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
  ).unwrap();
  assert_eq!((paid,credit,cash),(20,20,0));
  assert_eq!((unit_cost,qty_before,avg_before),(Some(10),Some(10),Some(10)));
  assert_eq!(scalar(&c,"SELECT customer_credit_balance FROM contacts WHERE name='Ali'"),10);
  core_void_invoice(&mut c,invoice).unwrap();
  assert_eq!(scalar(&c,"SELECT customer_credit_balance FROM contacts WHERE name='Ali'"),30);
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),10);
  assert_eq!(scalar(&c,&format!("SELECT voided FROM invoices WHERE id={invoice}")),1);
 }

 #[test]
 fn tauri_invoice_adapter_keeps_old_cogs_when_later_purchase_changes_average_cost(){
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Supplier','supplier',0)",[]).unwrap();
  let sale=save_invoice_with_connection(json!({"kind":"sale","customer":"Ali","payment_status":"credit","items":[{"item":"PEN","qty":2,"price":20}],"discount":0,"date":"2026-11-16"}),&mut c).unwrap();
  let purchase=save_invoice_with_connection(json!({"kind":"purchase","customer":"Supplier","items":[{"item":"PEN","qty":2,"price":30}],"discount":0,"date":"2026-11-17"}),&mut c).unwrap();
  let cost:i64=c.query_row("SELECT unit_cost FROM invoice_items WHERE invoice_id=?1",[sale],|r|r.get(0)).unwrap();
  let average:i64=scalar(&c,"SELECT average_cost FROM products WHERE code='PEN'");
  let purchase_cost:i64=c.query_row("SELECT unit_cost FROM invoice_items WHERE invoice_id=?1",[purchase],|r|r.get(0)).unwrap();
  assert_eq!(cost,10);
  assert_eq!(purchase_cost,30);
  assert!(average>10);
 }

 #[test]
 fn product_soft_deactivation_requires_zero_stock_and_can_be_reversed() {
  let c=database();
  let id=save_product_with_connection(json!({"name":"Soft item","code":"SOFT-1","barcode":"SOFT-1-B","qty":3,"buy":10,"sell":20,"min":1,"unit":"عدد"}),&c).unwrap();
  assert!(set_product_active_with_connection(&c,id,false).is_err());
  c.execute("UPDATE products SET quantity=0 WHERE id=?1",[id]).unwrap();
  set_product_active_with_connection(&c,id,false).unwrap();
  let active:i64=c.query_row("SELECT is_active FROM products WHERE id=?1",[id],|r|r.get(0)).unwrap();
  assert_eq!(active,0);
  set_product_active_with_connection(&c,id,true).unwrap();
  let active:i64=c.query_row("SELECT is_active FROM products WHERE id=?1",[id],|r|r.get(0)).unwrap();
  assert_eq!(active,1);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM products WHERE code='SOFT-1'"),1);
 }
 #[test]
 fn service_product_is_excluded_from_stock_and_duplicate_barcodes_are_rejected() {
  let c=database();
  let service=json!({"name":"Installation","code":"SERVICE-01","barcode":"SRV-001","qty":0,"buy":0,"sell":250,"min":0,"category":"خدمات","unit":"سرویس","wholesale":0,"is_service":true});
  let id=save_product_with_connection(service,&c).unwrap();
  let (is_service,quantity,average):(i64,i64,i64)=c.query_row(
   "SELECT is_service,quantity,average_cost FROM products WHERE id=?1",[id],
   |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
  ).unwrap();
  assert_eq!((is_service,quantity,average),(1,0,0));
  let duplicate=json!({"name":"Duplicate","code":"SERVICE-02","barcode":"SRV-001","qty":0,"buy":0,"sell":100,"min":0,"unit":"عدد","is_service":false});
  assert!(save_product_with_connection(duplicate,&c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM products WHERE barcode='SRV-001'"),1);
 }

}
