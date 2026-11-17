import { useCallback, useEffect, useMemo, useState, useRef } from 'react';
import type { FormEvent, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import JsBarcode from 'jsbarcode';
import {
  Activity, ArrowDownLeft, ArrowUpRight, BarChart3, Bell, Boxes, CalendarDays,
  Check, ChevronDown, ChevronLeft, ChevronRight, CircleHelp, Command, CreditCard,
  FileText, LayoutDashboard, Menu, Package, Plus, Pencil, Eye, Printer, Search, Settings,
  ShoppingCart, SlidersHorizontal, Sparkles, Store, Users, Wallet, X, Trash2,
  Download, Upload, RefreshCw, ScanBarcode
} from 'lucide-react';

type Page = 'dashboard'|'contacts'|'invoices'|'payments'|'products'|'reports'|'printing'|'settings';
type Entry = 'contact'|'product'|'sale'|'purchase'|'receipt'|'payment';
type RecordRow = {id:number;name:string;detail:string;amount:number;status:string;date:string};
type Product = {id:number;name:string;code:string;barcode:string;quantity:number;buy_price:number;sell_price:number;min_quantity:number};
type Summary = {sales_today:number;receipts_today:number;customer_balance:number;inventory_value:number;contacts:number;products:number;invoices:number};
type Trend = {date:string;total:number};
type InvoicePrint = {invoice:{id:number;number:number;kind:string;contact_name:string;invoice_date:string;subtotal:number;discount:number;total:number;paid:number;notes:string};items:{item_name:string;quantity:number;unit_price:number;line_total:number}[];shop:Record<string,string>};
type ContactLedger = {contact:{id:number;name:string;kind:string;phone:string;opening_balance:number;customer_credit_balance:number;supplier_credit_balance:number};entries:{id:number;number?:number;entry_type:string;kind:string;date:string;amount:number;paid?:number;method?:string;notes:string}[]};

const navigation: {id:Page;label:string;icon:typeof LayoutDashboard;group:string}[] = [
  {id:'dashboard',label:'نمای کلی',icon:LayoutDashboard,group:'مدیریت'},
  {id:'contacts',label:'طرف‌حساب‌ها',icon:Users,group:'مدیریت'},
  {id:'invoices',label:'فاکتورها',icon:FileText,group:'عملیات'},
  {id:'payments',label:'دریافت و پرداخت',icon:Wallet,group:'عملیات'},
  {id:'products',label:'کالا و انبار',icon:Boxes,group:'عملیات'},
  {id:'reports',label:'گزارش‌ها',icon:BarChart3,group:'تحلیل'},
  {id:'printing',label:'چاپ و بارکد',icon:ScanBarcode,group:'ابزارها'},
  {id:'settings',label:'تنظیمات',icon:Settings,group:'ابزارها'},
];
const pageTitles:Record<Page,string> = {dashboard:'نمای کلی',contacts:'طرف‌حساب‌ها',invoices:'فاکتورها',payments:'دریافت و پرداخت',products:'کالا و انبار',reports:'گزارش‌ها',printing:'چاپ و بارکد',settings:'تنظیمات'};
const money=(n:number)=>new Intl.NumberFormat('fa-IR').format(Number.isFinite(n)?n:0);
const today=new Intl.DateTimeFormat('fa-IR-u-ca-persian',{day:'numeric',month:'long',year:'numeric'}).format(new Date());
const isoToday=new Date().toISOString().slice(0,10);
const emptySummary:Summary={sales_today:0,receipts_today:0,customer_balance:0,inventory_value:0,contacts:0,products:0,invoices:0};

function App(){
  const [page,setPage]=useState<Page>('dashboard');
  const [summary,setSummary]=useState<Summary>(emptySummary);
  const [records,setRecords]=useState<RecordRow[]>([]);
  const [products,setProducts]=useState<Product[]>([]);
  const [trend,setTrend]=useState<Trend[]>([]);
  const [days,setDays]=useState(7);
  const [query,setQuery]=useState('');
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [toast,setToast]=useState('');
  const [entry,setEntry]=useState<Entry|null>(null);
  const [editingId,setEditingId]=useState<number|null>(null);
  const [ledger,setLedger]=useState<ContactLedger|null>(null);
  const [form,setForm]=useState<Record<string,string>>({});
  const [lines,setLines]=useState([{item:'',qty:'1',price:'0'}]);
  const [settings,setSettings]=useState<Record<string,string>>({});
  const [mobileOpen,setMobileOpen]=useState(false);

  const notify=useCallback((message:string)=>{setToast(message);window.setTimeout(()=>setToast(''),3200)},[]);
  const load=useCallback(async()=>{
    setBusy(true);setError('');
    try{
      if(page==='dashboard'){
        const [s,t,inv,pay]=await Promise.all([
          invoke<Summary>('dashboard_summary'),
          invoke<Trend[]>('sales_trend',{days}),
          invoke<RecordRow[]>('list_records',{kind:'invoices'}),
          invoke<RecordRow[]>('list_records',{kind:'payments'})
        ]);
        setSummary(s);setTrend(t);
        setRecords([...inv,...pay].sort((a,b)=>b.date.localeCompare(a.date)).slice(0,5));
      }else if(page==='contacts'||page==='invoices'||page==='payments'){
        const kind=page==='contacts'?'contacts':page==='invoices'?'invoices':'payments';
        setRecords(await invoke<RecordRow[]>('list_records',{kind}));
      }else if(page==='products'){
        setProducts(await invoke<Product[]>('list_products'));
      }else if(page==='reports'){
        const r=await invoke<Summary & Record<string,number>>('reports_summary');
        setSummary({...emptySummary,...r,customer_balance:r.customer_debt??r.customer_balance??0});
      }else if(page==='settings'){
        setSettings(await invoke<Record<string,string>>('get_settings'));
      }
    }catch(e){
      setError('اتصال به پایگاه داده برقرار نشد. این بخش در نسخه دسکتاپ نبض فعال است.');
      if(String(e).includes('dashboard_summary')) setSummary(emptySummary);
    }finally{setBusy(false)}
  },[page,days]);
  useEffect(()=>{void load()},[load]);

  const filteredRecords=useMemo(()=>records.filter(r=>(r.name+' '+r.detail+' '+r.status).toLocaleLowerCase().includes(query.toLocaleLowerCase())),[records,query]);
  const filteredProducts=useMemo(()=>products.filter(p=>(p.name+' '+p.code+' '+p.barcode).toLocaleLowerCase().includes(query.toLocaleLowerCase())),[products,query]);
  const openEntry=(kind:Entry)=>{
    setEditingId(null);setEntry(kind);setQuery('');
    setForm({name:'',kind:kind==='payment'?'supplier':kind==='receipt'?'customer':'',phone:'',balance:'0',notes:'',code:'',barcode:'',qty:'0',buy:'0',sell:'0',min:'0',customer:'',amount:'',method:'cash',date:isoToday,item:'',price:'0',discount:'0',status:'paid',paid:'',paper:'A4'});
    setLines([{item:'',qty:'1',price:'0'}]);
  };
  const closeEntry=()=>{setEntry(null);setEditingId(null)};
  const openContactEdit=(r:RecordRow)=>{
    setEditingId(r.id);setEntry('contact');setQuery('');
    setForm({name:r.name,kind:r.status,phone:r.detail,balance:String(r.amount),notes:''});
  };
  const viewContactLedger=async(id:number)=>{
    try{setLedger(await invoke<ContactLedger>('get_contact_ledger',{id}))}
    catch(e){notify('دریافت گردش حساب ناموفق بود: '+String(e))}
  };

  const openProductEdit=(p:Product)=>{
    setEditingId(p.id);setEntry('product');setQuery('');
    setForm({name:p.name,code:p.code,barcode:p.barcode,qty:String(p.quantity),buy:String(p.buy_price),sell:String(p.sell_price),min:String(p.min_quantity)});
  };
  const setField=(key:string,value:string)=>setForm(old=>({...old,[key]:value}));
  const saveEntry=async(e:FormEvent)=>{
    e.preventDefault();if(!entry)return;setBusy(true);
    try{
      let id:number|void=undefined;
      if(entry==='contact'){
        id=await invoke<number>('save_contact',{payload:{...(editingId?{id:editingId}:{}),name:form.name,kind:form.kind,phone:form.phone,balance:Number(form.balance||0),...(editingId?{}:{notes:form.notes})}});
      }else if(entry==='product'){
        id=await invoke<number>('save_product',{payload:{...(editingId?{id:editingId}:{}),name:form.name,code:form.code,barcode:form.barcode,qty:Number(form.qty||0),buy:Number(form.buy||0),sell:Number(form.sell||0),min:Number(form.min||0)}});
      }else if(entry==='sale'||entry==='purchase'){
        const items=lines.filter(x=>x.item.trim()).map(x=>({item:x.item.trim(),qty:Number(x.qty),price:Number(x.price)}));
        id=await invoke<number>('save_invoice',{payload:{kind:entry==='purchase'?'purchase':'sale',customer:form.customer,payment_status:form.status,paid_amount:Number(form.paid||0),method:form.method,items,discount:Number(form.discount||0),date:form.date,notes:form.notes}});
      }else{
        id=await invoke<number>('save_payment',{payload:{kind:entry==='payment'?'payment':'receipt',customer:form.customer,amount:Number(form.amount||0),method:form.method,date:form.date,notes:form.notes}});
      }
      notify('اطلاعات با موفقیت ثبت شد'+(id?' · شناسه '+id:''));
      closeEntry();await load();
    }catch(e){notify('ثبت انجام نشد: '+String(e))}
    finally{setBusy(false)}
  };
  const removeRecord=async(kind:string,id:number)=>{
    if(!window.confirm('این رکورد حذف شود؟'))return;
    try{await invoke('delete_record',{kind,id});notify('رکورد حذف شد.');await load()}
    catch(e){notify('حذف انجام نشد: '+String(e))}
  };
  const backup=async()=>{
    try{const data=await invoke<Record<string,unknown>>('export_data');const blob=new Blob([JSON.stringify(data,null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='nabz-backup-'+new Date().toISOString().slice(0,10)+'.json';a.click();URL.revokeObjectURL(url);notify('نسخه پشتیبان دریافت شد.')}
    catch(e){notify('تهیه نسخه پشتیبان ناموفق بود: '+String(e))}
  };
  const restore=async(file?:File)=>{
    if(!file)return;if(!window.confirm('بازیابی، اطلاعات فعلی را جایگزین می‌کند. ادامه می‌دهی؟'))return;
    try{const data=JSON.parse(await file.text());await invoke('restore_data',{data});notify('بازیابی انجام شد.');await load()}
    catch(e){notify('بازیابی انجام نشد: '+String(e))}
  };
  const printInvoice=async(id:number)=>{
    try{
      const data=await invoke<InvoicePrint>('get_invoice_print',{id});
      const rows=data.items.map(x=>'<tr><td>'+x.item_name+'</td><td>'+money(x.quantity)+'</td><td>'+money(x.unit_price)+'</td><td>'+money(x.line_total)+'</td></tr>').join('');
      const w=window.open('','_blank','width=900,height=700');if(!w)throw new Error('پنجره چاپ مسدود است');
      w.document.write('<!doctype html><html lang="fa" dir="rtl"><meta charset="utf-8"><title>فاکتور '+data.invoice.number+'</title><style>body{font-family:Tahoma,sans-serif;padding:32px;color:#182536}h1{font-size:22px}table{width:100%;border-collapse:collapse;margin-top:28px}td,th{border:1px solid #ddd;padding:10px;text-align:right}footer{margin-top:25px;font-weight:bold}</style><h1>'+((data.shop.shop_name)||'نبض')+'</h1><h2>فاکتور '+(data.invoice.kind==='sale'?'فروش':'خرید')+' شماره '+money(data.invoice.number)+'</h2><p>طرف حساب: '+data.invoice.contact_name+'　 تاریخ: '+formatJalaliDate(data.invoice.invoice_date)+'</p><table><thead><tr><th>شرح</th><th>تعداد</th><th>فی</th><th>مبلغ</th></tr></thead><tbody>'+rows+'</tbody></table><footer>جمع کل: '+money(data.invoice.total)+' تومان　 تخفیف: '+money(data.invoice.discount)+'　 پرداخت‌شده: '+money(data.invoice.paid)+'</footer><script>window.onload=()=>window.print()<\/script></html>');
      w.document.close();
    }catch(e){notify('آماده‌سازی چاپ ناموفق بود: '+String(e))}
  };
  const saveSettings=async()=>{
    try{await Promise.all(Object.entries(settings).map(([key,value])=>invoke('save_setting',{key,value})));notify('تنظیمات ذخیره شد.')}
    catch(e){notify('ذخیره تنظیمات ناموفق بود: '+String(e))}
  };

  const active=pageTitles[page];
  return <div className="app-shell">
    <aside className={'sidebar '+(mobileOpen?'sidebar-open':'')}>
      <div className="brand"><div className="brand-mark">ن</div><div><b>نبض</b><small>حسابداری فروشگاهی</small></div><button className="mobile-close icon-btn" onClick={()=>setMobileOpen(false)}><X size={18}/></button></div>
      <button className="store-switch"><span className="store-icon"><Store size={18}/></span><span><b>{settings.shop_name||'فروشگاه من'}</b><small>فضای کاری فعال</small></span><ChevronDown size={15}/></button>
      {['مدیریت','عملیات','تحلیل','ابزارها'].map(group=><div className="nav-group" key={group}><div className="nav-caption">{group}</div>{navigation.filter(n=>n.group===group).map(n=><button key={n.id} className={'nav-link '+(page===n.id?'active':'')} onClick={()=>{setPage(n.id);setQuery('');setMobileOpen(false)}}><n.icon size={18}/><span>{n.label}</span>{page===n.id&&<span className="active-dot"/>}</button>)}</div>)}
      <div className="sidebar-bottom"><div className="help-card"><div className="help-icon"><Sparkles size={16}/></div><b>همراه نبض</b><p>برای شروع سریع، از میانبرهای صفحه‌کلید استفاده کن.</p><span><Command size={12}/> Ctrl + K</span></div><div className="profile"><div className="avatar">م</div><div><b>مدیر فروشگاه</b><small>دسترسی کامل</small></div><button className="icon-btn" onClick={()=>setPage('settings')}><Settings size={17}/></button></div></div>
    </aside>
    {mobileOpen&&<button className="mobile-scrim" onClick={()=>setMobileOpen(false)} aria-label="بستن منو"/>}
    <main className="main-area">
      <header className="topbar">
        <div className="topbar-right"><button className="mobile-menu icon-btn" onClick={()=>setMobileOpen(true)}><Menu size={20}/></button><div className="breadcrumbs"><span>فروشگاه</span><ChevronLeft size={14}/><b>{active}</b></div></div>
        <div className="topbar-left"><div className="date-chip"><CalendarDays size={15}/>{today}</div><button className="icon-btn notification-btn" onClick={()=>notify('اعلان تازه‌ای وجود ندارد.')}><Bell size={18}/><i/></button><div className="top-avatar">م</div></div>
      </header>
      <section className="page-content">
        {error&&<div className="error-banner">{error}<button onClick={()=>void load()}><RefreshCw size={14}/> تلاش دوباره</button></div>}
        {page==='dashboard'&&<Dashboard summary={summary} trend={trend} days={days} setDays={setDays} records={records} busy={busy} onAction={openEntry} onNavigate={setPage} onPrint={printInvoice}/>}
        {page==='contacts'&&<RecordsPage title="طرف‌حساب‌ها" subtitle="مدیریت مشتریان، تأمین‌کنندگان و مانده حساب آن‌ها" records={filteredRecords} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('contact')} onDelete={id=>removeRecord('contacts',id)} onEdit={openContactEdit} onView={viewContactLedger} columns={['نام طرف‌حساب','شماره تماس','مانده حساب','نوع','تاریخ ثبت']} />}
        {page==='invoices'&&<RecordsPage title="فاکتورها" subtitle="فاکتورهای فروش و خرید ثبت‌شده در فروشگاه" records={filteredRecords} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('sale')} onDelete={id=>removeRecord('invoices',id)} onPrint={printInvoice} columns={['شرح فاکتور','نوع','مبلغ کل','وضعیت','تاریخ']} extra={<><button className="secondary-btn" onClick={()=>openEntry('purchase')}><Plus size={15}/> خرید جدید</button></>} />}
        {page==='payments'&&<RecordsPage title="دریافت و پرداخت" subtitle="ثبت و پیگیری دریافت وجه و پرداخت به تأمین‌کنندگان" records={filteredRecords} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('receipt')} columns={['طرف‌حساب','نوع عملیات','مبلغ','روش','تاریخ']} extra={<><button className="secondary-btn" onClick={()=>openEntry('payment')}><ArrowUpRight size={15}/> ثبت پرداخت</button></>} />}
        {page==='products'&&<ProductsPage products={filteredProducts} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('product')} onEdit={openProductEdit} onDelete={id=>removeRecord('products',id)}/>}
        {page==='reports'&&<ReportsPage summary={summary}/>}
        {page==='printing'&&<PrintingPage notify={notify}/>}
        {page==='settings'&&<SettingsPage settings={settings} setSettings={setSettings} onSave={saveSettings} onBackup={backup} onRestore={restore}/>}
        <footer className="page-footer"><span>نبض · مدیریت مالی هوشمند فروشگاه</span><span>نسخه آزمایشی رابط React</span></footer>
      </section>
    </main>
    {entry&&<EntryModal entry={entry} form={form} setField={setField} lines={lines} setLines={setLines} onClose={closeEntry} onSubmit={saveEntry} busy={busy} editing={editingId!==null}/>}
    {ledger&&<ContactLedgerModal ledger={ledger} onClose={()=>setLedger(null)}/>}
    {toast&&<div className="toast"><Check size={16}/>{toast}</div>}
  </div>;
}


