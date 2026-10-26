# نبض — رابط وب آزمایشی Tauri

این نمونهٔ مستقل رابط کاربری فارسی و RTL برای نبض است. منطق حسابداری و داده‌های واقعی هنوز به آن متصل نشده‌اند؛ اعداد داشبورد نمونه‌اند.

## اجرا
نیازمند Node.js 20+، Rust stable و پیش‌نیازهای Tauri 2:
https://v2.tauri.app/start/prerequisites/

```bash
cd tauri-web
npm install
npm run tauri dev
```

## ساخت
```bash
npm run tauri build
```

این پروژه در شاخهٔ `tauri-web-prototype` و زیرپوشهٔ `tauri-web/` قرار دارد تا کد فعلی egui دست‌نخورده بماند.
