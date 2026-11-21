import { lazy, Suspense, useCallback, useEffect, useMemo, useState, useRef, useDeferredValue } from 'react';
import type { FormEvent, ReactNode, KeyboardEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import JsBarcode from 'jsbarcode';
import { formatJalaliDate, latinDigits } from './utils/jalali';
const JalaliDateField=lazy(()=>import('./components/forms/JalaliDateField').then(m=>({default:m.JalaliDateField})));
import { PageHeading, PrimaryButton } from './components/layout/PagePrimitives';
const RecordsPage=lazy(()=>import('./components/tables/RecordsPage').then(m=>({default:m.RecordsPage})));
const Dashboard=lazy(()=>import('./components/pages/DashboardProducts').then(m=>({default:m.Dashboard})));
const ProductsPage=lazy(()=>import('./components/pages/DashboardProducts').then(m=>({default:m.ProductsPage})));
const ReportsPage=lazy(()=>import('./components/pages/ReportsPage').then(m=>({default:m.ReportsPage})));
import type { RecordRow } from './types';
import {
  ArrowDownLeft, ArrowUpRight, BarChart3, Bell, Boxes, CalendarDays,
  Check, ChevronDown, ChevronLeft, CircleHelp, Command, CreditCard, Download,
  FileText, LayoutDashboard, Menu, Package, Plus, Pencil, Eye, Printer, Search, Settings,
  ShoppingCart, SlidersHorizontal, Sparkles, Store, Users, Wallet, X, Trash2,
  Upload, RefreshCw, ScanBarcode
} from 'lucide-react';

type Page = 'dashboard'|'contacts'|'invoices'|'payments'|'products'|'reports'|'printing'|'settings';
type Entry = 'contact'|'product'|'sale'|'purchase'|'receipt'|'payment';
type Product = {id:number;name:string;code:string;barcode:string;quantity:number;buy_price:number;average_cost:number;sell_price:number;consumer_price:number;discount_type:'none'|'percent'|'fixed';discount_value:number;min_quantity:number;category:string;unit:string;wholesale_price:number;is_service:boolean;is_active:boolean};
type InventoryMovement = {product_id:number;product_name:string;movement_date:string;movement_type:string;reference:string;quantity_delta:number;balance_after:number};
type Summary = {sales_today:number;receipts_today:number;customer_balance:number;supplier_balance:number;inventory_value:number;contacts:number;products:number;invoices:number;sales_month:number;purchases_month:number;gross_profit_month:number;net_profit_month:number;uncosted_sales_month:number;low_stock_count:number};
type Trend = {date:string;total:number};
type InvoicePrint = {invoice:{id:number;number:number;kind:string;contact_name:string;invoice_date:string;subtotal:number;discount:number;total:number;paid:number;notes:string};items:{item_name:string;quantity:number;unit_price:number;line_total:number}[];shop:Record<string,string>};
type ContactLedger = {contact:{id:number;name:string;kind:string;phone:string;opening_balance:number;customer_credit_balance:number;supplier_credit_balance:number};entries:{id:number;number?:number;entry_type:string;kind:string;date:string;amount:number;paid?:number;method?:string;notes:string}[]};
type InvoiceDraft = {id:number;kind:'sale'|'purchase';title:string;updated_at:string};
type InvoiceItemDetail = {id:number;product_id:number|null;item_name:string;quantity:number;unit_price:number;line_total:number;unit_cost:number|null;sales_returned:number;purchase_returned:number;code:string;barcode:string};
type InvoiceDetail = {invoice:{id:number;number:number;kind:'sale'|'purchase';contact_id:number|null;contact_name:string;invoice_date:string;subtotal:number;discount:number;total:number;paid:number;notes:string;voided:boolean;credit_applied:number};items:InvoiceItemDetail[];payments:{id:number;amount:number;method:string;date:string;notes:string;allocated:number}[];returns:{id:number;date:string;amount:number;credit_refund:number;notes:string;kind:string}[];refunds:{id:number;date:string;amount:number;method:string;notes:string;kind:string}[];void:{id:number;date:string;cash_refund:number;credit_refund:number;notes:string}|null};


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
const productDiscount=(p:Product)=>p.discount_type==='percent'?Math.round(p.sell_price*p.discount_value/100):p.discount_type==='fixed'?p.discount_value:0;
const productFinalPrice=(p:Product)=>Math.max(0,p.sell_price-productDiscount(p));
const hasProductDiscount=(p:Product)=>p.discount_type!=='none'&&p.discount_value>0;
const escapeHtml=(value:unknown)=>String(value??'').replace(/[&<>\"']/g,ch=>({'&':'&amp;','<':'&lt;','>':'&gt;','\"':'&quot;',"'":'&#39;'}[ch]||ch));
const today=new Intl.DateTimeFormat('fa-IR-u-ca-persian',{day:'numeric',month:'long',year:'numeric'}).format(new Date());
const localIso=(date:Date)=>date.getFullYear()+'-'+String(date.getMonth()+1).padStart(2,'0')+'-'+String(date.getDate()).padStart(2,'0');
const isoToday=localIso(new Date());
const emptySummary:Summary={sales_today:0,receipts_today:0,customer_balance:0,supplier_balance:0,inventory_value:0,contacts:0,products:0,invoices:0,sales_month:0,purchases_month:0,gross_profit_month:0,net_profit_month:0,uncosted_sales_month:0,low_stock_count:0};

function App(){
  const [page,setPage]=useState<Page>('dashboard');
  const [summary,setSummary]=useState<Summary>(emptySummary);
  const [records,setRecords]=useState<RecordRow[]>([]);
  const [products,setProducts]=useState<Product[]>([]);
  const [movements,setMovements]=useState<InventoryMovement[]>([]);
  const [trend,setTrend]=useState<Trend[]>([]);
  const [days,setDays]=useState(7);
  const [query,setQuery]=useState('');
  const deferredQuery=useDeferredValue(query);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [toast,setToast]=useState('');
  const [entry,setEntry]=useState<Entry|null>(null);
  const [editingId,setEditingId]=useState<number|null>(null);
  const [ledger,setLedger]=useState<ContactLedger|null>(null);
  const [invoiceDrafts,setInvoiceDrafts]=useState<InvoiceDraft[]>([]);
  const [invoiceDetail,setInvoiceDetail]=useState<InvoiceDetail|null>(null);
  const [returnInvoice,setReturnInvoice]=useState<InvoiceDetail|null>(null);
  const [paymentInvoice,setPaymentInvoice]=useState<InvoiceDetail|null>(null);
  const [invoiceKindFilter,setInvoiceKindFilter]=useState('all');
  const [invoiceStatusFilter,setInvoiceStatusFilter]=useState('all');
  const [invoiceDateFrom,setInvoiceDateFrom]=useState('');
  const [invoiceDateTo,setInvoiceDateTo]=useState('');
  const [draftId,setDraftId]=useState<number|null>(null);
  const [form,setForm]=useState<Record<string,string>>({});
  const [lines,setLines]=useState([{item:'',qty:'1',price:'0'}]);
  const [settings,setSettings]=useState<Record<string,string>>({});
  const [autoBackups,setAutoBackups]=useState<{name:string;size:number;modified:number}[]>([]);
  const [mobileOpen,setMobileOpen]=useState(false);
  const invoiceSubmitLock=useRef(false);
  const draftSaveLock=useRef(false);
  const invoiceRequestKey=useRef('');

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
      }else if(page==='invoices'){
        const [rows,drafts]=await Promise.all([
          invoke<RecordRow[]>('list_records',{kind:'invoices'}),
          invoke<InvoiceDraft[]>('list_invoice_drafts')
        ]);
        setRecords(rows);setInvoiceDrafts(drafts);
      }else if(page==='contacts'||page==='payments'){
        const kind=page==='contacts'?'contacts':'payments';
        setRecords(await invoke<RecordRow[]>('list_records',{kind}));
      }else if(page==='products'){
        const [items,ledger]=await Promise.all([invoke<Product[]>('list_products'),invoke<InventoryMovement[]>('inventory_movements_command')]);
        setProducts(items);setMovements(ledger);
      }else if(page==='reports'){
        const r=await invoke<Summary & Record<string,number>>('reports_summary');
        setSummary({...emptySummary,...r,customer_balance:r.customer_debt??r.customer_balance??0});
      }else if(page==='printing'){
        setProducts(await invoke<Product[]>('list_products'));
      }else if(page==='settings'){
        setSettings(await invoke<Record<string,string>>('get_settings'));
      }
    }catch(e){
      setError('اتصال به پایگاه داده برقرار نشد. این بخش در نسخه دسکتاپ نبض فعال است.');
      if(String(e).includes('dashboard_summary')) setSummary(emptySummary);
    }finally{setBusy(false)}
  },[page,days]);
  useEffect(()=>{void invoke<Record<string,string>>('get_settings').then(setSettings).catch(()=>{});},[]);
  useEffect(()=>{void load()},[load]);

  const filteredRecords=useMemo(()=>records.filter(r=>(r.name+' '+r.detail+' '+r.status).toLocaleLowerCase().includes(deferredQuery.toLocaleLowerCase())),[records,deferredQuery]);
  const filteredInvoices=useMemo(()=>filteredRecords.filter(r=>(invoiceKindFilter==='all'||r.detail===invoiceKindFilter)&&(invoiceStatusFilter==='all'||(invoiceStatusFilter==='void'&&r.status.includes('باطل'))||(invoiceStatusFilter==='settled'&&r.status==='تسویه')||(invoiceStatusFilter==='partial'&&r.status==='پرداخت جزئی')||(invoiceStatusFilter==='unpaid'&&r.status==='پرداخت‌نشده'))&&(!invoiceDateFrom||r.date>=invoiceDateFrom)&&(!invoiceDateTo||r.date<=invoiceDateTo)),[filteredRecords,invoiceKindFilter,invoiceStatusFilter,invoiceDateFrom,invoiceDateTo]);
  const filteredProducts=useMemo(()=>products.filter(p=>(p.name+' '+p.code+' '+p.barcode+' '+p.category).toLocaleLowerCase().includes(deferredQuery.toLocaleLowerCase())),[products,deferredQuery]);
  const openEntry=(kind:Entry)=>{
    setEditingId(null);setDraftId(null);invoiceRequestKey.current=(kind==='sale'||kind==='purchase')?crypto.randomUUID():'';setEntry(kind);setQuery('');
    setForm({name:'',kind:kind==='payment'?'supplier':kind==='receipt'?'customer':'',phone:'',balance:'0',notes:'',code:'',barcode:'',qty:'0',buy:'0',consumer_price:'0',sell:'0',discount_type:'none',discount_value:'0',min:'0',customer:'',amount:'',method:'cash',date:isoToday,item:'',price:'0',discount:'0',status:'paid',paid:'',paper:'A4'});
    setLines([{item:'',qty:'1',price:'0'}]);
    if(kind==='sale'||kind==='purchase'){void invoke<Product[]>('list_products').then(setProducts).catch(e=>notify('بارگذاری فهرست کالاها ناموفق بود: '+String(e)))}
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
  const viewInvoice=async(id:number)=>{
    try{setInvoiceDetail(await invoke<InvoiceDetail>('get_invoice_detail',{id}))}
    catch(e){notify('دریافت جزئیات فاکتور ناموفق بود: '+String(e))}
  };
  const saveInvoiceDraft=async()=>{
    if((entry!=='sale'&&entry!=='purchase')||draftSaveLock.current)return;
    draftSaveLock.current=true;
    const payload={kind:entry==='purchase'?'purchase':'sale',customer:form.customer||'',date:form.date||isoToday,
      discount:Number(form.discount||0),notes:form.notes||'',payment_status:form.status||'paid',
      paid_amount:Number(form.paid||0),method:form.method||'cash',items:lines};
    try{
      const id=await invoke<number>('save_invoice_draft',{payload,draftId});
      setDraftId(id);
      setInvoiceDrafts(await invoke<InvoiceDraft[]>('list_invoice_drafts'));
      notify('پیش‌نویس فاکتور ذخیره شد؛ می‌توانی بعداً ادامه بدهی.');
    }catch(e){notify('ذخیره پیش‌نویس ناموفق بود: '+String(e))}
    finally{draftSaveLock.current=false}
  };
  const resumeInvoiceDraft=async(id:number)=>{
    try{
      const payload=await invoke<Record<string,unknown>>('get_invoice_draft',{id});
      const kind=payload.kind==='purchase'?'purchase':'sale';
      setDraftId(id);invoiceRequestKey.current=crypto.randomUUID();setEditingId(null);setEntry(kind);
      setForm({name:'',kind:'',phone:'',balance:'0',notes:String(payload.notes||''),code:'',barcode:'',qty:'0',buy:'0',sell:'0',min:'0',
        customer:String(payload.customer||''),amount:'',method:String(payload.method||'cash'),date:String(payload.date||isoToday),
        item:'',price:'0',discount:String(payload.discount??0),status:String(payload.payment_status||'paid'),paid:String(payload.paid_amount??''),paper:'A4'});
      const saved=Array.isArray(payload.items)?payload.items as {item:string;qty:string|number;price:string|number}[]:[{item:'',qty:'1',price:'0'}];
      setLines(saved.map(x=>({item:String(x.item||''),qty:String(x.qty??1),price:String(x.price??0)})));
      setQuery('');
      void invoke<Product[]>('list_products').then(setProducts).catch(e=>notify('بارگذاری کالاها ناموفق بود: '+String(e)));
    }catch(e){notify('بازکردن پیش‌نویس ناموفق بود: '+String(e))}
  };
  const deleteInvoiceDraft=async(id:number)=>{
    if(!window.confirm('این پیش‌نویس حذف شود؟'))return;
    try{await invoke('delete_invoice_draft',{id});setInvoiceDrafts(await invoke<InvoiceDraft[]>('list_invoice_drafts'));notify('پیش‌نویس حذف شد.')}
    catch(e){notify('حذف پیش‌نویس ناموفق بود: '+String(e))}
  };
  const submitInvoicePayment=async(payload:{amount:number;method:string;date:string;notes:string})=>{
    if(!paymentInvoice)return;
    try{
      await invoke('save_invoice_payment',{invoiceId:paymentInvoice.invoice.id,payload:{kind:paymentInvoice.invoice.kind==='sale'?'receipt':'payment',
        customer:paymentInvoice.invoice.contact_name,amount:payload.amount,method:payload.method,date:payload.date,notes:payload.notes}});
      notify('پرداخت به همین فاکتور تخصیص داده شد.');
      const id=paymentInvoice.invoice.id;setPaymentInvoice(null);await load();await viewInvoice(id);
    }catch(e){notify('ثبت پرداخت فاکتور ناموفق بود: '+String(e))}
  };
  const submitInvoiceReturn=async(method:string,returnLines:{invoice_item_id:number;item:string;qty:number}[])=>{
    if(!returnInvoice)return;
    const invoice=returnInvoice.invoice;
    try{
      const result=await invoke<{credit:number;cash_refund:number}>(invoice.kind==='sale'?'return_sale_invoice':'return_purchase_invoice',
        {invoiceId:invoice.id,lines:returnLines,method});
      notify('برگشت ثبت شد · مبلغ سند: '+money(result.credit)+' تومان · بازپرداخت نقدی: '+money(result.cash_refund)+' تومان');
      const id=invoice.id;setReturnInvoice(null);await load();await viewInvoice(id);
    }catch(e){notify('ثبت برگشت ناموفق بود: '+String(e))}
  };

  const openProductEdit=(p:Product)=>{
    setEditingId(p.id);setEntry('product');setQuery('');
    setForm({name:p.name,code:p.code,barcode:p.barcode,qty:String(p.quantity),buy:String(p.buy_price),consumer_price:String(p.consumer_price||0),sell:String(p.sell_price),discount_type:p.discount_type||'none',discount_value:String(p.discount_value||0),min:String(p.min_quantity),category:p.category||'',unit:p.unit||'عدد',wholesale:String(p.wholesale_price||0),is_service:p.is_service?'true':'false'});
  };
  const setField=(key:string,value:string)=>setForm(old=>({...old,[key]:value}));
  const saveEntry=async(e:FormEvent)=>{
    e.preventDefault();if(!entry)return;
    if(invoiceSubmitLock.current){notify('درخواست ثبت قبلی هنوز در حال انجام است.');return}
    if(entry==='sale'||entry==='purchase'){
      if(!lines.length||lines.some(x=>!x.item.trim())){notify('تمام ردیف‌های فاکتور باید کالا یا خدمت داشته باشند.');return}
      if(lines.some(x=>!Number.isSafeInteger(Number(x.qty))||Number(x.qty)<=0||!Number.isSafeInteger(Number(x.price))||Number(x.price)<0)){notify('تعداد باید عدد صحیح مثبت و قیمت واحد باید عدد صحیح نامنفی باشد.');return}
      const subtotal=lines.reduce((sum,x)=>sum+Number(x.qty)*Number(x.price),0);
      const discount=Number(form.discount||0);
      if(!Number.isSafeInteger(subtotal)||!Number.isSafeInteger(discount)||discount<0||discount>subtotal){notify('مبلغ تخفیف یا جمع فاکتور معتبر نیست.');return}
      if(entry==='sale'&&(form.status==='credit'||form.status==='partial')&&!form.customer.trim()){notify('برای فروش نسیه یا پرداخت جزئی، ابتدا مشتری ثبت‌شده را انتخاب کنید.');return}if(entry==='sale'&&form.status==='partial'&&(!Number.isSafeInteger(Number(form.paid))||Number(form.paid)<=0||Number(form.paid)>=subtotal-discount)){notify('مبلغ دریافتی باید بیشتر از صفر و کمتر از مبلغ نهایی باشد.');return}
    }
    if(entry==='sale'||entry==='purchase')invoiceSubmitLock.current=true;
    setBusy(true);
    try{
      let id:number|void=undefined;
      if(entry==='contact'){
        id=await invoke<number>('save_contact',{payload:{...(editingId?{id:editingId}:{}),name:form.name,kind:form.kind,phone:form.phone,balance:Number(form.balance||0),...(editingId?{}:{notes:form.notes})}});
      }else if(entry==='product'){
        id=await invoke<number>('save_product',{payload:{...(editingId?{id:editingId}:{}),name:form.name,code:form.code,barcode:form.barcode,qty:Number(form.qty||0),buy:Number(form.buy||0),consumer_price:Number(form.consumer_price||0),sell:Number(form.sell||0),discount_type:form.discount_type||'none',discount_value:Number(form.discount_value||0),min:Number(form.min||0),category:form.category||'',unit:form.unit||'عدد',wholesale:Number(form.wholesale||0),is_service:form.is_service==='true'}});
      }else if(entry==='sale'||entry==='purchase'){
        const items=lines.filter(x=>x.item.trim()).map(x=>({item:x.item.trim(),qty:Number(x.qty),price:Number(x.price)}));
        id=await invoke<number>('save_invoice',{payload:{kind:entry==='purchase'?'purchase':'sale',customer:form.customer,payment_status:form.status,paid_amount:Number(form.paid||0),method:form.method,items,discount:Number(form.discount||0),date:form.date,notes:form.notes,idempotency_key:invoiceRequestKey.current}});
      }else{
        id=await invoke<number>('save_payment',{payload:{kind:entry==='payment'?'payment':'receipt',customer:form.customer,amount:Number(form.amount||0),method:form.method,date:form.date,notes:form.notes}});
      }
      notify('اطلاعات با موفقیت ثبت شد'+(id?' · شناسه '+id:''));
      if((entry==='sale'||entry==='purchase')&&draftId!==null){try{await invoke('delete_invoice_draft',{id:draftId});setDraftId(null)}catch(e){notify('فاکتور ثبت شد اما پاک‌کردن پیش‌نویس ناموفق بود: '+String(e))}}
      closeEntry();await load();
    }catch(e){notify('ثبت انجام نشد: '+String(e))}
    finally{invoiceSubmitLock.current=false;setBusy(false)}
  };
  const adjustProductStock=async(p:Product)=>{
    if(p.is_service){notify('خدمت موجودی انبار ندارد.');return}
    const raw=window.prompt('موجودی واقعی جدید برای «'+p.name+'» را وارد کن:',String(p.quantity));
    if(raw===null)return;
    const quantity=Number(latinDigits(raw).replace(/,/g,''));
    if(!Number.isSafeInteger(quantity)||quantity<0){notify('موجودی باید عدد صحیح و نامنفی باشد.');return}
    const reason=window.prompt('علت اصلاح موجودی (حداقل ۳ نویسه):','شمارش انبار');
    if(reason===null)return;
    try{await invoke('adjust_inventory_command',{productId:p.id,newQuantity:quantity,reason});notify('موجودی اصلاح و در دفتر انبار ثبت شد.');await load()}
    catch(e){notify('اصلاح موجودی انجام نشد: '+String(e))}
  };
  const printProductLabel=(p:Product)=>{
    const value=p.barcode||p.code;
    const w=window.open('','_blank','width=520,height=420');
    if(!w){notify('پنجره چاپ مسدود است؛ اجازه بازشدن پنجره را فعال کن.');return}
    w.document.write('<!doctype html><html lang="fa" dir="rtl"><meta charset="utf-8"><title>برچسب کالا</title><style>body{font-family:Tahoma,sans-serif;padding:22px;text-align:center;color:#18251e}h1{font-size:18px;margin:4px 0 12px}svg{width:100%;height:110px}p{font:12px monospace;letter-spacing:1px;margin:8px}small{color:#66736a}@page{size:auto;margin:6mm}</style><h1>'+escapeHtml(p.name)+'</h1><svg id="product-label-code"></svg><p>'+escapeHtml(value)+'</p><small>'+escapeHtml(p.code)+'</small></html>');
    w.document.close();
    try{const svg=w.document.getElementById('product-label-code') as unknown as SVGSVGElement;JsBarcode(svg,value,{format:'CODE128',displayValue:false,lineColor:'#1d2c24',width:2,height:76,margin:8});w.focus();w.print()}
    catch(e){w.close();notify('ساخت بارکد برچسب ناموفق بود: '+String(e))}
  };
  const toggleProductActive=async(p:Product)=>{const active=!p.is_active;if(!window.confirm(active?'این کالا دوباره در فاکتورهای جدید قابل انتخاب شود؟':'این کالا از فهرست انتخاب فاکتورهای جدید خارج شود؟ سابقه آن حفظ می‌شود.'))return;try{await invoke('set_product_active',{productId:p.id,active});notify(active?'کالا فعال شد.':'کالا غیرفعال شد و سوابق آن باقی ماند.');await load()}catch(e){notify('تغییر وضعیت کالا انجام نشد: '+String(e))}};
  const removeRecord=async(kind:string,id:number)=>{
    if(kind==='invoices'){
      if(!window.confirm('این فاکتور به‌صورت امن باطل شود؟ سابقه فاکتور حذف نمی‌شود و عملیات معکوس فقط در صورت نبود گردش ناسازگار انجام خواهد شد.'))return;
      try{await invoke('void_invoice',{invoiceId:id});notify('فاکتور با موفقیت باطل شد؛ سابقه آن حفظ شده است.');await load()}
      catch(e){notify('ابطال فاکتور انجام نشد: '+String(e))}
      return;
    }
    if(!window.confirm('این رکورد حذف شود؟'))return;
    try{await invoke('delete_record',{kind,id});notify('رکورد حذف شد.');await load()}
    catch(e){notify('حذف انجام نشد: '+String(e))}
  };
  const refreshAutoBackups=async()=>{try{setAutoBackups(await invoke<{name:string;size:number;modified:number}[]>('list_auto_backups'))}catch{}};
  const createNativeBackup=async(force=true)=>{
    try{const result=await invoke<{created:boolean;name:string;size:number;modified:number}>('create_auto_backup',{force});await refreshAutoBackups();notify(result.created?'پشتیبان محلی امن ساخته شد.':'پشتیبان ۲۴ ساعت اخیر موجود است.');}
    catch(e){notify('پشتیبان خودکار ناموفق بود: '+String(e))}
  };
  useEffect(()=>{void invoke('create_auto_backup',{force:false}).then(()=>refreshAutoBackups()).catch(e=>notify('پشتیبان خودکار شروع نشد: '+String(e)));const timer=window.setInterval(()=>void invoke('create_auto_backup',{force:false}).then(()=>refreshAutoBackups()).catch(()=>{}),24*60*60*1000);return()=>window.clearInterval(timer)},[]);
  const backup=async()=>{
    try{const data=await invoke<Record<string,unknown>>('export_data');const blob=new Blob([JSON.stringify(data,null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='nabz-backup-'+new Date().toISOString().slice(0,10)+'.json';a.click();URL.revokeObjectURL(url);notify('نسخه پشتیبان دریافت شد.')}
    catch(e){notify('تهیه نسخه پشتیبان ناموفق بود: '+String(e))}
  };
  const restore=async(file?:File)=>{
    if(!file)return;
    try{
      if(file.size>100*1024*1024)throw new Error('حجم فایل پشتیبان بیشتر از سقف ۱۰۰ مگابایت است.');
      const data=JSON.parse(await file.text()) as Record<string,unknown>;
      const counts=['contacts','products','invoices','invoice_items','payments','expenses','invoice_drafts'].map(k=>k+': '+(Array.isArray(data[k])?data[k].length:0)).join(' · ');
      if(!window.confirm('این پشتیبان جایگزین اطلاعات فعلی می‌شود.\n\n'+counts+'\n\nابتدا ساختار و ارتباط رکوردها بررسی و سپس پشتیبان ایمنی ساخته می‌شود. ادامه؟'))return;
      await invoke('restore_data',{data});notify('بازیابی انجام شد.');await load();
    }catch(e){notify('بازیابی انجام نشد: '+String(e))}
  };
  const restoreNative=async(name:string)=>{
    if(!window.confirm('بازیابی نسخه «'+name+'» تمام اطلاعات فعلی را با اطلاعات آن نسخه جایگزین می‌کند. پیش از آن یک پشتیبان ایمنی ساخته می‌شود. ادامه می‌دهی؟'))return;
    try{await invoke('restore_auto_backup',{name});await refreshAutoBackups();notify('بازیابی پشتیبان محلی با موفقیت انجام شد.');await load();}
    catch(e){notify('بازیابی پشتیبان محلی ناموفق بود: '+String(e))}
  };
  const printInvoice=async(id:number)=>{
    try{
      const data=await invoke<InvoicePrint>('get_invoice_print',{id});
      const rows=data.items.map(x=>'<tr><td>'+escapeHtml(x.item_name)+'</td><td>'+money(x.quantity)+'</td><td>'+money(x.unit_price)+'</td><td>'+money(x.line_total)+'</td></tr>').join('');
      const w=window.open('','_blank','width=900,height=700');if(!w)throw new Error('پنجره چاپ مسدود است');
      const paper=settings.print_paper||'A4';
      const pageSize=paper==='A5'?'A5':paper==='80mm'?'80mm auto':paper==='58mm'?'58mm auto':'A4';
      const compact=paper==='80mm'||paper==='58mm';
      const shopName=escapeHtml(data.shop.shop_name||'نبض'),shopPhone=escapeHtml(data.shop.shop_phone||''),shopAddress=escapeHtml(data.shop.shop_address||''),shopTax=escapeHtml(data.shop.shop_tax||'');
      w.document.write(`<!doctype html><html lang="fa" dir="rtl"><meta charset="utf-8"><title>فاکتور ${escapeHtml(data.invoice.number)}</title><style>
        @page{size:${pageSize};margin:${compact?'3mm':'10mm'}}
        body{font-family:Tahoma,sans-serif;padding:${compact?'0':'10px'};color:#182536;font-size:${compact?'10px':'12px'}}
        h1{font-size:${compact?'15px':'22px'};margin:0 0 6px}
        h2{font-size:${compact?'12px':'18px'}}
        table{width:100%;border-collapse:collapse;margin-top:${compact?'10px':'24px'}}
        td,th{border:1px solid #ddd;padding:${compact?'4px':'8px'};text-align:right;font-size:inherit}
        footer{margin-top:12px;font-weight:bold;line-height:2}.shop-info{line-height:1.8;font-size:inherit}
        @media print{body{print-color-adjust:exact}}
      </style><h1>${shopName}</h1><div class="shop-info">${shopPhone?('تماس: '+shopPhone+'<br>'):''}${shopAddress?('نشانی: '+shopAddress+'<br>'):''}${shopTax?('شناسه مالیاتی: '+shopTax):''}</div><h2>فاکتور ${data.invoice.kind==='sale'?'فروش':'خرید'} شماره ${money(data.invoice.number)}</h2><p>طرف حساب: ${escapeHtml(data.invoice.contact_name)}　 تاریخ: ${escapeHtml(formatJalaliDate(data.invoice.invoice_date))}</p><table><thead><tr><th>شرح</th><th>تعداد</th><th>فی</th><th>مبلغ</th></tr></thead><tbody>${rows}</tbody></table><footer>جمع جزء: ${money(data.invoice.subtotal)} تومان<br>تخفیف: ${money(data.invoice.discount)} تومان<br>جمع نهایی: ${money(data.invoice.total)} تومان<br>پرداخت‌شده: ${money(data.invoice.paid)} تومان<br>مانده: ${money(Math.max(0,data.invoice.total-data.invoice.paid))} تومان</footer><script>window.onload=()=>window.print()</script></html>`);
      w.document.close();
    }catch(e){notify('آماده‌سازی چاپ ناموفق بود: '+String(e))}
  };
  const saveSettings=async()=>{
    try{
      if((settings.selling_price_mode||'manual')==='auto'){
        const margin=Number(settings.selling_price_margin_percent??0);
        if(!Number.isSafeInteger(margin)||margin<0||margin>1000){notify('درصد سود باید عدد صحیحی بین صفر تا ۱۰۰۰ باشد.');return}
        if(!['buy','consumer'].includes(settings.selling_price_basis||'buy')){notify('مبنای محاسبه قیمت فروش معتبر نیست.');return}
      }
      await Promise.all(Object.entries(settings).map(([key,value])=>invoke('save_setting',{key,value})));
      if((settings.selling_price_mode||'manual')==='auto'){
        const changed=await invoke<number>('recalculate_selling_prices');
        notify('تنظیمات ذخیره شد؛ قیمت فروش '+money(changed)+' کالا بر اساس قاعده جدید محاسبه شد.');
      }else notify('تنظیمات ذخیره شد.');
    }catch(e){notify('ذخیره تنظیمات ناموفق بود: '+String(e))}
  };

  useEffect(()=>{
    const onShortcut=(event:KeyboardEvent)=>{
      const target=event.target as HTMLElement|null;
      const typing=!!target&&(target.isContentEditable||!!target.closest('input,textarea,select'));
      if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k'){event.preventDefault();setQuery('');window.setTimeout(()=>document.querySelector<HTMLInputElement>('.search-box input')?.focus(),0);return;}
      if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='s'&&(entry==='sale'||entry==='purchase')){event.preventDefault();void saveInvoiceDraft();return;}
      if(event.key==='Escape'&&entry){event.preventDefault();closeEntry();return;}
      if(typing)return;
      if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='n'){event.preventDefault();openEntry(page==='contacts'?'contact':page==='products'?'product':page==='payments'?'receipt':'sale');return;}
      if(event.key==='F1'){event.preventDefault();notify('میانبرها: Ctrl+K جست‌وجو · Ctrl+N مورد جدید · Ctrl+S ذخیره پیش‌نویس · F2 افزودن · F4 دریافت/پرداخت · F8 کالا · Esc بستن');}
      else if(event.key==='F2'){event.preventDefault();openEntry(page==='contacts'?'contact':page==='products'?'product':page==='payments'?'receipt':'sale');}
      else if(event.key==='F4'){event.preventDefault();setPage('payments');setQuery('');}
      else if(event.key==='F8'){event.preventDefault();setPage('products');setQuery('');}
    };
    window.addEventListener('keydown',onShortcut);return()=>window.removeEventListener('keydown',onShortcut);
  },[page,entry,openEntry,saveInvoiceDraft,closeEntry,notify]);

  const active=pageTitles[page];
  return <Suspense fallback={<div className="app-loading" role="status">در حال بارگذاری نبض…</div>}><div className="app-shell">
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
        {page==='invoices'&&<><InvoiceDraftsPanel drafts={invoiceDrafts} onResume={resumeInvoiceDraft} onDelete={deleteInvoiceDraft}/><RecordsPage title="فاکتورها" subtitle="فاکتورهای فروش و خرید ثبت‌شده در فروشگاه" records={filteredInvoices} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('sale')} onDelete={id=>removeRecord('invoices',id)} onPrint={printInvoice} onView={viewInvoice} columns={['شرح فاکتور','نوع','مبلغ کل','وضعیت','تاریخ']} extra={<><select className="invoice-filter" aria-label="نوع فاکتور" value={invoiceKindFilter} onChange={e=>setInvoiceKindFilter(e.target.value)}><option value="all">همه انواع</option><option value="sale">فروش</option><option value="purchase">خرید</option></select><select className="invoice-filter" aria-label="وضعیت فاکتور" value={invoiceStatusFilter} onChange={e=>setInvoiceStatusFilter(e.target.value)}><option value="all">همه وضعیت‌ها</option><option value="unpaid">پرداخت‌نشده</option><option value="partial">پرداخت جزئی</option><option value="settled">تسویه</option><option value="void">باطل‌شده</option></select><JalaliDateField label="از تاریخ" value={invoiceDateFrom} onChange={setInvoiceDateFrom} todayValue={isoToday}/><JalaliDateField label="تا تاریخ" value={invoiceDateTo} onChange={setInvoiceDateTo} todayValue={isoToday}/><button className="secondary-btn" onClick={()=>openEntry('purchase')}><Plus size={15}/> خرید جدید</button></>} /></>}
        {page==='payments'&&<RecordsPage title="دریافت و پرداخت" subtitle="ثبت و پیگیری دریافت وجه و پرداخت به تأمین‌کنندگان" records={filteredRecords} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('receipt')} columns={['طرف‌حساب','نوع عملیات','مبلغ','روش','تاریخ']} extra={<><button className="secondary-btn" onClick={()=>openEntry('payment')}><ArrowUpRight size={15}/> ثبت پرداخت</button></>} />}
        {page==='products'&&<ProductsPage products={filteredProducts} movements={movements} query={query} setQuery={setQuery} busy={busy} onAdd={()=>openEntry('product')} onEdit={openProductEdit} onToggleActive={toggleProductActive} onAdjust={adjustProductStock} onPrintLabel={printProductLabel}/> }
        {page==='reports'&&<ReportsPage/>}
        {page==='printing'&&<PrintingPage notify={notify} products={filteredProducts}/>}
        {page==='settings'&&<SettingsPage settings={settings} setSettings={setSettings} onSave={saveSettings} onBackup={backup} onRestore={restore} autoBackups={autoBackups} onNativeBackup={()=>createNativeBackup(true)} onRestoreNative={restoreNative}/>}
        <footer className="page-footer"><span>نبض · مدیریت مالی هوشمند فروشگاه</span><span>نسخه آزمایشی رابط React</span></footer>
      </section>
    </main>
    {entry&&<EntryModal entry={entry} products={products.filter(p=>p.is_active)} settings={settings} form={form} setField={setField} lines={lines} setLines={setLines} onClose={closeEntry} onSubmit={saveEntry} onSaveDraft={saveInvoiceDraft} hasDraft={draftId!==null} busy={busy} editing={editingId!==null}/>}
    {ledger&&<ContactLedgerModal ledger={ledger} onClose={()=>setLedger(null)}/>}
    {invoiceDetail&&<InvoiceDetailModal detail={invoiceDetail} onClose={()=>setInvoiceDetail(null)} onPrint={()=>printInvoice(invoiceDetail.invoice.id)} onReturn={()=>{setReturnInvoice(invoiceDetail);setInvoiceDetail(null)}} onPay={()=>{setPaymentInvoice(invoiceDetail);setInvoiceDetail(null)}} onVoid={()=>{setInvoiceDetail(null);void removeRecord('invoices',invoiceDetail.invoice.id)}}/>}
    {returnInvoice&&<InvoiceReturnModal detail={returnInvoice} onClose={()=>{setInvoiceDetail(returnInvoice);setReturnInvoice(null)}} onSubmit={submitInvoiceReturn}/>}
    {paymentInvoice&&<InvoicePaymentModal detail={paymentInvoice} onClose={()=>{setInvoiceDetail(paymentInvoice);setPaymentInvoice(null)}} onSubmit={submitInvoicePayment}/>}
    {toast&&<div className="toast"><Check size={16}/>{toast}</div>}
  </div></Suspense>;
}