function latinDigits(value:string){return value.replace(/[۰-۹]/g,d=>String('۰۱۲۳۴۵۶۷۸۹'.indexOf(d))).replace(/[٠-٩]/g,d=>String('٠١٢٣٤٥٦٧٨٩'.indexOf(d)));}
function jalaliParts(iso:string){
 if(!iso)return {year:1405,month:1,day:1};
 const d=new Date(iso+'T12:00:00');
 const p=new Intl.DateTimeFormat('en-u-ca-persian',{year:'numeric',month:'numeric',day:'numeric'}).formatToParts(d);
 return {year:Number(p.find(x=>x.type==='year')?.value),month:Number(p.find(x=>x.type==='month')?.value),day:Number(p.find(x=>x.type==='day')?.value)};
}
function formatJalaliDate(iso:string){
 const day=iso?.slice(0,10)||'';
 if(!/^\d{4}-\d{2}-\d{2}$/.test(day))return iso||'—';
 const p=gregorianToJalali(day).split('/');
 return p.map((v,i)=>new Intl.NumberFormat('fa-IR',{useGrouping:false,minimumIntegerDigits:i===0?4:2}).format(Number(v))).join('/');
}
function gregorianToJalali(iso:string){
 if(!iso)return '';
 const p=jalaliParts(iso);
 return p.year+'/'+String(p.month).padStart(2,'0')+'/'+String(p.day).padStart(2,'0');
}
function jalaliToGregorian(raw:string):string|null{
 const m=latinDigits(raw).trim().match(/^(\d{4})[\/-](\d{1,2})[\/-](\d{1,2})$/);
 if(!m)return null;
 const y=Number(m[1]),mo=Number(m[2]),day=Number(m[3]);
 if(mo<1||mo>12||day<1||day>31)return null;
 const start=Date.UTC(y+621,0,1);
 for(let i=0;i<460;i++){
  const d=new Date(start+i*86400000);
  const p=jalaliParts(d.toISOString().slice(0,10));
  if(p.year===y&&p.month===mo&&p.day===day)return d.toISOString().slice(0,10);
 }
 return null;
}
const jalaliMonths=['فروردین','اردیبهشت','خرداد','تیر','مرداد','شهریور','مهر','آبان','آذر','دی','بهمن','اسفند'];
const jalaliWeekdays=['ش','ی','د','س','چ','پ','ج'];
function JalaliDateField({value,onChange,label}:{value:string;onChange:(v:string)=>void;label:string}){
 const selected=jalaliParts(value);
 const [text,setText]=useState(gregorianToJalali(value));
 const [invalid,setInvalid]=useState(false);
 const [open,setOpen]=useState(false);
 const [view,setView]=useState({year:selected.year,month:selected.month});
 useEffect(()=>{setText(gregorianToJalali(value));setInvalid(false);setView({year:selected.year,month:selected.month})},[value]);
 const days=view.month<=6?31:view.month<=11?30:(jalaliToGregorian(String(view.year)+'/12/30')?30:29);
 const first=jalaliToGregorian(String(view.year)+'/'+String(view.month).padStart(2,'0')+'/01');
 const offset=first?(new Date(first+'T12:00:00').getDay()+1)%7:0;
 const cells=[...Array(offset).fill(0),...Array.from({length:days},(_,i)=>i+1)];
 while(cells.length%7)cells.push(0);
 const move=(delta:number)=>setView(v=>{let month=v.month+delta,year=v.year;if(month<1){month=12;year--}if(month>12){month=1;year++}return {year,month}});
 const choose=(day:number)=>{const iso=jalaliToGregorian(String(view.year)+'/'+String(view.month).padStart(2,'0')+'/'+String(day).padStart(2,'0'));if(iso){onChange(iso);setOpen(false)}};
 return <div className="field-label jalali-date-field"><span>{label}</span><div className="jalali-input-row"><input type="text" inputMode="numeric" dir="ltr" value={text} placeholder="۱۴۰۵/۰۸/۲۶" aria-invalid={invalid} className={invalid?'date-invalid':''} onChange={e=>{
  const next=e.target.value;setText(next);
  const converted=jalaliToGregorian(next);
  if(converted){onChange(converted);setInvalid(false)}else setInvalid(next.length>=10);
 }} onBlur={()=>{if(!jalaliToGregorian(text)){setText(gregorianToJalali(value));setInvalid(false)}}}/><button type="button" className="calendar-trigger" aria-label="باز کردن تقویم شمسی" onClick={()=>setOpen(v=>!v)}><CalendarDays size={16}/></button></div><small className="date-hint">{invalid?'تاریخ شمسی معتبر وارد کن':'سال/ماه/روز شمسی'}</small>{open&&<div className="jalali-calendar"><div className="calendar-head"><button type="button" onClick={()=>move(-1)}><ChevronRight size={17}/></button><b>{jalaliMonths[view.month-1]} {money(view.year)}</b><button type="button" onClick={()=>move(1)}><ChevronLeft size={17}/></button></div><div className="calendar-grid">{jalaliWeekdays.map(d=><span className="calendar-weekday" key={d}>{d}</span>)}{cells.map((day,i)=>day?<button type="button" key={i} className={selected.year===view.year&&selected.month===view.month&&selected.day===day?'selected':''} onClick={()=>choose(day)}>{money(day)}</button>:<span key={i}/>)}</div><button type="button" className="calendar-today" onClick={()=>{onChange(isoToday);setOpen(false)}}>امروز</button></div>}</div>;
}

