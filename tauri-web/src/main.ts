import './style.css';

const nav = [['▦','داشبورد'],['♙','طرف‌حساب‌ها'],['▤','فاکتورها'],['↔','دریافت و پرداخت'],['▧','کالا و انبار'],['▥','گزارش‌ها']];
const app = document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML = `
<div class="layout">
 <aside class="sidebar"><div class="brand"><span class="mark">ن</span><div><b>نبض</b><small>مدیریت مالی کسب‌وکار</small></div></div>
 <div class="company">▣ <span><b>شرکت هیراد</b><small>نسخه آزمایشی</small></span>⌄</div><p class="caption">منوی اصلی</p>
 <nav>${nav.map((n,i)=>`<button class="nav ${i===0?'active':''}" data-nav="${n[1]}"><span>${n[0]}</span>${n[1]}</button>`).join('')}</nav>
 <div class="bottom"><div class="help"><b>✦ راهنمای نبض</b><p>راهنمای شروع سریع</p></div><div class="user"><span class="avatar">ح‌س</span><span><b>مدیر سیستم</b><small>دسترسی کامل</small></span></div></div></aside>
 <main><header><span>خانه　/　<b id="crumb">داشبورد</b></span><div>شنبه، ۴ آبان ۱۴۰۵　 <span class="avatar">ح‌س</span></div></header>
 <section class="content"><div class="welcome"><div><small>نمای کلی کسب‌وکار</small><h1 id="heading">سلام، روزت بخیر 👋</h1><p>وضعیت مالی کسب‌وکارت رو در یک نگاه بررسی کن.</p></div><button class="primary" id="new">＋ فاکتور جدید</button></div>
 <div class="stats">
 ${[['فروش امروز','۲۴,۸۵۰,۰۰۰','↑ ۱۲.۸٪','blue'],['دریافت‌های امروز','۱۸,۲۰۰,۰۰۰','↑ ۸.۲٪','green'],['مانده طرف‌حساب‌ها','۷۶,۴۰۰,۰۰۰','↓ ۳.۴٪','orange'],['ارزش موجودی کالا','۳۱۲,۶۰۰,۰۰۰','بدون تغییر','purple']].map(s=>`<article class="stat"><div>${s[0]}<i class="${s[3]}">◈</i></div><strong>${s[1]} <small>تومان</small></strong><footer><b>${s[2]}</b><span>نسبت به بازه قبل</span></footer></article>`).join('')}
 </div>
 <div class="grid"><article class="panel"><div class="panel-head"><div><h2>گزارش فروش</h2><p>روند فروش در بازه انتخابی</p></div><select id="range"><option>۷ روز گذشته</option><option>۳۰ روز گذشته</option></select></div><div class="chart-title"><span>● فروش</span><b id="total">۱۲۸,۴۰۰,۰۰۰ تومان</b></div><div class="chart"><div class="y"><span>۳۰ م</span><span>۲۰ م</span><span>۱۰ م</span><span>۰</span></div><div class="plot"><div class="lines"><i></i><i></i><i></i><i></i></div><svg viewBox="0 0 600 180" preserveAspectRatio="none"><defs><linearGradient id="shade" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="#3768db" stop-opacity=".25"/><stop offset="1" stop-color="#3768db" stop-opacity="0"/></linearGradient></defs><path fill="url(#shade)" d="M0 145 C35 135 45 105 90 119 S145 145 185 92 S240 108 278 75 S340 92 370 62 S420 84 462 50 S520 74 555 36 S580 45 600 18 L600 180 L0 180Z"/><path d="M0 145 C35 135 45 105 90 119 S145 145 185 92 S240 108 278 75 S340 92 370 62 S420 84 462 50 S520 74 555 36 S580 45 600 18" fill="none" stroke="#3568d5" stroke-width="3"/></svg><div class="x"><span>یکشنبه</span><span>دوشنبه</span><span>سه‌شنبه</span><span>چهارشنبه</span><span>پنجشنبه</span><span>جمعه</span><span>شنبه</span></div></div></div></article>
 <article class="panel"><div class="panel-head"><div><h2>دسترسی سریع</h2><p>کارهای پرکاربرد</p></div></div><div class="quick">${[['＋','ثبت فاکتور فروش','ایجاد فاکتور برای مشتری'],['↓','ثبت دریافت','ثبت وجه دریافتی'],['♙','افزودن طرف‌حساب','مشتری یا تأمین‌کننده'],['▦','افزودن کالا','ثبت کالا در انبار']].map(q=>`<button data-quick="${q[1]}"><i>${q[0]}</i><span><b>${q[1]}</b><small>${q[2]}</small></span><em>‹</em></button>`).join('')}</div></article></div>
 <article class="panel transactions"><div class="panel-head"><div><h2>آخرین تراکنش‌ها</h2><p>آخرین رویدادهای ثبت‌شده در سیستم</p></div><button class="link" id="all">مشاهده همه ←</button></div><div class="table-wrap"><table><thead><tr><th>شرح تراکنش</th><th>طرف‌حساب</th><th>تاریخ</th><th>مبلغ (تومان)</th><th>وضعیت</th></tr></thead><tbody>
 <tr><td><b>↗　فاکتور فروش #۱۰۲۴</b></td><td>فروشگاه آریا</td><td>۴ آبان ۱۴۰۵</td><td><b>۱۲,۵۰۰,۰۰۰</b></td><td><label class="ok">تسویه شده</label></td></tr>
 <tr><td><b>↓　دریافت وجه #۰۸۹</b></td><td>شرکت پارس</td><td>۴ آبان ۱۴۰۵</td><td><b>۸,۲۰۰,۰۰۰</b></td><td><label class="ok">موفق</label></td></tr>
 <tr><td><b>↗　فاکتور فروش #۱۰۲۳</b></td><td>بازرگانی سپهر</td><td>۳ آبان ۱۴۰۵</td><td><b>۶,۳۰۰,۰۰۰</b></td><td><label class="pending">در انتظار</label></td></tr>
 <tr><td><b>↓　پرداخت هزینه #۰۴۱</b></td><td>خدمات رایانه‌ای</td><td>۳ آبان ۱۴۰۵</td><td><b>۲,۱۵۰,۰۰۰</b></td><td><label class="ok">ثبت شده</label></td></tr></tbody></table></div></article>
 <p class="note">نبض · نمونه رابط کاربری ۰.۱ — اعداد و تراکنش‌ها نمایشی هستند.</p></section></main>
</div>
<div id="toast" role="status"></div>
<div class="overlay" id="modal"><div class="dialog"><button class="close" id="close">×</button><div class="mark">ن</div><h2 id="modalTitle">ایجاد فاکتور فروش</h2><p>این نسخه فقط نمونه رابط است؛ اطلاعات هنوز در پایگاه داده ذخیره نمی‌شود.</p><label>نام طرف‌حساب<input id="customer" placeholder="مثلاً فروشگاه آریا"></label><label>مبلغ (تومان)<input id="amount" inputmode="numeric" placeholder="مثلاً ۱۲۵۰۰۰۰"></label><div class="actions"><button id="cancel">انصراف</button><button class="primary" id="save">ثبت نمایشی</button></div></div></div>`;
const toast = document.querySelector<HTMLDivElement>('#toast')!;
let timer=0;
function notify(s:string){toast.textContent=s;toast.classList.add('show');window.clearTimeout(timer);timer=window.setTimeout(()=>toast.classList.remove('show'),2800)}
const modal=document.querySelector<HTMLDivElement>('#modal')!;
function openModal(title:string){(document.querySelector('#modalTitle') as HTMLElement).textContent=title;modal.classList.add('open')}
function closeModal(){modal.classList.remove('open')}
document.querySelector('#new')!.addEventListener('click',()=>openModal('ایجاد فاکتور فروش'));
document.querySelector('#close')!.addEventListener('click',closeModal);
document.querySelector('#cancel')!.addEventListener('click',closeModal);
modal.addEventListener('click',e=>{if(e.target===modal)closeModal()});
document.querySelector('#save')!.addEventListener('click',()=>{const c=(document.querySelector('#customer') as HTMLInputElement).value.trim();const a=(document.querySelector('#amount') as HTMLInputElement).value.trim();if(!c||!a){notify('نام طرف‌حساب و مبلغ را وارد کن.');return}closeModal();notify('ثبت نمایشی بود؛ هنوز ذخیره واقعی انجام نمی‌شود.')});
document.querySelectorAll<HTMLButtonElement>('[data-quick]').forEach(b=>b.addEventListener('click',()=>openModal(b.dataset.quick||'عملیات جدید')));
document.querySelectorAll<HTMLButtonElement>('[data-nav]').forEach(b=>b.addEventListener('click',()=>{document.querySelectorAll('[data-nav]').forEach(x=>x.classList.remove('active'));b.classList.add('active');const t=b.dataset.nav||'داشبورد';(document.querySelector('#crumb') as HTMLElement).textContent=t;(document.querySelector('#heading') as HTMLElement).textContent=t==='داشبورد'?'سلام، روزت بخیر 👋':t;if(t!=='داشبورد')notify('این بخش در نسخه بعدی به منطق برنامه متصل می‌شود.')}));
document.querySelector('#range')!.addEventListener('change',e=>{(document.querySelector('#total') as HTMLElement).textContent=(e.target as HTMLSelectElement).selectedIndex?'۴۸۶,۲۰۰,۰۰۰ تومان':'۱۲۸,۴۰۰,۰۰۰ تومان'});
document.querySelector('#all')!.addEventListener('click',()=>notify('فهرست کامل تراکنش‌ها هنوز متصل نیست.'));
