use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{Manager, State};

struct Db(Mutex<Connection>);
#[derive(Serialize)]
struct Summary { sales_today:i64, receipts_today:i64, customer_balance:i64, inventory_value:i64, contacts:i64, products:i64, invoices:i64 }
#[derive(Serialize)]
struct Record { id:i64, name:String, detail:String, amount:i64, status:String, date:String }
#[derive(Serialize)]
struct ProductRecord { id:i64, name:String, code:String, quantity:i64, buy_price:i64, average_cost:i64, sell_price:i64, min_quantity:i64 }
#[tauri::command]
fn inventory_report(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let products=nabz_core::accounting::inventory_valuation(&c)?;
 let movements=nabz_core::accounting::inventory_movements(&c)?;
 let total_units=products.iter().try_fold(0_i64,|sum,p|sum.checked_add(p.quantity))
  .ok_or("جمع تعداد موجودی بیش از ظرفیت عددی است")?;
 let inventory_value=products.iter().try_fold(0_i64,|sum,p|sum.checked_add(p.stock_value))
  .ok_or("ارزش کل انبار بیش از ظرفیت عددی است")?;
 let low_stock_count=products.iter().filter(|p|p.quantity<=p.min_quantity).count();
 let zero_stock_count=products.iter().filter(|p|p.quantity==0).count();
 Ok(json!({
  "summary":{
   "total_products":products.len(),"total_units":total_units,
   "inventory_value":inventory_value,"low_stock_count":low_stock_count,
   "zero_stock_count":zero_stock_count
  },
  "products":products.iter().map(|p|json!({
   "product_id":p.product_id,"product_name":p.product_name,"code":p.code,
   "quantity":p.quantity,"buy_price":p.buy_price,"average_cost":p.average_cost,"stock_value":p.stock_value,
   "min_quantity":p.min_quantity
  })).collect::<Vec<_>>(),
  "movements":movements.iter().take(500).map(|m|json!({
   "product_id":m.product_id,"product_name":m.product_name,
   "movement_date":m.movement_date,"movement_type":m.movement_type,
   "reference":m.reference,"quantity_delta":m.quantity_delta,
   "balance_after":m.balance_after
  })).collect::<Vec<_>>()
 }))
}


#[tauri::command]
fn save_expense(payload: Value, db: State<'_,Db>) -> Result<i64,String> {
 let c=db.0.lock().map_err(|e|e.to_string())?;
 nabz_core::accounting::create_expense(&c,&nabz_core::accounting::ExpenseInput{
  category:text(&payload,"category"),description:text(&payload,"description"),
  amount:num(&payload,"amount"),date:text(&payload,"date")
 })
}
#[tauri::command]
fn profit_loss_report(from:String,to:String,db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let report=nabz_core::accounting::profit_loss_report(&c,&from,&to)?;
 Ok(json!({
  "sales":report.sales,"discounts":report.discounts,"net_sales":report.net_sales,
  "cost_of_goods_sold":report.cost_of_goods_sold,"gross_profit":report.gross_profit,
  "expenses":report.expenses,"net_profit":report.net_profit,
  "uncosted_sales":report.uncosted_sales,
  "expense_rows":report.expense_rows.iter().map(|e|json!({
   "id":e.id,"category":e.category,"description":e.description,"amount":e.amount,"date":e.date
  })).collect::<Vec<_>>()
 }))
}