function PrintingPage({notify,products}:{notify:(s:string)=>void;products:Product[]}){
 const [barcode,setBarcode]=useState('626000001');
 const [format,setFormat]=useState<'CODE128'|'EAN13'>('CODE128');
 const [barcodeError,setBarcodeError]=useState('');
 const barcodeRef=useRef<SVGSVGElement>(null);
 const [printValue,setPrintValue]=useState('');
 const [selectedIds,setSelectedIds]=useState<number[]>([]);
 const [selectedLabelIds,setSelectedLabelIds]=useState<number[]>([]);
 const [copies,setCopies]=useState<Record<number,string>>({});
 const printable=products.filter(p=>p.is_active&&!p.is_service&&(p.barcode||p.code)).slice(0,200);
 const labelProducts=products.filter(p=>p.is_active&&!p.is_service).slice(0,500);
 useEffect(()=>{
  if(!barcodeRef.current)return;
  let value=barcode.trim();
  if(format==='EAN13'){
   if(/^\d{12}$/.test(value)){const sum=value.split('').reduce((n,d,i)=>n+Number(d)*(i%2?3:1),0);value+=String((10-sum%10)%10);}
   if(!/^\d{13}$/.test(value)){setBarcodeError('EAN-13 باید ۱۲ یا ۱۳ رقم داشته باشد.');setPrintValue('');barcodeRef.current.innerHTML='';return;}
   const digits=value.split('').map(Number),sum=digits.slice(0,12).reduce((n,d,i)=>n+d*(i%2?3:1),0),check=(10-sum%10)%10;
   if(digits[12]!==check){setBarcodeError('رقم کنترلی EAN-13 معتبر نیست.');setPrintValue('');barcodeRef.current.innerHTML='';return;}
  }
  try{JsBarcode(barcodeRef.current,value,{format,displayValue:true,lineColor:'#1d2c24',width:2,height:76,margin:10,font:'Tahoma',fontSize:13});setBarcodeError('');setPrintValue(value)}
  catch{setBarcodeError('کد بارکد خالی یا نامعتبر است.');setPrintValue('')}
 },[barcode,format]);
 const printDocument=(html:string)=>{
  const frame=document.createElement('iframe');
  frame.setAttribute('title','پیش‌نمایش چاپ');
  frame.setAttribute('aria-hidden','true');
  frame.style.cssText='position:fixed;left:-10000px;top:0;width:210mm;height:297mm;border:0;pointer-events:none';
  document.body.appendChild(frame);
  frame.onload=()=>{
   const win=frame.contentWindow;
   if(!win){frame.remove();notify('ساخت پنجره چاپ ناموفق بود.');return}
   const cleanup=()=>{if(frame.isConnected)frame.remove()};
   win.addEventListener('afterprint',cleanup,{once:true});
   win.focus();
   win.print();
   window.setTimeout(cleanup,60000);
  };
  frame.srcdoc=html;
 };
 const printSingle=()=>{
  if(!printValue||!barcodeRef.current)return;
  printDocument('<!doctype html><html lang="fa" dir="rtl"><meta charset="utf-8"><title>برچسب بارکد</title><style>@page{size:auto;margin:6mm}body{font-family:Vazirmatn,Tahoma,sans-serif;text-align:center;color:#18251e;padding:8mm}h2{font-size:14pt}svg{width:100%;max-width:100mm;height:35mm}p{font:11pt monospace;direction:ltr}</style><h2>برچسب کالا</h2>'+barcodeRef.current.outerHTML+'<p>'+escapeHtml(printValue)+'</p></html>');
  notify('برچسب برای چاپ یا ذخیره PDF آماده شد.');
 };
 const printSelected=()=>{
  const chosen=printable.filter(p=>selectedIds.includes(p.id));
  if(!chosen.length){notify('حداقل یک کالا را برای چاپ گروهی انتخاب کن.');return}
  const cards=chosen.map((p,i)=>'<article class="label"><b>'+escapeHtml(p.name)+'</b><svg id="batch-code-'+i+'"></svg><small>'+escapeHtml(p.barcode||p.code)+'</small></article>').join('');
  const html='<!doctype html><html lang="fa" dir="rtl"><meta charset="utf-8"><title>برچسب کالا</title><style>@page{size:A4;margin:8mm}*{box-sizing:border-box}body{font-family:Vazirmatn,Tahoma,sans-serif;margin:0;color:#18251e}.labels{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:5mm}.label{height:42mm;border:1px dashed #9aa79f;padding:3mm;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:2mm;break-inside:avoid;overflow:hidden}.label b{font-size:10pt;text-align:center;max-height:10mm;overflow:hidden}.label svg{width:100%;height:20mm}.label small{font:9pt monospace;direction:ltr}@media print{.label{break-inside:avoid}}</style><main class="labels">'+cards+'</main></html>';
  const frame=document.createElement('iframe');frame.setAttribute('title','چاپ بارکد');frame.style.cssText='position:fixed;left:-10000px;top:0;width:210mm;height:297mm;border:0;pointer-events:none';document.body.appendChild(frame);
  frame.onload=()=>{try{chosen.forEach((p,i)=>{const node=frame.contentDocument?.getElementById('batch-code-'+i) as unknown as SVGSVGElement;JsBarcode(node,p.barcode||p.code,{format:'CODE128',displayValue:false,lineColor:'#1d2c24',width:1.5,height:48,margin:2});});const win=frame.contentWindow;if(!win)throw new Error('پنجره چاپ در دسترس نیست');const cleanup=()=>{if(frame.isConnected)frame.remove()};win.addEventListener('afterprint',cleanup,{once:true});win.focus();win.print();window.setTimeout(cleanup,60000);notify('برگه بارکدهای انتخاب‌شده آماده چاپ یا ذخیره PDF است.')}catch(e){frame.remove();notify('ساخت برچسب گروهی ناموفق بود: '+String(e))}};
  frame.srcdoc=html;
 };
 const printPriceLabels=()=>{
  const chosen=labelProducts.filter(p=>selectedLabelIds.includes(p.id));
  if(!chosen.length){notify('برای چاپ اتیکت، حداقل یک کالا را انتخاب کن.');return}
  const rows=chosen.flatMap(p=>{
   const rawCopies=Number(copies[p.id]||1);
   const count=Number.isInteger(rawCopies)?Math.max(1,Math.min(999,rawCopies)):1;
   return Array.from({length:count},()=>({product:p}));
  });
  const invalid=chosen.find(p=>p.discount_type==='percent'&&(p.discount_value<0||p.discount_value>100)||p.discount_type==='fixed'&&(p.discount_value<0||p.discount_value>=p.sell_price&&p.discount_value>0));
  if(invalid){notify('تخفیف ثبت‌شده برای کالا معتبر نیست: '+invalid.name+'؛ ابتدا مشخصات کالا را اصلاح کن.');return}
  const pages:Array<typeof rows>=[];
  for(let i=0;i<rows.length;i+=28)pages.push(rows.slice(i,i+28));
  const pageMarkup=pages.map((page,index)=>{
   const cells=Array.from({length:28},(_,i)=>{
    const item=page[i];
    if(!item)return '<article class="price-label empty"></article>';
    const p=item.product;
    const priceLine=hasProductDiscount(p)?'<div class="price-line discount"><span>قیمت با تخفیف</span><strong>'+money(productFinalPrice(p))+' <small>تومان</small></strong></div>':'<div class="price-line consumer"><span>قیمت مصرف‌کننده</span><strong>'+money(p.sell_price)+' <small>تومان</small></strong></div>';
    return '<article class="price-label"><div class="price-label-name">'+escapeHtml(p.name)+'</div>'+priceLine+'</article>';
   }).join('');
   return '<section class="a4-sheet"'+(index===pages.length-1?'':' style="break-after:page"')+'>'+cells+'</section>';
  }).join('');
  const html='<!doctype html><html lang="fa" dir="rtl"><head><meta charset="utf-8"><title>اتیکت قیمت کالاها</title><style>'+
   '@page{size:A4 portrait;margin:0}*{box-sizing:border-box}html,body{margin:0;padding:0;width:210mm;background:#fff;color:#17251d;font-family:Vazirmatn,Tahoma,"Noto Sans Arabic",sans-serif;-webkit-print-color-adjust:exact;print-color-adjust:exact}'+
   '.a4-sheet{width:210mm;height:297mm;padding:8.5mm 5mm;display:grid;grid-template-columns:repeat(4,50mm);grid-template-rows:repeat(7,40mm);gap:0;overflow:hidden;break-after:page;page-break-after:always}'+
   '.a4-sheet:last-child{break-after:auto;page-break-after:auto}.price-label{width:50mm;height:40mm;border:.25mm dashed #aebbb2;padding:2.2mm 2.5mm;display:flex;flex-direction:column;justify-content:center;gap:2mm;overflow:hidden;text-align:right;background:#fff}'+
   '.price-label-name{font-size:10pt;font-weight:700;line-height:1.5;text-align:center;max-height:9mm;overflow:hidden;color:#1c3025;margin-bottom:.5mm}'+
   '.price-line{display:flex;align-items:center;justify-content:space-between;gap:1mm;white-space:nowrap;line-height:1.4}.price-line span{font-size:7pt;color:#637269}.price-line strong{font-size:9pt;font-weight:800;color:#1d3327}.price-line small{font-size:6pt;font-weight:500;color:#65756a}.price-line.discount{border-top:.2mm solid #e4ebe6;padding-top:1.3mm}.price-line.discount strong{color:#138454}.empty{background:#fff}'+
   '@media screen{body{background:#eef2ee}.a4-sheet{margin:8mm auto;background:#fff;box-shadow:0 2mm 8mm #20332822}}'+
   '</style></head><body>'+pageMarkup+'</body></html>';
  printDocument(html);
  notify('اتیکت‌ها در قالب A4 و با اندازه واقعی ۵×۴ سانتی‌متر برای چاپ یا ذخیره PDF آماده شدند.');
 };
 const selectedLabelProducts=labelProducts.filter(p=>selectedLabelIds.includes(p.id));
 return <><PageHeading title="چاپ و بارکد" subtitle="تولید بارکد و چاپ اتیکت قیمت کالاها روی برگه A4"/><div className="surface print-panel"><div><h2>بارکد کالا</h2><p>می‌توانی کد را تایپ کنی یا با اسکنر USB/HID در همین کادر اسکن کنی.</p><label className="field-label">نوع بارکد<select value={format} onChange={e=>setFormat(e.target.value as 'CODE128'|'EAN13')}><option value="CODE128">Code 128 (SKU/کد داخلی)</option><option value="EAN13">EAN-13 (کد تجاری)</option></select></label><label className="field-label">کد یا بارکد<input value={barcode} onChange={e=>setBarcode(e.target.value)} dir="ltr" autoFocus/></label>{barcodeError&&<p className="barcode-error">{barcodeError}</p>}<button className="primary-btn" disabled={!printValue||!!barcodeError} onClick={printSingle}><Printer size={16}/> چاپ / ذخیره PDF</button></div><div className="barcode-preview"><svg ref={barcodeRef} role="img" aria-label="پیش‌نمایش بارکد تولیدشده"/></div></div>
 <section className="surface settings-card price-label-tool"><div className="surface-heading"><div><h2>چاپ اتیکت قیمت</h2><p>کالاها را انتخاب کن، در صورت نیاز قیمت تخفیفی و تعداد هر اتیکت را وارد کن، سپس پیش‌نمایش و چاپ بگیر.</p></div><span>{selectedLabelIds.length} کالا · {selectedLabelProducts.reduce((n,p)=>n+Math.max(1,Math.min(999,Number(copies[p.id]||1)||1)),0)} اتیکت</span></div>
 <div className="price-label-toolbar"><button className="secondary-btn" onClick={()=>setSelectedLabelIds(labelProducts.map(p=>p.id))}>انتخاب همه کالاها</button><button className="secondary-btn" onClick={()=>setSelectedLabelIds([])}>پاک‌کردن انتخاب</button><button className="primary-btn" disabled={!selectedLabelIds.length} onClick={printPriceLabels}><Printer size={16}/> چاپ A4 / ذخیره PDF</button></div>
 <div className="price-label-table"><div className="price-label-table-head"><span>انتخاب</span><span>نام کالا</span><span>قیمت فروش / مصرف‌کننده</span><span>قیمت با تخفیف</span><span>تعداد</span></div>
 {labelProducts.map(p=><div className="price-label-product" key={p.id}><label><input type="checkbox" checked={selectedLabelIds.includes(p.id)} onChange={e=>setSelectedLabelIds(old=>e.target.checked?[...old,p.id]:old.filter(id=>id!==p.id))}/></label><strong>{p.name}</strong><span>{money(p.sell_price)} تومان</span><span>{hasProductDiscount(p)?money(productFinalPrice(p))+' تومان':'—'}</span><input aria-label={'تعداد اتیکت '+p.name} type="number" min="1" max="999" value={copies[p.id]||'1'} onChange={e=>setCopies(old=>({...old,[p.id]:e.target.value}))}/></div>)}
 {!labelProducts.length&&<p>کالای فعالی برای چاپ اتیکت وجود ندارد.</p>}</div>
 {selectedLabelProducts.length>0&&<div className="price-label-preview"><div className="surface-heading"><div><h3>پیش‌نمایش اتیکت</h3><p>ابعاد هر اتیکت ۵۰×۴۰ میلی‌متر است؛ هر صفحه A4 شامل ۲۸ جایگاه (۴ ستون × ۷ ردیف) می‌شود.</p></div></div><div className="price-label-preview-grid">{selectedLabelProducts.slice(0,4).map(p=><article className="price-label-preview-card" key={p.id}><b>{p.name}</b>{hasProductDiscount(p)?<div><span>قیمت با تخفیف</span><strong>{money(productFinalPrice(p))} تومان</strong></div>:<div><span>قیمت مصرف‌کننده</span><strong>{money(p.sell_price)} تومان</strong></div>}</article>)}</div></div>}
 </section>
 <section className="surface settings-card batch-label-panel"><div className="surface-heading"><div><h2>چاپ گروهی بارکد کالا</h2><p>چند کالا را انتخاب کن تا برچسب‌های Code128 در یک برگه A4 چیده شوند.</p></div><span>{selectedIds.length} انتخاب</span></div><div className="backup-actions"><button className="secondary-btn" onClick={()=>setSelectedIds(printable.map(p=>p.id))}>انتخاب همه نتایج</button><button className="secondary-btn" onClick={()=>setSelectedIds([])}>پاک‌کردن انتخاب</button><button className="primary-btn" disabled={!selectedIds.some(id=>printable.some(p=>p.id===id))} onClick={printSelected}><Printer size={16}/> چاپ برچسب‌های انتخاب‌شده</button></div><div className="batch-product-list">{printable.map(p=><label key={p.id} className="batch-product-row"><input type="checkbox" checked={selectedIds.includes(p.id)} onChange={e=>setSelectedIds(old=>e.target.checked?[...old,p.id]:old.filter(id=>id!==p.id))}/><span>{p.name}<small>{p.barcode||p.code}</small></span></label>)}{!printable.length&&<p>کالای فعال دارای کد یا بارکد برای چاپ وجود ندارد.</p>}</div>{products.length>200&&<small>برای جلوگیری از کندشدن رابط، حداکثر ۲۰۰ نتیجه نخست نمایش داده می‌شود؛ از جست‌وجوی بالای برنامه برای محدودکردن فهرست استفاده کن.</small>}</section></>;
}
function SettingsPage({settings,setSettings,onSave,onBackup,onRestore,autoBackups,onNativeBackup,onRestoreNative}:{settings:Record<string,string>;setSettings:(v:Record<string,string>)=>void;onSave:()=>void;onBackup:()=>void;onRestore:(f?:File)=>void;autoBackups:{name:string;size:number;modified:number}[];onNativeBackup:()=>void;onRestoreNative:(name:string)=>void}){
  const update=(k:string,v:string)=>setSettings({...settings,[k]:v});
  const date=(seconds:number)=>seconds?new Intl.DateTimeFormat('fa-IR',{dateStyle:'short',timeStyle:'short'}).format(new Date(seconds*1000)):'—';
  return <><PageHeading title="تنظیمات" subtitle="اطلاعات فروشگاه، تنظیمات عمومی و حفاظت از داده‌ها"/><div className="settings-grid"><section className="surface settings-card"><h2><SlidersHorizontal size={18}/> روش قیمت‌گذاری فروش</h2><p>مدیر می‌تواند قیمت فروش را دستی وارد کند یا آن را بر اساس قیمت خرید/مصرف‌کننده و درصد سود به‌صورت خودکار محاسبه کند.</p><label className="field-label">روش تعیین قیمت فروش<select value={settings.selling_price_mode||'manual'} onChange={e=>update('selling_price_mode',e.target.value)}><option value="manual">ورود دستی توسط مدیر</option><option value="auto">محاسبه خودکار</option></select></label>{(settings.selling_price_mode||'manual')==='auto'&&<><label className="field-label">مبنای محاسبه قیمت فروش<select value={settings.selling_price_basis||'buy'} onChange={e=>update('selling_price_basis',e.target.value)}><option value="buy">قیمت خرید</option><option value="consumer">قیمت مصرف‌کننده درج‌شده روی بسته</option></select></label><label className="field-label">درصد سود روی قیمت مبنا<input type="number" min="0" max="1000" step="1" value={settings.selling_price_margin_percent??'0'} onChange={e=>update('selling_price_margin_percent',e.target.value)}/></label><p className="edit-stock-note">با ذخیره تنظیمات، قیمت فروش کالاها بر اساس قاعده جدید دوباره محاسبه می‌شود. قیمت خرید و قیمت مصرف‌کننده تغییر نمی‌کنند.</p></>}<button className="primary-btn" onClick={onSave}>ذخیره روش قیمت‌گذاری</button></section><section className="surface settings-card"><h2><Store size={18}/> اطلاعات فروشگاه</h2><label className="field-label">نام فروشگاه<input value={settings.shop_name||''} onChange={e=>update('shop_name',e.target.value)} placeholder="نام فروشگاه"/></label><label className="field-label">شماره تماس<input value={settings.shop_phone||''} onChange={e=>update('shop_phone',e.target.value)} placeholder="شماره تماس"/></label><label className="field-label">نشانی<input value={settings.shop_address||''} onChange={e=>update('shop_address',e.target.value)} placeholder="نشانی فروشگاه"/></label><label className="field-label">شناسه مالیاتی<input value={settings.shop_tax||''} onChange={e=>update('shop_tax',e.target.value)} placeholder="اختیاری"/></label><label className="field-label">قالب کاغذ فاکتور<select value={settings.print_paper||'A4'} onChange={e=>update('print_paper',e.target.value)}><option value="A4">A4</option><option value="A5">A5</option><option value="80mm">حرارتی ۸۰ میلی‌متر</option><option value="58mm">حرارتی ۵۸ میلی‌متر</option></select></label><button className="primary-btn" onClick={onSave}>ذخیره تنظیمات</button></section><section className="surface settings-card"><h2><ShieldIcon/> پشتیبان‌گیری و بازیابی</h2><p>پشتیبان SQLite روزانه به‌صورت محلی ساخته می‌شود؛ حداکثر ۳۰ نسخه نگه‌داری و فایل‌ها با دسترسی محدود ذخیره می‌شوند. بازیابی نسخه محلی پیش از جایگزینی، یک نسخه ایمنی می‌سازد.</p><div className="backup-status"><b>پشتیبان‌های محلی: {autoBackups.length}</b><small>{autoBackups[0]?('آخرین نسخه: '+date(autoBackups[0].modified)+' · '+(autoBackups[0].size/1024/1024).toFixed(2)+' MB'):"هنوز پشتیبان خودکار ثبت نشده"}</small></div><button className="secondary-btn wide" onClick={onNativeBackup}><ShieldIcon/> ایجاد پشتیبان SQLite فوری</button><button className="secondary-btn wide" onClick={onBackup}><Download size={16}/> خروجی JSON قابل انتقال</button><label className="secondary-btn wide file-label"><Upload size={16}/> بازیابی از فایل پشتیبان JSON<input type="file" accept=".json,application/json" onChange={e=>onRestore(e.target.files?.[0])}/></label>{autoBackups.length>0&&<div className="backup-list"><b>نسخه‌های محلی قابل بازیابی</b>{autoBackups.map(item=><div className="backup-row" key={item.name}><span><strong>{date(item.modified)}</strong><small>{item.name} · {(item.size/1024/1024).toFixed(2)} MB</small></span><button className="secondary-btn" onClick={()=>onRestoreNative(item.name)}>بازیابی</button></div>)}</div>}</section></div></>;
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

function InvoiceDraftsPanel({drafts,onResume,onDelete}:{drafts:InvoiceDraft[];onResume:(id:number)=>void;onDelete:(id:number)=>void}){
 if(!drafts.length)return null;
 return <section className="surface invoice-drafts"><div className="surface-heading"><div><h2>پیش‌نویس‌های ذخیره‌شده</h2><p>فاکتورهایی که هنوز نهایی نشده‌اند</p></div><span>{money(drafts.length)} پیش‌نویس</span></div><div className="draft-list">{drafts.map(d=><div className="draft-row" key={d.id}><div><b>{d.kind==='sale'?'پیش‌نویس فروش':'پیش‌نویس خرید'}{d.title?' · '+d.title:''}</b><small>آخرین ذخیره: {formatJalaliDate(d.updated_at.slice(0,10))}</small></div><div><button className="secondary-btn" onClick={()=>onResume(d.id)}>ادامه فاکتور</button><button className="icon-btn" aria-label="حذف پیش‌نویس" title="حذف پیش‌نویس" onClick={()=>onDelete(d.id)}><Trash2 size={16}/></button></div></div>)}</div></section>;
}
function InvoiceDetailModal({detail,onClose,onPrint,onReturn,onPay,onVoid}:{detail:InvoiceDetail;onClose:()=>void;onPrint:()=>void;onReturn:()=>void;onPay:()=>void;onVoid:()=>void}){
 const inv=detail.invoice;
 const outstanding=Math.max(0,inv.total-inv.paid);
 const returnable=detail.items.some(x=>(inv.kind==='sale'||x.product_id!==null)&&x.quantity-(inv.kind==='sale'?x.sales_returned:x.purchase_returned)>0);
 const method=(v:string)=>v==='cash'?'نقدی':v==='card'?'کارت‌خوان':v==='transfer'?'انتقال بانکی':v||'—';
 return <div className="modal-backdrop invoice-detail-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)onClose()}}><section className="entry-modal invoice-detail-modal">
  <div className="modal-heading"><div><span className="eyebrow">{inv.kind==='sale'?'فاکتور فروش':'فاکتور خرید'}</span><h2>شماره {money(inv.number)}</h2><small>{inv.contact_name} · {formatJalaliDate(inv.invoice_date)}</small></div><button className="icon-btn" onClick={onClose}><X size={20}/></button></div>
  {inv.voided&&<div className="error-banner">این فاکتور باطل شده است؛ عملیات مالی جدید روی آن مجاز نیست.</div>}
  <div className="invoice-detail-summary"><div><span>جمع اولیه</span><b>{money(inv.subtotal)} تومان</b></div><div><span>تخفیف</span><b>{money(inv.discount)} تومان</b></div><div><span>مبلغ نهایی</span><b>{money(inv.total)} تومان</b></div><div><span>تسویه‌شده</span><b>{money(inv.paid)} تومان</b></div><div><span>مانده</span><b>{money(outstanding)} تومان</b></div><div><span>اعتبار اعمال‌شده</span><b>{money(inv.credit_applied)} تومان</b></div></div>
  <div className="surface invoice-detail-section"><div className="surface-heading"><div><h3>اقلام فاکتور</h3><p>{money(detail.items.length)} ردیف</p></div></div><div className="table-wrap"><table className="data-table"><thead><tr><th>شرح</th><th>تعداد</th><th>قیمت واحد</th><th>مبلغ</th><th>برگشتی</th></tr></thead><tbody>{detail.items.map(x=><tr key={x.id}><td>{x.item_name}</td><td>{money(x.quantity)}</td><td>{money(x.unit_price)}</td><td>{money(x.line_total)}</td><td>{money(inv.kind==='sale'?x.sales_returned:x.purchase_returned)}</td></tr>)}</tbody></table></div></div>
  <div className="surface invoice-detail-section"><div className="surface-heading"><div><h3>دریافت‌ها و پرداخت‌های تخصیص‌یافته</h3><p>{money(detail.payments.length)} تراکنش</p></div></div><div className="table-wrap"><table className="data-table"><thead><tr><th>تاریخ</th><th>مبلغ</th><th>روش</th><th>یادداشت</th></tr></thead><tbody>{detail.payments.map(p=><tr key={p.id}><td>{formatJalaliDate(p.date)}</td><td>{money(p.allocated)} تومان</td><td>{method(p.method)}</td><td>{p.notes||'—'}</td></tr>)}{!detail.payments.length&&<tr><td colSpan={4} className="empty-cell">پرداختی به این فاکتور تخصیص نیافته است.</td></tr>}</tbody></table></div></div>
  {(detail.returns.length>0||detail.refunds.length>0||detail.void)&&<div className="surface invoice-detail-section"><div className="surface-heading"><div><h3>سوابق برگشت و ابطال</h3><p>سوابق غیرقابل حذف سند</p></div></div><div className="detail-audit-list">{detail.returns.map(x=><p key={'r'+x.id}>برگشت {x.kind==='sale'?'فروش':'خرید'} · {formatJalaliDate(x.date)} · {money(x.amount)} تومان</p>)}{detail.refunds.map(x=><p key={'f'+x.id}>بازپرداخت {method(x.method)} · {formatJalaliDate(x.date)} · {money(x.amount)} تومان</p>)}{detail.void&&<p>ابطال سند · {formatJalaliDate(detail.void.date)} · بازپرداخت نقدی {money(detail.void.cash_refund)} تومان</p>}</div></div>}
  {inv.notes&&<p className="invoice-notes">یادداشت: {inv.notes}</p>}
  <div className="modal-actions invoice-detail-actions"><button className="secondary-btn" onClick={onClose}>بستن</button><button className="secondary-btn" onClick={onPrint}><Printer size={15}/> چاپ</button>{!inv.voided&&outstanding>0&&inv.contact_id!==null&&<button className="secondary-btn" onClick={onPay}><Wallet size={15}/> ثبت تسویه</button>}{!inv.voided&&returnable&&<button className="secondary-btn" onClick={onReturn}>ثبت برگشت</button>}{!inv.voided&&<button className="danger-btn" onClick={onVoid}>ابطال فاکتور</button>}</div>
 </section></div>;
}
function InvoiceReturnModal({detail,onClose,onSubmit}:{detail:InvoiceDetail;onClose:()=>void;onSubmit:(method:string,lines:{invoice_item_id:number;item:string;qty:number}[])=>Promise<void>}){
 const [quantities,setQuantities]=useState<Record<number,string>>({});
 const [method,setMethod]=useState('cash');
 const [busy,setBusy]=useState(false);
 const inv=detail.invoice;
 const rows=detail.items.filter(x=>(inv.kind==='sale'||x.product_id!==null)&&x.quantity-(inv.kind==='sale'?x.sales_returned:x.purchase_returned)>0);
 const submit=async()=>{
  const entered=rows.filter(x=>(quantities[x.id]||'').trim()!=='').map(x=>({
   invoice_item_id:x.id,item:x.code||x.barcode||x.item_name,
   qty:Number(latinDigits(quantities[x.id]||'0')),
   remaining:x.quantity-(inv.kind==='sale'?x.sales_returned:x.purchase_returned)
  }));
  if(!entered.length){window.alert('حداقل تعداد برگشتی یک ردیف را وارد کن.');return}
  if(entered.some(x=>!Number.isSafeInteger(x.qty)||x.qty<=0)){window.alert('تعداد برگشتی باید عدد صحیح مثبت باشد.');return}
  if(entered.some(x=>x.qty>x.remaining)){window.alert('تعداد برگشتی از مقدار باقی‌ماندهٔ ردیف بیشتر است.');return}
  const selected=entered.map(x=>({invoice_item_id:x.invoice_item_id,item:x.item,qty:x.qty}));
  setBusy(true);try{await onSubmit(method,selected)}finally{setBusy(false)}
 };
 return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget&&!busy)onClose()}}><section className="entry-modal invoice-action-modal"><div className="modal-heading"><div><span className="eyebrow">{inv.kind==='sale'?'برگشت از فروش':'مرجوعی خرید'}</span><h2>فاکتور شماره {money(inv.number)}</h2></div><button className="icon-btn" disabled={busy} onClick={onClose}><X size={20}/></button></div><p className="invoice-payment-hint">ردیف‌های کالایی و خدماتی فروش قابل برگشت‌اند؛ برگشت خدمت فقط مبلغ فاکتور و بازپرداخت را اصلاح می‌کند و اثری بر انبار ندارد. مرجوعی خرید فقط برای کالاهای انباری است.</p><div className="return-line-list">{rows.map(x=>{const returned=inv.kind==='sale'?x.sales_returned:x.purchase_returned;const remaining=x.quantity-returned;return <label className="return-line" key={x.id}><span><b>{x.item_name}</b><small>{x.product_id===null?'خدمت · بدون اثر بر موجودی':'کالا · دارای گردش انبار'} · اولیه {money(x.quantity)} · قبلاً برگشته {money(returned)} · قابل برگشت {money(remaining)}</small></span><input aria-label={'تعداد برگشتی '+x.item_name} type="number" min="0" max={remaining} value={quantities[x.id]||''} placeholder="۰" onChange={e=>setQuantities(old=>({...old,[x.id]:e.target.value}))}/></label>})}{!rows.length&&<p className="empty-cell">ردیف قابل برگشتی برای این فاکتور باقی نمانده است.</p>}</div><label className="field-label">روش بازپرداخت نقدی<select value={method} onChange={e=>setMethod(e.target.value)}><option value="cash">نقدی</option><option value="card">کارت‌خوان</option><option value="transfer">انتقال بانکی</option></select></label><div className="modal-actions"><button className="secondary-btn" disabled={busy} onClick={onClose}>انصراف</button><button className="primary-btn" disabled={busy||!rows.length} onClick={submit}>ثبت برگشت</button></div></section></div>;
}
function InvoicePaymentModal({detail,onClose,onSubmit}:{detail:InvoiceDetail;onClose:()=>void;onSubmit:(p:{amount:number;method:string;date:string;notes:string})=>Promise<void>}){
 const inv=detail.invoice;const outstanding=Math.max(0,inv.total-inv.paid);
 const [amount,setAmount]=useState(String(outstanding));const [method,setMethod]=useState('cash');const [date,setDate]=useState(isoToday);const [notes,setNotes]=useState('');const [busy,setBusy]=useState(false);
 const submit=async()=>{const value=Number(latinDigits(amount).replace(/,/g,''));if(!Number.isSafeInteger(value)||value<=0||value>outstanding){window.alert('مبلغ باید مثبت و حداکثر برابر ماندهٔ فاکتور باشد.');return}setBusy(true);try{await onSubmit({amount:value,method,date,notes})}finally{setBusy(false)}};
 return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget&&!busy)onClose()}}><form className="entry-modal invoice-action-modal" onSubmit={e=>{e.preventDefault();void submit()}}><div className="modal-heading"><div><span className="eyebrow">تخصیص مستقیم پرداخت</span><h2>فاکتور شماره {money(inv.number)}</h2></div><button type="button" className="icon-btn" disabled={busy} onClick={onClose}><X size={20}/></button></div><div className="invoice-payment-balance"><span>مانده قابل تسویه</span><b>{money(outstanding)} تومان</b></div><label className="field-label">مبلغ پرداخت<input type="number" min="1" max={outstanding} disabled={busy} value={amount} onChange={e=>setAmount(e.target.value)}/></label><div className="form-two"><label className="field-label">روش پرداخت<select disabled={busy} value={method} onChange={e=>setMethod(e.target.value)}><option value="cash">نقدی</option><option value="card">کارت‌خوان</option><option value="transfer">انتقال بانکی</option></select></label><JalaliDateField label="تاریخ پرداخت" value={date} onChange={setDate} todayValue={isoToday}/></div><label className="field-label">یادداشت<input disabled={busy} value={notes} onChange={e=>setNotes(e.target.value)} placeholder="اختیاری"/></label><div className="modal-actions"><button type="button" className="secondary-btn" disabled={busy} onClick={onClose}>انصراف</button><button type="submit" className="primary-btn" disabled={busy}>{busy?'در حال ثبت…':'ثبت پرداخت'}</button></div></form></div>;
}