function PageHeading({eyebrow,title,subtitle,children}:{eyebrow?:string;title:string;subtitle:string;children?:ReactNode}){
 return <div className="page-heading"><div><div className="eyebrow">{eyebrow||'مدیریت فروشگاه'}</div><h1>{title}</h1><p>{subtitle}</p></div><div className="heading-actions">{children}</div></div>;
}
function PrimaryButton({children,onClick}:{children:ReactNode;onClick:()=>void}){
 return <button className="primary-btn" onClick={onClick}><Plus size={17}/>{children}</button>;
}
function Dashboard({summary,trend,days,setDays,records,busy,onAction,onNavigate,onPrint}:{summary:Summary;trend:Trend[];days:number;setDays:(n:number)=>void;records:RecordRow[];busy:boolean;onAction:(e:Entry)=>void;onNavigate:(p:Page)=>void;onPrint:(id:number)=>void}){
 const max=Math.max(1,...trend.map(x=>x.total));
 const coords=trend.map((x,i)=>({x:trend.length<2?50:i*100/(trend.length-1),y:92-(x.total/max)*76}));
 const path=coords.map((p,i)=>(i?'L':'M')+p.x+' '+p.y).join(' ');
 const cards=[{label:'فروش امروز',value:summary.sales_today,icon:ShoppingCart,tone:'blue',hint:'مجموع فاکتورهای فروش'},{label:'دریافت‌های امروز',value:summary.receipts_today,icon:ArrowDownLeft,tone:'green',hint:'وجوه دریافت‌شده'},{label:'مانده حساب مشتریان',value:summary.customer_balance,icon:Wallet,tone:'amber',hint:'مانده بدهی مشتریان'},{label:'ارزش موجودی انبار',value:summary.inventory_value,icon:Package,tone:'violet',hint:'بر اساس قیمت خرید'}];
 return <><PageHeading title="خوش آمدی 👋" subtitle="نمایی روشن از وضعیت مالی فروشگاهت در یک نگاه."><span className="today-label"><CalendarDays size={15}/>{today}</span><PrimaryButton onClick={()=>onAction('sale')}>فاکتور فروش</PrimaryButton></PageHeading>
  <div className="stat-grid">{cards.map(c=><article className="stat-card" key={c.label}><div className="stat-top"><span>{c.label}</span><i className={'stat-icon '+c.tone}><c.icon size={19}/></i></div><div className="stat-value">{money(c.value)}<small>تومان</small></div><div className="stat-foot"><span>{c.hint}</span><Activity size={14}/></div></article>)}</div>
  <div className="dashboard-grid"><article className="surface chart-card"><div className="surface-heading"><div><h2>روند فروش</h2><p>مقایسه فروش در روزهای اخیر</p></div><select value={days} onChange={e=>setDays(Number(e.target.value))}><option value={7}>۷ روز اخیر</option><option value={30}>۳۰ روز اخیر</option></select></div><div className="chart-legend"><span><i/> فروش روزانه</span><b>{money(trend.reduce((a,b)=>a+b.total,0))} <small>تومان</small></b></div><div className="sales-chart"><div className="chart-y"><span>{money(max)}</span><span>{money(Math.round(max*.66))}</span><span>{money(Math.round(max*.33))}</span><span>۰</span></div><div className="chart-main"><div className="chart-grid"><i/><i/><i/><i/></div><svg viewBox="0 0 100 100" preserveAspectRatio="none"><defs><linearGradient id="salesFill" x1="0" x2="0" y1="0" y2="1"><stop offset="0%" stopColor="#16a36b" stopOpacity=".20"/><stop offset="100%" stopColor="#16a36b" stopOpacity="0"/></linearGradient></defs><path d={path+' L100 100 L0 100 Z'} fill="url(#salesFill)"/><path d={path||'M0 92 L100 92'} fill="none" stroke="#159b67" strokeWidth="1.8" vectorEffect="non-scaling-stroke"/>{coords.map((p,i)=><circle key={i} cx={p.x} cy={p.y} r="1.4" fill="#fff" stroke="#159b67" strokeWidth=".8" vectorEffect="non-scaling-stroke"/>)}</svg><div className="chart-x">{trend.filter((_,i)=>i===0||i===trend.length-1||i%Math.max(1,Math.floor(trend.length/5))===0).map(p=><span key={p.date}>{new Date(p.date+'T12:00:00').toLocaleDateString('fa-IR-u-ca-persian',{day:'numeric',month:'short'})}</span>)}</div></div></div>{busy&&<div className="loading-line"/>}</article>
   <article className="surface quick-card"><div className="surface-heading"><div><h2>دسترسی سریع</h2><p>کارهای روزمره فروشگاه</p></div><Sparkles size={18} className="muted-icon"/></div><div className="quick-list"><button onClick={()=>onAction('sale')}><i className="quick-icon green"><ShoppingCart size={18}/></i><span><b>فاکتور فروش</b><small>ثبت فروش جدید</small></span><ChevronLeft size={17}/></button><button onClick={()=>onAction('receipt')}><i className="quick-icon blue"><ArrowDownLeft size={18}/></i><span><b>ثبت دریافت</b><small>دریافت وجه از مشتری</small></span><ChevronLeft size={17}/></button><button onClick={()=>onAction('contact')}><i className="quick-icon violet"><Users size={18}/></i><span><b>طرف‌حساب جدید</b><small>افزودن مشتری یا تأمین‌کننده</small></span><ChevronLeft size={17}/></button><button onClick={()=>onAction('product')}><i className="quick-icon amber"><Package size={18}/></i><span><b>کالای جدید</b><small>ثبت کالا در انبار</small></span><ChevronLeft size={17}/></button></div></article></div>
  <article className="surface recent-card"><div className="surface-heading"><div><h2>آخرین رویدادها</h2><p>آخرین فاکتورها و دریافت / پرداخت‌ها</p></div><button className="text-btn" onClick={()=>onNavigate('invoices')}>همه فاکتورها <ChevronLeft size={15}/></button></div><RecordsTable records={records} kind="recent" onPrint={onPrint}/></article>
 </>;
}
function Pagination({page,pages,onChange}:{page:number;pages:number;onChange:(p:number)=>void}){
 if(pages<=1)return null;
 return <div className="pagination"><span>صفحه {money(page)} از {money(pages)}</span><div><button className="secondary-btn" disabled={page<=1} onClick={()=>onChange(page-1)}>قبلی</button><button className="secondary-btn" disabled={page>=pages} onClick={()=>onChange(page+1)}>بعدی</button></div></div>;
}
function RecordsPage({title,subtitle,records,query,setQuery,busy,onAdd,onDelete,onPrint,onEdit,onView,columns,extra}:{title:string;subtitle:string;records:RecordRow[];query:string;setQuery:(s:string)=>void;busy:boolean;onAdd:()=>void;onDelete?:(id:number)=>void;onPrint?:(id:number)=>void;onEdit?:(r:RecordRow)=>void;onView?:(id:number)=>void;columns:string[];extra?:ReactNode}){
 const [page,setPage]=useState(1);const pageSize=10;const pages=Math.max(1,Math.ceil(records.length/pageSize));
 useEffect(()=>setPage(1),[query,records.length]);
 const visible=records.slice((page-1)*pageSize,page*pageSize);
 return <><PageHeading title={title} subtitle={subtitle}><div className="button-row">{extra}<PrimaryButton onClick={onAdd}>{title==='طرف‌حساب‌ها'?'طرف‌حساب جدید':title==='فاکتورها'?'فاکتور فروش':'ثبت دریافت'}</PrimaryButton></div></PageHeading><div className="surface list-surface"><div className="list-toolbar"><div className="search-box"><Search size={17}/><input value={query} onChange={e=>setQuery(e.target.value)} placeholder="جست‌وجو در فهرست..."/></div><div className="toolbar-meta"><span>{money(records.length)} مورد</span></div></div><RecordsTable records={visible} kind={title==='طرف‌حساب‌ها'?'contacts':title==='فاکتورها'?'invoices':'payments'} columns={columns} onDelete={onDelete} onPrint={onPrint} onEdit={onEdit} onView={onView}/><Pagination page={page} pages={pages} onChange={setPage}/>{busy&&<div className="table-loading">در حال دریافت اطلاعات...</div>}</div></>;
}
function RecordsTable({records,kind,columns,onDelete,onPrint,onEdit,onView}:{records:RecordRow[];kind:string;columns?:string[];onDelete?:(id:number)=>void;onPrint?:(id:number)=>void;onEdit?:(r:RecordRow)=>void;onView?:(id:number)=>void}){
 const heads=columns||['شرح تراکنش','جزئیات','مبلغ','وضعیت','تاریخ'];
 const contactKind=(v:string)=>v==='customer'?'مشتری':v==='supplier'?'تأمین‌کننده':v==='both'?'مشتری و تأمین‌کننده':v;
 const paymentMethod=(v:string)=>v==='cash'?'نقدی':v==='card'?'کارت':v==='transfer'?'انتقال بانکی':v;
 return <div className="table-wrap"><table className="data-table"><thead><tr>{heads.map(h=><th key={h}>{h}</th>)}<th>عملیات</th></tr></thead><tbody>{records.map(r=>{
  const second=kind==='contacts'?(r.detail||'—'):kind==='invoices'?(r.detail==='sale'?'فروش':r.detail==='purchase'?'خرید':r.detail):kind==='payments'?(r.detail==='receipt'?'دریافت وجه':r.detail==='payment'?'پرداخت وجه':r.detail):r.detail;
  const status=kind==='contacts'?contactKind(r.status):kind==='payments'?paymentMethod(r.status):r.status||'ثبت‌شده';
  const statusClass=kind==='contacts'?'info':kind==='payments'?'success':(/بدهکار|کم‌موجود|در انتظار/.test(status)?'warning':/بستانکار/.test(status)?'info':'success');
  return <tr key={r.id}><td><b>{r.name}</b>{kind==='recent'&&<small className="sub-cell">{second}</small>}</td><td>{kind==='recent'?'—':second}</td><td className="amount-cell">{money(r.amount)} <small>تومان</small></td><td><span className={'status '+statusClass}>{status}</span></td><td>{formatJalaliDate(r.date)}</td><td><div className="row-actions">{kind==='invoices'&&onPrint&&<button title="چاپ فاکتور" onClick={()=>onPrint(r.id)}><Printer size={16}/></button>}{kind==='contacts'&&onView&&<button title="گردش حساب" onClick={()=>onView(r.id)}><Eye size={16}/></button>}{kind==='contacts'&&onEdit&&<button title="ویرایش طرف‌حساب" onClick={()=>onEdit(r)}><Pencil size={16}/></button>}{onDelete&&<button title="حذف" onClick={()=>onDelete(r.id)}><Trash2 size={16}/></button>}</div></td></tr>;
 })}{!records.length&&<tr><td colSpan={6} className="empty-cell">هنوز اطلاعاتی ثبت نشده است.</td></tr>}</tbody></table></div>;
}
function ProductsPage({products,query,setQuery,busy,onAdd,onEdit,onDelete}:{products:Product[];query:string;setQuery:(s:string)=>void;busy:boolean;onAdd:()=>void;onEdit:(p:Product)=>void;onDelete:(id:number)=>void}){
 const [stockFilter,setStockFilter]=useState('all');const [page,setPage]=useState(1);const pageSize=10;
 const filtered=products.filter(p=>stockFilter==='low'?p.quantity<=p.min_quantity:stockFilter==='available'?p.quantity>0:true);
 const pages=Math.max(1,Math.ceil(filtered.length/pageSize));const visible=filtered.slice((page-1)*pageSize,page*pageSize);
 useEffect(()=>setPage(1),[query,stockFilter,products.length]);
 return <><PageHeading title="کالا و انبار" subtitle="کنترل موجودی، بارکد و قیمت خرید و فروش"><PrimaryButton onClick={onAdd}>افزودن کالا</PrimaryButton></PageHeading><div className="mini-stat-grid"><MiniStat label="تعداد اقلام" value={products.length} icon={Boxes}/><MiniStat label="مجموع موجودی" value={products.reduce((n,p)=>n+p.quantity,0)} icon={Package}/><MiniStat label="نیازمند تأمین" value={products.filter(p=>p.quantity<=p.min_quantity).length} icon={Activity}/></div><div className="surface list-surface"><div className="list-toolbar"><div className="search-box"><Search size={17}/><input value={query} onChange={e=>setQuery(e.target.value)} placeholder="جست‌وجوی نام، کد یا بارکد..."/></div><div className="toolbar-meta"><span>{money(filtered.length)} کالا</span><select className="stock-select" value={stockFilter} onChange={e=>setStockFilter(e.target.value)}><option value="all">همه کالاها</option><option value="low">موجودی کم</option><option value="available">موجود</option></select></div></div><div className="table-wrap"><table className="data-table"><thead><tr>{['نام کالا','کد کالا','بارکد','موجودی','قیمت خرید','قیمت فروش','وضعیت','عملیات'].map(x=><th key={x}>{x}</th>)}</tr></thead><tbody>{visible.map(p=><tr key={p.id}><td><b>{p.name}</b></td><td>{p.code}</td><td className="mono">{p.barcode||'—'}</td><td>{money(p.quantity)}</td><td>{money(p.buy_price)}</td><td className="amount-cell">{money(p.sell_price)}</td><td><span className={'status '+(p.quantity<=p.min_quantity?'warning':'success')}>{p.quantity<=p.min_quantity?'کم‌موجود':'موجود'}</span></td><td><div className="row-actions"><button title="ویرایش کالا" onClick={()=>onEdit(p)}><Pencil size={16}/></button><button title="حذف کالا" onClick={()=>onDelete(p.id)}><Trash2 size={16}/></button></div></td></tr>)}{!visible.length&&<tr><td colSpan={8} className="empty-cell">{busy?'در حال دریافت...':'کالایی با این فیلتر پیدا نشد.'}</td></tr>}</tbody></table></div><Pagination page={page} pages={pages} onChange={setPage}/></div></>;
}
function MiniStat({label,value,icon:Icon}:{label:string;value:number;icon:typeof Boxes}){return <div className="mini-stat"><i><Icon size={18}/></i><span>{label}</span><b>{money(value)}</b></div>}
function ReportsPage({summary}:{summary:Summary}){
 const [start,setStart]=useState(()=>new Date(Date.now()-29*86400000).toISOString().slice(0,10));
 const [end,setEnd]=useState(isoToday);
 const [report,setReport]=useState<Record<string,number|string>>({});
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState('');
 useEffect(()=>{
  let active=true;setBusy(true);setError('');
  invoke<Record<string,number|string>>('reports_period',{startDate:start,endDate:end})
   .then(v=>{if(active)setReport(v)})
   .catch(e=>{if(active)setError(String(e))})
   .finally(()=>{if(active)setBusy(false)});
  return ()=>{active=false};
 },[start,end]);
 const tiles=[
  {label:'فروش در بازه',value:Number(report.sales_total||0)},
  {label:'خرید در بازه',value:Number(report.purchases_total||0)},
  {label:'دریافت در بازه',value:Number(report.receipts_total||0)},
  {label:'پرداخت در بازه',value:Number(report.payments_total||0)},
  {label:'بدهی فعلی مشتریان',value:summary.customer_balance},
  {label:'ارزش فعلی موجودی',value:summary.inventory_value}
 ];
 return <><PageHeading title="گزارش‌ها" subtitle="گزارش مالی با بازه شمسی و محاسبه از پایگاه داده محلی"/><div className="surface report-filter"><JalaliDateField label="از تاریخ" value={start} onChange={setStart}/><JalaliDateField label="تا تاریخ" value={end} onChange={setEnd}/><div className="report-range-info">{busy?'در حال محاسبه گزارش…':error?'خطا در دریافت گزارش':money(Number(report.sales_count||0))+' فاکتور فروش · '+money(Number(report.purchases_count||0))+' فاکتور خرید'}</div></div>{error&&<div className="error-banner">{error}</div>}<div className="report-grid">{tiles.map(x=><div className="surface report-tile" key={x.label}><span>{x.label}</span><b>{money(x.value)} <small>تومان</small></b></div>)}</div><div className="surface report-note"><BarChart3 size={22}/><div><b>بازه گزارش</b><p>جمع فروش، خرید، دریافت و پرداخت فقط از تاریخ شروع تا تاریخ پایان انتخاب‌شده محاسبه می‌شود. مانده مشتریان و ارزش موجودی، وضعیت فعلی هستند و به بازه تاریخی محدود نمی‌شوند.</p></div></div></>;
}
function PrintingPage({notify}:{notify:(s:string)=>void}){
 const [barcode,setBarcode]=useState('626000001');
 const [barcodeError,setBarcodeError]=useState('');
 const barcodeRef=useRef<SVGSVGElement>(null);
 useEffect(()=>{
  if(!barcodeRef.current)return;
  try{JsBarcode(barcodeRef.current,barcode,{format:'CODE128',displayValue:true,lineColor:'#1d2c24',width:2,height:76,margin:10,font:'Tahoma',fontSize:13});setBarcodeError('')}
  catch{setBarcodeError('کد بارکد خالی یا نامعتبر است.')}
 },[barcode]);
 return <><PageHeading title="چاپ و بارکد" subtitle="تولید بارکد استاندارد و چاپ برچسب کالا"/><div className="surface print-panel"><div><h2>بارکد کالا</h2><p>می‌توانی کد را تایپ کنی یا با اسکنر USB/HID در همین کادر اسکن کنی.</p><label className="field-label">کد یا بارکد<input value={barcode} onChange={e=>setBarcode(e.target.value)} dir="ltr" autoFocus/></label>{barcodeError&&<p className="barcode-error">{barcodeError}</p>}<button className="primary-btn" onClick={()=>{if(!barcodeError){window.print();notify('پنجره چاپ برچسب باز شد.')}}}><Printer size={16}/> چاپ برچسب</button></div><div className="barcode-preview"><svg ref={barcodeRef} role="img" aria-label="پیش‌نمایش بارکد تولیدشده"/></div></div></>;
}
function SettingsPage({settings,setSettings,onSave,onBackup,onRestore}:{settings:Record<string,string>;setSettings:(v:Record<string,string>)=>void;onSave:()=>void;onBackup:()=>void;onRestore:(f?:File)=>void}){
 const update=(k:string,v:string)=>setSettings({...settings,[k]:v});
 return <><PageHeading title="تنظیمات" subtitle="اطلاعات فروشگاه، تنظیمات عمومی و حفاظت از داده‌ها"/><div className="settings-grid"><section className="surface settings-card"><h2><Store size={18}/> اطلاعات فروشگاه</h2><label className="field-label">نام فروشگاه<input value={settings.shop_name||''} onChange={e=>update('shop_name',e.target.value)} placeholder="نام فروشگاه"/></label><label className="field-label">شماره تماس<input value={settings.shop_phone||''} onChange={e=>update('shop_phone',e.target.value)} placeholder="شماره تماس"/></label><label className="field-label">نشانی<input value={settings.shop_address||''} onChange={e=>update('shop_address',e.target.value)} placeholder="نشانی فروشگاه"/></label><label className="field-label">شناسه مالیاتی<input value={settings.shop_tax||''} onChange={e=>update('shop_tax',e.target.value)} placeholder="اختیاری"/></label><button className="primary-btn" onClick={onSave}>ذخیره تنظیمات</button></section><section className="surface settings-card"><h2><ShieldIcon/> پشتیبان‌گیری و بازیابی</h2><p>برای جلوگیری از ازدست‌رفتن اطلاعات، به‌صورت منظم نسخه پشتیبان تهیه کن.</p><button className="secondary-btn wide" onClick={onBackup}><Download size={16}/> دریافت نسخه پشتیبان</button><label className="secondary-btn wide file-label"><Upload size={16}/> بازیابی از فایل پشتیبان<input type="file" accept=".json,application/json" onChange={e=>onRestore(e.target.files?.[0])}/></label></section></div></>;
}
function ShieldIcon(){return <CreditCard size={18}/>}

