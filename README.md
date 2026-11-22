# نبض | حسابداری شخصی

برنامهٔ سبک حسابداری شخصی با Rust، رابط فارسی راست‌چین، فونت فارسی Vazirmatn تعبیه‌شده، تاریخ شمسی و ذخیره‌سازی محلی SQLite.

## ویژگی‌ها
- ثبت درآمد و هزینه به تومان
- نسخهٔ فروشگاهی: کالا، SKU و بارکد، سبد فروش و کسر موجودی
- ثبت فاکتور خرید و افزایش موجودی به‌صورت تراکنشی
- ثبت مشتری و تأمین‌کننده، ثبت هزینه‌های فروشگاه و گزارش فروش/خرید/هزینه
- خروجی CSV فروش‌ها و پشتیبان‌گیری محلی از پایگاه داده
- تاریخ شمسی و ارقام فارسی
- دسته‌بندی و یادداشت تراکنش
- جست‌وجو، حذف، و جمع درآمد/هزینه/خالص
- ذخیره‌سازی محلی در `~/.local/share/nabz/nabz.sqlite3`
- بدون نیاز به حساب کاربری یا اینترنت هنگام اجرا

## دریافت فایل نصب لینوکس
هر بار که GitHub Actions ساخت را با موفقیت تمام کند، فایل‌های خروجی در بخش **Artifacts** همان اجرای workflow قرار می‌گیرند:
[اجرای ساخت لینوکس](https://github.com/herotux/Nabz/actions/workflows/build-linux.yml)

سه خروجی ساخته می‌شوند: **AppImage**، بستهٔ **Flatpak** و آرشیو قابل‌حمل. برای نصب Flatpak روی مانجارو، ابتدا مطمئن شو Flatpak نصب است و Flathub اضافه شده:

```bash
sudo pacman -S --needed flatpak
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./Nabz-x86_64.flatpak
```

برای AppImage:

```bash
chmod +x Nabz-x86_64.AppImage
./Nabz-x86_64.AppImage
```

برای ساخت Release رسمی، یک تگ نسخه مانند `v0.1.0` ایجاد کنید؛ workflow فایل‌های نصب را به Release اضافه می‌کند.

## نسخهٔ کسب‌وکار

طرح نسخهٔ فروشگاهی سبک شامل کالا و بارکد، قیمت خرید و فروش، موجودی و هشدار کمبود، خرید از تأمین‌کننده، فروش سریع، برگشت کالا، حساب مشتری/تأمین‌کننده، هزینه‌های مغازه، گزارش سود، پشتیبان‌گیری، **چاپ فاکتور روی فیش‌زن ۵۸/۸۰ میلی‌متری و چاپگر A4، تولید و چاپ برچسب بارکد، و اسکنر USB** در [مشخصات نسخهٔ کسب‌وکار](docs/business-edition.md) ثبت شده است. این سند نقشهٔ راه است. نسخهٔ فعلی اکنون کالا و فروش بارکدی، ثبت خرید و ورود موجودی، ثبت طرف‌حساب، ثبت هزینهٔ فروشگاه، خلاصهٔ گزارش، خروجی CSV فروش‌ها و پشتیبان‌گیری محلی را دارد؛ سایر امکانات سند هنوز در صف توسعه‌اند.

## نسخهٔ Raspberry Pi (فقط ۶۴ بیتی)

Workflow [Build Raspberry Pi 64-bit package](https://github.com/herotux/Nabz/actions/workflows/build-raspberry-pi.yml) فقط خروجی ARM64 می‌سازد:

- `Nabz-linux-aarch64-unknown-linux-gnu.tar.gz` — مخصوص Raspberry Pi OS نسخهٔ ۶۴ بیتی (aarch64).

پس از موفقیت اجرای workflow:
1. در GitHub وارد مخزن و بخش **Actions** شو.
2. اجرای موفق **Build Raspberry Pi 64-bit package** را باز کن.
3. از بخش **Artifacts** فایل `Nabz-linux-aarch64-unknown-linux-gnu` را دریافت کن.
4. آرشیو را به رزبری پای منتقل و استخراج کن:
   ```bash
   tar -xzf Nabz-linux-aarch64-unknown-linux-gnu.tar.gz
   cd nabz
   ```
5. وابستگی‌های زمان اجرا را نصب کن:
   ```bash
   sudo apt update
   sudo apt install -y libx11-6 libxcb1 libxcb-shape0 libxcb-xfixes0 libxkbcommon0 libwayland-client0 libegl1 libfontconfig1
   ```
6. نصب را انجام بده:
   ```bash
   sudo ./install.sh
   ```
7. از منوی برنامه‌ها **Nabz** را اجرا کن یا در ترمینال بزن:
   ```bash
   /opt/nabz-slint/nabz-slint
   ```

اطلاعات برنامه به‌صورت پیش‌فرض در `~/.local/share/com.herotux.nabz.webprototype/nabz.sqlite3` ذخیره می‌شود. قبل از استفاده از اطلاعات واقعی، از دیتابیس نسخهٔ پشتیبان بگیر و فعلاً برنامه را روی کپی دیتابیس آزمایش کن؛ نسخهٔ Slint هنوز نمونهٔ اولیه است. این خروجی آرشیو قابل‌حمل ARM64 است، نه AppImage یا بستهٔ deb.

## ساخت از سورس
نیازمندی‌ها: Rust stable و وابستگی‌های توسعهٔ لینوکس برای `eframe`.

```bash
./scripts/fetch-font.sh
cargo test
cargo run --release
```

## تست‌ها
تست‌های واحد برای تبدیل تاریخ میلادی به شمسی، اعتبارسنجی تاریخ شمسی، ارقام فارسی و قالب‌بندی مبلغ در `cargo test` اجرا می‌شوند. هر push و pull request در GitHub Actions بررسی و ساخته می‌شود.

## حریم خصوصی
اطلاعات حسابداری به‌صورت محلی ذخیره می‌شوند و برنامه آن‌ها را به سرور ارسال نمی‌کند.

## مجوز
برنامه تحت MIT است. فونت Vazirmatn تحت SIL Open Font License است؛ فایل مجوز در `assets/OFL.txt` قرار می‌گیرد.
