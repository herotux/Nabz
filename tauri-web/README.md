# نبض — نسخهٔ دسکتاپ React/Tauri

این پوشه نسخهٔ فعال نبض است: React + TypeScript + Vite در پوستهٔ Tauri 2، با منطق Rust و دیتابیس SQLite محلی.

## بخش‌های رابط

- داشبورد
- طرف‌حساب‌ها
- کالا و انبار
- فاکتورهای فروش و خرید
- دریافت و پرداخت
- گزارش‌ها
- چاپ و بارکد
- تنظیمات

## اجرای توسعه

نیازمندی‌ها: Node.js 20 یا جدیدتر، Rust stable و پیش‌نیازهای Tauri 2 / WebKitGTK برای لینوکس.

```bash
npm install
npm run tauri dev
```

## بررسی frontend و ساخت باینری

```bash
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml --bin nabz-tauri
```

## بستهٔ Arch / Manjaro

بستهٔ بومی در CI ساخته می‌شود. Artifact با نام `nabz-manjaro-arch-x86_64` و فایل `nabz-tauri-0.1.0-1-x86_64.pkg.tar.zst` در اجرای موفق workflow قرار می‌گیرد.

```bash
sudo pacman -U ./nabz-tauri-0.1.0-1-x86_64.pkg.tar.zst
```

پیکربندی Tauri عمداً bundler داخلی deb/AppImage را غیرفعال کرده است؛ بستهٔ Arch از باینری Release و PKGBUILD ساخته می‌شود.

## نصب روی Arch / Manjaro

راهنمای نصب/ارتقا و اسکریپت نصب یک‌مرحله‌ای در [INSTALL.md](INSTALL.md) و [install.sh](install.sh) قرار دارد. نصب‌کننده پس از انتشار Release عمومی دارای checksum فعال می‌شود؛ تا آن زمان از artifact آزمایشی CI استفاده کنید.

## وضعیت و نقشهٔ راه

- [وضعیت پیاده‌سازی و شواهد آزمون](IMPLEMENTATION_STATUS.md)
- [فهرست کارهای باقی‌مانده](TODO.md)
- [تصمیم معماری و انتقال هسته](../docs/architecture/DECISIONS.md)

## هشدار پیش از استفاده

این نسخه آزمایشی است و آمادهٔ ثبت حساب‌های واقعی فروشگاه نیست. import محافظت‌شدهٔ دیتابیس egui فقط در مقصد خالی انجام می‌شود؛ فایل مبدأ حفظ می‌شود و قبل از import backup گرفته می‌شود. آزمون نصب روی Manjaro واقعی هنوز باقی مانده است. ردیف‌های قدیمی فاقد بهای تمام‌شده با مقدار حدسی پر نمی‌شوند؛ برگشت/ابطال خودکار آن‌ها تا تکمیل snapshotهای لازم مسدود است. چاپ فعلی از مسیر پیش‌نمایش/چاپ داخلی مرورگر است و سازگاری فیزیکی چاپگر و بارکدخوان تأیید نشده است.

شناسهٔ پایدار برنامه `com.herotux.nabz` است. دیتابیس قبلی Tauri از شناسهٔ `com.herotux.nabz.webprototype` در اولین اجرا با SQLite backup منتقل می‌شود و فایل قدیمی باقی می‌ماند. اگر دیتابیس Tauri قبلی وجود نداشته باشد، import egui فقط در مقصد خالی اجرا می‌شود. هستهٔ `nabz-core` در شاخهٔ مرجع schema نسخهٔ ۱۴ را مدیریت می‌کند؛ برای وضعیت و تاریخچهٔ migrationها به [TODO](TODO.md) و [گزارش فاز یک](../docs/architecture/PHASE_ONE.md) مراجعه کنید. جزئیات در [گزارش فاز یک](../docs/architecture/PHASE_ONE.md) است.
