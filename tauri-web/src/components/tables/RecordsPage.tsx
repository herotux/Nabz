import { useEffect, useState, type ReactNode } from 'react';
import { Search } from 'lucide-react';
import { PageHeading, PrimaryButton } from '../layout/PagePrimitives';
import { RecordsTable } from './RecordsTable';
import type { RecordRow } from '../../types';
const money=(n:number)=>new Intl.NumberFormat('fa-IR').format(Number.isFinite(n)?n:0);
function Pagination({page,pages,onChange}:{page:number;pages:number;onChange:(p:number)=>void}){
 if(pages<=1)return null;
 return <div className="pagination"><span>صفحه {money(page)} از {money(pages)}</span><div><button className="secondary-btn" disabled={page<=1} onClick={()=>onChange(page-1)}>قبلی</button><button className="secondary-btn" disabled={page>=pages} onClick={()=>onChange(page+1)}>بعدی</button></div></div>;
}
export function RecordsPage({title,subtitle,records,query,setQuery,busy,onAdd,onDelete,onPrint,onEdit,onView,columns,extra}:{title:string;subtitle:string;records:RecordRow[];query:string;setQuery:(s:string)=>void;busy:boolean;onAdd:()=>void;onDelete?:(id:number)=>void;onPrint?:(id:number)=>void;onEdit?:(r:RecordRow)=>void;onView?:(id:number)=>void;columns:string[];extra?:ReactNode}){
 const [page,setPage]=useState(1);const pageSize=10;const pages=Math.max(1,Math.ceil(records.length/pageSize));
 useEffect(()=>setPage(1),[query,records.length]);
 const visible=records.slice((page-1)*pageSize,page*pageSize);
 return <><PageHeading title={title} subtitle={subtitle}><div className="button-row">{extra}<PrimaryButton onClick={onAdd}>{title==='طرف‌حساب‌ها'?'طرف‌حساب جدید':title==='فاکتورها'?'فاکتور فروش':'ثبت دریافت'}</PrimaryButton></div></PageHeading><div className="surface list-surface"><div className="list-toolbar"><div className="search-box"><Search size={17}/><input value={query} onChange={e=>setQuery(e.target.value)} placeholder="جست‌وجو در فهرست..."/></div><div className="toolbar-meta"><span>{money(records.length)} مورد</span></div></div><RecordsTable records={visible} kind={title==='طرف‌حساب‌ها'?'contacts':title==='فاکتورها'?'invoices':'payments'} columns={columns} onDelete={onDelete} onPrint={onPrint} onEdit={onEdit} onView={onView}/><Pagination page={page} pages={pages} onChange={setPage}/>{busy&&<div className="table-loading">در حال دریافت اطلاعات...</div>}</div></>;
}
