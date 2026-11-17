use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};
use tauri::{Manager, State};
use nabz_core::accounting::{
    adjust_inventory, create_expense, create_invoice_with_payment_split, inventory_valuation,
    list_expenses, profit_loss_report, record_payment, return_purchase_items, return_sale_items,
    void_invoice as core_void_invoice, ExpenseInput, InvoiceInput, InvoiceLineInput,
    PaymentInput, ReturnLineInput,
};

struct Db(Mutex<Connection>);
#[derive(Serialize)]
struct Summary { sales_today:i64, receipts_today:i64, customer_balance:i64, inventory_value:i64, contacts:i64, products:i64, invoices:i64 }
#[derive(Serialize)]
struct Record { id:i64, name:String, detail:String, amount:i64, status:String, date:String }
#[derive(Serialize)]
struct ProductRecord { id:i64, name:String, code:String, barcode:String, quantity:i64, buy_price:i64, sell_price:i64, min_quantity:i64 }

fn text(v:&Value,k:&str)->String { v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned() }
fn num(v:&Value,k:&str)->i64 { v.get(k).and_then(|x|x.as_i64().or_else(||x.as_str().and_then(|s|s.replace(',',"").replace('٬',"").parse().ok()))).unwrap_or(0) }
fn normalize_contact_kind(raw:&str)->Option<&'static str>{ nabz_core::normalize_contact_kind(raw) }
fn init(c:&Connection)->Result<(),String>{ nabz_core::initialize(c) }
#[tauri::command] fn app_version()-> &'static str { env!("CARGO_PKG_VERSION") }
#[tauri::command] fn db_status(db:State<'_,Db>)->Result<String,String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.query_row("PRAGMA database_list",[],|r|r.get(2)).map_err(|e|e.to_string())}
#[tauri::command] fn dashboard_summary(db:State<'_,Db>)->Result<Summary,String>{let c=db.0.lock().map_err(|e|e.to_string())?;Ok(Summary{sales_today:c.query_row("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date=date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?,receipts_today:c.query_row("SELECT MAX(0,COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date=date('now','localtime')),0))",[],|r|r.get(0)).map_err(|e|e.to_string())?,customer_balance:c.query_row("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)-COALESCE((SELECT SUM(customer_credit_balance) FROM contacts WHERE kind IN ('customer','both')),0)",[],|r|r.get(0)).map_err(|e|e.to_string())?,inventory_value:c.query_row("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,contacts:c.query_row("SELECT COUNT(*) FROM contacts",[],|r|r.get(0)).map_err(|e|e.to_string())?,products:c.query_row("SELECT COUNT(*) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,invoices:c.query_row("SELECT COUNT(*) FROM invoices",[],|r|r.get(0)).map_err(|e|e.to_string())?})}
#[tauri::command] fn list_records(kind:String,db:State<'_,Db>)->Result<Vec<Record>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let sql=match kind.as_str(){"contacts"=>"SELECT id,name,phone,opening_balance,kind,datetime(created_at) FROM contacts ORDER BY id DESC","products"=>"SELECT id,name,code,quantity,sell_price,'موجود', '' FROM products ORDER BY id DESC","invoices"=>"SELECT id,'فاکتور #'||number||' · '||contact_name,kind,total,CASE WHEN voided=1 THEN 'باطل‌شده' WHEN paid>=total THEN 'تسویه' ELSE 'بدهکار' END,invoice_date FROM invoices ORDER BY id DESC","payments"=>"SELECT id,contact_name,kind,amount,method,payment_date FROM payments ORDER BY id DESC",_=>return Err("فهرست نامعتبر است".into())};let mut s=c.prepare(sql).map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Record{id:r.get(0)?,name:r.get(1)?,detail:r.get(2)?,amount:r.get(3)?,status:r.get(4)?,date:r.get(5)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn list_products(db:State<'_,Db>)->Result<Vec<ProductRecord>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT id,name,code,barcode,quantity,buy_price,sell_price,min_quantity FROM products ORDER BY id DESC").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(ProductRecord{id:r.get(0)?,name:r.get(1)?,code:r.get(2)?,barcode:r.get(3)?,quantity:r.get(4)?,buy_price:r.get(5)?,sell_price:r.get(6)?,min_quantity:r.get(7)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command]
fn delete_record(kind:String,id:i64,db:State<'_,Db>)->Result<(),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 match kind.as_str(){
  "contacts"=>{let n:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM invoices WHERE contact_id=?1)+(SELECT COUNT(*) FROM payments WHERE contact_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این طرف‌حساب سابقه فاکتور یا دریافت/پرداخت دارد و حذف نمی‌شود.".into())}tx.execute("DELETE FROM contacts WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "products"=>{let n:i64=tx.query_row("SELECT COUNT(*) FROM invoice_items WHERE product_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این کالا در فاکتور ثبت شده و برای حفظ سابقه حذف نمی‌شود.".into())}tx.execute("DELETE FROM products WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
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
 let n=text(&payload,"name");
 if n.is_empty(){return Err("نام طرف‌حساب الزامی است".into())}
 let raw_kind=text(&payload,"kind");
 let kind=normalize_contact_kind(&raw_kind).ok_or("نوع طرف‌حساب را از گزینه‌های موجود انتخاب کنید")?;
 let balance=num(&payload,"balance");
 if let Some(id)=payload.get("id").and_then(Value::as_i64){
  if id<=0{return Err("شناسه طرف‌حساب نامعتبر است".into())}
  let old:Option<(String,i64)>=c.query_row("SELECT kind,opening_balance FROM contacts WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
  let Some((old_kind,old_balance))=old else{return Err("طرف‌حساب پیدا نشد".into())};
  let history:i64=c.query_row("SELECT (SELECT COUNT(*) FROM invoices WHERE contact_id=?1)+(SELECT COUNT(*) FROM payments WHERE contact_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if history>0&&(kind!=old_kind||balance!=old_balance){return Err("این طرف‌حساب سابقه مالی دارد؛ نوع و مانده افتتاحیه برای حفظ دفتر مالی قابل تغییر نیست.".into())}
  if history==0{validate_opening_balance(kind,balance)?}
  let duplicate:i64=c.query_row("SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE AND id<>?2",params![n,id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if duplicate>0{return Err("طرف‌حساب دیگری با این نام وجود دارد".into())}
  c.execute("UPDATE contacts SET name=?1,kind=?2,phone=?3,opening_balance=?4 WHERE id=?5",params![n,kind,text(&payload,"phone"),balance,id]).map_err(|e|e.to_string())?;
  return Ok(id)
 }
 let existing:i64=c.query_row("SELECT COUNT(*) FROM contacts WHERE name=?1 COLLATE NOCASE",[&n],|r|r.get(0)).map_err(|e|e.to_string())?;
 if existing>0{return Err("طرف‌حسابی با این نام قبلاً ثبت شده است".into())}
 validate_opening_balance(kind,balance)?;
 c.execute("INSERT INTO contacts(name,kind,phone,opening_balance,notes) VALUES(?1,?2,?3,?4,?5)",params![n,kind,text(&payload,"phone"),balance,text(&payload,"notes")]).map_err(|e|e.to_string())?;
 Ok(c.last_insert_rowid())
}
#[tauri::command]
fn save_product(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 save_product_with_connection(payload,&c)
}
fn save_product_with_connection(payload:Value,c:&Connection)->Result<i64,String>{
 let n=text(&payload,"name");let code=text(&payload,"code");let qty=num(&payload,"qty");let buy=num(&payload,"buy");let sell=num(&payload,"sell");let min=num(&payload,"min");let barcode=text(&payload,"barcode");
 if n.is_empty()||code.is_empty(){return Err("نام و کد کالا الزامی است".into())}
 if qty<0||buy<0||sell<0||min<0{return Err("موجودی، حداقل موجودی و قیمت نمی‌تواند منفی باشد".into())}
 if let Some(id)=payload.get("id").and_then(Value::as_i64){
  if id<=0{return Err("شناسه کالا نامعتبر است".into())}
  let exists:i64=c.query_row("SELECT COUNT(*) FROM products WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if exists==0{return Err("کالا پیدا نشد".into())}
  let has_history:i64=c.query_row("SELECT COUNT(*) FROM invoice_items WHERE product_id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
  let result=if has_history>0{
   c.execute("UPDATE products SET name=?1,code=?2,barcode=?3,buy_price=?4,sell_price=?5,min_quantity=?6 WHERE id=?7",params![n,code,barcode,buy,sell,min,id])
  }else{
   c.execute("UPDATE products SET name=?1,code=?2,barcode=?3,buy_price=?4,sell_price=?5,quantity=?6,min_quantity=?7,average_cost=CASE WHEN ?6=0 THEN 0 WHEN quantity=0 THEN ?4 ELSE average_cost END WHERE id=?8",params![n,code,barcode,buy,sell,qty,min,id])
  };
  result.map_err(|e|if e.to_string().contains("UNIQUE"){"کد کالا تکراری است".into()}else{e.to_string()})?;
  return Ok(id)
 }
 c.execute("INSERT INTO products(name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost) VALUES(?1,?2,?3,?4,?5,?6,?7,CASE WHEN ?6>0 THEN ?4 ELSE 0 END)",params![n,code,barcode,buy,sell,qty,min]).map_err(|e|if e.to_string().contains("UNIQUE"){"کد کالا تکراری است".into()}else{e.to_string()})?;
 Ok(c.last_insert_rowid())
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
 create_invoice_with_payment_split(c,&input,cash_received,credit_used)
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
  "receipts_today":q("SELECT MAX(0,COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date=date('now','localtime')),0))?,
  "sales_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0")?,
  "purchases_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase' AND voided=0")?,
  "receipts_total":q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt'),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds),0)")?,
  "payments_total":q("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='payment'),0)-COALESCE((SELECT SUM(amount) FROM purchase_refunds),0)")?,
  "customer_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('customer','both')),0)-COALESCE((SELECT SUM(customer_credit_balance) FROM contacts WHERE kind IN ('customer','both')),0)")?,
  "supplier_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='purchase' AND voided=0),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind IN ('supplier','both')),0)-COALESCE((SELECT SUM(supplier_credit_balance) FROM contacts WHERE kind IN ('supplier','both')),0)")?,
  "inventory_value":q("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products")?,
  "contacts":q("SELECT COUNT(*) FROM contacts")?,
  "products":q("SELECT COUNT(*) FROM products")?,
  "invoices":q("SELECT COUNT(*) FROM invoices")?
 }))
}
#[tauri::command]
fn reports_period(start_date:String,end_date:String,db:State<'_,Db>)->Result<Value,String>{
 if start_date>end_date{return Err("تاریخ شروع باید قبل از تاریخ پایان باشد.".into())}
 let valid=|s:&str|{let b=s.as_bytes();s.len()==10&&b[4]==b'-'&&b[7]==b'-'&&b.iter().enumerate().all(|(i,x)|i==4||i==7||x.is_ascii_digit())};
 if !valid(&start_date)||!valid(&end_date){return Err("تاریخ باید با قالب میلادی ISO ارسال شود.".into())}
 let c=db.0.lock().map_err(|e|e.to_string())?;
 reports_period_with_connection(&c,&start_date,&end_date)
}
fn reports_period_with_connection(c:&Connection,start:&str,end:&str)->Result<Value,String>{
 let sum=|sql:&str|->Result<i64,String>{c.query_row(sql,params![start,end],|r|r.get(0)).map_err(|e|e.to_string())};
 Ok(json!({
  "start_date":start,
  "end_date":end,
  "sales_total":sum("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2")?,
  "purchases_total":sum("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2")?,
  "receipts_total":sum("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='receipt' AND payment_date BETWEEN ?1 AND ?2),0)-COALESCE((SELECT SUM(amount) FROM sales_refunds WHERE refund_date BETWEEN ?1 AND ?2),0)")?,
  "payments_total":sum("SELECT COALESCE((SELECT SUM(amount) FROM payments WHERE kind='payment' AND payment_date BETWEEN ?1 AND ?2),0)-COALESCE((SELECT SUM(amount) FROM purchase_refunds WHERE refund_date BETWEEN ?1 AND ?2),0)")?,
  "sales_count":sum("SELECT COUNT(*) FROM invoices WHERE kind='sale' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2")?,
  "purchases_count":sum("SELECT COUNT(*) FROM invoices WHERE kind='purchase' AND voided=0 AND invoice_date BETWEEN ?1 AND ?2")?
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
 let products=collect_json_rows(&c,"SELECT id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost FROM products",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"code":r.get::<_,String>(2)?,
  "barcode":r.get::<_,String>(3)?,"buy_price":r.get::<_,i64>(4)?,"sell_price":r.get::<_,i64>(5)?,
  "quantity":r.get::<_,i64>(6)?,"min_quantity":r.get::<_,i64>(7)?,"average_cost":r.get::<_,i64>(8)?
})))?;
 let invoices=collect_json_rows(&c,"SELECT id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied FROM invoices",|r|Ok(json!({
  "id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,
  "contact_id":r.get::<_,Option<i64>>(3)?,"contact_name":r.get::<_,String>(4)?,"invoice_date":r.get::<_,String>(5)?,
  "subtotal":r.get::<_,i64>(6)?,"discount":r.get::<_,i64>(7)?,"total":r.get::<_,i64>(8)?,
  "paid":r.get::<_,i64>(9)?,"notes":r.get::<_,String>(10)?,"voided":r.get::<_,i64>(11)?,
  "credit_applied":r.get::<_,i64>(12)?
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
  "invoice_items":invoice_items,"payments":payments,"settings":settings,
  "sales_returns":sales_returns,"sales_return_items":sales_return_items,"sales_refunds":sales_refunds,
  "purchase_returns":purchase_returns,"purchase_return_items":purchase_return_items,"purchase_refunds":purchase_refunds,
  "inventory_adjustments":inventory_adjustments,"expenses":expenses
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
 let optional_keys=["sales_returns","sales_return_items","sales_refunds","purchase_returns","purchase_return_items","purchase_refunds","inventory_adjustments","expenses"];
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
      &&(x.get("average_cost").is_none()||x["average_cost"].as_i64().unwrap_or(-1)>=0)),
    "invoices"=>!(backup_has_int(x,"id")&&backup_has_int(x,"number")&&backup_has_string(x,"kind")
      &&["sale","purchase"].contains(&x["kind"].as_str().unwrap_or(""))&&backup_has_optional_int(x,"contact_id")
      &&backup_has_string(x,"contact_name")&&backup_has_string(x,"invoice_date")&&backup_has_int(x,"subtotal")
      &&backup_has_int(x,"discount")&&backup_has_int(x,"total")&&backup_has_int(x,"paid")&&backup_has_string(x,"notes")
      &&x["subtotal"].as_i64().unwrap_or(-1)>=0&&x["discount"].as_i64().unwrap_or(-1)>=0
      &&x["total"].as_i64().unwrap_or(-1)>=0&&x["paid"].as_i64().unwrap_or(-1)>=0
      &&x["paid"].as_i64().unwrap_or(i64::MAX)<=x["total"].as_i64().unwrap_or(-1)
      &&(x.get("voided").is_none()||[0,1].contains(&x["voided"].as_i64().unwrap_or(-1)))
      &&(x.get("credit_applied").is_none()||(x["credit_applied"].as_i64().unwrap_or(-1)>=0&&x["credit_applied"].as_i64().unwrap_or(i64::MAX)<=x["paid"].as_i64().unwrap_or(-1)))),
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
 let item_ids=ids("invoice_items")?;let payment_ids=ids("payments")?;let _=payment_ids;
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
  tx.execute("INSERT INTO products(id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
   params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["code"].as_str().unwrap_or(""),
    x["barcode"].as_str().unwrap_or(""),buy,x["sell_price"].as_i64().unwrap_or(0),quantity,
    x["min_quantity"].as_i64().unwrap_or(0),average]).map_err(|e|e.to_string())?;
 }
 for x in data["invoices"].as_array().unwrap(){
  tx.execute("INSERT INTO invoices(id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes,voided,credit_applied)
   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
   params![x["id"].as_i64(),x["number"].as_i64(),x["kind"].as_str().unwrap_or("sale"),x["contact_id"].as_i64(),
    x["contact_name"].as_str().unwrap_or(""),x["invoice_date"].as_str().unwrap_or(""),x["subtotal"].as_i64().unwrap_or(0),
    x["discount"].as_i64().unwrap_or(0),x["total"].as_i64().unwrap_or(0),x["paid"].as_i64().unwrap_or(0),
    x["notes"].as_str().unwrap_or(""),x.get("voided").and_then(Value::as_i64).unwrap_or(0),
    x.get("credit_applied").and_then(Value::as_i64).unwrap_or(0)]).map_err(|e|e.to_string())?;
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
  "CREATE TRIGGER IF NOT EXISTS inventory_adjustments_no_update BEFORE UPDATE ON inventory_adjustments
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
  let quantity=x.get("qty").or_else(||x.get("quantity")).and_then(|v|v.as_i64().or_else(||v.as_str().and_then(|n|n.parse().ok()))).unwrap_or(0);
  ReturnLineInput{product_lookup:lookup,quantity}
 }).collect::<Vec<_>>();
 if parsed.is_empty()||parsed.iter().any(|x|x.product_lookup.is_empty()||x.quantity<=0){return Err("ردیف‌های برگشت باید کالا و تعداد مثبت داشته باشند".into())}
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
fn get_contact_ledger(id:i64,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let contact:Value=c.query_row("SELECT id,name,kind,phone,opening_balance,customer_credit_balance,supplier_credit_balance FROM contacts WHERE id=?1",[id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"phone":r.get::<_,String>(3)?,"opening_balance":r.get::<_,i64>(4)?,"customer_credit_balance":r.get::<_,i64>(5)?,"supplier_credit_balance":r.get::<_,i64>(6)?}))).optional().map_err(|e|e.to_string())?.ok_or("طرف‌حساب پیدا نشد")?;
 let mut entries:Vec<Value>=Vec::new();
 {
  let mut st=c.prepare("SELECT id,number,kind,invoice_date,total,paid,notes,voided,credit_applied FROM invoices WHERE contact_id=?1 ORDER BY invoice_date,id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|{
   let voided=r.get::<_,i64>(7)?;
   let total=r.get::<_,i64>(4)?;
   let paid=r.get::<_,i64>(5)?;
   Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"entry_type":"invoice","kind":r.get::<_,String>(2)?,"date":r.get::<_,String>(3)?,"amount":total,"paid":paid,"balance_delta":if voided==1{0}else{total-paid},"voided":voided==1,"credit_applied":r.get::<_,i64>(8)?,"notes":r.get::<_,String>(6)?}))
  }).map_err(|e|e.to_string())?;
  for row in rows{entries.push(row.map_err(|e|e.to_string())?);}
 }
 {
  let mut st=c.prepare("SELECT id,kind,payment_date,amount,method,notes FROM payments WHERE contact_id=?1 ORDER BY payment_date,id").map_err(|e|e.to_string())?;
  let rows=st.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"entry_type":"payment","kind":r.get::<_,String>(1)?,"date":r.get::<_,String>(2)?,"amount":r.get::<_,i64>(3)?,"method":r.get::<_,String>(4)?,"notes":r.get::<_,String>(5)?}))).map_err(|e|e.to_string())?;
  for row in rows{entries.push(row.map_err(|e|e.to_string())?);}
 }
 entries.sort_by(|a,b|a["date"].as_str().unwrap_or("").cmp(b["date"].as_str().unwrap_or("")));
 Ok(json!({"contact":contact,"entries":entries}))
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
#[tauri::command] fn save_setting(key:String,value:String,db:State<'_,Db>)->Result<(),String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command] fn get_settings(db:State<'_,Db>)->Result<std::collections::HashMap<String,String>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;rows.collect::<Result<_,_>>().map_err(|e|e.to_string())}
#[cfg_attr(mobile,tauri::mobile_entry_point)] pub fn run(){tauri::Builder::default().setup(|app|{let c=nabz_core::open_database().map_err(std::io::Error::other)?;app.manage(Db(Mutex::new(c)));Ok(())}).invoke_handler(tauri::generate_handler![app_version,db_status,dashboard_summary,reports_summary,export_data,restore_data,list_records,list_products,delete_record,save_contact,save_product,save_invoice,save_payment,return_sale_invoice,return_purchase_invoice,void_invoice,adjust_inventory_command,inventory_valuation_command,create_expense_command,list_expenses_command,profit_loss_report_command,get_invoice_print,get_contact_ledger,save_setting,get_settings,sales_trend,reports_period]).run(tauri::generate_context!()).expect("error while running Nabz");}


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

}
