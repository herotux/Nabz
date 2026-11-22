import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Activity, BarChart3, Download } from 'lucide-react';
import { JalaliDateField } from '../forms/JalaliDateField';
import { PageHeading } from '../layout/PagePrimitives';
import { formatJalaliDate } from '../../utils/jalali';
import type { RecordRow } from '../../types';

type ReportTrend = {date:string;total:number};
type ReportProduct = {name:string;code:string;quantity:number};
type HistoricalCostCandidate = {invoice_item_id:number;invoice_number:number;date:string;item_name:string;quantity:number;returned_quantity:number;unit_price:number;line_total:number;proposed_unit_cost:number|null;eligible:boolean;reason:string|null};
type ReportData = {start_date:string;end_date:string;sales_total:number;purchases_total:number;receipts_total:number;payments_total:number;sales_returns_total:number;purchase_returns_total:number;sales_count:number;purchases_count:number;cost_of_goods_sold:number;gross_profit:number;expenses_total:number;net_profit:number;uncosted_sales:number;inventory_value:number;customer_balance:number;supplier_balance:number;low_stock_count:number;daily_sales:ReportTrend[];top_products:ReportProduct[];expenses_are_store_wide:boolean};
const money=(n:number)=>new Intl.NumberFormat('fa-IR').format(Number.isFinite(n)?n:0);
const localIso=(date:Date)=>date.getFullYear()+'-'+String(date.getMonth()+1).padStart(2,'0')+'-'+String(date.getDate()).padStart(2,'0');
const isoToday=localIso(new Date());
function reportCsvCell(value:string|number){
 const raw=String(value??'');
 const safe=/^[=+@\t\r\n-]/.test(raw)?"'"+raw:raw;
 return '"'+safe.replace(/"/g,'""')+'"';
}
function downloadReportCsv(report:ReportData){
 const rows:(string|number)[][]=[
  ['بخش','عنوان','مقدار','واحد'],
  ['خلاصه','فروش خالص فاکتورها',report.sales_total,'تومان'],
  ['خلاصه','خرید خالص فاکتورها',report.purchases_total,'تومان'],
  ['خلاصه','دریافت خالص',report.receipts_total,'تومان'],
  ['خلاصه','پرداخت خالص',report.payments_total,'تومان'],
  ['خلاصه','برگشت فروش (اطلاعاتی)',report.sales_returns_total,'تومان'],
  ['خلاصه','مرجوعی خرید (اطلاعاتی)',report.purchase_returns_total,'تومان'],
  ['خلاصه','بهای تمام‌شده کالای فروش‌رفته',report.cost_of_goods_sold,'تومان'],
  ['خلاصه','سود ناخالص',report.gross_profit,'تومان'],
  ['خلاصه','هزینه‌های عملیاتی',report.expenses_total,'تومان'],
  ['خلاصه','سود خالص',report.net_profit,'تومان'],
  ['خلاصه','فروش فاقد بهای تاریخی',report.uncosted_sales,'تومان'],
  ['وضعیت فعلی','بدهی مشتریان',report.customer_balance,'تومان'],
  ['وضعیت فعلی','بدهی تأمین‌کنندگان',report.supplier_balance,'تومان'],
  ['وضعیت فعلی','ارزش موجودی',report.inventory_value,'تومان'],
  ['کالای پرفروش','نام کالا','کد','تعداد خالص'],
  ...report.top_products.map(p=>['کالای پرفروش',p.name,p.code,p.quantity])
 ];
 const csv='\uFEFF'+rows.map(row=>row.map(reportCsvCell).join(',')).join('\r\n');
 const url=URL.createObjectURL(new Blob([csv],{type:'text/csv;charset=utf-8'}));
 const anchor=document.createElement('a');anchor.href=url;anchor.download='nabz-report-'+report.start_date+'-'+report.end_date+'.csv';anchor.click();URL.revokeObjectURL(url);
}

export function ReportsPage(){
 const [start,setStart]=useState(()=>{const d=new Date();d.setDate(d.getDate()-29);return localIso(d)});
 const [end,setEnd]=useState(isoToday);
 const [contactId,setContactId]=useState('');
 const [contacts,setContacts]=useState<RecordRow[]>([]);
 const [report,setReport]=useState<ReportData|null>(null);
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState('');
 const [costCandidates,setCostCandidates]=useState<HistoricalCostCandidate[]>([]);
 const [costRepairBusy,setCostRepairBusy]=useState(false);
 const [costRepairMessage,setCostRepairMessage]=useState('');
 const loadCostCandidates=async()=>{
  setCostRepairBusy(true);setCostRepairMessage('');
  try{setCostCandidates(await invoke<HistoricalCostCandidate[]>('historical_cost_repair_preview'))}
  catch(e){setCostRepairMessage('دریافت فهرست بهای تاریخی ناموفق بود: '+String(e))}
  finally{setCostRepairBusy(false)}
 };
 const applyCostRepairs=async()=>{
  const ids=costCandidates.filter(x=>x.eligible).map(x=>x.invoice_item_id);
  if(!ids.length)return;
  if(!window.confirm('بهای تاریخی '+money(ids.length)+' ردیف بر اساس میانگین موزون ذخیره‌شده پیش از فروش ثبت شود؟ این عملیات در سابقهٔ اصلاحات ثبت می‌شود و قابل برگشت خودکار نیست.'))return;
  setCostRepairBusy(true);setCostRepairMessage('');
  try{
   const result=await invoke<{repaired:number}>('apply_historical_cost_repairs',{invoiceItemIds:ids});
   setCostRepairMessage(result.repaired+' ردیف با موفقیت اصلاح شد. گزارش را دوباره بارگذاری کن.');
   await loadCostCandidates();
  }catch(e){setCostRepairMessage('اصلاح انجام نشد: '+String(e))}
  finally{setCostRepairBusy(false)}
 };
 useEffect(()=>{let active=true;invoke<RecordRow[]>('list_records',{kind:'contacts'}).then(v=>{if(active)setContacts(v)}).catch(e=>{if(active)setError(String(e))});return()=>{active=false}},[]);
 useEffect(()=>{void loadCostCandidates()},[]);
 useEffect(()=>{
  let active=true;setBusy(true);setError('');
  invoke<ReportData>('reports_period',{startDate:start,endDate:end,contactId:contactId?Number(contactId):null})
   .then(v=>{if(active)setReport(v)})
   .catch(e=>{if(active){setReport(null);setError(String(e))}})
   .finally(()=>{if(active)setBusy(false)});
  return ()=>{active=false};
 },[start,end,contactId]);
 const preset=(days:number)=>{const e=new Date();const s=new Date(e);s.setDate(s.getDate()-(days-1));setStart(localIso(s));setEnd(localIso(e))};
 const thisMonth=()=>{const d=new Date();setStart(localIso(new Date(d.getFullYear(),d.getMonth(),1)));setEnd(localIso(d))};
 const value=(key:keyof ReportData)=>Number(report?.[key]??0);
 const tiles=[
  {label:'فروش خالص فاکتورها',value:value('sales_total')},
  {label:'خرید خالص فاکتورها',value:value('purchases_total')},
  {label:'دریافت خالص',value:value('receipts_total')},
  {label:'پرداخت خالص',value:value('payments_total')},
  {label:'برگشت فروش',value:value('sales_returns_total')},
  {label:'مرجوعی خرید',value:value('purchase_returns_total')},
  {label:'بهای تمام‌شده فروش',value:value('cost_of_goods_sold')},
  {label:'سود ناخالص',value:value('gross_profit')},
  {label:'هزینه‌های عملیاتی',value:value('expenses_total')},
  {label:'سود خالص',value:value('net_profit')},
  {label:'بدهی فعلی مشتریان',value:value('customer_balance')},
  {label:'بدهی فعلی تأمین‌کنندگان',value:value('supplier_balance')},
  {label:'ارزش فعلی موجودی',value:value('inventory_value')},
  {label:'کالاهای کم‌موجود',value:value('low_stock_count'),unit:'قلم'}
 ];
 const trend=report?.daily_sales||[];
 const max=Math.max(1,...trend.map(x=>x.total));
 const coords=trend.map((x,i)=>({x:trend.length<2?50:i*100/(trend.length-1),y:92-(x.total/max)*76}));
 const path=coords.map((p,i)=>(i?'L':'M')+p.x+' '+p.y).join(' ');
 return <><PageHeading title="گزارش‌ها" subtitle="گزارش مالی و عملیاتی از دفتر واقعی SQLite"><button className="secondary-btn" disabled={!report||busy} onClick={()=>report&&downloadReportCsv(report)}><Download size={15}/> خروجی CSV فارسی</button></PageHeading>
  <div className="surface report-filter report-filter-expanded">
   <JalaliDateField label="از تاریخ" value={start} onChange={setStart} todayValue={isoToday}/>
   <JalaliDateField label="تا تاریخ" value={end} onChange={setEnd} todayValue={isoToday}/>
   <label className="field-label">طرف‌حساب<select value={contactId} onChange={e=>setContactId(e.target.value)}><option value="">همه طرف‌حساب‌ها</option>{contacts.map(c=><option key={c.id} value={c.id}>{c.name} · {c.status==='supplier'?'تأمین‌کننده':c.status==='both'?'هر دو':'مشتری'}</option>)}</select></label>
   <div className="report-presets"><button className="secondary-btn" onClick={()=>preset(7)}>۷ روز</button><button className="secondary-btn" onClick={()=>preset(30)}>۳۰ روز</button><button className="secondary-btn" onClick={thisMonth}>ماه جاری</button></div>
   <div className="report-range-info">{busy?'در حال محاسبه گزارش…':error?'خطا در دریافت گزارش':report?money(report.sales_count)+' فاکتور فروش · '+money(report.purchases_count)+' فاکتور خرید':'بازه را برای دریافت گزارش انتخاب کن'}</div>
  </div>
  {error&&<div className="error-banner">{error}</div>}
  <div className="report-grid report-grid-expanded">{tiles.map(x=><div className="surface report-tile" key={x.label}><span>{x.label}</span><b>{money(x.value)} <small>{x.unit||'تومان'}</small></b></div>)}</div>
  {report&&report.uncosted_sales>0&&<div className="report-warning"><Activity size={18}/><div><b>سود ناخالص کامل نیست</b><p>برای مبلغ {money(report.uncosted_sales)} تومان از فروش کالاها، بهای خرید در زمان فروش ثبت نشده است؛ بنابراین این مبلغ در محاسبه بهای تمام‌شده و سود ناخالص وارد نشده. قیمت خرید فعلی کالا را نمی‌توان با اطمینان جایگزین قیمت تاریخی کرد.</p></div></div>}
  <section className="surface report-analysis-card historical-cost-repair">
   <div className="surface-heading"><div><h2>بررسی و تکمیل بهای تاریخی فروش</h2><p>فقط ردیف‌هایی قابل اصلاح‌اند که میانگین بهای موزون پیش از فروش در همان فاکتور ذخیره شده باشد.</p></div><button className="secondary-btn" disabled={costRepairBusy} onClick={()=>void loadCostCandidates()}>{costRepairBusy?'در حال بررسی…':'بررسی دوباره'}</button></div>
   {costRepairMessage&&<div className="report-range-info">{costRepairMessage}</div>}
   <div className="report-range-info">قابل اصلاح: {money(costCandidates.filter(x=>x.eligible).length)} ردیف · غیرقابل بازسازی مطمئن: {money(costCandidates.filter(x=>!x.eligible).length)} ردیف</div>
   <div className="table-wrap"><table className="data-table"><thead><tr><th>فاکتور / تاریخ</th><th>کالا</th><th>تعداد</th><th>بهای پیشنهادی هر واحد</th><th>وضعیت</th></tr></thead><tbody>
    {costCandidates.map(x=><tr key={x.invoice_item_id}><td>#{money(x.invoice_number)}<small>{formatJalaliDate(x.date)}</small></td><td>{x.item_name}</td><td>{money(x.quantity)}</td><td>{x.eligible?money(x.proposed_unit_cost||0)+' تومان':'—'}</td><td>{x.eligible?'قابل اصلاح':(x.reason||'غیرقابل اصلاح')}</td></tr>)}
    {!costCandidates.length&&<tr><td colSpan={5} className="empty-cell">ردیف فاقد بهای تاریخی پیدا نشد.</td></tr>}
   </tbody></table></div>
   <div className="report-note"><Activity size={18}/><div><b>مبنای بازسازی</b><p>بهای پیشنهادی از میانگین بهای موزون ذخیره‌شده پیش از ثبت همان فروش گرفته می‌شود؛ قیمت خرید فعلی کالا یا اطلاعات حدسی استفاده نمی‌شود. ردیف‌های فاقد این snapshot فقط گزارش می‌شوند و دست‌نخورده می‌مانند.</p></div></div>
   <button className="primary-btn" disabled={costRepairBusy||!costCandidates.some(x=>x.eligible)} onClick={()=>void applyCostRepairs()}> {costRepairBusy?'در حال انجام…':'ثبت اصلاح برای ردیف‌های قابل اعتماد ('+money(costCandidates.filter(x=>x.eligible).length)+')'}</button>
  </section>
  <div className="report-analysis-grid">
   <section className="surface report-analysis-card"><div className="surface-heading"><div><h2>روند روزانه فروش</h2><p>بر اساس تاریخ فاکتور؛ روزهای بدون فروش هم نمایش داده می‌شوند</p></div><b>{money(trend.reduce((sum,x)=>sum+x.total,0))} تومان</b></div><div className="report-trend-chart"><div className="chart-grid"><i/><i/><i/><i/></div><svg viewBox="0 0 100 100" preserveAspectRatio="none"><path d={path+' L100 100 L0 100 Z'} fill="#159b6714"/><path d={path||'M0 92 L100 92'} fill="none" stroke="#159b67" strokeWidth="1.8" vectorEffect="non-scaling-stroke"/>{coords.map((p,i)=><circle key={i} cx={p.x} cy={p.y} r="1.2" fill="#fff" stroke="#159b67" strokeWidth=".8" vectorEffect="non-scaling-stroke"/>)}</svg></div><div className="report-trend-labels">{trend.filter((_,i)=>i===0||i===trend.length-1||i%Math.max(1,Math.floor(trend.length/6))===0).map(p=><span key={p.date}>{formatJalaliDate(p.date)}</span>)}</div></section>
   <section className="surface report-analysis-card"><div className="surface-heading"><div><h2>کالاهای پرفروش</h2><p>بر اساس تعداد خالص پس از برگشت</p></div></div><div className="table-wrap"><table className="data-table report-products-table"><thead><tr><th>کالا / خدمت</th><th>کد</th><th>تعداد خالص</th></tr></thead><tbody>{(report?.top_products||[]).map((p,i)=><tr key={p.code+'-'+p.name}><td><b>{p.name}</b></td><td>{p.code||'—'}</td><td>{money(p.quantity)}</td></tr>)}{!report?.top_products?.length&&<tr><td colSpan={3} className="empty-cell">در این بازه فروش ثبت نشده است.</td></tr>}</tbody></table></div></section>
  </div>
  <div className="surface report-note"><BarChart3 size={22}/><div><b>روش محاسبه و دامنه فیلتر</b><p>فروش و خرید بر اساس تاریخ فاکتور و مبلغ فعلی سند پس از برگشت‌ها محاسبه می‌شوند؛ جمع برگشت‌ها جداگانه بر اساس تاریخ برگشت گزارش می‌شود و دوباره از فروش کسر نمی‌شود. بهای تمام‌شده بر اساس بهای خرید ثبت‌شده در هر ردیف فاکتور هنگام فروش و مقدار برگشت‌نخورده محاسبه می‌شود. هزینه‌ها و ارزش موجودی، داده‌های کل فروشگاه هستند و با فیلتر طرف‌حساب محدود نمی‌شوند.</p></div></div>
 </>;
}

