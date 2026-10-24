pub mod hardware;
pub mod business;

use chrono::{Datelike, Local};
use eframe::egui::{self, Align, Color32, FontData, FontDefinitions, FontFamily, Frame, Layout, RichText, Stroke, Vec2};
use rusqlite::{params, Connection};
use std::{path::PathBuf, time::Duration};

const APP_NAME: &str = "نبض | حسابداری شخصی";
const FONT: &[u8] = include_bytes!("../assets/NotoSansArabic-Regular.ttf");

#[derive(Clone, Debug)]
struct Transaction {
    id: i64,
    kind: String,
    amount: i64,
    category: String,
    note: String,
    date: String,
}

struct NabzApp {
    db: Connection,
    items: Vec<Transaction>,
    kind: usize,
    amount: String,
    category: String,
    note: String,
    date: String,
    search: String,
    status: String,
    total_income: i64,
    total_expense: i64,
    show_delete: Option<i64>,
    show_shortcuts: bool,
    show_date_picker: bool,
    show_hardware: bool,
    hardware_status: String,
    hardware_scan: String,
    hardware_barcode: String,
    hardware_printer: String,
    hardware_printers: Vec<hardware::Printer>,
    show_business: bool,
    business_status: String,
    product_name: String,
    product_sku: String,
    product_barcode: String,
    product_purchase_price: String,
    product_sale_price: String,
    product_stock: String,
    business_scan: String,
    business_products: Vec<business::Product>,
    business_basket: Vec<business::BasketItem>,
    purchase_product_id: i64,
    purchase_quantity: String,
    purchase_unit_cost: String,
    purchase_paid: String,
    purchase_supplier: String,
    party_name: String,
    party_phone: String,
    party_kind: usize,
    expense_category: String,
    expense_amount: String,
    expense_note: String,
    picker_year: i32,
    picker_month: i32,
}

impl NabzApp {
    fn new() -> Result<Self, String> {
        let path = data_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        business::init_business_extensions(&db).map_err(|e| format!("آماده‌سازی بخش فروشگاهی ناموفق بود: {e}"))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS transactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL CHECK(kind IN ('income','expense')),
                amount INTEGER NOT NULL CHECK(amount > 0),
                category TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                date TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .map_err(|e| e.to_string())?;
        let mut app = Self {
            db,
            items: vec![],
            kind: 1,
            amount: String::new(),
            category: "سایر".into(),
            note: String::new(),
            date: today_jalali(),
            search: String::new(),
            status: "اطلاعات فقط روی همین دستگاه ذخیره می‌شود.".into(),
            total_income: 0,
            total_expense: 0,
            show_delete: None,
            show_shortcuts: false,
            show_date_picker: false,
            show_hardware: false,
            hardware_status: "برای چاپگرها به CUPS و برای بارکدخوان USB/HID به حالت صفحه‌کلید نیاز است.".into(),
            hardware_scan: String::new(),
            hardware_barcode: "SKU-0001".into(),
            hardware_printer: String::new(),
            hardware_printers: vec![],
            show_business: false,
            business_status: "کالاها و فروش در پایگاه دادهٔ محلی ذخیره می‌شوند.".into(),
            product_name: String::new(),
            product_sku: String::new(),
            product_barcode: String::new(),
            product_purchase_price: "0".into(),
            product_sale_price: "0".into(),
            product_stock: "0".into(),
            business_scan: String::new(),
            business_products: vec![],
            business_basket: vec![],
            purchase_product_id: 0,
            purchase_quantity: "1".into(),
            purchase_unit_cost: "0".into(),
            purchase_paid: "0".into(),
            purchase_supplier: String::new(),
            party_name: String::new(),
            party_phone: String::new(),
            party_kind: 0,
            expense_category: "اجاره و قبوض".into(),
            expense_amount: String::new(),
            expense_note: String::new(),
            picker_year: today_jalali().split('/').next().and_then(|v| v.parse().ok()).unwrap_or(1405),
            picker_month: today_jalali().split('/').nth(1).and_then(|v| v.parse().ok()).unwrap_or(1),
        };
        app.reload().map_err(|e| e.to_string())?;
        app.refresh_business_products().map_err(|e| e.to_string())?;
        Ok(app)
    }

    fn refresh_business_products(&mut self) -> rusqlite::Result<()> {
        self.business_products = business::list_products(&self.db)?;
        Ok(())
    }

