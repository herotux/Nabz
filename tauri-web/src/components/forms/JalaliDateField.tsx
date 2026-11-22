import { useEffect, useState } from 'react';
import { CalendarDays, ChevronLeft, ChevronRight } from 'lucide-react';
import { gregorianToJalali, jalaliMonths, jalaliParts, jalaliToGregorian, jalaliWeekdays } from '../../utils/jalali';

const money = (n:number) => new Intl.NumberFormat('fa-IR',{useGrouping:false}).format(Number.isFinite(n) ? n : 0);

export function JalaliDateField({value,onChange,label,todayValue}:{value:string;onChange:(v:string)=>void;label:string;todayValue:string}){
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
 return <div className="field-label jalali-date-field"><span>{label}</span><div className="jalali-input-row"><input type="text" inputMode="numeric" dir="rtl" value={text} placeholder="۱۴۰۵/۰۸/۲۶" aria-invalid={invalid} className={invalid?'date-invalid':''} onChange={e=>{
  const next=e.target.value.replace(/[0-9]/g,d=>'۰۱۲۳۴۵۶۷۸۹'[Number(d)]);setText(next);
  const converted=jalaliToGregorian(next);
  if(converted){onChange(converted);setInvalid(false)}else setInvalid(next.length>=10);
 }} onBlur={()=>{if(!jalaliToGregorian(text)){setText(gregorianToJalali(value));setInvalid(false)}}}/><button type="button" className="calendar-trigger" aria-label="باز کردن تقویم شمسی" onClick={()=>setOpen(v=>!v)}><CalendarDays size={16}/></button></div><small className="date-hint">{invalid?'تاریخ شمسی معتبر وارد کن':'سال/ماه/روز شمسی'}</small>{open&&<div className="jalali-calendar"><div className="calendar-head"><button type="button" onClick={()=>move(-1)}><ChevronRight size={17}/></button><b>{jalaliMonths[view.month-1]} {money(view.year)}</b><button type="button" onClick={()=>move(1)}><ChevronLeft size={17}/></button></div><div className="calendar-grid">{jalaliWeekdays.map(d=><span className="calendar-weekday" key={d}>{d}</span>)}{cells.map((day,i)=>day?<button type="button" key={i} className={selected.year===view.year&&selected.month===view.month&&selected.day===day?'selected':''} onClick={()=>choose(day)}>{money(day)}</button>:<span key={i}/>)}</div><button type="button" className="calendar-today" onClick={()=>{onChange(todayValue);setOpen(false)}}>امروز</button></div>}</div>;
}