fn text(v:&Value,k:&str)->String { v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_owned() }
fn num(v:&Value,k:&str)->i64 { v.get(k).and_then(|x|x.as_i64().or_else(||x.as_str().and_then(|s|s.replace(',',"").replace('٬',"").parse().ok()))).unwrap_or(0) }
fn normalize_contact_kind(raw:&str)->Option<&'static str>{ nabz_core::normalize_contact_kind(raw) }
fn init(c:&Connection)->Result<(),String>{ nabz_core::initialize(c) }
#[tauri::command] fn app_version()-> &'static str { env!("CARGO_PKG_VERSION") }
#[tauri::command] fn db_status(db:State<'_,Db>)->Result<String,String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.query_row("PRAGMA database_list",[],|r|r.get(2)).map_err(|e|e.to_string())}
#[tauri::command] fn dashboard_summary(db:State<'_,Db>)->Result<Summary,String>{let c=db.0.lock().map_err(|e|e.to_string())?;Ok(Summary{sales_today:c.query_row("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?,receipts_today:c.query_row("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')",[],|r|r.get(0)).map_err(|e|e.to_string())?,customer_balance:c.query_row("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale'),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind='customer'),0)",[],|r|r.get(0)).map_err(|e|e.to_string())?,inventory_value:c.query_row("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,contacts:c.query_row("SELECT COUNT(*) FROM contacts",[],|r|r.get(0)).map_err(|e|e.to_string())?,products:c.query_row("SELECT COUNT(*) FROM products",[],|r|r.get(0)).map_err(|e|e.to_string())?,invoices:c.query_row("SELECT COUNT(*) FROM invoices",[],|r|r.get(0)).map_err(|e|e.to_string())?})}
#[tauri::command] fn list_records(kind:String,db:State<'_,Db>)->Result<Vec<Record>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let sql=match kind.as_str(){"contacts"=>"SELECT id,name,phone,opening_balance,kind,datetime(created_at) FROM contacts ORDER BY id DESC","products"=>"SELECT id,name,code,quantity,sell_price,'موجود', '' FROM products ORDER BY id DESC","invoices"=>"SELECT id,'فاکتور #'||number||' · '||contact_name,kind,total,CASE WHEN paid>=total THEN 'تسویه' ELSE 'بدهکار' END,invoice_date FROM invoices ORDER BY id DESC","payments"=>"SELECT id,contact_name,kind,amount,method,payment_date FROM payments ORDER BY id DESC",_=>return Err("فهرست نامعتبر است".into())};let mut s=c.prepare(sql).map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Record{id:r.get(0)?,name:r.get(1)?,detail:r.get(2)?,amount:r.get(3)?,status:r.get(4)?,date:r.get(5)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn list_products(db:State<'_,Db>)->Result<Vec<ProductRecord>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT id,name,code,quantity,buy_price,average_cost,sell_price,min_quantity FROM products ORDER BY id DESC").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(ProductRecord{id:r.get(0)?,name:r.get(1)?,code:r.get(2)?,quantity:r.get(3)?,buy_price:r.get(4)?,average_cost:r.get(5)?,sell_price:r.get(6)?,min_quantity:r.get(7)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command]
fn delete_record(kind:String,id:i64,db:State<'_,Db>)->Result<(),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 if kind=="invoices" {
  return nabz_core::accounting::void_invoice(&mut c,id);
 }
 let tx=c.transaction().map_err(|e|e.to_string())?;
 match kind.as_str(){
  "contacts"=>{let n:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM invoices WHERE contact_id=?1)+(SELECT COUNT(*) FROM payments WHERE contact_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این طرف‌حساب سابقه فاکتور یا دریافت/پرداخت دارد و حذف نمی‌شود.".into())}tx.execute("DELETE FROM contacts WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "products"=>{let n:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM invoice_items WHERE product_id=?1)+(SELECT COUNT(*) FROM inventory_adjustments WHERE product_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;if n>0{return Err("این کالا سابقهٔ فاکتور یا اصلاح موجودی دارد و برای حفظ سوابق حذف نمی‌شود.".into())}tx.execute("DELETE FROM products WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  "invoices"=>{let inv:Option<(String,i64)>=tx.query_row("SELECT kind,paid FROM invoices WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;let Some((ik,paid))=inv else{return Err("فاکتور پیدا نشد.".into())};if paid>0{return Err("فاکتور تسویه جزئی/کامل دارد؛ ابتدا گردش دریافت و پرداخت را اصلاح کنید.".into())}let mut st=tx.prepare("SELECT product_id,quantity FROM invoice_items WHERE invoice_id=?1").map_err(|e|e.to_string())?;let rows=st.query_map([id],|r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,i64>(1)?))).map_err(|e|e.to_string())?;let items=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;drop(st);for (pid,q) in items{if let Some(pid)=pid{if ik=="sale"{tx.execute("UPDATE products SET quantity=quantity+?1 WHERE id=?2",params![q,pid]).map_err(|e|e.to_string())?;}else{let stock:i64=tx.query_row("SELECT quantity FROM products WHERE id=?1",[pid],|r|r.get(0)).map_err(|e|e.to_string())?;if stock<q{return Err("حذف فاکتور خرید موجودی منفی ایجاد می‌کند؛ ابتدا فروش‌های بعدی را بررسی کنید.".into())}tx.execute("UPDATE products SET quantity=quantity-?1 WHERE id=?2",params![q,pid]).map_err(|e|e.to_string())?;}}}tx.execute("DELETE FROM invoice_items WHERE invoice_id=?1",[id]).map_err(|e|e.to_string())?;tx.execute("DELETE FROM invoices WHERE id=?1",[id]).map_err(|e|e.to_string())?;},
  _=>return Err("حذف این نوع رکورد برای جلوگیری از آسیب به گردش مالی مجاز نیست.".into())
 }
 tx.commit().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command]