    fn reload(&mut self) -> rusqlite::Result<()> {
        let mut stmt = self.db.prepare(
            "SELECT id, kind, amount, category, note, date
             FROM transactions ORDER BY id DESC LIMIT 500",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Transaction {
                id: r.get(0)?,
                kind: r.get(1)?,
                amount: r.get(2)?,
                category: r.get(3)?,
                note: r.get(4)?,
                date: r.get(5)?,
            })
        })?;
        self.items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let sums: (i64, i64) = self.db.query_row(
            "SELECT COALESCE(SUM(CASE WHEN kind='income' THEN amount ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN kind='expense' THEN amount ELSE 0 END),0)
             FROM transactions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        self.total_income = sums.0;
        self.total_expense = sums.1;
        Ok(())
    }

    fn add(&mut self) {
        let normalized: String = self.amount.chars().map(normalize_digit).collect();
        let amount = normalized.trim().replace(',', "").replace('٬', "").parse::<i64>();
        let amount = match amount {
            Ok(v) if v > 0 => v,
            _ => {
                self.status = "مبلغ باید عددی بزرگ‌تر از صفر باشد.".into();
                return;
            }
        };
        if !valid_jalali(&self.date) {
            self.status = "تاریخ شمسی معتبر نیست؛ نمونه: ۱۴۰۵/۰۷/۰۲".into();
            return;
        }
        let category = self.category.trim();
        if category.is_empty() {
            self.status = "دسته‌بندی را وارد کن.".into();
            return;
        }
        let kind = if self.kind == 0 { "income" } else { "expense" };
        match self.db.execute(
            "INSERT INTO transactions(kind,amount,category,note,date) VALUES (?1,?2,?3,?4,?5)",
            params![kind, amount, category, self.note.trim(), self.date.trim()],
        ) {
            Ok(_) => {
                self.amount.clear();
                self.note.clear();
                self.status = "تراکنش ذخیره شد.".into();
                if let Err(e) = self.reload() {
                    self.status = format!("خطا در خواندن اطلاعات: {e}");
                }
            }
            Err(e) => self.status = format!("ذخیره انجام نشد: {e}"),
        }
    }

    fn delete(&mut self, id: i64) {
        match self.db.execute("DELETE FROM transactions WHERE id=?1", [id]) {
            Ok(_) => {
                self.status = "تراکنش حذف شد.".into();
                if let Err(e) = self.reload() {
                    self.status = format!("خطا در به‌روزرسانی: {e}");
                }
            }
            Err(e) => self.status = format!("حذف انجام نشد: {e}"),
        }
        self.show_delete = None;
    }
}