function EntryModal({entry,products,settings,form,setField,lines,setLines,onClose,onSubmit,onSaveDraft,hasDraft,busy,editing}:{entry:Entry;products:Product[];settings:Record<string,string>;form:Record<string,string>;setField:(k:string,v:string)=>void;lines:{item:string;qty:string;price:string}[];setLines:(v:{item:string;qty:string;price:string}[])=>void;onClose:()=>void;onSubmit:(e:FormEvent)=>void;onSaveDraft:()=>void;hasDraft:boolean;busy:boolean;editing:boolean}){
 const [simpleSale,setSimpleSale]=useState(true);
 const [showSaleDetails,setShowSaleDetails]=useState(false);
 const [productQuery,setProductQuery]=useState('');
 const productSearchRef=useRef<HTMLInputElement>(null);
 const quickProducts=products.filter(p=>p.is_active&&!p.is_service&&(!productQuery.trim()||[p.name,p.code,p.barcode,p.category].some(v=>(v||'').toLocaleLowerCase().includes(productQuery.trim().toLocaleLowerCase())))).slice(0,30);
 const addQuickProduct=(product:Product)=>{const existing=lines.findIndex(x=>x.item===product.name);if(existing>=0){setLines(lines.map((x,i)=>i===existing?{...x,qty:String((Number(x.qty)||0)+1)}:x));}else{const firstEmpty=lines.findIndex(x=>!x.item.trim());const next={item:product.name,qty:'1',price:String(productFinalPrice(product))};setLines(firstEmpty>=0?lines.map((x,i)=>i===firstEmpty?next:x):[...lines,next]);}setProductQuery('');productSearchRef.current?.focus();};
 const handleSimpleSaleKeys=(e:KeyboardEvent<HTMLFormElement>)=>{if(entry!=='sale'||!simpleSale)return;const key=e.key.toLowerCase();if((e.ctrlKey||e.metaKey)&&key==='k'){e.preventDefault();productSearchRef.current?.focus();productSearchRef.current?.select();return;}if(e.key==='F2'){e.preventDefault();productSearchRef.current?.focus();productSearchRef.current?.select();return;}if(e.key==='F4'){e.preventDefault();setField('status','paid');setField('method','cash');return;}if(e.key==='F6'){e.preventDefault();setField('status','paid');setField('method','card');return;}if(e.key==='F8'){e.preventDefault();setField('status','credit');setShowSaleDetails(true);return;}if(e.key==='F9'||(e.ctrlKey&&e.key==='Enter')){e.preventDefault();e.currentTarget.requestSubmit();return;}if(e.key==='Escape'){e.preventDefault();onClose();}};
 const titles:Record<Entry,string>={contact:editing?'ویرایش طرف‌حساب':'ثبت طرف‌حساب جدید',product:editing?'ویرایش کالا':'ثبت کالای جدید',sale:'فاکتور فروش جدید',purchase:'فاکتور خرید جدید',receipt:'ثبت دریافت وجه',payment:'ثبت پرداخت وجه'};
 const field=(key:string,label:string,placeholder='',type='text')=><label className="field-label">{label}<input type={type} value={form[key]||''} onChange={e=>setField(key,e.target.value)} placeholder={placeholder}/></label>;
 const invoiceSubtotal=lines.reduce((sum,line)=>sum+Math.max(0,Number(line.qty)||0)*Math.max(0,Number(line.price)||0),0);
 const invoiceDiscount=Math.max(0,Number(form.discount)||0);
 const invoiceTotal=Math.max(0,invoiceSubtotal-invoiceDiscount);
 const invoicePaid=entry==='sale'?(form.status==='paid'?invoiceTotal:form.status==='partial'?Math.max(0,Number(form.paid)||0):0):0;
 const invoiceRemaining=Math.max(0,invoiceTotal-invoicePaid);
 const autoPricing=(settings.selling_price_mode||'manual')==='auto';
 const pricingBasis=(settings.selling_price_basis||'buy')==='consumer'?Number(form.consumer_price||0):Number(form.buy||0);
 const pricingMargin=Math.max(0,Number(settings.selling_price_margin_percent||0));
 const suggestedSell=Math.round(pricingBasis*(100+pricingMargin)/100);
  const updateInvoiceItem=(index:number,value:string)=>{const normalized=value.startsWith('خدمت:')?value.slice(6).trim():value;const found=products.find(p=>p.is_active&&(p.name.toLocaleLowerCase()===normalized.toLocaleLowerCase()||p.code===normalized||p.barcode===normalized)&&((entry==='purchase'&&!p.is_service)||entry==='sale'));setLines(lines.map((line,i)=>i===index?{...line,item:value,price:found?String(entry==='purchase'?found.buy_price:productFinalPrice(found)):line.price}:line));};
 return <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)onClose()}}><form className={"entry-modal "+(entry==="sale"&&simpleSale?"simple-sale-modal":"")} onSubmit={onSubmit} onKeyDown={handleSimpleSaleKeys}><div className="modal-heading"><div><span className="eyebrow">{entry==='sale'&&simpleSale?'فروش سریع':'فرم ثبت اطلاعات'}</span><h2>{titles[entry]}</h2></div><button type="button" className="icon-btn" onClick={onClose}><X size={20}/></button></div>
  <div className="modal-fields">
   {entry==='contact'&&<>{field('name','نام طرف‌حساب','نام و نام خانوادگی یا شرکت')}<div className="form-two">{<label className="field-label">نوع طرف‌حساب<select value={form.kind} onChange={e=>setField('kind',e.target.value)}><option value="customer">مشتری</option><option value="supplier">تأمین‌کننده</option><option value="both">هر دو</option></select></label>}{field('phone','شماره تماس','اختیاری')}</div>{field('balance','مانده افتتاحیه','۰','number')}{!editing&&field('notes','یادداشت','توضیحات تکمیلی')}</>}
   {entry==='product'&&<>{field('name','نام کالا / خدمت','مثلاً کابل افشان')}<div className="form-two">{field('code','کد کالا','SKU-001')}{field('barcode','بارکد','اختیاری')}</div><div className="form-two">{field('category','دسته‌بندی','مثلاً سیم و کابل')}{field('unit','واحد شمارش','عدد، متر، حلقه، بسته...')}</div><div className="form-three">{field('qty','موجودی اولیه','۰','number')}{field('buy','قیمت خرید','۰','number')}{field('consumer_price','قیمت مصرف‌کننده روی بسته','۰','number')}</div><label className="field-label">قیمت فروش {autoPricing?'(محاسبه خودکار)':'(ورود دستی مدیر)'}<input type="number" min="0" value={autoPricing?String(suggestedSell):form.sell||''} disabled={autoPricing} onChange={e=>setField('sell',e.target.value)} placeholder="۰"/></label>{autoPricing&&<p className="edit-stock-note">قیمت پیشنهادی بر اساس {pricingBasis===Number(form.consumer_price||0)&&settings.selling_price_basis==='consumer'?'قیمت مصرف‌کننده':'قیمت خرید'} و {money(pricingMargin)}٪ سود محاسبه شده است.</p>}<div className="form-three"><label className="field-label">نوع تخفیف کالا<select value={form.discount_type||'none'} onChange={e=>setField('discount_type',e.target.value)}><option value="none">بدون تخفیف</option><option value="percent">درصدی</option><option value="fixed">مبلغ ثابت</option></select></label>{form.discount_type!=='none'&&field('discount_value',form.discount_type==='percent'?'درصد تخفیف':'مبلغ تخفیف (تومان)','۰','number')}{field('wholesale','قیمت عمده','۰','number')}</div><div className="form-two">{field('min','حداقل موجودی هشدار','۰','number')}<span className="edit-stock-note">قیمت فروش مبنای فاکتور فروش و تخفیف اختصاصی کالا است.</span></div><label className="field-label service-toggle"><input type="checkbox" checked={form.is_service==='true'} onChange={e=>{setField('is_service',e.target.checked?'true':'false');if(e.target.checked)setField('qty','0')}}/> این مورد خدمت است و موجودی انبار ندارد</label>{form.is_service==='true'&&<p className="edit-stock-note">خدمت در ارزش موجودی و گردش انبار محاسبه نمی‌شود. در فاکتور، عنوان آن را با پیشوند «خدمت:» وارد کن.</p>}{editing&&<p className="edit-stock-note">برای حفظ سوابق، موجودی کالای دارای فاکتور از این فرم تغییر نمی‌کند؛ اصلاح موجودی را از عملیات انبار انجام بده.</p>}</>}
   {(entry==='sale'||entry==='purchase')&&<>
    {entry==='sale'&&simpleSale?<section className="simple-sale">
      <div className="simple-sale-top">
       <div><b>برای فروش، فقط کالاها را انتخاب کن</b><p>تعداد و مبلغ به‌صورت خودکار حساب می‌شود.</p></div>
       <button type="button" className="secondary-btn" onClick={()=>setSimpleSale(false)}>حالت کامل</button>
      </div>
      <div className="simple-sale-search">
       <label htmlFor="simple-sale-product-search">جست‌وجوی کالا</label>
       <div className="simple-sale-searchbox"><Search size={19}/><input id="simple-sale-product-search" ref={productSearchRef} autoFocus placeholder="نام کالا، کد یا دسته‌بندی..." value={productQuery} onChange={e=>setProductQuery(e.target.value)} onKeyDown={e=>{if(e.key==='Enter'&&quickProducts.length){e.preventDefault();e.stopPropagation();addQuickProduct(quickProducts[0]);}if(e.key==='ArrowDown'&&quickProducts.length){e.preventDefault();(document.querySelector('.simple-product-tile') as HTMLButtonElement|null)?.focus();}}}/><kbd>F2</kbd></div>
       <div className="simple-sale-shortcuts"><span><kbd>Ctrl</kbd>+<kbd>K</kbd> جست‌وجو</span><span><kbd>F4</kbd> نقدی</span><span><kbd>F6</kbd> کارت‌خوان</span><span><kbd>F8</kbd> نسیه</span><span><kbd>F9</kbd> ثبت</span></div>
      </div>
      <div className="simple-product-grid" aria-label="انتخاب سریع کالا">
       {quickProducts.map((product,i)=><button type="button" className="simple-product-tile" key={product.id} onClick={()=>addQuickProduct(product)} onKeyDown={e=>{if(e.key==='ArrowRight'||e.key==='ArrowDown'){e.preventDefault();const tiles=Array.from(document.querySelectorAll<HTMLButtonElement>('.simple-product-tile'));tiles[Math.min(i+1,tiles.length-1)]?.focus();}if(e.key==='ArrowLeft'||e.key==='ArrowUp'){e.preventDefault();if(i===0)productSearchRef.current?.focus();else{const tiles=Array.from(document.querySelectorAll<HTMLButtonElement>('.simple-product-tile'));tiles[i-1]?.focus();}}}}><b>{product.name}</b><span>{money(productFinalPrice(product))} تومان</span><small>{product.unit||'عدد'}{product.quantity<=product.min_quantity?' · موجودی کم':''}</small></button>)}
       {!quickProducts.length&&<p className="simple-no-products">{products.some(p=>p.is_active&&!p.is_service)?'کالایی با این عبارت پیدا نشد. عبارت دیگری جست‌وجو کن.':'هنوز کالای فعالی ثبت نشده است.'}</p>}
      </div>
      <div className="simple-sale-items">
       <div className="simple-cart-heading"><b>سبد فروش</b><span>{money(lines.filter(x=>x.item.trim()).length)} ردیف</span></div>
       {lines.filter(line=>line.item.trim()).map((line)=>{const i=lines.indexOf(line);return <div className="simple-sale-item" key={i}>
        <div className="simple-sale-product"><b>{line.item}</b><span>{money(Number(line.price)||0)} تومان · هر {products.find(p=>p.name===line.item)?.unit||'عدد'}</span></div>
        <div className="simple-sale-item-controls"><div className="simple-qty"><button type="button" aria-label="افزایش تعداد" onClick={()=>setLines(lines.map((x,j)=>j===i?{...x,qty:String((Number(x.qty)||0)+1)}:x))}>+</button><b>{money(Number(line.qty)||1)}</b><button type="button" aria-label="کاهش تعداد" disabled={Number(line.qty)<=1} onClick={()=>setLines(lines.map((x,j)=>j===i?{...x,qty:String(Math.max(1,(Number(x.qty)||1)-1))}:x))}>−</button></div><button type="button" className="simple-remove" aria-label="حذف کالا" onClick={()=>setLines(lines.filter((_,j)=>j!==i))}>حذف</button></div>
       </div>})}
       {!lines.some(x=>x.item.trim())&&<p className="simple-cart-empty">برای شروع، یک کالا را از فهرست بالا انتخاب کن.</p>}
      </div>
      <div className="simple-sale-total">
       <div><span>تعداد اقلام</span><b>{money(lines.filter(x=>x.item.trim()).length)} قلم</b></div>
       {invoiceDiscount>0&&<div><span>تخفیف فاکتور</span><b>{money(invoiceDiscount)} تومان</b></div>}
       <div className="simple-grand-total"><span>مبلغ قابل پرداخت</span><strong>{money(invoiceTotal)} <small>تومان</small></strong></div>
      </div>
      <div className="simple-payment">
       <span>روش پرداخت</span>
       <div className="simple-payment-options">
        <button type="button" className={form.status==='paid'&&form.method==='cash'?'selected':''} onClick={()=>{setField('status','paid');setField('method','cash')}}>نقدی</button>
        <button type="button" className={form.status==='paid'&&form.method==='card'?'selected':''} onClick={()=>{setField('status','paid');setField('method','card')}}>کارت‌خوان</button>
        <button type="button" className={form.status==='credit'?'selected':''} onClick={()=>{setField('status','credit');setShowSaleDetails(true)}}>نسیه</button>
       </div>
      </div>
      <button type="button" className="simple-details-toggle" disabled={form.status==='credit'} onClick={()=>setShowSaleDetails(!showSaleDetails)}>{showSaleDetails?'بستن جزئیات بیشتر':'افزودن نام مشتری یا تخفیف فاکتور'} <span>{showSaleDetails?'−':'＋'}</span></button>
      {showSaleDetails&&<div className="simple-sale-details">
       {field('customer',form.status==='credit'?'نام مشتری ثبت‌شده (برای نسیه الزامی)':'نام مشتری',form.status==='credit'?'نام مشتری ثبت‌شده را دقیق وارد کن':'برای فروش نقدی گذری خالی بگذار')}
       <div className="form-two">{field('discount','تخفیف کل فاکتور','۰','number')}<JalaliDateField label="تاریخ فاکتور" value={form.date||isoToday} onChange={v=>setField('date',v)} todayValue={isoToday}/></div>
       {field('notes','یادداشت','اختیاری')}
      </div>}
      <p className="simple-sale-footnote">تاریخ فاکتور به‌صورت خودکار امروز ثبت می‌شود. برای اطلاعات و تنظیمات بیشتر، «حالت کامل» را انتخاب کن.</p>
     </section>:<>
      {entry==='sale'&&<div className="invoice-mode-switch"><span>فرم کامل فروش</span><button type="button" className="text-btn" onClick={()=>setSimpleSale(true)}>بازگشت به حالت ساده</button></div>}
      {entry==='sale'&&field('customer','نام مشتری','برای فروش نقدی گذری خالی بگذار')}
      {entry==='purchase'&&field('customer','نام تأمین‌کننده','نام تأمین‌کننده')}
      <div className="invoice-lines-title"><b>اقلام فاکتور</b><button type="button" className="text-btn" onClick={()=>setLines([...lines,{item:'',qty:'1',price:'0'}])}><Plus size={15}/> افزودن ردیف</button></div>
      {lines.map((line,i)=><div className="invoice-line" key={i}><input aria-label="نام کالا یا خدمت" list={"invoice-products-"+i} placeholder="نام، کد یا بارکد کالا / خدمت" value={line.item} onChange={e=>updateInvoiceItem(i,e.target.value)} onKeyDown={e=>{if(e.key==="Enter"){e.preventDefault();e.currentTarget.closest(".invoice-line")?.querySelector<HTMLInputElement>('input[aria-label="تعداد"]')?.focus()}}}/><datalist id={"invoice-products-"+i}>{products.filter(p=>p.is_active&&(entry==="sale"||!p.is_service)).map(p=><option key={p.id} value={p.is_service?"خدمت:"+(p.barcode||p.code):(p.barcode||p.code)} label={p.name+" · "+p.code}/>)}</datalist><input aria-label="تعداد" type="number" min="1" value={line.qty} onChange={e=>setLines(lines.map((x,j)=>j===i?{...x,qty:e.target.value}:x))}/><input aria-label="قیمت واحد" type="number" min="0" value={line.price} onChange={e=>setLines(lines.map((x,j)=>j===i?{...x,price:e.target.value}:x))}/><button type="button" className="icon-btn" aria-label="حذف ردیف" disabled={lines.length===1} title={lines.length===1?'فاکتور باید حداقل یک ردیف داشته باشد':'حذف ردیف'} onClick={()=>setLines(lines.filter((_,j)=>j!==i))}><X size={16}/></button></div>)}
      <div className="form-two">{field('discount','تخفیف','۰','number')}<JalaliDateField label="تاریخ فاکتور" value={form.date||isoToday} onChange={v=>setField('date',v)} todayValue={isoToday}/></div>
      <div className="invoice-totals"><div><span>جمع ردیف‌ها</span><b>{money(invoiceSubtotal)} تومان</b></div><div><span>تخفیف</span><b>{money(invoiceDiscount)} تومان</b></div><div className="invoice-grand-total"><span>مبلغ نهایی</span><b>{money(invoiceTotal)} تومان</b></div>{entry==='sale'&&<><div><span>مبلغ مبنای تسویه</span><b>{money(invoicePaid)} تومان</b></div><div className="invoice-remaining"><span>مانده پیش از اعمال اعتبار</span><b>{money(invoiceRemaining)} تومان</b></div></>}</div>
      {entry==='sale'&&<><div className="form-two"><label className="field-label">وضعیت پرداخت<select value={form.status} onChange={e=>setField('status',e.target.value)}><option value="paid">تسویه کامل</option><option value="partial">پرداخت جزئی</option><option value="credit">نسیه</option></select></label>{(form.status==='paid'||form.status==='partial')&&<label className="field-label">روش دریافت<select value={form.method||'cash'} onChange={e=>setField('method',e.target.value)}><option value="cash">نقدی</option><option value="card">کارت‌خوان</option><option value="transfer">انتقال بانکی</option></select></label>}</div>{form.status==='partial'&&field('paid','مبلغ دریافتی','۰','number')}</>}
      {entry==='purchase'&&<p className="invoice-payment-hint">پرداخت به تأمین‌کننده را پس از ثبت فاکتور از بخش «دریافت و پرداخت ← ثبت پرداخت» ثبت کن.</p>}
      {field('notes','یادداشت','اختیاری')}
     </>}
    </>}
   {(entry==='receipt'||entry==='payment')&&<>{field('customer','نام طرف‌حساب','ابتدا طرف‌حساب را ثبت کن')}{field('amount','مبلغ','۰','number')}<div className="form-two"><label className="field-label">روش پرداخت<select value={form.method} onChange={e=>setField('method',e.target.value)}><option value="cash">نقدی</option><option value="card">کارت</option><option value="transfer">انتقال بانکی</option></select></label><JalaliDateField label="تاریخ تراکنش" value={form.date||isoToday} onChange={v=>setField('date',v)} todayValue={isoToday}/></div>{field('notes','یادداشت','اختیاری')}</>}
  </div><div className="modal-actions"><button type="button" className="secondary-btn" onClick={onClose}>انصراف</button>{(entry==='purchase'||(entry==='sale'&&!simpleSale))&&<button type="button" className="secondary-btn" onClick={onSaveDraft} disabled={busy}><Download size={15}/>{hasDraft?'به‌روزرسانی پیش‌نویس':'ذخیره پیش‌نویس'}</button>}<button type="submit" className="primary-btn" disabled={busy}>{busy?'در حال ثبت...':entry==='sale'&&simpleSale?'ثبت فروش':entry==='sale'?'ثبت فاکتور فروش':entry==='purchase'?'ثبت فاکتور خرید':'ثبت اطلاعات'}</button></div>
 </form></div>;
}
export default App;
