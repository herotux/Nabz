import { Eye, Pencil, Printer, Trash2, X } from 'lucide-react';
import type { RecordRow } from '../../types';
import { formatJalaliDate } from '../../utils/jalali';
const money=(n:number)=>new Intl.NumberFormat('fa-IR').format(Number.isFinite(n)?n:0);
const labels:Record<string,string>={
 customer:'مشتری',supplier:'تأمین‌کننده',both:'مشتری و تأمین‌کننده',
 sale:'فروش',purchase:'خرید',receipt:'دریافت وجه',payment:'پرداخت وجه',
 cash:'نقدی',card:'کارت‌خوان',transfer:'انتقال بانکی',
 paid:'تسویه کامل',partial:'پرداخت جزئی',credit:'نسیه',unpaid:'پرداخت‌نشده',
 settled:'تسویه',void:'باطل‌شده',voided:'باطل‌شده',
};
const localized=(value:string|undefined|null)=>value?labels[value.toLowerCase()]||value:'—';
export function RecordsTable({records,kind,columns,onDelete,onPrint,onEdit,onView}:{records:RecordRow[];kind:string;columns?:string[];onDelete?:(id:number)=>void;onPrint?:(id:number)=>void;onEdit?:(r:RecordRow)=>void;onView?:(id:number)=>void}){
 const heads=columns||['شرح تراکنش','جزئیات','مبلغ','وضعیت','تاریخ'];
 return <div className="table-wrap"><table className="data-table"><thead><tr>{heads.map(h=><th key={h}>{h}</th>)}<th>عملیات</th></tr></thead><tbody>{records.map(r=>{
  const second=kind==='contacts'?localized(r.detail):kind==='invoices'?localized(r.detail):localized(r.detail);
  const status=localized(r.status)||'ثبت‌شده';
  const statusClass=kind==='contacts'?'info':kind==='payments'?'success':(/بدهکار|کم‌موجود|در انتظار|پرداخت جزئی|پرداخت‌نشده|نسیه/.test(status)?'warning':/باطل/.test(status)?'danger':/بستانکار/.test(status)?'info':'success');
  return <tr key={r.id}><td><b>{r.name}</b>{kind==='recent'&&<small className="sub-cell">{second}</small>}</td><td>{kind==='recent'?'—':second}</td><td className="amount-cell">{money(r.amount)} <small>تومان</small></td><td><span className={'status '+statusClass}>{status||'ثبت‌شده'}</span></td><td>{formatJalaliDate(r.date)}</td><td><div className="row-actions">{kind==='invoices'&&onPrint&&<button title="چاپ فاکتور" onClick={()=>onPrint(r.id)}><Printer size={16}/></button>}{kind==='contacts'&&onView&&<button title="گردش حساب" aria-label="گردش حساب" onClick={()=>onView(r.id)}><Eye size={16}/></button>}{kind==='invoices'&&onView&&<button title="جزئیات فاکتور" aria-label="جزئیات فاکتور" onClick={()=>onView(r.id)}><Eye size={16}/></button>}{kind==='contacts'&&onEdit&&<button title="ویرایش طرف‌حساب" onClick={()=>onEdit(r)}><Pencil size={16}/></button>}{onDelete&&!(kind==='invoices'&&r.status==='باطل‌شده')&&<button title={kind==='invoices'?'ابطال امن فاکتور':'حذف'} aria-label={kind==='invoices'?'ابطال امن فاکتور':'حذف رکورد'} onClick={()=>onDelete(r.id)}>{kind==='invoices'?<X size={16}/>:<Trash2 size={16}/>}</button>}</div></td></tr>;
 })}{!records.length&&<tr><td colSpan={6} className="empty-cell">هنوز اطلاعاتی ثبت نشده است.</td></tr>}</tbody></table></div>;
}