impl eframe::App for NabzApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let save_shortcut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter));
        let help_shortcut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Slash));
        let help_f1 = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1));
        let search_shortcut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F));
        let new_shortcut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N));
        if save_shortcut { self.add(); }
        if help_shortcut || help_f1 { self.show_shortcuts = !self.show_shortcuts; }
        if search_shortcut { ctx.memory_mut(|m| m.request_focus(egui::Id::new("transaction_search"))); }
        if new_shortcut { self.amount.clear(); self.note.clear(); ctx.memory_mut(|m| m.request_focus(egui::Id::new("transaction_amount"))); }
        ctx.set_visuals(egui::Visuals::light());
        let screen = ctx.available_rect();
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        ctx.set_style(style);

        egui::TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(24, 39, 68))
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(APP_NAME)
                                .size(25.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!(
                                "مدیریت ساده دخل‌وخرج • امروز {}",
                                persian_digits(&today_jalali())
                            ))
                            .size(13.0)
                            .color(Color32::from_rgb(196, 210, 233)),
                        );
                        if ui.button("چاپگر و بارکد").clicked() {
                            self.hardware_printers = hardware::list_printers().unwrap_or_default();
                            if self.hardware_printer.is_empty() {
                                self.hardware_printer = self.hardware_printers.iter()
                                    .find(|p| p.is_default).or(self.hardware_printers.first())
                                    .map(|p| p.name.clone()).unwrap_or_default();
                            }
                            self.show_hardware = true;
                        }
                        if ui.button("فروشگاه").clicked() {
                            if let Err(e) = self.refresh_business_products() {
                                self.business_status = format!("خواندن کالاها ناموفق بود: {e}");
                            }
                            self.show_business = true;
                        }
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(245, 247, 251))
                    .inner_margin(egui::Margin::symmetric(18, 18)),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    ui.horizontal(|ui| {
                        summary_card(
                            ui,
                            "موجودی خالص",
                            self.total_income - self.total_expense,
                            Color32::from_rgb(30, 75, 135),
                        );
                        summary_card(
                            ui,
                            "درآمد کل",
                            self.total_income,
                            Color32::from_rgb(20, 130, 92),
                        );
                        summary_card(
                            ui,
                            "هزینه کل",
                            self.total_expense,
                            Color32::from_rgb(190, 73, 73),
                        );
                    });
                    ui.add_space(14.0);
                    ui.columns(2, |cols| {
                        cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                            panel_title(ui, "ثبت تراکنش جدید");
                            Frame::new()
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(1.0, Color32::from_rgb(225, 230, 238)))
                                .corner_radius(egui::CornerRadius::same(12))
                                .inner_margin(16.0)
                                .show(ui, |ui| {
                                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                                        ui.label("نوع تراکنش");
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(self.kind == 1, "هزینه").clicked() {
                                                self.kind = 1;
                                            }
                                            if ui.selectable_label(self.kind == 0, "درآمد").clicked() {
                                                self.kind = 0;
                                            }
                                        });
                                        ui.label("مبلغ (تومان)");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.amount)
                                                .id(egui::Id::new("transaction_amount"))
                                                .hint_text("مثلاً ۲۵۰۰۰۰")
                                                .desired_width(f32::INFINITY),
                                        );
                                        ui.label("دسته‌بندی");
                                        egui::ComboBox::from_id_salt("category")
                                            .selected_text(&self.category)
                                            .width(ui.available_width())
                                            .show_ui(ui, |ui| {
                                                for c in [
                                                    "خوراک", "حمل‌ونقل", "خانه", "قبض و خدمات",
                                                    "درمان", "خرید", "حقوق", "فریلنسری",
                                                    "سرمایه‌گذاری", "سایر",
                                                ] {
                                                    ui.selectable_value(
                                                        &mut self.category,
                                                        c.to_string(),
                                                        c,
                                                    );
                                                }
                                            });
                                        ui.label("تاریخ شمسی");
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::TextEdit::singleline(&mut self.date)
                                                    .hint_text("۱۴۰۵/۰۷/۰۲")
                                                    .desired_width(ui.available_width() - 42.0),
                                            );
                                            if ui.button("📅").on_hover_text("باز کردن تاریخ‌گزین شمسی").clicked() {
                                                let normalized: String = self.date.chars().map(normalize_digit).collect();
                                                let parts: Vec<&str> = normalized.split('/').collect();
                                                if parts.len() == 3 {
                                                    if let Ok(y) = parts[0].parse() { self.picker_year = y; }
                                                    if let Ok(m) = parts[1].parse::<i32>() { self.picker_month = m.clamp(1, 12); }
                                                }
                                                self.show_date_picker = true;
                                            }
                                        });
                                        ui.label("یادداشت (اختیاری)");
                                        ui.add(
                                            egui::TextEdit::multiline(&mut self.note)
                                                .hint_text("توضیح کوتاه...")
                                                .desired_rows(2)
                                                .desired_width(f32::INFINITY),
                                        );
                                        ui.add_space(8.0);
                                        if ui
                                            .add_sized(
                                                [ui.available_width(), 42.0],
                                                egui::Button::new(
                                                    RichText::new("ثبت تراکنش")
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                )
                                                .fill(Color32::from_rgb(31, 91, 163)),
                                            )
                                            .clicked()
                                        {
                                            self.add();
                                        }
                                    });
                                });
                            ui.add_space(8.0);
                            ui.label(RichText::new(&self.status).size(12.0));
                        });

                        cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.search)
                                        .id(egui::Id::new("transaction_search"))
                                        .hint_text("جست‌وجوی شرح یا دسته‌بندی")
                                        .desired_width(ui.available_width() - 100.0),
                                );
                                panel_title(ui, "تراکنش‌ها");
                            });
                            Frame::new()
                                .fill(Color32::WHITE)
                                .stroke(Stroke::new(1.0, Color32::from_rgb(225, 230, 238)))
                                .corner_radius(egui::CornerRadius::same(12))
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .max_height((screen.height() - 210.0).max(180.0))
                                        .auto_shrink([false; 2])
                                        .show(ui, |ui| {
                                            let needle = self.search.to_lowercase();
                                            let filtered: Vec<Transaction> = self
                                                .items
                                                .iter()
                                                .filter(|t| {
                                                    needle.is_empty()
                                                        || t.note.to_lowercase().contains(&needle)
                                                        || t.category.to_lowercase().contains(&needle)
                                                        || t.date.contains(&needle)
                                                })
                                                .cloned()
                                                .collect();
                                            if filtered.is_empty() {
                                                ui.vertical_centered(|ui| {
                                                    ui.add_space(36.0);
                                                    ui.label("هنوز تراکنشی ثبت نشده");
                                                    ui.label("از فرم کنار صفحه اولین درآمد یا هزینه‌ات را ثبت کن.");
                                                    ui.add_space(36.0);
                                                });
                                            } else {
                                                for t in filtered {
                                                    ui.horizontal(|ui| {
                                                        if ui.small_button("حذف").clicked() {
                                                            self.show_delete = Some(t.id);
                                                        }
                                                        ui.with_layout(
                                                            Layout::right_to_left(Align::Center),
                                                            |ui| {
                                                                ui.vertical(|ui| {
                                                                    ui.label(
                                                                        RichText::new(if t.note.is_empty() {
                                                                            t.category.clone()
                                                                        } else {
                                                                            t.note.clone()
                                                                        })
                                                                        .strong(),
                                                                    );
                                                                    ui.label(
                                                                        RichText::new(format!(
                                                                            "{} • {}",
                                                                            t.category,
                                                                            persian_digits(&t.date)
                                                                        ))
                                                                        .size(11.0)
                                                                        .color(Color32::GRAY),
                                                                    );
                                                                });
                                                                let color = if t.kind == "income" {
                                                                    Color32::from_rgb(20, 130, 92)
                                                                } else {
                                                                    Color32::from_rgb(190, 73, 73)
                                                                };
                                                                ui.label(
                                                                    RichText::new(format!(
                                                                        "{} تومان",
                                                                        money(t.amount)
                                                                    ))
                                                                    .strong()
                                                                    .color(color),
                                                                );
                                                            },
                                                        );
                                                    });
                                                    ui.separator();
                                                }
                                            }
                                        });
                                });
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("میان‌برها: Ctrl+Enter ثبت • Ctrl+F جست‌وجو • Ctrl+N فرم جدید • F1 راهنما").size(11.0).color(Color32::from_rgb(100, 110, 125)));
                        if ui.small_button("راهنمای کلیدها").clicked() { self.show_shortcuts = true; }
                    });
                    ui.label(
                        RichText::new("نبض • نسخهٔ آغازین ۰٫۱ • داده‌ها روی دستگاه شما می‌مانند")
                            .size(11.0)
                            .color(Color32::from_rgb(130, 140, 155)),
                    );
                });
            });

        if self.show_shortcuts {
            egui::Window::new("راهنمای میان‌برهای نبض")
                .open(&mut self.show_shortcuts)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("shortcut_help_grid").striped(true).show(ui, |ui| {
                        ui.label(RichText::new("کلید").strong()); ui.label(RichText::new("عملکرد").strong()); ui.end_row();
                        ui.label("Ctrl + Enter"); ui.label("ثبت درآمد یا هزینه"); ui.end_row();
                        ui.label("Ctrl + N"); ui.label("شروع فرم ثبت جدید و رفتن به فیلد مبلغ"); ui.end_row();
                        ui.label("Ctrl + F"); ui.label("رفتن به جست‌وجوی تراکنش‌ها"); ui.end_row();
                        ui.label("F1 یا Ctrl + /"); ui.label("نمایش یا بستن همین راهنما"); ui.end_row();
                        ui.label("دکمهٔ 📅"); ui.label("باز کردن تاریخ‌گزین شمسی"); ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.label("هنگام تایپ داخل بعضی فیلدها، ممکن است سیستم‌عامل یا ابزار ورودی کلیدهای میان‌بر را زودتر دریافت کند.");
                });
        }

        if self.show_date_picker {
            let mut close_picker = false;
            egui::Window::new("انتخاب تاریخ شمسی")
                .open(&mut self.show_date_picker)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("ماه بعد ←").clicked() {
                            self.picker_month += 1;
                            if self.picker_month > 12 { self.picker_month = 1; self.picker_year += 1; }
                        }
                        ui.label(RichText::new(format!("{} {}", persian_digits(&self.picker_year.to_string()), jalali_month_name(self.picker_month))).strong().size(16.0));
                        if ui.button("→ ماه قبل").clicked() {
                            self.picker_month -= 1;
                            if self.picker_month < 1 { self.picker_month = 12; self.picker_year -= 1; }
                        }
                    });
                    ui.separator();
                    let days = if self.picker_month <= 6 { 31 } else if self.picker_month <= 11 { 30 } else { 29 };
                    let weekdays = ["ش", "ی", "د", "س", "چ", "پ", "ج"];
                    egui::Grid::new("jalali_date_grid").spacing(Vec2::new(8.0, 8.0)).show(ui, |ui| {
                        for w in weekdays { ui.label(RichText::new(w).strong()); }
                        ui.end_row();
                        let first_offset = jalali_weekday_offset(self.picker_year, self.picker_month);
                        for cell in 0..(first_offset + days + 6) / 7 * 7 {
                            if cell < first_offset || cell >= first_offset + days {
                                ui.label(" ");
                            } else {
                                let day = cell - first_offset + 1;
                                if ui.button(persian_digits(&day.to_string())).clicked() {
                                    self.date = persian_digits(&format!("{:04}/{:02}/{:02}", self.picker_year, self.picker_month, day));
                                    close_picker = true;
                                }
                            }
                            if cell % 7 == 6 { ui.end_row(); }
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("امروز").clicked() { self.date = today_jalali(); close_picker = true; }
                        if ui.button("بستن").clicked() { close_picker = true; }
                    });
                });
            if close_picker { self.show_date_picker = false; }
        }

        if let Some(id) = self.show_delete {
            egui::Window::new("حذف تراکنش؟")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("این تراکنش برای همیشه از پایگاه داده محلی حذف می‌شود.");
                    ui.horizontal(|ui| {
                        if ui.button("انصراف").clicked() {
                            self.show_delete = None;
                        }
                        if ui.button(RichText::new("حذف کن").color(Color32::RED)).clicked() {
                            self.delete(id);
                        }
                    });
                });
        }
        if self.show_business {
            let mut show_business = self.show_business;
            egui::Window::new("فروش و انبار کالا")
                .open(&mut show_business)
                .default_width(680.0)
                .default_height(620.0)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        ui.heading("ثبت کالای جدید");
                        egui::Grid::new("product_entry_grid").num_columns(2).spacing(Vec2::new(8.0, 5.0)).show(ui, |ui| {
                            ui.label("نام کالا"); ui.text_edit_singleline(&mut self.product_name); ui.end_row();
                            ui.label("شناسه داخلی (SKU)"); ui.text_edit_singleline(&mut self.product_sku); ui.end_row();
                            ui.label("بارکد"); ui.text_edit_singleline(&mut self.product_barcode); ui.end_row();
                            ui.label("قیمت خرید (تومان)"); ui.text_edit_singleline(&mut self.product_purchase_price); ui.end_row();
                            ui.label("قیمت فروش (تومان)"); ui.text_edit_singleline(&mut self.product_sale_price); ui.end_row();
                            ui.label("موجودی اولیه"); ui.text_edit_singleline(&mut self.product_stock); ui.end_row();
                        });
                        if ui.button("ذخیره کالا").clicked() {
                            let purchase = parse_business_number(&self.product_purchase_price);
                            let sale = parse_business_number(&self.product_sale_price);
                            let stock = parse_business_number(&self.product_stock);
                            match (purchase, sale, stock) {
                                (Some(purchase), Some(sale), Some(stock)) => {
                                    match business::add_product(&self.db, &self.product_name, &self.product_sku, &self.product_barcode, purchase, sale, stock) {
                                        Ok(_) => {
                                            self.business_status = "کالا ذخیره شد.".into();
                                            self.product_name.clear();
                                            self.product_sku.clear();
                                            self.product_barcode.clear();
                                            if let Err(e) = self.refresh_business_products() { self.business_status = format!("کالا ثبت شد؛ بارگذاری مجدد ناموفق بود: {e}"); }
                                        }
                                        Err(e) => self.business_status = e,
                                    }
                                }
                                _ => self.business_status = "قیمت‌ها و موجودی باید عدد صحیح نامنفی باشند.".into(),
                            }
                        }
                        ui.separator();
                        ui.heading("فروش با بارکدخوان USB/HID");
                        ui.label("فیلد زیر را انتخاب کن و بارکد را اسکن کن؛ Enter در پایان هر اسکن کالا را به سبد اضافه می‌کند.");
                        let scan_response = ui.add(egui::TextEdit::singleline(&mut self.business_scan).hint_text("اسکن بارکد کالا").desired_width(f32::INFINITY));
                        if scan_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match business::scan_into_basket(&self.db, &mut self.business_basket, &self.business_scan) {
                                Ok(()) => self.business_status = "کالا به سبد فروش اضافه شد.".into(),
                                Err(e) => self.business_status = e,
                            }
                            self.business_scan.clear();
                            scan_response.request_focus();
                        }
                        ui.separator();
                        ui.heading(format!("سبد فروش • {} قلم", self.business_basket.len()));
                        egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                            egui::Grid::new("business_basket_grid").striped(true).show(ui, |ui| {
                                ui.label("کالا"); ui.label("تعداد"); ui.label("قیمت"); ui.label("جمع"); ui.end_row();
                                for item in &self.business_basket {
                                    ui.label(&item.name);
                                    ui.label(item.quantity.to_string());
                                    ui.label(money(item.unit_price));
                                    ui.label(money(item.line_total()));
                                    ui.end_row();
                                }
                            });
                        });
                        let total = self.business_basket.iter().fold(0_i64, |sum, item| sum.saturating_add(item.line_total()));
                        ui.label(RichText::new(format!("جمع کل: {} تومان", money(total))).strong().size(16.0));
                        ui.horizontal(|ui| {
                            if ui.button("ثبت فروش و کسر موجودی").clicked() {
                                match business::record_sale(&mut self.db, &self.business_basket, &today_jalali()) {
                                    Ok(sale_id) => {
                                        self.business_basket.clear();
                                        self.business_status = format!("فروش شمارهٔ {sale_id} ثبت شد و موجودی به‌روز شد.");
                                        if let Err(e) = self.refresh_business_products() { self.business_status = format!("فروش ثبت شد؛ به‌روزرسانی فهرست کالا ناموفق بود: {e}"); }
                                    }
                                    Err(e) => self.business_status = e,
                                }
                            }
                            if ui.button("خالی کردن سبد").clicked() { self.business_basket.clear(); }
                            if ui.button("به‌روزرسانی کالاها").clicked() {
                                if let Err(e) = self.refresh_business_products() { self.business_status = format!("خطا: {e}"); }
                            }
                        });
                        ui.separator();
                        ui.heading(format!("فهرست کالاها • {} کالا", self.business_products.len()));
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            egui::Grid::new("business_products_grid").striped(true).show(ui, |ui| {
                                ui.label("نام"); ui.label("بارکد"); ui.label("قیمت فروش"); ui.label("موجودی"); ui.end_row();
                                for product in &self.business_products {
                                    ui.label(&product.name);
                                    ui.label(&product.barcode);
                                    ui.label(money(product.sale_price));
                                    ui.label(product.stock.to_string());
                                    ui.end_row();
                                }
                            });
                        });
                        ui.label(RichText::new(&self.business_status).size(12.0));
                    });
                });
            self.show_business = show_business;
        }

        if self.show_hardware {
            egui::Window::new("چاپگر و بارکدخوان")
                .open(&mut self.show_hardware)
                .collapsible(false)
                .default_width(460.0)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        ui.heading("چاپ آزمایشی");
                        ui.label("چاپگرهای لینوکس از مسیر CUPS شناسایی می‌شوند.");
                        ui.horizontal(|ui| {
                            if ui.button("بازخوانی چاپگرها").clicked() {
                                match hardware::list_printers() {
                                    Ok(printers) => {
                                        self.hardware_printers = printers;
                                        self.hardware_status = format!("{} چاپگر پیدا شد.", self.hardware_printers.len());
                                    }
                                    Err(e) => self.hardware_status = e,
                                }
                            }
                            egui::ComboBox::from_id_salt("hardware_printer")
                                .selected_text(if self.hardware_printer.is_empty() { "چاپگر پیش‌فرض" } else { &self.hardware_printer })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.hardware_printer, String::new(), "چاپگر پیش‌فرض");
                                    for printer in &self.hardware_printers {
                                        ui.selectable_value(&mut self.hardware_printer, printer.name.clone(), &printer.name);
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            if ui.button("تست چاپ A4").clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::A4, &mut self.hardware_status);
                            }
                            if ui.button("تست فیش‌زن ESC/POS").clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::ThermalEscPos, &mut self.hardware_status);
                            }
                        });
                        ui.separator();
                        ui.heading("آزمون بارکدخوان USB/HID");
                        ui.label("فیلد را انتخاب کن و یک بارکد اسکن کن؛ بارکدخوان معمولاً در پایان Enter می‌فرستد.");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.hardware_scan)
                                .hint_text("اسکن بارکد در این فیلد")
                                .desired_width(f32::INFINITY)
                        );
                        if response.clicked() { response.request_focus(); }
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if self.hardware_scan.is_empty() {
                                self.hardware_status = "هیچ بارکدی دریافت نشد.".into();
                            } else {
                                self.hardware_status = format!("بارکد دریافت شد: {}", self.hardware_scan);
                                self.hardware_scan.clear();
                                response.request_focus();
                            }
                        }
                        ui.separator();
                        ui.heading("تولید بارکد");
                        ui.label("Code 128 برای کد داخلی کالا؛ EAN-13 فقط با رقم کنترلی معتبر.");
                        ui.text_edit_singleline(&mut self.hardware_barcode);
                        ui.horizontal(|ui| {
                            if ui.button("ساخت SVG از Code 128").clicked() {
                                save_barcode_svg(&self.hardware_barcode, false, &mut self.hardware_status);
                            }
                            if ui.button("ساخت SVG از EAN-13").clicked() {
                                save_barcode_svg(&self.hardware_barcode, true, &mut self.hardware_status);
                            }
                        });
                        ui.label(RichText::new(&self.hardware_status).size(12.0));
                    });
                });
        }

        ctx.request_repaint_after(Duration::from_millis(150));
    }
}