function ContactLedgerModal({ledger,onClose}:{ledger:ContactLedger;onClose:()=>void}){
 const contact=ledger.contact;
 const kind=contact.kind==='customer'?'مشتری':contact.kind==='supplier'?'تأمین‌کننده':'مشتری و تأمین‌کننده';
 return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)onClose()}}><section className="entry-modal ledger-modal"><div className="modal-heading"><div><span className="eyebrow">گردش مالی طرف‌حساب</span><h2>{contact.name}</h2><small className="ledger-contact-meta">{kind} · {contact.phone||'بدون شماره تماس'}</small></div><button type="button" className="icon-btn" onClick={onClose}><X size={20}/></button></div><div className="ledger-summary"><div><span>مانده افتتاحیه</span><b>{money(contact.opening_balance)} تومان</b></div><div><span>اعتبار مشتری</span><b>{money(contact.customer_credit_balance)} تومان</b></div><div><span>اعتبار تأمین‌کننده</span><b>{money(contact.supplier_credit_balance)} تومان</b></div></div><div className="surface ledger-list"><div className="surface-heading"><div><h2>اسناد و تراکنش‌ها</h2><p>{money(ledger.entries.length)} رویداد ثبت‌شده</p></div></div><div className="table-wrap"><table className="data-table"><thead><tr><th>تاریخ</th><th>شرح</th><th>مبلغ</th><th>تسویه / روش</th><th>یادداشت</th></tr></thead><tbody>{ledger.entries.map((e,i)=>{
  const label=e.entry_type==='invoice'?(e.kind==='sale'?'فاکتور فروش':'فاکتور خرید'):(e.kind==='receipt'?'دریافت وجه':'پرداخت وجه');
  const detail=e.entry_type==='invoice'?'شماره '+money(e.number||0):e.method==='cash'?'نقدی':e.method==='card'?'کارت':e.method==='transfer'?'انتقال بانکی':e.method||'—';
  return <tr key={e.entry_type+'-'+e.id+'-'+i}><td>{formatJalaliDate(e.date)}</td><td><b>{label}</b><small className="sub-cell">{detail}</small></td><td className="amount-cell">{money(e.amount)} <small>تومان</small></td><td>{e.entry_type==='invoice'?money(e.paid||0)+' تومان پرداخت':'—'}</td><td>{e.notes||'—'}</td></tr>;
 })}{!ledger.entries.length&&<tr><td colSpan={5} className="empty-cell">هنوز سند یا تراکنشی برای این طرف‌حساب ثبت نشده است.</td></tr>}</tbody></table></div></div><div className="modal-actions"><button className="secondary-btn" onClick={onClose}>بستن</button></div></section></div>;
}

