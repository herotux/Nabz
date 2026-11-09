# نصب بومی Nabz روی Manjaro و Arch Linux

این راهنما مخصوص شاخه معماری دو رابطی است:

- `slint-native`: رابط بومی Slint
- `tauri-web`: رابط Web بسته‌بندی‌شده با Tauri
- `crates/nabz-core`: هسته Rust و پایگاه داده مشترک

این دو بسته مستقل‌اند و می‌توان یکی یا هر دو را نصب کرد. هر دو از یک مسیر داده و یک SQLite استفاده می‌کنند؛ حذف یکی نباید داده‌ها را حذف کند.

## نصب یک‌دستوری

تا وقتی تغییرات بسته‌بندی در شاخه اصلی ادغام نشده‌اند، نصب‌کننده را از شاخه بسته‌بندی اجرا کن:

```bash
curl -fsSL https://raw.githubusercontent.com/herotux/Nabz/packaging/slint-tauri-arch/install-arch.sh | bash
```

اسکریپت نوع رابط را می‌پرسد. می‌توان انتخاب را مستقیم هم فرستاد:

```bash
curl -fsSL https://raw.githubusercontent.com/herotux/Nabz/packaging/slint-tauri-arch/install-arch.sh | bash -s -- slint
curl -fsSL https://raw.githubusercontent.com/herotux/Nabz/packaging/slint-tauri-arch/install-arch.sh | bash -s -- tauri
curl -fsSL https://raw.githubusercontent.com/herotux/Nabz/packaging/slint-tauri-arch/install-arch.sh | bash -s -- both
```

## اجرا

- `nabz-slint`: رابط بومی
- `nabz-tauri`: رابط Web

برای سوئیچ، هر دو بسته باید نصب باشند. سوئیچ بین رابط‌ها فایل دیتابیس را کپی یا تبدیل نمی‌کند.

## محل داده‌ها

```text
${XDG_DATA_HOME:-~/.local/share}/com.herotux.nabz.webprototype/
├── nabz.sqlite3
├── backups/
├── attachments/
└── logs/
```

حذف یا نصب مجدد هر بسته فقط فایل‌های همان رابط را مدیریت می‌کند و نباید پوشه بالا را پاک کند.

## محدودیت‌های فعلی

این بسته‌بندی نسخه معماری دو رابطی را هدف می‌گیرد، نه برنامه egui موجود در `main`. بسته‌ها از سورس شاخه `feature/shared-core-ui-switching` ساخته می‌شوند. CI ساخت روی Arch را بررسی می‌کند؛ بااین‌حال تست نهایی روی Manjaro واقعی، RTL فارسی، چاپگر، بارکدخوان و تست پذیرش با یک کپی از دیتابیس واقعی همچنان لازم است. موفقیت ساخت به‌تنهایی به معنی کامل بودن محصول نیست.