fn main() -> eframe::Result<()> {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "noto-persian".to_owned(),
        FontData::from_static(FONT).into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "noto-persian".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "noto-persian".to_owned());

    let app = NabzApp::new().map_err(|e| {
        eprintln!("Nabz startup failed: {e}");
        eframe::Error::AppCreation(Box::new(std::io::Error::other(e)))
    })?;
    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app))
        }),
    )
}


fn parse_business_number(value: &str) -> Option<i64> {
    let normalized: String = value.chars().map(normalize_digit).collect();
    normalized.trim().replace(',', "").replace('٬', "").parse::<i64>().ok().filter(|v| *v >= 0)
}

fn run_printer_test(printer: &str, mode: hardware::PrinterMode, status: &mut String) {
    match hardware::create_test_receipt() {
        Ok(path) => {
            let selected = if printer.trim().is_empty() { None } else { Some(printer) };
            *status = match hardware::print_file(&path, selected, mode, 1) {
                Ok(message) => format!("درخواست چاپ به صف فرستاده شد: {message}"),
                Err(error) => format!("چاپ ناموفق بود: {error}"),
            };
            let _ = std::fs::remove_file(path);
        }
        Err(error) => *status = error,
    }
}

fn save_barcode_svg(value: &str, ean13: bool, status: &mut String) {
    let generated = if ean13 {
        hardware::ean13_svg(value.trim(), 80)
    } else {
        hardware::code128_svg(value.trim(), 80)
    };
    match generated {
        Ok(svg) => match data_path() {
            Ok(db_path) => {
                let result = (|| -> Result<PathBuf, String> {
                    let dir = db_path.parent().ok_or("مسیر دادهٔ نبض معتبر نیست")?.join("barcodes");
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let path = dir.join(if ean13 { "ean13.svg" } else { "code128.svg" });
                    std::fs::write(&path, svg).map_err(|e| e.to_string())?;
                    Ok(path)
                })();
                *status = match result {
                    Ok(path) => format!("بارکد SVG ذخیره شد: {}", path.display()),
                    Err(error) => format!("ذخیرهٔ بارکد ناموفق بود: {error}"),
                };
            }
            Err(error) => *status = error,
        },
        Err(error) => *status = error,
    }
}