function EntryModal({entry,form,setField,lines,setLines,onClose,onSubmit,busy,editing}:{entry:Entry;form:Record<string,string>;setField:(k:string,v:string)=>void;lines:{item:string;qty:string;price:string}[];setLines:(v:{item:string;qty:string;price:string}[])=>void;onClose:()=>void;onSubmit:(e:FormEvent)=>void;busy:boolean;editing:boolean}){
 const titles:Record<Entry,string>={contact:editing?'ویرایش طرف‌حساب':'ثبت طرف‌حساب جدید',product:editing?'ویرایش کالا':'ثبت کالای جدید',sale:'فاکتور فروش جدید',purchase:'فاکتور خرید جدید',receipt:'ثبت دریافت وجه',payment:'ثبت پرداخت وجه'};
 const field=(key:string,label:string,placeholder='',type='text')=><label className="field-label">{label}<input type={type} value={form[key]||''} onChange={e=>setField(key,e.target.value)} placeholder={placeholder}/></label>;
 return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)onClose()}}><form className="entry-modal" onSubmit={onSubmit}><div className="modal-heading"><div><span className="eyebrow">فرم ثبت اطلاعات</span><h2>{titles[entry]}</h2></div><button type="button" className="icon-btn" onClick={onClose}><X size={20}/></button></div>
  <div className="modal-fields">
   {entry==='contact'&&<>{field('name','نام طرف‌حساب','نام و نام خانوادگی یا شرکت')}<div className="form-two">{<label className="field-label">نوع طرف‌حساب<select value={form.kind} onChange={e=>setField('kind',e.target.value)}><option value="customer">مشتری</option><option value="supplier">تأمین‌کننده</option><option value="both">هر دو</option></select></label>}{field('phone','شماره تماس','اختیاری')}</div>{field('balance','مانده افتتاحیه','۰','number')}{!editing&&field('notes','یادداشت','توضیحات تکمیلی')}</>}
   {entry==='product'&&<>{field('name','نام کالا','مثلاً کابل افشان')}<div className="form-two">{field('code','کد کالا','SKU-001')}{field('barcode','بارکد','اختیاری')}</div><div className="form-three">{field('qty','موجودی اولیه','۰','number')}{field('buy','قیمت خرید','۰','number')}{field('sell','قیمت فروش','۰','number')}</div>{field('min','حداقل موجودی','۰','number')}{editing&&<p className="edit-stock-note">برای حفظ سوابق، موجودی کالایی که در فاکتور ثبت شده از این فرم تغییر نمی‌کند؛ اصلاح موجودی را با فاکتور خرید یا فروش انجام بده.</p>}</>}
   {(entry==='sale'||entry==='purchase')&&<>{field('customer',entry==='sale'?'نام مشتری':'نام تأمین‌کننده','برای فروش نقدی گذری خالی بگذار')}<div className="invoice-lines-title"><b>اقلام فاکتور</b><button type="button" className="text-btn" onClick={()=>setLines([...lines,{item:'',qty:'1',price:'0'}])}><Plus size={15}/> افزودن ردیف</button></div>{lines.map((line,i)=><div className="invoice-line" key={i}><input aria-label="نام کالا یا خدمت" placeholder="نام کالا یا خدمت" value={line.item} onChange={e=>setLines(lines.map((x,j)=>j===i?{...x,item:e.target.value}:x))}/><input aria-label="تعداد" type="number" min="1" value={line.qty} onChange={e=>setLines(lines.map((x,j)=>j===i?{...x,qty:e.target.value}:x))}/><input aria-label="قیمت واحد" type="number" min="0" value={line.price} onChange={e=>setLines(lines.map((x,j)=>j===i?{...x,price:e.target.value}:x))}/><button type="button" className="icon-btn" onClick={()=>setLines(lines.filter((_,j)=>j!==i))}><X size={16}/></button></div>)}<div className="form-two">{field('discount','تخفیف','۰','number')}<JalaliDateField label="تاریخ فاکتور" value={form.date||isoToday} onChange={v=>setField('date',v)}/></div>{entry==='sale'&&<div className="form-two"><label className="field-label">وضعیت پرداخت<select value={form.status} onChange={e=>setField('status',e.target.value)}><option value="paid">تسویه کامل</option><option value="partial">پرداخت جزئی</option><option value="credit">نسیه</option></select></label>{form.status==='partial'&&field('paid','مبلغ دریافتی','۰','number')}</div>}{field('notes','یادداشت','اختیاری')}</>}
   {(entry==='receipt'||entry==='payment')&&<>{field('customer','نام طرف‌حساب','ابتدا طرف‌حساب را ثبت کن')}{field('amount','مبلغ','۰','number')}<div className="form-two"><label className="field-label">روش پرداخت<select value={form.method} onChange={e=>setField('method',e.target.value)}><option value="cash">نقدی</option><option value="card">کارت</option><option value="transfer">انتقال بانکی</option></select></label><JalaliDateField label="تاریخ تراکنش" value={form.date||isoToday} onChange={v=>setField('date',v)}/></div>{field('notes','یادداشت','اختیاری')}</>}
  </div><div className="modal-actions"><button type="button" className="secondary-btn" onClick={onClose}>انصراف</button><button type="submit" className="primary-btn" disabled={busy}>{busy?'در حال ثبت...':'ثبت اطلاعات'}</button></div>
 </form></div>;
}
export default App;
