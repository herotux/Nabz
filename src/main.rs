pub mod hardware;
pub mod business;

use chrono::{Datelike, Local};
use eframe::egui::{self, Align, Color32, FontData, FontDefinitions, FontFamily, Frame, Layout, RichText as EguiRichText, Stroke, Vec2};
use rusqlite::{params, Connection};
use std::{path::PathBuf, time::Duration};

const APP_NAME: &str = "نبض | فروشگاه";
const FONT: &[u8] = include_bytes!("../assets/Vazirmatn.ttf");
// egui does not shape Persian/Arabic text or consistently order RTL runs.
// Keep application strings in logical Unicode order and reshape only for display.
struct RichText;
impl RichText {
    fn new(text: impl ToString) -> EguiRichText {
        // egui 0.31 renders text in left-to-right visual order. fa-shaper reshapes
        // Persian glyphs and reverses RTL runs; mirror paired punctuation after
        // that visual reordering so parentheses remain readable.
        let shaped = fa_shaper::reshape(&text.to_string());
        let display = shaped
            .chars()
            .map(|ch| match ch {
                '(' => ')',
                ')' => '(',
                '[' => ']',
                ']' => '[',
                '{' => '}',
                '}' => '{',
                other => other,
            })
            .collect::<String>();
        EguiRichText::new(display)
    }
}


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
    business_tab: usize,
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
        business::init_schema(&db).map_err(|e| format!("آماده‌سازی کالاها و فروش ناموفق بود: {e}"))?;
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
            show_business: true,
            business_tab: 5,
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Do not create the store window in the same frame as the button click.
        // egui 0.31.1 can panic in hit_test when a newly opened, scrollable Window
        // overlaps the pointer widget from that same frame.
        let show_business_at_frame_start = self.show_business;
        // Defer every popup/window until the frame after its opening click. egui 0.32
        // can panic in hit_test when a new window overlaps the widget clicked to open it.
        let show_hardware_at_frame_start = self.show_hardware;
        let show_shortcuts_at_frame_start = self.show_shortcuts;
        let show_date_picker_at_frame_start = self.show_date_picker;
        let show_delete_at_frame_start = self.show_delete.is_some();
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
        ui.style_mut().spacing.item_spacing = Vec2::new(10.0, 9.0);
        ui.style_mut().spacing.button_padding = Vec2::new(12.0, 8.0);

        egui::Panel::top("header")
            .frame(
                Frame::new()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(224, 230, 238)))
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(APP_NAME)
                                .size(24.0)
                                .strong()
                                .color(Color32::from_rgb(20, 122, 92)),
                        );
                        ui.label(
                            RichText::new(format!("مدیریت فروش، کالا و انبار  •  {}", persian_digits(&today_jalali())))
                                .size(12.0)
                                .color(Color32::from_rgb(107, 122, 143)),
                        );
                    });
                    ui.add_space(18.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text(RichText::new("جستجو در کالاها، فاکتورها و تراکنش‌ها…"))
                            .desired_width(250.0),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("مدیر فروشگاه").size(11.0).strong().color(Color32::from_rgb(28,41,59)));
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("♙  {}", persian_digits(&today_jalali()))).size(11.0).color(Color32::from_rgb(107,122,143)));
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        if ui.button(RichText::new("⚙ چاپگر و بارکد")).clicked() {
                            self.hardware_printers = hardware::list_printers().unwrap_or_default();
                            if self.hardware_printer.is_empty() {
                                self.hardware_printer = self.hardware_printers.iter()
                                    .find(|p| p.is_default).or(self.hardware_printers.first())
                                    .map(|p| p.name.clone()).unwrap_or_default();
                            }
                            self.show_hardware = true;
                        }
                    });
                });
            });

        if !show_business_at_frame_start {
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(245, 247, 251))
                    .inner_margin(egui::Margin::symmetric(18, 18)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("dashboard_content")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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
                                        ui.label(RichText::new("نوع تراکنش"));
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(self.kind == 1, RichText::new("هزینه")).clicked() {
                                                self.kind = 1;
                                            }
                                            if ui.selectable_label(self.kind == 0, RichText::new("درآمد")).clicked() {
                                                self.kind = 0;
                                            }
                                        });
                                        ui.label(RichText::new("مبلغ به تومان"));
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.amount)
                                                .id(egui::Id::new("transaction_amount"))
                                                .hint_text(RichText::new("مثلاً ۲۵۰۰۰۰"))
                                                .desired_width(ui.available_width()),
                                        );
                                        ui.label(RichText::new("دسته‌بندی"));
                                        egui::ComboBox::from_id_salt("category")
                                            .selected_text(RichText::new(&self.category))
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
                                        ui.label(RichText::new("تاریخ شمسی"));
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::TextEdit::singleline(&mut self.date)
                                                    .hint_text(RichText::new("۱۴۰۵/۰۷/۰۲"))
                                                    .desired_width(ui.available_width() - 42.0),
                                            );
                                            if ui.button(RichText::new("📅")).on_hover_text(RichText::new("باز کردن تاریخ‌گزین شمسی")).clicked() {
                                                let normalized: String = self.date.chars().map(normalize_digit).collect();
                                                let parts: Vec<&str> = normalized.split('/').collect();
                                                if parts.len() == 3 {
                                                    if let Ok(y) = parts[0].parse() { self.picker_year = y; }
                                                    if let Ok(m) = parts[1].parse::<i32>() { self.picker_month = m.clamp(1, 12); }
                                                }                                                self.show_date_picker = true;
                                            }
                                        });
                                        ui.label(RichText::new("یادداشت (اختیاری)"));
                                        ui.add(
                                            egui::TextEdit::multiline(&mut self.note)
                                                .hint_text(RichText::new("توضیح کوتاه..."))
                                                .desired_rows(2)
                                                .desired_width(ui.available_width()),
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
                                        .hint_text(RichText::new("جست‌وجوی شرح یا دسته‌بندی"))
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
                                                    ui.label(RichText::new("هنوز تراکنشی ثبت نشده"));
                                                    ui.label(RichText::new("از فرم کنار صفحه اولین درآمد یا هزینه‌ات را ثبت کن."));
                                                    ui.add_space(36.0);
                                                });
                                            } else {
                                                for t in filtered {
                                                    ui.horizontal(|ui| {
                                                        if ui.small_button(RichText::new("حذف")).clicked() {
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
                        if ui.small_button(RichText::new("راهنمای کلیدها")).clicked() { self.show_shortcuts = true; }
                    });
                    ui.label(
                        RichText::new("نبض • نسخهٔ آغازین ۰٫۱ • داده‌ها روی دستگاه شما می‌مانند")
                            .size(11.0)
                            .color(Color32::from_rgb(130, 140, 155)),
                    );
                });
                    });
            });
        }

        if show_shortcuts_at_frame_start && self.show_shortcuts {
            egui::Window::new(RichText::new("راهنمای میان‌برهای نبض"))
                .open(&mut self.show_shortcuts)
                .collapsible(false)
                .resizable(false)
                .show(ui, |ui| {
                    egui::Grid::new("shortcut_help_grid").striped(true).show(ui, |ui| {
                        ui.label(RichText::new("کلید").strong()); ui.label(RichText::new("عملکرد").strong()); ui.end_row();
                        ui.label(RichText::new("Ctrl + Enter")); ui.label(RichText::new("ثبت درآمد یا هزینه")); ui.end_row();
                        ui.label(RichText::new("Ctrl + N")); ui.label(RichText::new("شروع فرم ثبت جدید و رفتن به فیلد مبلغ")); ui.end_row();
                        ui.label(RichText::new("Ctrl + F")); ui.label(RichText::new("رفتن به جست‌وجوی تراکنش‌ها")); ui.end_row();
                        ui.label(RichText::new("F1 یا Ctrl + /")); ui.label(RichText::new("نمایش یا بستن همین راهنما")); ui.end_row();
                        ui.label(RichText::new("دکمهٔ 📅")); ui.label(RichText::new("باز کردن تاریخ‌گزین شمسی")); ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("هنگام تایپ داخل بعضی فیلدها، ممکن است سیستم‌عامل یا ابزار ورودی کلیدهای میان‌بر را زودتر دریافت کند."));
                });
        }

        if show_date_picker_at_frame_start && self.show_date_picker {
            let mut close_picker = false;
            egui::Window::new(RichText::new("انتخاب تاریخ شمسی"))
                .open(&mut self.show_date_picker)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("ماه بعد ←")).clicked() {
                            self.picker_month += 1;
                            if self.picker_month > 12 { self.picker_month = 1; self.picker_year += 1; }
                        }
                        ui.label(RichText::new(format!("{} {}", persian_digits(&self.picker_year.to_string()), jalali_month_name(self.picker_month))).strong().size(16.0));
                        if ui.button(RichText::new("→ ماه قبل")).clicked() {
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
                                ui.label(RichText::new(" "));
                            } else {
                                let day = cell - first_offset + 1;
                                if ui.button(RichText::new(persian_digits(&day.to_string()))).clicked() {
                                    self.date = persian_digits(&format!("{:04}/{:02}/{:02}", self.picker_year, self.picker_month, day));
                                    close_picker = true;
                                }
                            }
                            if cell % 7 == 6 { ui.end_row(); }
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("امروز")).clicked() { self.date = today_jalali(); close_picker = true; }
                        if ui.button(RichText::new("بستن")).clicked() { close_picker = true; }
                    });
                });
            if close_picker { self.show_date_picker = false; }
        }
        if let Some(id) = self.show_delete.filter(|_| show_delete_at_frame_start) {
            egui::Window::new(RichText::new("حذف تراکنش؟"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("این تراکنش برای همیشه از پایگاه داده محلی حذف می‌شود."));
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("انصراف")).clicked() {
                            self.show_delete = None;
                        }
                        if ui.button(RichText::new("حذف کن").color(Color32::RED)).clicked() {
                            self.delete(id);
                        }
                    });
                });
        }
        egui::Panel::right("navigation")
            .resizable(false)
            .exact_width(240.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(26, 41, 61))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    ui.label(RichText::new("منوی اصلی").size(11.0).color(Color32::from_rgb(148, 168, 189)));
                    ui.add_space(12.0);
                    for (index, label, glyph) in [
                        (5, "نمای کلی", "▦"),
                        (0, "فروش و فاکتورها", "＋"),
                        (1, "کالاها و انبار", "▤"),
                        (2, "خریدها", "⇄"),
                        (3, "مشتریان و تأمین‌کنندگان", "♙"),
                        (6, "دریافت و پرداخت", "◈"),
                        (4, "هزینه‌ها و گزارش", "▧"),
                    ] {
                        let active = if index == 6 { !self.show_business } else { self.show_business && self.business_tab == index };
                        let button = egui::Button::new(
                            RichText::new(format!("{glyph}   {label}"))
                                .size(12.0)
                                .strong()
                                .color(if active { Color32::from_rgb(143, 222, 191) } else { Color32::from_rgb(214, 224, 235) }),
                        )
                        .fill(if active { Color32::from_rgb(36, 56, 79) } else { Color32::TRANSPARENT })
                        .corner_radius(egui::CornerRadius::same(8))
                        .min_size(Vec2::new(ui.available_width(), 40.0));
                        if ui.add(button).clicked() {
                            if index == 6 {
                                self.show_business = false;
                            } else {
                                self.business_tab = index;
                                self.show_business = true;
                            }
                        }
                        ui.add_space(5.0);
                    }
                    ui.add_space(14.0);
                    ui.separator();
                    ui.label(RichText::new("ابزارها").size(11.0).color(Color32::from_rgb(148, 168, 189)));
                    if ui.button(RichText::new("⚙  تنظیمات دستگاه و بارکد")).clicked() {
                        self.hardware_printers = hardware::list_printers().unwrap_or_default();
                        if self.hardware_printer.is_empty() {
                            self.hardware_printer = self.hardware_printers.iter()
                                .find(|p| p.is_default).or(self.hardware_printers.first())
                                .map(|p| p.name.clone()).unwrap_or_default();
                        }
                        self.show_hardware = true;
                    }
                    if ui.button(RichText::new("⇧  پشتیبان‌گیری")).clicked() {
                        let result = (|| -> Result<PathBuf, String> {
                            self.db.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e| e.to_string())?;
                            let source = data_path()?;
                            let parent = source.parent().ok_or("مسیر داده معتبر نیست")?;
                            let dir = parent.join("backups");
                            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                            let target = dir.join(format!("nabz-backup-{}.sqlite3", Local::now().format("%Y%m%d-%H%M%S")));
                            std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                            Ok(target)
                        })();
                        self.business_status = match result { Ok(path) => format!("پشتیبان ذخیره شد: {}", path.display()), Err(e) => format!("پشتیبان‌گیری ناموفق بود: {e}") };
                    }
                    if ui.button(RichText::new("؟  راهنما")).clicked() { self.show_shortcuts = true; }
                    ui.add_space(8.0);
                    ui.label(RichText::new("نبض حسابداری").strong().color(Color32::WHITE));
                    ui.label(RichText::new("نسخه دسکتاپ • ذخیره‌سازی محلی").size(10.0).color(Color32::from_rgb(173, 191, 209)));
                });
            });

        if show_business_at_frame_start {
            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(Color32::from_rgb(245, 247, 251))
                        .inner_margin(egui::Margin::symmetric(18, 18)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        if self.business_tab == 5 {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("نمای کلی فروشگاه").size(22.0).strong().color(Color32::from_rgb(28, 41, 59)));
                                    ui.label(RichText::new(format!("خلاصه وضعیت کسب‌وکار • {}", persian_digits(&today_jalali()))).size(12.0).color(Color32::from_rgb(107, 122, 143)));
                                });
                            });
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                for (label, tab) in [("＋ فاکتور فروش جدید",0usize),("＋ ثبت هزینه",4),("＋ افزودن کالا",1)] {
                                    if ui.button(RichText::new(label).strong()).clicked() { self.business_tab = tab; }
                                }
                                if ui.button(RichText::new("پشتیبان‌گیری")).clicked() {
                                    let result = (|| -> Result<PathBuf, String> {
                                        self.db.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e| e.to_string())?;
                                        let source = data_path()?;
                                        let parent = source.parent().ok_or("مسیر داده معتبر نیست")?;
                                        let dir = parent.join("backups");
                                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                                        let target = dir.join(format!("nabz-backup-{}.sqlite3", Local::now().format("%Y%m%d-%H%M%S")));
                                        std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                                        Ok(target)
                                    })();
                                    self.business_status = match result { Ok(path) => format!("پشتیبان ذخیره شد: {}", path.display()), Err(e) => format!("پشتیبان‌گیری ناموفق بود: {e}") };
                                }
                            });
                            ui.add_space(8.0);
                        }
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("business_tab_content")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                Frame::new()
                                    .fill(Color32::WHITE)
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(225, 230, 238)))
                                    .corner_radius(egui::CornerRadius::same(12))
                                    .inner_margin(egui::Margin::same(16))
                                    .show(ui, |ui| {

                        if self.business_tab == 5 {
                            let today = today_jalali();
                            let (today_sales, today_sales_count): (i64, i64) = self.db.query_row(
                                "SELECT COALESCE(SUM(total),0), COUNT(*) FROM sales WHERE date=?1",
                                [&today], |r| Ok((r.get(0)?, r.get(1)?))
                            ).unwrap_or((0, 0));
                            let today_receipts: i64 = self.db.query_row(
                                "SELECT COALESCE(SUM(amount),0) FROM transactions WHERE kind='income' AND date=?1",
                                [&today], |r| r.get(0)
                            ).unwrap_or(0);
                            let inventory_value: i64 = self.db.query_row(
                                "SELECT COALESCE(SUM(purchase_price * stock),0) FROM products", [], |r| r.get(0)
                            ).unwrap_or(0);
                            let receivables: i64 = self.db.query_row(
                                "SELECT COALESCE(SUM(CASE WHEN balance>0 THEN balance ELSE 0 END),0) FROM parties WHERE kind IN ('customer','both')",
                                [], |r| r.get(0)
                            ).unwrap_or(0);
                            let payables: i64 = self.db.query_row(
                                "SELECT COALESCE(SUM(CASE WHEN balance>0 THEN balance ELSE 0 END),0) FROM parties WHERE kind IN ('supplier','both')",
                                [], |r| r.get(0)
                            ).unwrap_or(0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("وضعیت امروز").size(16.0).strong().color(Color32::from_rgb(28, 41, 59)));
                                    ui.label(RichText::new(format!("{} فاکتور فروش ثبت شده", persian_digits(&today_sales_count.to_string()))).size(11.0).color(Color32::from_rgb(107, 122, 143)));
                                });
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    if ui.button(RichText::new("پشتیبان‌گیری")).clicked() {
                                        let result = (|| -> Result<PathBuf, String> {
                                            self.db.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e| e.to_string())?;
                                            let source = data_path()?;
                                            let parent = source.parent().ok_or("مسیر داده معتبر نیست")?;
                                            let dir = parent.join("backups");
                                            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                                            let target = dir.join(format!("nabz-backup-{}.sqlite3", Local::now().format("%Y%m%d-%H%M%S")));
                                            std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                                            Ok(target)
                                        })();
                                        self.business_status = match result { Ok(path) => format!("پشتیبان ذخیره شد: {}", path.display()), Err(e) => format!("پشتیبان‌گیری ناموفق بود: {e}") };
                                    }
                                });
                            });
                            ui.add_space(8.0);
                            ui.columns(4, |cols| {
                                let cards = [
                                    ("فروش امروز", money(today_sales), "تومان", Color32::from_rgb(20,122,92)),
                                    ("دریافت‌های امروز", money(today_receipts), "تومان", Color32::from_rgb(44,103,173)),
                                    ("مطالبات مشتریان", money(receivables), "تومان", Color32::from_rgb(190,126,39)),
                                    ("ارزش موجودی کالا", money(inventory_value), "تومان", Color32::from_rgb(99,91,176)),
                                ];
                                for (i, (title, value, unit, color)) in cards.iter().enumerate() {
                                    Frame::new().fill(Color32::WHITE)
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(224,230,238)))
                                        .corner_radius(egui::CornerRadius::same(10))
                                        .inner_margin(12.0)
                                        .show(&mut cols[i], |ui| {
                                            ui.set_min_height(100.0);
                                            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                                                ui.label(RichText::new(*title).size(11.0).color(Color32::from_rgb(107,122,143)));
                                                ui.add_space(6.0);
                                                ui.label(RichText::new(value).size(17.0).strong().color(*color));
                                                ui.label(RichText::new(*unit).size(10.0).color(Color32::GRAY));
                                            });
                                        });
                                }
                            });
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                for (label, tab) in [("＋ فاکتور فروش جدید",0usize),("＋ افزودن کالا",1),("＋ ثبت خرید",2),("＋ ثبت مشتری",3),("＋ ثبت هزینه",4)] {
                                    if ui.button(RichText::new(label).strong()).clicked() { self.business_tab = tab; }
                                }
                            });
                            ui.add_space(10.0);
                            ui.columns(2, |cols| {
                                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                                    ui.heading(RichText::new("پنج فاکتور اخیر"));
                                    Frame::new().fill(Color32::WHITE)
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(225,230,238)))
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .inner_margin(8.0).show(ui, |ui| {
                                            egui::Grid::new("dashboard_recent_sales").striped(true).num_columns(4).spacing(Vec2::new(8.0,7.0)).show(ui, |ui| {
                                                ui.label(RichText::new("شماره").strong());
                                                ui.label(RichText::new("تاریخ").strong());
                                                ui.label(RichText::new("مبلغ").strong());
                                                ui.label(RichText::new("وضعیت").strong()); ui.end_row();
                                                let needle: String = self.search.chars().map(normalize_digit).collect();
                                                let mut stmt = self.db.prepare("SELECT s.id,s.date,s.total FROM sales s WHERE (?1='' OR CAST(s.id AS TEXT) LIKE '%'||?1||'%' OR s.date LIKE '%'||?1||'%' OR CAST(s.total AS TEXT) LIKE '%'||?1||'%' OR EXISTS (SELECT 1 FROM sale_items si WHERE si.sale_id=s.id AND si.product_name LIKE '%'||?1||'%')) ORDER BY s.id DESC LIMIT 5").ok();
                                                if let Some(ref mut stmt) = stmt {
                                                    let rows = stmt.query_map([needle], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?)));
                                                    if let Ok(rows) = rows {
                                                        let mut any = false;
                                                        for row in rows.flatten() {
                                                            any = true;
                                                            ui.label(RichText::new(format!("#{}", persian_digits(&row.0.to_string()))));
                                                            ui.label(RichText::new(persian_digits(&row.1)));
                                                            ui.label(RichText::new(money(row.2)));
                                                            ui.label(RichText::new("ثبت‌شده").color(Color32::from_rgb(20,122,92))); ui.end_row();
                                                        }
                                                        if !any {
                                                            ui.label(RichText::new("هنوز فاکتوری ثبت نشده است."));
                                                            ui.label(""); ui.label(""); ui.label(""); ui.end_row();
                                                        }
                                                    }
                                                }
                                            });
                                        });
                                });
                                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                                    ui.heading(RichText::new("وضعیت حساب‌ها و انبار"));
                                    Frame::new().fill(Color32::WHITE)
                                        .stroke(Stroke::new(1.0, Color32::from_rgb(225,230,238)))
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .inner_margin(12.0).show(ui, |ui| {
                                            ui.label(RichText::new("مطالبات مشتریان").size(12.0).color(Color32::from_rgb(107,122,143)));
                                            ui.label(RichText::new(format!("{} تومان", money(receivables))).strong().color(Color32::from_rgb(190,126,39)));
                                            ui.separator();
                                            ui.label(RichText::new("بدهی تأمین‌کنندگان").size(12.0).color(Color32::from_rgb(107,122,143)));
                                            ui.label(RichText::new(format!("{} تومان", money(payables))).strong().color(Color32::from_rgb(190,73,73)));
                                            ui.separator();
                                            let low: Vec<business::Product> = self.business_products.iter().filter(|p| p.stock <= 3).cloned().collect();
                                            ui.label(RichText::new(format!("کالاهای کم‌موجودی: {}", persian_digits(&low.len().to_string()))).strong());
                                            for p in low.iter().take(4) {
                                                ui.label(RichText::new(format!("{} • موجودی {}", p.name, persian_digits(&p.stock.to_string()))).size(11.0));
                                            }
                                            if low.is_empty() { ui.label(RichText::new("همه کالاها موجودی کافی دارند.").size(11.0).color(Color32::from_rgb(20,122,92))); }
                                        });
                                });
                            });
                            ui.add_space(10.0);
                            ui.heading(RichText::new("فعالیت‌های اخیر"));
                            Frame::new().fill(Color32::WHITE)
                                .stroke(Stroke::new(1.0, Color32::from_rgb(225,230,238)))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(10.0).show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new(format!("فروش امروز: {} تومان", money(today_sales))).color(Color32::from_rgb(20,122,92)));
                                        ui.separator();
                                        ui.label(RichText::new(format!("خرید کل: {} تومان", money(business::business_totals(&self.db).map(|v| v.1).unwrap_or(0)))));
                                        ui.separator();
                                        ui.label(RichText::new(format!("هزینه فروشگاه: {} تومان", money(business::business_totals(&self.db).map(|v| v.2).unwrap_or(0)))));
                                        ui.separator();
                                        ui.label(RichText::new(format!("موجودی کل: {} قلم", persian_digits(&self.business_products.len().to_string()))));
                                    });
                                });
                        }
                        if self.business_tab == 1 {
                        ui.heading(RichText::new("ثبت کالای جدید"));
                        egui::Grid::new("product_entry_grid").num_columns(2).spacing(Vec2::new(8.0, 5.0)).show(ui, |ui| {
                            ui.label(RichText::new("نام کالا")); ui.text_edit_singleline(&mut self.product_name); ui.end_row();
                            ui.label(RichText::new("شناسه داخلی (SKU)")); ui.text_edit_singleline(&mut self.product_sku); ui.end_row();
                            ui.label(RichText::new("بارکد")); ui.text_edit_singleline(&mut self.product_barcode); ui.end_row();
                            ui.label(RichText::new("قیمت خرید به تومان")); ui.text_edit_singleline(&mut self.product_purchase_price); ui.end_row();
                            ui.label(RichText::new("قیمت فروش به تومان")); ui.text_edit_singleline(&mut self.product_sale_price); ui.end_row();
                            ui.label(RichText::new("موجودی اولیه")); ui.text_edit_singleline(&mut self.product_stock); ui.end_row();
                        });
                        if ui.button(RichText::new("ذخیره کالا")).clicked() {
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
                        }
                        if self.business_tab == 0 {
                        ui.heading(RichText::new("فروش با بارکدخوان USB/HID"));
                        ui.label(RichText::new("فیلد زیر را انتخاب کن و بارکد را اسکن کن؛ Enter در پایان هر اسکن کالا را به سبد اضافه می‌کند."));
                        let scan_response = ui.add(egui::TextEdit::singleline(&mut self.business_scan).hint_text(RichText::new("اسکن بارکد کالا")).desired_width(ui.available_width()));
                        if scan_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match business::scan_into_basket(&self.db, &mut self.business_basket, &self.business_scan) {
                                Ok(()) => self.business_status = "کالا به سبد فروش اضافه شد.".into(),
                                Err(e) => self.business_status = e,
                            }
                            self.business_scan.clear();
                            scan_response.request_focus();
                        }
                        ui.separator();
                        ui.heading(RichText::new(format!("سبد فروش • {} قلم", self.business_basket.len())));
                        egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                            egui::Grid::new("business_basket_grid").striped(true).show(ui, |ui| {
                                ui.label(RichText::new("کالا")); ui.label(RichText::new("تعداد")); ui.label(RichText::new("قیمت")); ui.label(RichText::new("جمع")); ui.end_row();
                                for item in &self.business_basket {
                                    ui.label(RichText::new(&item.name));
                                    ui.label(RichText::new(item.quantity.to_string()));
                                    ui.label(RichText::new(money(item.unit_price)));
                                    ui.label(RichText::new(money(item.line_total())));
                                    ui.end_row();
                                }
                            });
                        });
                        let total = self.business_basket.iter().fold(0_i64, |sum, item| sum.saturating_add(item.line_total()));
                        ui.label(RichText::new(format!("جمع کل: {} تومان", money(total))).strong().size(16.0));
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("ثبت فروش و کسر موجودی")).clicked() {
                                match business::record_sale(&mut self.db, &self.business_basket, &today_jalali()) {
                                    Ok(sale_id) => {
                                        self.business_basket.clear();
                                        self.business_status = format!("فروش شمارهٔ {sale_id} ثبت شد و موجودی به‌روز شد.");
                                        if let Err(e) = self.refresh_business_products() { self.business_status = format!("فروش ثبت شد؛ به‌روزرسانی فهرست کالا ناموفق بود: {e}"); }
                                    }
                                    Err(e) => self.business_status = e,
                                }
                            }
                            if ui.button(RichText::new("خالی کردن سبد")).clicked() { self.business_basket.clear(); }
                            if ui.button(RichText::new("به‌روزرسانی کالاها")).clicked() {
                                if let Err(e) = self.refresh_business_products() { self.business_status = format!("خطا: {e}"); }
                            }
                        });
                        ui.separator();
                        ui.heading(RichText::new(format!("فهرست کالاها • {} کالا", self.business_products.len())));
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            egui::Grid::new("business_products_grid").striped(true).show(ui, |ui| {
                                ui.label(RichText::new("نام")); ui.label(RichText::new("بارکد")); ui.label(RichText::new("قیمت فروش")); ui.label(RichText::new("موجودی")); ui.end_row();
                                for product in &self.business_products {
                                    ui.label(RichText::new(&product.name));
                                    ui.label(RichText::new(&product.barcode));
                                    ui.label(RichText::new(money(product.sale_price)));
                                    ui.label(RichText::new(product.stock.to_string()));
                                    ui.end_row();
                                }
                            });
                        });

                        }
                        if self.business_tab == 2 {
                        ui.heading(RichText::new("خرید و ورود موجودی"));
                        egui::Grid::new("purchase_entry_grid").num_columns(2).show(ui, |ui| {
                            ui.label(RichText::new("کالا"));
                            egui::ComboBox::from_id_salt("purchase_product")
                                .selected_text(RichText::new(self.business_products.iter().find(|p| p.id == self.purchase_product_id).map(|p| p.name.clone()).unwrap_or_else(|| "انتخاب کالا".into())))
                                .show_ui(ui, |ui| {
                                    for p in &self.business_products {
                                        ui.selectable_value(&mut self.purchase_product_id, p.id, RichText::new(format!("{} • موجودی {}", p.name, p.stock)));
                                    }
                                });
                            ui.end_row();
                            ui.label(RichText::new("تعداد")); ui.text_edit_singleline(&mut self.purchase_quantity); ui.end_row();
                            ui.label(RichText::new("قیمت خرید واحد")); ui.text_edit_singleline(&mut self.purchase_unit_cost); ui.end_row();
                            ui.label(RichText::new("پرداخت‌شده")); ui.text_edit_singleline(&mut self.purchase_paid); ui.end_row();
                            ui.label(RichText::new("تأمین‌کننده")); ui.text_edit_singleline(&mut self.purchase_supplier); ui.end_row();
                        });
                        if ui.button(RichText::new("ثبت خرید و افزایش موجودی")).clicked() {
                            let qty = parse_business_number(&self.purchase_quantity);
                            let cost = parse_business_number(&self.purchase_unit_cost);
                            let paid = parse_business_number(&self.purchase_paid);
                            match (qty, cost, paid) {
                                (Some(qty), Some(cost), Some(paid)) if self.purchase_product_id > 0 => {
                                    match business::record_purchase(&mut self.db, self.purchase_product_id, qty, cost, &self.purchase_supplier, &today_jalali(), paid, "") {
                                        Ok(id) => {
                                            self.business_status = format!("فاکتور خرید شمارهٔ {id} ثبت شد؛ موجودی افزایش یافت.");
                                            self.purchase_quantity = "1".into();
                                            let _ = self.refresh_business_products();
                                        }
                                        Err(e) => self.business_status = e,
                                    }
                                }
                                _ => self.business_status = "کالا را انتخاب کن و مقادیر معتبر وارد کن.".into(),
                            }
                        }
                        }
                        if self.business_tab == 3 {
                        ui.heading(RichText::new("مشتری و تأمین‌کننده"));
                        egui::Grid::new("party_entry_grid").num_columns(2).show(ui, |ui| {
                            ui.label(RichText::new("نام")); ui.text_edit_singleline(&mut self.party_name); ui.end_row();
                            ui.label(RichText::new("تلفن")); ui.text_edit_singleline(&mut self.party_phone); ui.end_row();
                            ui.label(RichText::new("نوع"));
                            egui::ComboBox::from_id_salt("party_kind").selected_text(RichText::new(match self.party_kind { 0 => "مشتری", 1 => "تأمین‌کننده", _ => "هر دو" })).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.party_kind, 0, RichText::new("مشتری"));
                                ui.selectable_value(&mut self.party_kind, 1, RichText::new("تأمین‌کننده"));
                                ui.selectable_value(&mut self.party_kind, 2, RichText::new("هر دو"));
                            });
                            ui.end_row();
                        });
                        if ui.button(RichText::new("ثبت طرف حساب")).clicked() {
                            let kind = match self.party_kind { 0 => "customer", 1 => "supplier", _ => "both" };                            match business::add_party(&self.db, &self.party_name, kind, &self.party_phone) {
                                Ok(_) => { self.business_status = "طرف حساب ثبت شد.".into(); self.party_name.clear(); self.party_phone.clear(); }
                                Err(e) => self.business_status = e,
                            }
                        }
                        match business::list_parties(&self.db) {
                            Ok(parties) => {
                                ui.label(RichText::new(format!("فهرست طرف حساب‌ها: {}", parties.len())));
                                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                                    for party in parties {
                                        ui.label(RichText::new(format!("{} • {} • مانده: {} تومان", party.name,
                                            match party.kind.as_str() { "customer" => "مشتری", "supplier" => "تأمین‌کننده", _ => "هر دو" },
                                            money(party.balance))));
                                    }
                                });
                            }
                            Err(e) => self.business_status = format!("خواندن طرف حساب‌ها ناموفق بود: {e}"),
                        }
                        }
                        if self.business_tab == 4 {
                        ui.heading(RichText::new("هزینه‌های فروشگاه"));
                        egui::Grid::new("expense_entry_grid").num_columns(2).show(ui, |ui| {
                            ui.label(RichText::new("دسته هزینه")); ui.text_edit_singleline(&mut self.expense_category); ui.end_row();
                            ui.label(RichText::new("مبلغ")); ui.text_edit_singleline(&mut self.expense_amount); ui.end_row();
                            ui.label(RichText::new("توضیح")); ui.text_edit_singleline(&mut self.expense_note); ui.end_row();
                        });
                        if ui.button(RichText::new("ثبت هزینه فروشگاه")).clicked() {
                            match parse_business_number(&self.expense_amount) {
                                Some(amount) => match business::record_expense(&self.db, &today_jalali(), &self.expense_category, amount, "cash", &self.expense_note) {
                                    Ok(id) => { self.business_status = format!("هزینهٔ شمارهٔ {id} ثبت شد."); self.expense_amount.clear(); self.expense_note.clear(); }
                                    Err(e) => self.business_status = e,
                                },
                                None => self.business_status = "مبلغ هزینه باید عدد معتبر باشد.".into(),
                            }
                        }
                        ui.separator();
                        ui.heading(RichText::new("گزارش فروشگاه"));
                        match business::business_totals(&self.db) {
                            Ok((sales, purchases, expenses, low_stock, gross_profit)) => {
                                ui.label(RichText::new(format!("فروش: {} تومان", money(sales))));
                                ui.label(RichText::new(format!("خرید: {} تومان", money(purchases))));
                                ui.label(RichText::new(format!("سود ناخالص فروش‌های ثبت‌شده: {} تومان", money(gross_profit))));
                                ui.label(RichText::new(format!("هزینه‌های فروشگاه: {} تومان", money(expenses))));
                                ui.label(RichText::new(format!("کالاهای کم‌موجودی (۳ یا کمتر): {}", low_stock)));
                            }
                            Err(e) => self.business_status = format!("گزارش ناموفق بود: {e}"),
                        }
                        if ui.button(RichText::new("خروجی CSV فروش‌ها")).clicked() {
                            match business::sales_csv(&self.db) {
                                Ok(csv) => match data_path() {
                                    Ok(path) => {
                                        let result = (|| -> Result<PathBuf, String> {
                                            let dir = path.parent().ok_or("مسیر داده معتبر نیست")?.join("exports");
                                            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                                            let file = dir.join(format!("nabz-sales-{}.csv", today_jalali().replace('/', "-")));
                                            std::fs::write(&file, csv).map_err(|e| e.to_string())?;
                                            Ok(file)
                                        })();
                                        self.business_status = match result { Ok(file) => format!("CSV ذخیره شد: {}", file.display()), Err(e) => e };
                                    }
                                    Err(e) => self.business_status = e,
                                },
                                Err(e) => self.business_status = format!("ساخت CSV ناموفق بود: {e}"),
                            }
                        }
                        if ui.button(RichText::new("پشتیبان‌گیری از پایگاه داده")).clicked() {
                            let result = (|| -> Result<PathBuf, String> {
                                self.db.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e| e.to_string())?;
                                let source = data_path()?;
                                let parent = source.parent().ok_or("مسیر داده معتبر نیست")?;
                                let dir = parent.join("backups");
                                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                                let target = dir.join(format!("nabz-backup-{}.sqlite3", Local::now().format("%Y%m%d-%H%M%S")));
                                std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
                                Ok(target)
                            })();
                            self.business_status = match result { Ok(path) => format!("پشتیبان ذخیره شد: {}", path.display()), Err(e) => format!("پشتیبان‌گیری ناموفق بود: {e}") };
                        }
                        }
                                    });
                            });
                    });
                    ui.separator();
                    ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(80, 95, 115)));
                });
        }

        if show_hardware_at_frame_start && self.show_hardware {
            egui::Window::new(RichText::new("چاپگر و بارکدخوان"))
                .open(&mut self.show_hardware)
                .collapsible(false)
                .default_width(460.0)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        ui.heading(RichText::new("چاپ آزمایشی"));
                        ui.label(RichText::new("چاپگرهای لینوکس از مسیر CUPS شناسایی می‌شوند."));
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("بازخوانی چاپگرها")).clicked() {
                                match hardware::list_printers() {
                                    Ok(printers) => {
                                        self.hardware_printers = printers;
                                        self.hardware_status = format!("{} چاپگر پیدا شد.", self.hardware_printers.len());
                                    }
                                    Err(e) => self.hardware_status = e,
                                }
                            }
                            egui::ComboBox::from_id_salt("hardware_printer")
                                .selected_text(RichText::new(if self.hardware_printer.is_empty() { "چاپگر پیش‌فرض" } else { &self.hardware_printer }))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.hardware_printer, String::new(), RichText::new("چاپگر پیش‌فرض"));
                                    for printer in &self.hardware_printers {
                                        ui.selectable_value(&mut self.hardware_printer, printer.name.clone(), RichText::new(&printer.name));
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("تست چاپ A4")).clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::A4, &mut self.hardware_status);
                            }
                            if ui.button(RichText::new("تست فیش‌زن ESC/POS")).clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::ThermalEscPos, &mut self.hardware_status);
                            }
                        });
                        ui.separator();
                        ui.heading(RichText::new("آزمون بارکدخوان USB/HID"));
                        ui.label(RichText::new("فیلد را انتخاب کن و یک بارکد اسکن کن؛ بارکدخوان معمولاً در پایان Enter می‌فرستد."));
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.hardware_scan)
                                .hint_text(RichText::new("اسکن بارکد در این فیلد"))
                                .desired_width(ui.available_width())
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
                        ui.heading(RichText::new("تولید بارکد"));
                        ui.label(RichText::new("Code 128 برای کد داخلی کالا؛ EAN-13 فقط با رقم کنترلی معتبر."));
                        ui.text_edit_singleline(&mut self.hardware_barcode);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("ساخت SVG از Code 128")).clicked() {
                                save_barcode_svg(&self.hardware_barcode, false, &mut self.hardware_status);
                            }
                            if ui.button(RichText::new("ساخت SVG از EAN-13")).clicked() {
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

fn run_printer_test(printer: &str, mode: hardware::PrinterMode, status: &mut String) {    match hardware::create_test_receipt() {
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