fn summary_card(ui: &mut egui::Ui, title: &str, value: i64, color: Color32) {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(225, 230, 238)))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() / 3.0).max(130.0));
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                ui.label(RichText::new(title).size(12.0).color(Color32::from_rgb(100, 112, 130)));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} تومان", money(value)))
                        .size(19.0)
                        .strong()
                        .color(color),
                );
            });
        });
}

fn panel_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(17.0).strong().color(Color32::from_rgb(35, 48, 69)));
}

fn money(v: i64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('٬');
        }
        out.push(c);
    }
    persian_digits(&out)
}

fn persian_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => '۰', '1' => '۱', '2' => '۲', '3' => '۳', '4' => '۴',
            '5' => '۵', '6' => '۶', '7' => '۷', '8' => '۸', '9' => '۹',
            _ => c,
        })
        .collect()
}

fn normalize_digit(c: char) -> char {
    match c {
        '۰' => '0', '۱' => '1', '۲' => '2', '۳' => '3', '۴' => '4',
        '۵' => '5', '۶' => '6', '۷' => '7', '۸' => '8', '۹' => '9',
        '٠' => '0', '١' => '1', '٢' => '2', '٣' => '3', '٤' => '4',
        '٥' => '5', '٦' => '6', '٧' => '7', '٨' => '8', '٩' => '9',
        other => other,
    }
}

