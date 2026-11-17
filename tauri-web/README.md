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

## وضعیت و نقشهٔ راه

- [وضعیت پیاده‌سازی و شواهد آزمون](IMPLEMENTATION_STATUS.md)
- [فهرست کارهای باقی‌مانده](TODO.md)
- [تصمیم معماری و انتقال هسته](../docs/architecture/DECISIONS.md)

## هشدار پیش از استفاده

این نسخه آزمایشی است و آمادهٔ ثبت حساب‌های واقعی فروشگاه نیست. مهاجرت دیتابیس egui، ادغام کامل هستهٔ حسابداری، برگشت و ابطال امن اسناد، و آزمون سخت‌افزار واقعی هنوز باقی مانده‌اند. چاپ فعلی از مسیر پیش‌نمایش/چاپ داخلی مرورگر است و سازگاری فیزیکی چاپگر و بارکدخوان تأیید نشده است.

شناسهٔ برنامه فعلاً `com.herotux.nabz.webprototype` باقی مانده تا مسیر دیتابیس موجود تغییر نکند. آن را بدون مهاجرت نسخه‌بندی‌شده تغییر ندهید.
