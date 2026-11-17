# نبض | حسابداری فروشگاهی

نبض یک نرم‌افزار حسابداری فروشگاهی فارسی و راست‌چین برای کسب‌وکارهای کوچک و متوسط است.

## مسیر فنی قطعی

نسخهٔ فعال محصول با **React + TypeScript + Vite + Tauri 2 + Rust + SQLite** توسعه داده می‌شود.

- رابط کاربری: `tauri-web/src`
- adapter فرمان‌های Tauri: `tauri-web/src-tauri`
- هستهٔ حسابداری، migration و SQLite: `crates/nabz-core`
- بستهٔ لینوکس هدف: Arch/Manjaro با فرمت `.pkg.tar.zst`
- هستهٔ حسابداری و دیتابیس: [گزارش فاز یک](docs/architecture/PHASE_ONE.md) و [تصمیم‌های معماری](docs/architecture/DECISIONS.md)
- وضعیت و نقشهٔ راه: [وضعیت پیاده‌سازی](tauri-web/IMPLEMENTATION_STATUS.md) و [فهرست کارها](tauri-web/TODO.md)
- تصمیم‌های معماری: [Architecture decisions](docs/architecture/DECISIONS.md)
- گزارش فاز صفر: [Phase zero](docs/architecture/PHASE_ZERO.md)

## اجرای نسخهٔ توسعه

پیش‌نیازها: Node.js 20 یا جدیدتر، Rust stable و پیش‌نیازهای Tauri 2 برای لینوکس.

```bash
cd tauri-web
npm install
npm run tauri dev
```

برای بررسی frontend:

```bash
cd tauri-web
npm run build
```

برای ساخت باینری Tauri:

```bash
cargo build --release --manifest-path tauri-web/src-tauri/Cargo.toml --bin nabz-tauri
```

## بستهٔ Arch / Manjaro

گردش‌کار فعال GitHub Actions بستهٔ بومی Arch/Manjaro را می‌سازد و artifact آن را با نام `nabz-manjaro-arch-x86_64` منتشر می‌کند. پس از دریافت فایل:

```bash
sudo pacman -U ./nabz-tauri-0.1.0-1-x86_64.pkg.tar.zst
```

این بسته هنوز تا زمان نصب و آزمون روی دستگاه واقعی Manjaro، نسخهٔ پایدار محسوب نمی‌شود.

**مسیرهای AppImage، Flatpak و deb دیگر روش انتشار نبض نیستند.** گردش‌کارهای قدیمی مربوط به برنامهٔ egui و prototype از شاخهٔ مرجع حذف شده‌اند. ساخت Raspberry Pi با Tauri در مرحلهٔ بعدی اضافه خواهد شد.

## حفظ داده و نسخهٔ قدیمی

کد قدیمی egui در ریشهٔ مخزن فعلاً فقط برای بررسی و طراحی مهاجرت داده نگه داشته شده و خروجی انتشار فعال نیست.

- دیتابیس Tauri فعلی در لینوکس معمولاً: `~/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3`
- دیتابیس egui قدیمی: `~/.local/share/nabz/nabz.sqlite3`

این دو مسیر جدا هستند. مهاجرت خودکار هنوز انجام نشده است؛ هیچ‌کدام را حذف، جایگزین یا با دیگری ادغام نکنید. پیش از استفاده از نسخهٔ آزمایشی، از هر دو دیتابیس نسخهٔ پشتیبان تهیه کنید.

## وضعیت انتشار

شاخهٔ `ui/react-tauri-redesign` و [PR شمارهٔ ۸](https://github.com/herotux/Nabz/pull/8) مرجع توسعه‌اند. PR هنوز Draft است و تا تکمیل هستهٔ مالی، مهاجرت امن داده، آزمون پذیرش و تست Manjaro نباید در `main` ادغام یا برای حسابداری واقعی استفاده شود.