fn data_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/nabz/nabz.sqlite3"))
}

fn today_jalali() -> String {
    let now = Local::now().date_naive();
    let (jy, jm, jd) = gregorian_to_jalali(now.year(), now.month() as i32, now.day() as i32);
    format!("{jy:04}/{jm:02}/{jd:02}")
}

fn gregorian_to_jalali(gy: i32, gm: i32, gd: i32) -> (i32, i32, i32) {
    let g_days = [31, if is_gregorian_leap(gy) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let j_days = [31, 31, 31, 31, 31, 31, 30, 30, 30, 30, 30, 29];
    let gy2 = gy - 1600;
    let gm2 = gm - 1;
    let gd2 = gd - 1;
    let mut g_day_no = 365 * gy2 + (gy2 + 3) / 4 - (gy2 + 99) / 100 + (gy2 + 399) / 400;
    for i in 0..gm2 {
        g_day_no += g_days[i as usize];
    }
    g_day_no += gd2;
    let mut j_day_no = g_day_no - 79;
    let j_np = j_day_no / 12053;
    j_day_no %= 12053;
    let mut jy = 979 + 33 * j_np + 4 * (j_day_no / 1461);
    j_day_no %= 1461;
    if j_day_no >= 366 {
        jy += (j_day_no - 1) / 365;
        j_day_no = (j_day_no - 1) % 365;
    }
    let mut jm = 0;
    while jm < 11 && j_day_no >= j_days[jm as usize] {
        j_day_no -= j_days[jm as usize];
        jm += 1;
    }
    (jy, jm + 1, j_day_no + 1)
}

fn jalali_month_name(month: i32) -> &'static str {
    match month {
        1 => "فروردین", 2 => "اردیبهشت", 3 => "خرداد", 4 => "تیر",
        5 => "مرداد", 6 => "شهریور", 7 => "مهر", 8 => "آبان",
        9 => "آذر", 10 => "دی", 11 => "بهمن", _ => "اسفند",
    }
}