fn adjust_inventory(product_id:i64,new_quantity:i64,reason:String,db:State<'_,Db>)->Result<(i64,i64,i64),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 let result=nabz_core::accounting::adjust_inventory(&mut c,product_id,new_quantity,&reason)?;
 Ok((result.previous_quantity,result.new_quantity,result.delta))
}
#[derive(Serialize)]
struct InventoryAdjustmentRecord { id:i64,product_id:i64,product_name:String,previous_quantity:i64,new_quantity:i64,delta:i64,reason:String,created_at:String }
#[tauri::command]
fn list_inventory_adjustments(product_id:Option<i64>,db:State<'_,Db>)->Result<Vec<InventoryAdjustmentRecord>,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut st=c.prepare("SELECT id,product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at FROM inventory_adjustments WHERE (?1 IS NULL OR product_id=?1) ORDER BY id DESC LIMIT 500").map_err(|e|e.to_string())?;
 let rows=st.query_map([product_id],|r|Ok(InventoryAdjustmentRecord{id:r.get(0)?,product_id:r.get(1)?,product_name:r.get(2)?,previous_quantity:r.get(3)?,new_quantity:r.get(4)?,delta:r.get(5)?,reason:r.get(6)?,created_at:r.get(7)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn save_contact(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let kind=normalize_contact_kind(&text(&payload,"kind")).ok_or("نوع طرف‌حساب را از گزینه‌های موجود انتخاب کنید")?;
 nabz_core::accounting::create_contact(&c,&nabz_core::accounting::ContactInput{
  name:text(&payload,"name"),kind:kind.into(),phone:text(&payload,"phone"),
  opening_balance:num(&payload,"balance"),notes:text(&payload,"notes")
 })
}
#[tauri::command]
fn save_product(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 save_product_with_connection(payload,&c)
}
fn save_product_with_connection(payload:Value,c:&Connection)->Result<i64,String>{
 nabz_core::accounting::create_product(c,&nabz_core::accounting::ProductInput{
  name:text(&payload,"name"),code:text(&payload,"code"),barcode:text(&payload,"barcode"),
  quantity:num(&payload,"qty"),buy_price:num(&payload,"buy"),sell_price:num(&payload,"sell"),
  min_quantity:num(&payload,"min")
 })
}
#[tauri::command]
fn save_invoice(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 save_invoice_with_connection(payload,&mut c)
}
fn save_invoice_with_connection(payload:Value,c:&mut Connection)->Result<i64,String>{
 let kind=text(&payload,"kind");
 let mut lines:Vec<nabz_core::accounting::InvoiceLineInput>=Vec::new();
 if let Some(items)=payload.get("items").and_then(Value::as_array){
  for item in items {
   let lookup=item.get("item").and_then(Value::as_str).unwrap_or("").trim().to_owned();
   if !lookup.is_empty(){
    let quantity=item.get("qty").and_then(Value::as_i64).unwrap_or(0);
    let unit_price=item.get("price").and_then(Value::as_i64).unwrap_or(0);
    lines.push(nabz_core::accounting::InvoiceLineInput{product_lookup:lookup,quantity,unit_price});
   }
  }
 }else{
  for index in 1..=5{
   let suffix=if index==1{String::new()}else{index.to_string()};
   let lookup=text(&payload,&format!("item{suffix}"));
   if !lookup.is_empty(){
    lines.push(nabz_core::accounting::InvoiceLineInput{
     product_lookup:lookup,quantity:num(&payload,&format!("qty{suffix}")),
     unit_price:num(&payload,&format!("price{suffix}"))
    });
   }
  }
 }
 let discount=num(&payload,"discount");
 let subtotal=lines.iter().try_fold(0_i64,|sum,line|{
  line.quantity.checked_mul(line.unit_price).and_then(|value|sum.checked_add(value))
 }).ok_or("مبلغ فاکتور بیش از حد بزرگ است")?;
 let total=subtotal.checked_sub(discount).ok_or("تخفیف از جمع فاکتور بیشتر است")?;
 let payment_status=if kind=="sale"{let value=text(&payload,"payment_status");if value.is_empty(){"credit".to_owned()}else{value}}else{"credit".to_owned()};
 if !["paid","partial","credit"].contains(&payment_status.as_str()){
  return Err("وضعیت پرداخت نامعتبر است".into());
 }
 let paid=if kind=="sale"{match payment_status.as_str(){"paid"=>total,"partial"=>num(&payload,"paid_amount"),_=>0}}else{0};
 if payment_status=="partial"&&(paid<=0||paid>=total){
  return Err("مبلغ دریافت‌شده در پرداخت جزئی باید بیشتر از صفر و کمتر از مبلغ فاکتور باشد".into());
 }
 nabz_core::accounting::create_invoice(c,&nabz_core::accounting::InvoiceInput{
  kind,contact_name:text(&payload,"customer"),date:text(&payload,"date"),
  discount,paid,method:text(&payload,"method"),notes:text(&payload,"notes"),lines
 })
}

#[tauri::command]
fn save_payment(payload:Value,db:State<'_,Db>)->Result<i64,String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 save_payment_with_connection(payload,&mut c)
}
fn save_payment_with_connection(payload:Value,c:&mut Connection)->Result<i64,String>{
 nabz_core::accounting::record_payment(c,&nabz_core::accounting::PaymentInput{
  kind:text(&payload,"kind"),contact_name:text(&payload,"customer"),amount:num(&payload,"amount"),
  method:text(&payload,"method"),date:text(&payload,"date"),notes:text(&payload,"notes")
 })
}

fn return_sale_items_with_connection(invoice_id:i64,payload:Value,c:&mut Connection)->Result<(i64,i64),String>{
 let raw=payload.get("items").or_else(||payload.get("lines"))
  .and_then(Value::as_array).ok_or("فهرست اقلام برگشتی معتبر نیست")?;
 let mut lines=Vec::with_capacity(raw.len());
 for (index,item) in raw.iter().enumerate(){
  let lookup=item.get("item").or_else(||item.get("product_lookup"))
   .and_then(Value::as_str).unwrap_or("").trim().to_owned();
  let quantity=item.get("qty").or_else(||item.get("quantity"))
   .and_then(Value::as_i64).or_else(||item.get("qty").and_then(Value::as_str).and_then(|s|s.parse().ok())).unwrap_or(0);
  if lookup.is_empty() || quantity<=0 {
   return Err(format!("ردیف {}: کالا و تعداد برگشتی مثبت الزامی است",index+1));
  }
  lines.push(nabz_core::accounting::ReturnLineInput{product_lookup:lookup,quantity});
 }
 nabz_core::accounting::return_sale_items(c,invoice_id,&lines,&text(&payload,"refund_method"))
}

fn return_purchase_items_with_connection(invoice_id:i64,payload:Value,c:&mut Connection)->Result<(i64,i64),String>{
 let raw=payload.get("items").or_else(||payload.get("lines")).and_then(Value::as_array).ok_or("فهرست اقلام مرجوعی معتبر نیست")?;
 let mut lines=Vec::with_capacity(raw.len());
 for (index,item) in raw.iter().enumerate(){
  let lookup=item.get("item").or_else(||item.get("product_lookup")).and_then(Value::as_str).unwrap_or("").trim().to_owned();
  let quantity=item.get("qty").or_else(||item.get("quantity")).and_then(Value::as_i64).or_else(||item.get("qty").and_then(Value::as_str).and_then(|s|s.parse().ok())).unwrap_or(0);
  if lookup.is_empty()||quantity<=0{return Err(format!("ردیف {}: کالا و تعداد مرجوعی مثبت الزامی است",index+1));}
  lines.push(nabz_core::accounting::ReturnLineInput{product_lookup:lookup,quantity});
 }
 nabz_core::accounting::return_purchase_items(c,invoice_id,&lines,&text(&payload,"refund_method"))
}

#[tauri::command]
fn return_purchase_items(invoice_id:i64,payload:Value,db:State<'_,Db>)->Result<(i64,i64),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 return_purchase_items_with_connection(invoice_id,payload,&mut c)
}

#[tauri::command]
fn return_sale_items(invoice_id:i64,payload:Value,db:State<'_,Db>)->Result<(i64,i64),String>{
 let mut c=db.0.lock().map_err(|e|e.to_string())?;
 return_sale_items_with_connection(invoice_id,payload,&mut c)
}

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
 reports_summary_with_connection(&c)
}
fn reports_summary_with_connection(c:&Connection)->Result<Value,String>{
 let q=|sql:&str|->Result<i64,String>{c.query_row(sql,[],|r|r.get(0)).map_err(|e|e.to_string())};
 Ok(json!({
  "sales_today":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale' AND invoice_date=date('now','localtime')")?,
  "receipts_today":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt' AND payment_date=date('now','localtime')")?,
  "sales_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='sale'")?,
  "purchases_total":q("SELECT COALESCE(SUM(total),0) FROM invoices WHERE kind='purchase'")?,
  "receipts_total":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='receipt'")?,
  "payments_total":q("SELECT COALESCE(SUM(amount),0) FROM payments WHERE kind='payment'")?,
  "customer_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='sale'),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind='customer'),0)")?,
  "supplier_debt":q("SELECT COALESCE((SELECT SUM(total-paid) FROM invoices WHERE kind='purchase'),0)+COALESCE((SELECT SUM(opening_balance) FROM contacts WHERE kind='supplier'),0)")?,
  "inventory_value":q("SELECT COALESCE(SUM(quantity*average_cost),0) FROM products")?,
  "contacts":q("SELECT COUNT(*) FROM contacts")?,
  "products":q("SELECT COUNT(*) FROM products")?,
  "invoices":q("SELECT COUNT(*) FROM invoices")?
 }))
}
#[tauri::command]
fn export_data(db:State<'_,Db>)->Result<Value,String>{
 let c=db.0.lock().map_err(|e|e.to_string())?;
 let mut contacts=Vec::new();{let mut s=c.prepare("SELECT id,name,kind,phone,opening_balance,notes,created_at FROM contacts").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"phone":r.get::<_,String>(3)?,"opening_balance":r.get::<_,i64>(4)?,"notes":r.get::<_,String>(5)?,"created_at":r.get::<_,String>(6)?}))).map_err(|e|e.to_string())?;for x in rows{contacts.push(x.map_err(|e|e.to_string())?);}}
 let mut products=Vec::new();{let mut s=c.prepare("SELECT id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost FROM products").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"code":r.get::<_,String>(2)?,"barcode":r.get::<_,String>(3)?,"buy_price":r.get::<_,i64>(4)?,"sell_price":r.get::<_,i64>(5)?,"quantity":r.get::<_,i64>(6)?,"min_quantity":r.get::<_,i64>(7)?,"average_cost":r.get::<_,i64>(8)?}))).map_err(|e|e.to_string())?;for x in rows{products.push(x.map_err(|e|e.to_string())?);}}
 let mut invoices=Vec::new();{let mut s=c.prepare("SELECT id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes FROM invoices").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"number":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,"contact_id":r.get::<_,Option<i64>>(3)?,"contact_name":r.get::<_,String>(4)?,"invoice_date":r.get::<_,String>(5)?,"subtotal":r.get::<_,i64>(6)?,"discount":r.get::<_,i64>(7)?,"total":r.get::<_,i64>(8)?,"paid":r.get::<_,i64>(9)?,"notes":r.get::<_,String>(10)?}))).map_err(|e|e.to_string())?;for x in rows{invoices.push(x.map_err(|e|e.to_string())?);}}
 let mut items=Vec::new();{let mut s=c.prepare("SELECT id,invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost FROM invoice_items").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"invoice_id":r.get::<_,i64>(1)?,"product_id":r.get::<_,Option<i64>>(2)?,"item_name":r.get::<_,String>(3)?,"quantity":r.get::<_,i64>(4)?,"unit_price":r.get::<_,i64>(5)?,"line_total":r.get::<_,i64>(6)?,"unit_cost":r.get::<_,Option<i64>>(7)?}))).map_err(|e|e.to_string())?;for x in rows{items.push(x.map_err(|e|e.to_string())?);}}
 let mut payments=Vec::new();{let mut s=c.prepare("SELECT id,kind,contact_id,contact_name,amount,method,payment_date,notes FROM payments").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"kind":r.get::<_,String>(1)?,"contact_id":r.get::<_,Option<i64>>(2)?,"contact_name":r.get::<_,String>(3)?,"amount":r.get::<_,i64>(4)?,"method":r.get::<_,String>(5)?,"payment_date":r.get::<_,String>(6)?,"notes":r.get::<_,String>(7)?}))).map_err(|e|e.to_string())?;for x in rows{payments.push(x.map_err(|e|e.to_string())?);}}
 let mut expenses=Vec::new();{let mut s=c.prepare("SELECT id,category,description,amount,expense_date FROM expenses").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"category":r.get::<_,String>(1)?,"description":r.get::<_,String>(2)?,"amount":r.get::<_,i64>(3)?,"date":r.get::<_,String>(4)?}))).map_err(|e|e.to_string())?;for x in rows{expenses.push(x.map_err(|e|e.to_string())?);}}
 let mut settings=Vec::new();{let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"key":r.get::<_,String>(0)?,"value":r.get::<_,String>(1)?}))).map_err(|e|e.to_string())?;for x in rows{settings.push(x.map_err(|e|e.to_string())?);}}
 let mut inventory_adjustments=Vec::new();{let mut s=c.prepare("SELECT id,product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at FROM inventory_adjustments").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"product_id":r.get::<_,i64>(1)?,"product_name":r.get::<_,String>(2)?,"previous_quantity":r.get::<_,i64>(3)?,"new_quantity":r.get::<_,i64>(4)?,"delta":r.get::<_,i64>(5)?,"reason":r.get::<_,String>(6)?,"created_at":r.get::<_,String>(7)?}))).map_err(|e|e.to_string())?;for x in rows{inventory_adjustments.push(x.map_err(|e|e.to_string())?);}}
 Ok(json!({"format":"nabz-backup-v1","contacts":contacts,"products":products,"invoices":invoices,"invoice_items":items,"payments":payments,"settings":settings,"inventory_adjustments":inventory_adjustments,"expenses":expenses}))
}
fn backup_has_int(x:&Value,key:&str)->bool{x.get(key).and_then(Value::as_i64).is_some()}
fn backup_has_string(x:&Value,key:&str)->bool{x.get(key).and_then(Value::as_str).is_some()}
fn backup_has_optional_int(x:&Value,key:&str)->bool{x.get(key).is_some_and(|v|v.is_null()||v.as_i64().is_some())}
fn validate_backup(data:&Value)->Result<(),String>{
 if data.get("format").and_then(Value::as_str)!=Some("nabz-backup-v1"){return Err("فایل پشتیبان نبض معتبر نیست".into())}
 for k in ["contacts","products","invoices","invoice_items","payments","settings"]{if !data.get(k).and_then(Value::as_array).is_some(){return Err(format!("ساختار پشتیبان ناقص است: {k}"))}}
 if data.get("inventory_adjustments").is_some_and(|v|!v.is_array()){return Err("ساختار سابقهٔ اصلاح موجودی در پشتیبان معتبر نیست".into())}
 if data.get("expenses").is_some_and(|v|!v.is_array()){return Err("ساختار هزینه‌ها در پشتیبان معتبر نیست".into())}
 let arrays=["contacts","products","invoices","invoice_items","payments","settings","inventory_adjustments","expenses"];
 for k in arrays {
  let Some(records)=data.get(k).and_then(Value::as_array) else { if k=="inventory_adjustments" || k=="expenses" { continue; } else { return Err(format!("ساختار پشتیبان ناقص است: {k}")); } };
  for (i,x) in records.iter().enumerate(){
   let bad=match k {
    "contacts"=>!(backup_has_int(x,"id")&&backup_has_string(x,"name")&&!x["name"].as_str().unwrap_or("").trim().is_empty()&&backup_has_string(x,"kind")&&backup_has_string(x,"phone")&&backup_has_int(x,"opening_balance")&&backup_has_string(x,"notes")&&backup_has_string(x,"created_at")),
    "products"=>!(backup_has_int(x,"id")&&backup_has_string(x,"name")&&!x["name"].as_str().unwrap_or("").trim().is_empty()&&backup_has_string(x,"code")&&!x["code"].as_str().unwrap_or("").trim().is_empty()&&backup_has_string(x,"barcode")&&backup_has_int(x,"buy_price")&&backup_has_int(x,"sell_price")&&backup_has_int(x,"quantity")&&backup_has_int(x,"min_quantity")&&x.get("average_cost").map_or(true,|v|v.as_i64().is_some_and(|n|n>=0))&&x["buy_price"].as_i64().unwrap_or(-1)>=0&&x["sell_price"].as_i64().unwrap_or(-1)>=0&&x["quantity"].as_i64().unwrap_or(-1)>=0&&x["min_quantity"].as_i64().unwrap_or(-1)>=0),
    "invoices"=>!(backup_has_int(x,"id")&&backup_has_int(x,"number")&&backup_has_string(x,"kind")&&["sale","purchase"].contains(&x["kind"].as_str().unwrap_or(""))&&backup_has_optional_int(x,"contact_id")&&backup_has_string(x,"contact_name")&&backup_has_string(x,"invoice_date")&&backup_has_int(x,"subtotal")&&backup_has_int(x,"discount")&&backup_has_int(x,"total")&&backup_has_int(x,"paid")&&backup_has_string(x,"notes")&&x["subtotal"].as_i64().unwrap_or(-1)>=0&&x["discount"].as_i64().unwrap_or(-1)>=0&&x["total"].as_i64().unwrap_or(-1)>=0&&x["paid"].as_i64().unwrap_or(-1)>=0&&x["paid"].as_i64().unwrap_or(i64::MAX)<=x["total"].as_i64().unwrap_or(-1)),
    "invoice_items"=>!(backup_has_int(x,"id")&&backup_has_int(x,"invoice_id")&&backup_has_optional_int(x,"product_id")&&backup_has_string(x,"item_name")&&!x["item_name"].as_str().unwrap_or("").trim().is_empty()&&backup_has_int(x,"quantity")&&x["quantity"].as_i64().unwrap_or(0)>0&&backup_has_int(x,"unit_price")&&x["unit_price"].as_i64().unwrap_or(-1)>=0&&backup_has_int(x,"line_total")&&x["line_total"].as_i64().unwrap_or(-1)>=0&&(!x.get("unit_cost").is_some_and(|v|!(v.is_null()||v.as_i64().is_some())))),
    "payments"=>!(backup_has_int(x,"id")&&backup_has_string(x,"kind")&&["receipt","payment"].contains(&x["kind"].as_str().unwrap_or(""))&&backup_has_optional_int(x,"contact_id")&&backup_has_string(x,"contact_name")&&backup_has_int(x,"amount")&&x["amount"].as_i64().unwrap_or(0)>0&&backup_has_string(x,"method")&&backup_has_string(x,"payment_date")&&backup_has_string(x,"notes")),
    "settings"=>!(backup_has_string(x,"key")&&!x["key"].as_str().unwrap_or("").is_empty()&&backup_has_string(x,"value")),
    "expenses"=>!(backup_has_int(x,"id")&&backup_has_string(x,"category")&&!x["category"].as_str().unwrap_or("").trim().is_empty()&&backup_has_string(x,"description")&&!x["description"].as_str().unwrap_or("").trim().is_empty()&&backup_has_int(x,"amount")&&x["amount"].as_i64().unwrap_or(0)>0&&backup_has_string(x,"date")),
    "inventory_adjustments"=>!(backup_has_int(x,"id")&&backup_has_int(x,"product_id")&&backup_has_string(x,"product_name")&&backup_has_int(x,"previous_quantity")&&x["previous_quantity"].as_i64().unwrap_or(-1)>=0&&backup_has_int(x,"new_quantity")&&x["new_quantity"].as_i64().unwrap_or(-1)>=0&&backup_has_int(x,"delta")&&x["delta"].as_i64().unwrap_or(0)!=0&&backup_has_string(x,"reason")&&x["reason"].as_str().unwrap_or("").trim().chars().count()>=3&&backup_has_string(x,"created_at")),
    _=>true
   };
   if bad{return Err(format!("رکورد نامعتبر در بخش {k}، ردیف {}؛ داده‌های فعلی دست‌نخورده باقی می‌مانند.",i+1))}
  }
 }
 Ok(())
}
#[tauri::command]
fn restore_data(data:Value,db:State<'_,Db>)->Result<(),String>{
 validate_backup(&data)?;
 let mut c=db.0.lock().map_err(|e|e.to_string())?;let tx=c.transaction().map_err(|e|e.to_string())?;
 tx.execute_batch("DELETE FROM expenses; DELETE FROM inventory_adjustments; DELETE FROM invoice_items; DELETE FROM payments; DELETE FROM invoices; DELETE FROM products; DELETE FROM contacts; DELETE FROM settings;").map_err(|e|e.to_string())?;
 for x in data["contacts"].as_array().unwrap(){tx.execute("INSERT INTO contacts(id,name,kind,phone,opening_balance,notes,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["kind"].as_str().unwrap_or("customer"),x["phone"].as_str().unwrap_or(""),x["opening_balance"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or(""),x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 for x in data["products"].as_array().unwrap(){
  let quantity=x["quantity"].as_i64().unwrap_or(0);
  let buy_price=x["buy_price"].as_i64().unwrap_or(0);
  let average_cost=x["average_cost"].as_i64().unwrap_or(if quantity>0{buy_price}else{0});
  tx.execute("INSERT INTO products(id,name,code,barcode,buy_price,sell_price,quantity,min_quantity,average_cost) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![x["id"].as_i64(),x["name"].as_str().unwrap_or(""),x["code"].as_str().unwrap_or(""),x["barcode"].as_str().unwrap_or(""),buy_price,x["sell_price"].as_i64().unwrap_or(0),quantity,x["min_quantity"].as_i64().unwrap_or(0),average_cost]).map_err(|e|e.to_string())?;
 }
 if let Some(rows)=data.get("inventory_adjustments").and_then(Value::as_array){for x in rows{tx.execute("INSERT INTO inventory_adjustments(id,product_id,product_name,previous_quantity,new_quantity,delta,reason,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![x["id"].as_i64(),x["product_id"].as_i64(),x["product_name"].as_str().unwrap_or(""),x["previous_quantity"].as_i64(),x["new_quantity"].as_i64(),x["delta"].as_i64(),x["reason"].as_str().unwrap_or(""),x["created_at"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}}
 for x in data["invoices"].as_array().unwrap(){tx.execute("INSERT INTO invoices(id,number,kind,contact_id,contact_name,invoice_date,subtotal,discount,total,paid,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![x["id"].as_i64(),x["number"].as_i64(),x["kind"].as_str().unwrap_or("sale"),x["contact_id"].as_i64(),x["contact_name"].as_str().unwrap_or(""),x["invoice_date"].as_str().unwrap_or(""),x["subtotal"].as_i64().unwrap_or(0),x["discount"].as_i64().unwrap_or(0),x["total"].as_i64().unwrap_or(0),x["paid"].as_i64().unwrap_or(0),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 for x in data["invoice_items"].as_array().unwrap(){tx.execute("INSERT INTO invoice_items(id,invoice_id,product_id,item_name,quantity,unit_price,line_total,unit_cost) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![x["id"].as_i64(),x["invoice_id"].as_i64(),x["product_id"].as_i64(),x["item_name"].as_str().unwrap_or(""),x["quantity"].as_i64().unwrap_or(0),x["unit_price"].as_i64().unwrap_or(0),x["line_total"].as_i64().unwrap_or(0),x["unit_cost"].as_i64()]).map_err(|e|e.to_string())?;}
 for x in data["payments"].as_array().unwrap(){tx.execute("INSERT INTO payments(id,kind,contact_id,contact_name,amount,method,payment_date,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![x["id"].as_i64(),x["kind"].as_str().unwrap_or("receipt"),x["contact_id"].as_i64(),x["contact_name"].as_str().unwrap_or(""),x["amount"].as_i64().unwrap_or(0),x["method"].as_str().unwrap_or("cash"),x["payment_date"].as_str().unwrap_or(""),x["notes"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 if let Some(rows)=data.get("expenses").and_then(Value::as_array){for x in rows{tx.execute("INSERT INTO expenses(id,category,description,amount,expense_date) VALUES(?1,?2,?3,?4,?5)",params![x["id"].as_i64(),x["category"].as_str().unwrap_or(""),x["description"].as_str().unwrap_or(""),x["amount"].as_i64().unwrap_or(0),x["date"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}}
 for x in data["settings"].as_array().unwrap(){tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",params![x["key"].as_str().unwrap_or(""),x["value"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;}
 tx.commit().map_err(|e|e.to_string())?;Ok(())
}
#[tauri::command] fn save_setting(key:String,value:String,db:State<'_,Db>)->Result<(),String>{let c=db.0.lock().map_err(|e|e.to_string())?;c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command] fn get_settings(db:State<'_,Db>)->Result<std::collections::HashMap<String,String>,String>{let c=db.0.lock().map_err(|e|e.to_string())?;let mut s=c.prepare("SELECT key,value FROM settings").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;rows.collect::<Result<_,_>>().map_err(|e|e.to_string())}
#[tauri::command]
fn create_database_backup(db: State<'_, Db>) -> Result<String, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    let path = nabz_core::create_automatic_backup(&connection)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn restore_latest_database_backup(db: State<'_, Db>) -> Result<String, String> {
    let mut connection = db.0.lock().map_err(|e| e.to_string())?;
    let path = nabz_core::restore_latest_backup(&mut connection)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn switch_to_slint(app: tauri::AppHandle) -> Result<(), String> {
    nabz_core::launch_frontend(nabz_core::Frontend::Slint)?;
    app.exit(0);
    Ok(())
}

#[cfg_attr(mobile,tauri::mobile_entry_point)] pub fn run(){tauri::Builder::default().setup(|app|{let c=nabz_core::open_database().map_err(std::io::Error::other)?;app.manage(Db(Mutex::new(c)));Ok(())}).invoke_handler(tauri::generate_handler![app_version,db_status,dashboard_summary,reports_summary,export_data,restore_data,list_records,list_products,list_inventory_adjustments,inventory_report,profit_loss_report,save_expense,adjust_inventory,delete_record,save_contact,save_product,save_invoice,save_payment,return_sale_items,return_purchase_items,save_setting,get_settings,sales_trend,switch_to_slint,create_database_backup,restore_latest_database_backup]).run(tauri::generate_context!()).expect("error while running Nabz");}


#[cfg(test)]
mod accounting_tests {
 use super::*;
 use serde_json::json;

 fn database()->Connection {
  let c=Connection::open_in_memory().expect("in-memory SQLite");
  init(&c).expect("schema");
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Ali','customer',0)",[]).unwrap();
  c.execute("INSERT INTO products(name,code,buy_price,sell_price,quantity,min_quantity) VALUES('Pen','PEN',10,20,10,2)",[]).unwrap();
  c
 }
 fn sale(customer:&str,status:&str,paid:i64,qty:i64,price:i64,discount:i64)->Value {
  json!({"kind":"sale","customer":customer,"payment_status":status,"paid_amount":paid,"method":"cash","items":[{"item":"Pen","qty":qty,"price":price}],"discount":discount,"date":"2026-10-28"})
 }
 fn scalar(c:&Connection,sql:&str)->i64 { c.query_row(sql,[],|r|r.get(0)).unwrap() }

 #[test]
 fn backup_validation_rejects_malformed_records_before_restore_can_start() {
  let missing_fields=json!({"format":"nabz-backup-v1","contacts":[],"products":[{"id":1,"name":"Bad"}],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&missing_fields).is_err());
  let empty=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&empty).is_ok());
  let bad_adjustment=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[],"settings":[],"inventory_adjustments":[{"id":1,"product_id":1,"product_name":"Pen","previous_quantity":4,"new_quantity":4,"delta":0,"reason":"انبارگردانی","created_at":"2026-10-31"}]});
  assert!(validate_backup(&bad_adjustment).is_err());
  let old_backup_without_audit=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&old_backup_without_audit).is_ok());
  let weighted_average_backup=json!({"format":"nabz-backup-v1","contacts":[],"products":[{"id":1,"name":"Pen","code":"PEN","barcode":"","buy_price":20,"sell_price":30,"quantity":4,"min_quantity":1,"average_cost":17}],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&weighted_average_backup).is_ok());
  let invalid_average_backup=json!({"format":"nabz-backup-v1","contacts":[],"products":[{"id":1,"name":"Pen","code":"PEN","barcode":"","buy_price":20,"sell_price":30,"quantity":4,"min_quantity":1,"average_cost":-1}],"invoices":[],"invoice_items":[],"payments":[],"settings":[]});
  assert!(validate_backup(&invalid_average_backup).is_err());
  let wrong_kind=json!({"format":"nabz-backup-v1","contacts":[],"products":[],"invoices":[],"invoice_items":[],"payments":[{"id":1,"kind":"cash","contact_id":null,"contact_name":"A","amount":5,"method":"cash","payment_date":"2026-10-28","notes":""}],"settings":[]});
  assert!(validate_backup(&wrong_kind).is_err());
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
 fn tauri_return_command_adapter_restores_stock_and_records_cash_refund() {
  let mut c=database();
  save_invoice_with_connection(sale("Ali","paid",0,2,50,0),&mut c).unwrap();
  let invoice_id=scalar(&c,"SELECT id FROM invoices LIMIT 1");
  let result=return_sale_items_with_connection(invoice_id,json!({
   "items":[{"item":"PEN","qty":1}],"refund_method":"cash"
  }),&mut c).unwrap();
  assert_eq!(result,(50,50));
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),9);
  assert_eq!(scalar(&c,"SELECT total FROM invoices WHERE id=1"),50);
  assert_eq!(scalar(&c,"SELECT amount FROM sales_refunds WHERE invoice_id=1"),50);
 }

 #[test]
 fn tauri_purchase_return_adapter_reduces_supplier_invoice_and_stock() {
  let mut c=database();
  c.execute("INSERT INTO contacts(name,kind,opening_balance) VALUES('Vendor','supplier',0)",[]).unwrap();
  let invoice_id=nabz_core::accounting::create_invoice(&mut c,&nabz_core::accounting::InvoiceInput{
   kind:"purchase".into(),contact_name:"Vendor".into(),date:"2026-10-28".into(),discount:0,paid:0,
   method:"cash".into(),notes:String::new(),lines:vec![nabz_core::accounting::InvoiceLineInput{product_lookup:"PEN".into(),quantity:4,unit_price:20}]
  }).unwrap();
  nabz_core::accounting::record_payment(&mut c,&nabz_core::accounting::PaymentInput{
   kind:"payment".into(),contact_name:"Vendor".into(),amount:80,method:"cash".into(),date:"2026-10-28".into(),notes:String::new()
  }).unwrap();
  let result=return_purchase_items_with_connection(invoice_id,json!({
   "items":[{"item":"PEN","qty":1}],"refund_method":"cash"
  }),&mut c).unwrap();
  assert_eq!(result,(20,20));
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),13);
  assert_eq!(scalar(&c,"SELECT total FROM invoices WHERE id=1"),60);
  assert_eq!(scalar(&c,"SELECT amount FROM purchase_refunds WHERE invoice_id=1"),20);
 }

 #[test]
 fn tauri_return_command_adapter_rejects_invalid_lines_without_mutation() {
  let mut c=database();
  save_invoice_with_connection(sale("Ali","credit",0,2,50,0),&mut c).unwrap();
  let invoice_id=scalar(&c,"SELECT id FROM invoices LIMIT 1");
  assert!(return_sale_items_with_connection(invoice_id,json!({
   "items":[{"item":"PEN","qty":3}]
  }),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT quantity FROM products WHERE code='PEN'"),8);
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM sales_returns"),0);
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
 fn receipt_is_allocated_fifo_and_overpayment_is_rejected_without_partial_write() {
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
  assert!(save_payment_with_connection(json!({"kind":"receipt","customer":"Ali","amount":31,"method":"cash"}),&mut c).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM payments"),before);
  assert_eq!(scalar(&c,"SELECT SUM(total-paid) FROM invoices WHERE kind='sale'"),30);
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
}
