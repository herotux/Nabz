# بستهٔ Arch / Manjaro

پس از دانلود artifact با نام `nabz-manjaro-arch-x86_64`، فایل `.pkg.tar.zst` را نصب کن:

```bash
sudo pacman -U ./nabz-tauri-0.1.0-1-x86_64.pkg.tar.zst
```

اجرای برنامه از منوی برنامه‌ها با نام «نبض» یا از ترمینال:

```bash
nabz
```

حذف:

```bash
sudo pacman -R nabz-tauri
```

این بسته فعلاً فقط برای x86_64 ساخته می‌شود. وابستگی‌های WebKitGTK و GTK از مخازن رسمی Arch/Manjaro نصب می‌شوند. بستهٔ CI آزمایشی است و پیش از استفاده از داده‌های واقعی باید روی سیستم مقصد تست شود.