fn jalali_weekday_offset(year: i32, month: i32) -> usize {
    let mut offset = 0usize;
    if year >= 1400 {
        for y in 1400..year { offset = (offset + jalali_year_days(y)) % 7; }
    } else {
        for y in year..1400 { offset = (offset + jalali_year_days(y)) % 7; }
        offset = (7 - offset) % 7;
    }
    for m in 1..month { offset = (offset + if m <= 6 { 31 } else { 30 }) % 7; }
    offset
}

fn jalali_year_days(year: i32) -> usize {
    if [1399, 1403, 1408, 1412, 1416, 1420].contains(&year) { 366 } else { 365 }
}

fn is_gregorian_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn valid_jalali(s: &str) -> bool {
    let normalized: String = s.chars().map(normalize_digit).collect();
    let p: Vec<&str> = normalized.split('/').collect();
    if p.len() != 3 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        p[0].parse::<i32>(),
        p[1].parse::<i32>(),
        p[2].parse::<i32>(),
    ) else {
        return false;
    };
    (1200..=1600).contains(&y)
        && (1..=12).contains(&m)
        && d >= 1
        && d <= if m <= 6 { 31 } else if m <= 11 { 30 } else { 29 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_gregorian_dates_convert_to_jalali() {
        assert_eq!(gregorian_to_jalali(2024, 3, 20), (1403, 1, 1));
        assert_eq!(gregorian_to_jalali(2025, 3, 21), (1404, 1, 1));
        assert_eq!(gregorian_to_jalali(2024, 3, 19), (1402, 12, 29));
    }

    #[test]
    fn accepts_valid_jalali_dates_with_persian_digits() {
        assert!(valid_jalali("۱۴۰۵/۰۷/۰۲"));
        assert!(valid_jalali("1405/01/31"));
        assert!(valid_jalali("۱۴۰۵/۱۲/۲۹"));
    }

    #[test]
    fn rejects_invalid_jalali_dates() {
        assert!(!valid_jalali("1405/13/01"));
        assert!(!valid_jalali("1405/00/01"));
        assert!(!valid_jalali("1405/07/32"));
        assert!(!valid_jalali("1405/12/30"));
        assert!(!valid_jalali("not-a-date"));
    }

    #[test]
    fn formats_digits_in_persian() {
        assert_eq!(persian_digits("1405/07/02"), "۱۴۰۵/۰۷/۰۲");
    }

    #[test]
    fn normalizes_persian_and_arabic_indic_digits() {
        assert_eq!("۱۲۳٤".chars().map(normalize_digit).collect::<String>(), "1234");
    }

    #[test]
    fn groups_money_and_uses_persian_digits() {
        assert_eq!(money(1234567), "۱٬۲۳۴٬۵۶۷");
        assert_eq!(money(0), "۰");
    }

    #[test]
    fn validates_date_shape_and_year_range() {
        assert!(!valid_jalali("1199/01/01"));
        assert!(!valid_jalali("1601/01/01"));
        assert!(!valid_jalali("1405-01-01"));
    }
}
