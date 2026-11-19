import type { ReactNode } from 'react';
import { Plus } from 'lucide-react';

export function PageHeading({eyebrow,title,subtitle,children}:{eyebrow?:string;title:string;subtitle:string;children?:ReactNode}){
 return <div className="page-heading"><div><div className="eyebrow">{eyebrow||'مدیریت فروشگاه'}</div><h1>{title}</h1><p>{subtitle}</p></div><div className="heading-actions">{children}</div></div>;
}
export function PrimaryButton({children,onClick}:{children:ReactNode;onClick:()=>void}){
 return <button className="primary-btn" onClick={onClick}><Plus size={17}/>{children}</button>;
}
