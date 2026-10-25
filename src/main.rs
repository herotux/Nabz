pub mod hardware;
pub mod business;

use chrono::{Datelike, Local};
use eframe::egui::{self, Align, Color32, FontData, FontDefinitions, FontFamily, Frame, Layout, Margin, RichText as EguiRichText, Rounding, Stroke, Vec2};
use rusqlite::{params, Connection};
use egui_phosphor::regular as ph;
use std::{path::PathBuf, time::Duration};

const APP_NAME: &str = "نبض | حسابداری فروشگاه";
const FONT: &[u8] = include_bytes!("../assets/Vazirmatn.ttf");

struct RichText;
impl RichText {
    fn new(text: impl ToString) -> EguiRichText {
        EguiRichText::new(text.to_string())
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
            business_tab: 5, // 5 = نمای کلی (Dashboard)
            business_status: "سیستم آماده کار است.".into(),
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
            expense_category: "قبوض و امور جاری".into(),
            expense_amount: String::new(),
            expense_note: String::new(),
            picker_year: today_jalali().split('/').next().and_then(|v| v.parse().ok()).unwrap_or(1403),
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
            self.status = "تاریخ شمسی معتبر نیست؛ نمونه: ۱۴۰۳/۰۷/۰۲".into();
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
                self.status = "تراکنش با موفقیت ذخیره شد.".into();
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
        let screen_height = ctx.screen_rect().height();

        // Handle global keyboard shortcuts
        let save_shortcut = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
        let help_shortcut = ctx.input(|i| (i.modifiers.ctrl && i.key_pressed(egui::Key::Slash)) || i.key_pressed(egui::Key::F1));
        let search_shortcut = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F));
        let new_shortcut = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::N));

        if save_shortcut { self.add(); }
        if help_shortcut { self.show_shortcuts = !self.show_shortcuts; }
        if search_shortcut { ctx.memory_mut(|m| m.request_focus(egui::Id::new("global_search"))); }
        if new_shortcut { self.amount.clear(); self.note.clear(); ctx.memory_mut(|m| m.request_focus(egui::Id::new("transaction_amount"))); }

        // Top Navigation / Header Bar
        egui::TopBottomPanel::top("header")
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(255, 255, 255))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
                    .inner_margin(Margin::symmetric(20.0, 12.0))
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Right Side: Brand Logo & Title
                    draw_nabz_mark(ui, 38.0);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("حسابداری چابک نبض").size(18.0).strong().color(Color32::from_rgb(15, 118, 110)));
                        ui.label(RichText::new("ویژه فروشگاه لوازم الکتریکی و برق").size(11.0).color(Color32::from_rgb(100, 116, 139)));
                    });

                    ui.add_space(24.0);

                    // Quick Actions / Status Summary
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(format!("{}  تنظیمات سخت‌افزار", ph::GEAR)).size(12.0)))
                            .clicked() {
                            self.hardware_printers = hardware::list_printers().unwrap_or_default();
                            if self.hardware_printer.is_empty() {
                                self.hardware_printer = self.hardware_printers.iter().find(|p| p.is_default)
                                    .or(self.hardware_printers.first()).map(|p| p.name.clone()).unwrap_or_default();
                            }
                            self.show_hardware = true;
                        }

                        if ui.add(egui::Button::new(RichText::new(format!("{}  پشتیبان‌گیری", ph::FLOPPY_DISK)).size(12.0)))
                            .clicked() {
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

                        ui.add_space(10.0);
                        let search_width = ui.available_width().min(240.0).max(120.0);
                        ui.add(egui::TextEdit::singleline(&mut self.search).horizontal_align(Align::Max).id(egui::Id::new("global_search"))
                            .hint_text("جست‌وجو...").desired_width(search_width));
                    });
                });
            });

        // Right Sidebar Navigation (RTL Navigation)
        egui::SidePanel::right("navigation")
            .resizable(false)
            .exact_width(220.0)
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(15, 23, 42)) // Modern Dark Navy Slate Sidebar
                    .inner_margin(Margin::same(12.0)),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("منوی مدیریت").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(12.0);

                    let menu_items = [
                        (5, "داشبورد و نمای کلی", ph::HOUSE),
                        (0, "صندوق فروش و فاکتور", ph::RECEIPT),
                        (1, "انبار و کالاها", ph::PACKAGE),
                        (2, "فاکتور خرید / ورود", ph::TRUCK),
                        (3, "مشتریان و حساب‌ها", ph::USERS),
                        (4, "هزینه‌ها و گزارش سود", ph::CHART_BAR),
                        (6, "دفترچه درآمد و هزینه", ph::ARROWS_LEFT_RIGHT),
                    ];

                    for (index, label, glyph) in menu_items {
                        let active = if index == 6 { !self.show_business } else { self.show_business && self.business_tab == index };

                        let bg_color = if active { Color32::from_rgb(15, 118, 110) } else { Color32::TRANSPARENT };
                        let text_color = if active { Color32::WHITE } else { Color32::from_rgb(203, 213, 225) };

                        let btn = egui::Button::new(
                            RichText::new(format!("{glyph}   {label}"))
                                .size(13.0)
                                .strong()
                                .color(text_color)
                        )
                        .fill(bg_color)
                        .rounding(Rounding::same(8.0))
                        .min_size(Vec2::new(ui.available_width(), 42.0));

                        if ui.add(btn).clicked() {
                            if index == 6 {
                                self.show_business = false;
                            } else {
                                self.business_tab = index;
                                self.show_business = true;
                            }
                        }
                        ui.add_space(6.0);
                    }

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(RichText::new("راهنما و اطلاعات").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(6.0);

                    if ui.button(RichText::new(format!("{}  راهنمای کلیدها", ph::QUESTION)).color(Color32::from_rgb(203, 213, 225))).clicked() {
                        self.show_shortcuts = true;
                    }

                    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new("نسخه ۱٫۰ • آفلاین محلی").size(10.0).color(Color32::from_rgb(100, 116, 139)));
                        ui.label(RichText::new("حسابداری فروشگاه الکتریکی").size(11.0).color(Color32::from_rgb(148, 163, 184)));
                    });
                });
            });

        // Main Central Content Area
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(248, 250, 252)) // Soft Gray-Blue Background
                    .inner_margin(Margin::symmetric(20.0, 20.0)),
            )
            .show(ctx, |ui| {
                if !self.show_business {
                    // Personal Accounting / General Cash Book View
                    egui::ScrollArea::vertical()
                        .id_salt("cashbook_scroll")
                        .show(ui, |ui| {
                            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                                ui.label(RichText::new("دفترچه ثبت درآمد و هزینه ساده").size(20.0).strong().color(Color32::from_rgb(15, 23, 42)));
                                ui.add_space(12.0);

                                ui.columns(3, |cols| {
                                    summary_card(&mut cols[0], "موجودی صندوق", self.total_income - self.total_expense, Color32::from_rgb(15, 118, 110));
                                    summary_card(&mut cols[1], "جمع کل درآمدها", self.total_income, Color32::from_rgb(16, 185, 129));
                                    summary_card(&mut cols[2], "جمع کل هزینه‌ها", self.total_expense, Color32::from_rgb(239, 68, 68));
                                });

                                ui.add_space(16.0);

                                ui.columns(2, |cols| {
                                    // Left Column: Transactions List
                                    cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                                        panel_title(ui, "لیست آخرین تراکنش‌ها");
                                        ui.add_space(6.0);
                                        Frame::none()
                                            .fill(Color32::WHITE)
                                            .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
                                            .rounding(Rounding::same(12.0))
                                            .inner_margin(Margin::same(14.0))
                                            .show(ui, |ui| {
                                                egui::ScrollArea::vertical().max_height((screen_height - 280.0).max(200.0)).show(ui, |ui| {
                                                    let needle = self.search.to_lowercase();
                                                    let filtered: Vec<Transaction> = self.items.iter()
                                                        .filter(|t| needle.is_empty() || t.note.to_lowercase().contains(&needle) || t.category.to_lowercase().contains(&needle) || t.date.contains(&needle))
                                                        .cloned().collect();

                                                    if filtered.is_empty() {
                                                        ui.vertical_centered(|ui| {
                                                            ui.add_space(40.0);
                                                            ui.label(RichText::new("هیچ تراکنشی یافت نشد.").color(Color32::GRAY));
                                                            ui.add_space(40.0);
                                                        });
                                                    } else {
                                                        for t in filtered {
                                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                                if ui.button(RichText::new("حذف").color(Color32::from_rgb(239, 68, 68)).size(11.0)).clicked() {
                                                                    self.show_delete = Some(t.id);
                                                                }

                                                                ui.vertical(|ui| {
                                                                    ui.label(RichText::new(if t.note.is_empty() { t.category.clone() } else { t.note.clone() }).strong().size(13.0));
                                                                    ui.label(RichText::new(format!("{} • {}", t.category, persian_digits(&t.date))).size(11.0).color(Color32::GRAY));
                                                                });

                                                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                                                    let color = if t.kind == "income" { Color32::from_rgb(16, 185, 129) } else { Color32::from_rgb(239, 68, 68) };
                                                                    ui.label(RichText::new(format!("{} تومان", money(t.amount))).strong().color(color));
                                                                });
                                                            });
                                                            ui.separator();
                                                        }
                                                    }
                                                });
                                            });
                                    });

                                    // Right Column: Add Transaction Form
                                    cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                                        panel_title(ui, "ثبت تراکنش جدید");
                                        ui.add_space(6.0);
                                        Frame::none()
                                            .fill(Color32::WHITE)
                                            .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
                                            .rounding(Rounding::same(12.0))
                                            .inner_margin(Margin::same(16.0))
                                            .show(ui, |ui| {
                                                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                                                    ui.label(RichText::new("نوع تراکنش").strong());
                                                    ui.horizontal(|ui| {
                                                        if ui.selectable_label(self.kind == 1, RichText::new("هزینه")).clicked() { self.kind = 1; }
                                                        if ui.selectable_label(self.kind == 0, RichText::new("درآمد")).clicked() { self.kind = 0; }
                                                    });
                                                    ui.add_space(8.0);

                                                    ui.label(RichText::new("مبلغ (تومان) *").strong());
                                                    ui.add(egui::TextEdit::singleline(&mut self.amount).horizontal_align(Align::Max)
                                                        .id(egui::Id::new("transaction_amount")).hint_text("مثلاً ۲۵۰,۰۰۰").desired_width(ui.available_width()));
                                                    ui.add_space(8.0);

                                                    ui.label(RichText::new("دسته‌بندی").strong());
                                                    egui::ComboBox::from_id_salt("cashbook_category")
                                                        .selected_text(RichText::new(&self.category))
                                                        .width(ui.available_width())
                                                        .show_ui(ui, |ui| {
                                                            for c in ["قبوض و شارژ", "حمل و نقل", "خرید اقلام", "حقوق و دستمزد", "اجاره مغازه", "سایر"] {
                                                                ui.selectable_value(&mut self.category, c.to_string(), c);
                                                            }
                                                        });
                                                    ui.add_space(8.0);

                                                    ui.label(RichText::new("تاریخ شمسی *").strong());
                                                    ui.horizontal(|ui| {
                                                        ui.add(egui::TextEdit::singleline(&mut self.date).horizontal_align(Align::Max).desired_width(ui.available_width() - 40.0));
                                                        if ui.button(RichText::new(ph::CALENDAR_BLANK)).clicked() {
                                                            self.show_date_picker = true;
                                                        }
                                                    });
                                                    ui.add_space(8.0);

                                                    ui.label(RichText::new("توضیحات (اختیاری)").strong());
                                                    ui.add(egui::TextEdit::multiline(&mut self.note).horizontal_align(Align::Max).desired_rows(2).desired_width(ui.available_width()));
                                                    ui.add_space(12.0);

                                                    if ui.add_sized([ui.available_width(), 40.0], egui::Button::new(RichText::new("ثبت تراکنش").strong().color(Color32::WHITE)).fill(Color32::from_rgb(15, 118, 110))).clicked() {
                                                        self.add();
                                                    }
                                                });
                                            });
                                        ui.add_space(8.0);
                                        ui.label(RichText::new(&self.status).size(12.0).color(Color32::from_rgb(100, 116, 139)));
                                    });
                                });
                            });
                        });
                } else {
                    // Business / Shop Management Views
                    egui::ScrollArea::vertical()
                        .id_salt("business_main_scroll")
                        .show(ui, |ui| {
                            match self.business_tab {
                                5 => self.ui_dashboard(ui),
                                0 => self.ui_sales_pos(ui),
                                1 => self.ui_products(ui),
                                2 => self.ui_purchases(ui),
                                3 => self.ui_parties(ui),
                                4 => self.ui_expenses_reports(ui),
                                _ => {}
                            }
                        });
                }
            });

        // Modals / Windows
        if self.show_shortcuts {
            egui::Window::new(RichText::new("راهنمای میان‌برهای سیستم"))
                .open(&mut self.show_shortcuts)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("shortcuts_grid").striped(true).spacing(Vec2::new(16.0, 8.0)).show(ui, |ui| {
                        ui.label(RichText::new("کلید میان‌بر").strong());
                        ui.label(RichText::new("عملکرد").strong());
                        ui.end_row();

                        ui.label(RichText::new("Ctrl + Enter"));
                        ui.label(RichText::new("ثبت تراکنش / فرم سریع"));
                        ui.end_row();

                        ui.label(RichText::new("Ctrl + F"));
                        ui.label(RichText::new("تمرکز روی کادر جست‌وجو"));
                        ui.end_row();

                        ui.label(RichText::new("F1 یا Ctrl + /"));
                        ui.label(RichText::new("نمایش این راهنما"));
                        ui.end_row();
                    });
                });
        }

        if self.show_date_picker {
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
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("امروز")).clicked() { self.date = today_jalali(); close_picker = true; }
                        if ui.button(RichText::new("بستن")).clicked() { close_picker = true; }
                    });
                });
            if close_picker { self.show_date_picker = false; }
        }

        if let Some(id) = self.show_delete {
            egui::Window::new(RichText::new("تأیید حذف"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("آیا از حذف این ردیف اطمینان دارید؟"));
                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("انصراف")).clicked() {
                            self.show_delete = None;
                        }
                        if ui.button(RichText::new("حذف کن").color(Color32::from_rgb(239, 68, 68))).clicked() {
                            self.delete(id);
                        }
                    });
                });
        }

        if self.show_hardware {
            egui::Window::new(RichText::new("تنظیمات چاپگر و بارکدخوان"))
                .open(&mut self.show_hardware)
                .collapsible(false)
                .default_width(450.0)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        ui.heading(RichText::new("تست چاپگر فاکتور"));
                        ui.label(RichText::new("چاپگر متصل را از لیست انتخاب کنید:"));
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new(format!("{}  بازخوانی", ph::ARROW_CLOCKWISE))).clicked() {
                                if let Ok(printers) = hardware::list_printers() {
                                    self.hardware_printers = printers;
                                }
                            }
                            egui::ComboBox::from_id_salt("hardware_printer_select")
                                .selected_text(RichText::new(if self.hardware_printer.is_empty() { "چاپگر پیش‌فرض" } else { &self.hardware_printer }))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.hardware_printer, String::new(), RichText::new("چاپگر پیش‌فرض"));
                                    for p in &self.hardware_printers {
                                        ui.selectable_value(&mut self.hardware_printer, p.name.clone(), RichText::new(&p.name));
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("تست چاپ A4")).clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::A4, &mut self.hardware_status);
                            }
                            if ui.button(RichText::new("تست فیش‌زن")).clicked() {
                                run_printer_test(&self.hardware_printer, hardware::PrinterMode::ThermalEscPos, &mut self.hardware_status);
                            }
                        });
                        ui.separator();
                        ui.label(RichText::new(&self.hardware_status).size(12.0).color(Color32::from_rgb(100, 116, 139)));
                    });
                });
        }

        ctx.request_repaint_after(Duration::from_millis(150));
    }
}

impl NabzApp {
    fn ui_dashboard(&mut self, ui: &mut egui::Ui) {
        let today = today_jalali();
        let (today_sales, _today_sales_count): (i64, i64) = self.db.query_row(
            "SELECT COALESCE(SUM(total),0), COUNT(*) FROM sales WHERE date=?1",
            [&today], |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap_or((0, 0));

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

        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("داشبورد و نمای کلی فروشگاه").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new(format!("خلاصه وضعیت کسب‌وکار در تاریخ {}", persian_digits(&today))).size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(14.0);

            ui.columns(4, |cols| {
                summary_card(&mut cols[0], "فروش امروز", today_sales, Color32::from_rgb(16, 185, 129));
                summary_card(&mut cols[1], "ارزش کل انبار (خرید)", inventory_value, Color32::from_rgb(59, 130, 246));
                summary_card(&mut cols[2], "طلب از مشتریان", receivables, Color32::from_rgb(245, 158, 11));
                summary_card(&mut cols[3], "بدهی به تامین‌کنندگان", payables, Color32::from_rgb(239, 68, 68));
            });

            ui.add_space(20.0);

            ui.columns(2, |cols| {
                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "آخرین فاکتورهای فروش");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        egui::Grid::new("dash_sales_grid").striped(true).num_columns(4).spacing(Vec2::new(12.0, 8.0)).show(ui, |ui| {
                            ui.label(RichText::new("شماره").strong());
                            ui.label(RichText::new("تاریخ").strong());
                            ui.label(RichText::new("مبلغ کل").strong());
                            ui.label(RichText::new("وضعیت").strong());
                            ui.end_row();

                            let mut stmt = self.db.prepare("SELECT id, date, total FROM sales ORDER BY id DESC LIMIT 5").ok();
                            if let Some(ref mut stmt) = stmt {
                                if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_,i64>(0)?, r.get::<_,String>(1)?, r.get::<_,i64>(2)?))) {
                                    let mut count = 0;
                                    for row in rows.flatten() {
                                        count += 1;
                                        ui.label(RichText::new(format!("#{}", persian_digits(&row.0.to_string()))));
                                        ui.label(RichText::new(persian_digits(&row.1)));
                                        ui.label(RichText::new(format!("{} تومان", money(row.2))));
                                        ui.label(RichText::new("ثبت شده").color(Color32::from_rgb(16, 185, 129)));
                                        ui.end_row();
                                    }
                                    if count == 0 {
                                        ui.label(RichText::new("فاکتوری ثبت نشده است."));
                                        ui.label(""); ui.label(""); ui.label("");
                                        ui.end_row();
                                    }
                                }
                            }
                        });
                    });
                });

                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "هشدار موجودی کالاها");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        let low_stock: Vec<business::Product> = self.business_products.iter().filter(|p| p.stock <= 5).cloned().collect();
                        if low_stock.is_empty() {
                            ui.label(RichText::new("همه کالاها موجودی کافی دارند.").color(Color32::from_rgb(16, 185, 129)));
                        } else {
                            ui.label(RichText::new(format!("تعداد {} کالا نیاز به شارژ انبار دارند:", persian_digits(&low_stock.len().to_string()))).color(Color32::from_rgb(239, 68, 68)).strong());
                            ui.add_space(6.0);
                            for p in low_stock.iter().take(5) {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&p.name).strong());
                                    ui.label(RichText::new(format!("• موجودی فعلی: {}", persian_digits(&p.stock.to_string()))).color(Color32::from_rgb(239, 68, 68)));
                                });
                                ui.separator();
                            }
                        }
                    });
                });
            });
        });
    }

    fn ui_sales_pos(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("صندوق فروش سریع و صدور فاکتور").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new("کالاها را اسکن کنید یا از لیست زیر به سبد خرید اضافه کنید.").size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(12.0);

            // Barcode Input Box
            Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(10.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}  اسکن بارکد:", ph::BARCODE)).strong().size(14.0));
                    let response = ui.add(egui::TextEdit::singleline(&mut self.business_scan)
                        .horizontal_align(Align::Max)
                        .hint_text("کد کالا یا بارکدخوان...")
                        .desired_width(ui.available_width() - 100.0));

                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        match business::scan_into_basket(&self.db, &mut self.business_basket, &self.business_scan) {
                            Ok(()) => self.business_status = "کالا به سبد افزود شد.".into(),
                            Err(e) => self.business_status = e,
                        }
                        self.business_scan.clear();
                        response.request_focus();
                    }
                });
            });

            ui.add_space(14.0);

            ui.columns(2, |cols| {
                // Left Column: Shopping Basket
                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "سبد خرید مشتری");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                            egui::Grid::new("pos_basket_grid").striped(true).num_columns(5).spacing(Vec2::new(10.0, 8.0)).show(ui, |ui| {
                                ui.label(RichText::new("نام کالا").strong());
                                ui.label(RichText::new("تعداد").strong());
                                ui.label(RichText::new("فی (تومان)").strong());
                                ui.label(RichText::new("جمع").strong());
                                ui.label("");
                                ui.end_row();

                                for item in &self.business_basket {
                                    ui.label(RichText::new(&item.name));
                                    ui.label(RichText::new(persian_digits(&item.quantity.to_string())));
                                    ui.label(RichText::new(money(item.unit_price)));
                                    ui.label(RichText::new(money(item.line_total())));
                                    ui.label("");
                                    ui.end_row();
                                }
                            });
                        });

                        ui.separator();
                        let total = self.business_basket.iter().fold(0_i64, |sum, item| sum.saturating_add(item.line_total()));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("مبلغ قابل پرداخت:").size(16.0).strong());
                            ui.label(RichText::new(format!("{} تومان", money(total))).size(18.0).strong().color(Color32::from_rgb(15, 118, 110)));
                        });

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.add_sized([140.0, 36.0], egui::Button::new(RichText::new("ثبت و تسویه فاکتور").strong().color(Color32::WHITE)).fill(Color32::from_rgb(16, 185, 129))).clicked() {
                                match business::record_sale(&mut self.db, &self.business_basket, &today_jalali()) {
                                    Ok(sale_id) => {
                                        self.business_basket.clear();
                                        self.business_status = format!("فاکتور فروش شماره #{sale_id} با موفقیت صادر شد.");
                                        let _ = self.refresh_business_products();
                                    }
                                    Err(e) => self.business_status = e,
                                }
                            }

                            if ui.button(RichText::new("خالی کردن سبد").color(Color32::from_rgb(239, 68, 68))).clicked() {
                                self.business_basket.clear();
                            }
                        });
                    });
                });

                // Right Column: Quick Product Selector List
                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "انتخاب سریع کالاها");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                            for p in &self.business_products.clone() {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&p.name).strong());
                                        ui.label(RichText::new(format!("قیمت: {} تومان • موجودی: {}", money(p.sale_price), persian_digits(&p.stock.to_string()))).size(11.0).color(Color32::GRAY));
                                    });

                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        if ui.button(RichText::new("افزودن +").color(Color32::from_rgb(15, 118, 110))).clicked() {
                                            match business::scan_into_basket(&self.db, &mut self.business_basket, &p.barcode) {
                                                Ok(()) => {},
                                                Err(e) => self.business_status = e,
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                            }
                        });
                    });
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(15, 118, 110)));
        });
    }

    fn ui_products(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("مدیریت کالاها و انبار").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new("تعریف سیم، کابل، کلید و پریز، روشنایی و سایر قطعات الکتریکی.").size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(12.0);

            ui.columns(2, |cols| {
                // Form Column
                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "تعریف / ثبت کالای جدید");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        ui.label(RichText::new("نام کالا *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.product_name).horizontal_align(Align::Max).hint_text("مثلاً: سیم ۲٫۵ خراسان افشار").desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("بارکد / کد شناسه").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.product_barcode).horizontal_align(Align::Max).hint_text("در صورت خالی بودن خودکار ساخته می‌شود").desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("قیمت خرید واحد (تومان) *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.product_purchase_price).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("قیمت فروش واحد (تومان) *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.product_sale_price).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("موجودی اولیه *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.product_stock).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(14.0);

                        if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(RichText::new("ذخیره کالا در انبار").strong().color(Color32::WHITE)).fill(Color32::from_rgb(15, 118, 110))).clicked() {
                            let name = self.product_name.trim().to_owned();
                            let purchase = parse_business_number(&self.product_purchase_price);
                            let sale = parse_business_number(&self.product_sale_price);
                            let stock = parse_business_number(&self.product_stock);

                            if name.is_empty() {
                                self.business_status = "نام کالا الزامی است.".into();
                            } else if let (Some(purchase), Some(sale), Some(stock)) = (purchase, sale, stock) {
                                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                                let sku = if self.product_sku.trim().is_empty() { format!("SKU-{stamp}") } else { self.product_sku.clone() };
                                let barcode = if self.product_barcode.trim().is_empty() { format!("NABZ{stamp}") } else { self.product_barcode.clone() };

                                match business::add_product(&self.db, &name, &sku, &barcode, purchase, sale, stock) {
                                    Ok(_) => {
                                        self.business_status = format!("کالای «{name}» با موفقیت ثبت شد.");
                                        self.product_name.clear();
                                        self.product_barcode.clear();
                                        self.product_purchase_price = "0".into();
                                        self.product_sale_price = "0".into();
                                        self.product_stock = "0".into();
                                        let _ = self.refresh_business_products();
                                    }
                                    Err(e) => self.business_status = e,
                                }
                            } else {
                                self.business_status = "لطفاً مقادیر عددی معتبر وارد کنید.".into();
                            }
                        }
                    });
                });

                // List Column
                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "فهرست کالاهای موجود");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            egui::Grid::new("products_list_grid").striped(true).num_columns(4).spacing(Vec2::new(12.0, 8.0)).show(ui, |ui| {
                                ui.label(RichText::new("نام کالا").strong());
                                ui.label(RichText::new("قیمت فروش").strong());
                                ui.label(RichText::new("قیمت خرید").strong());
                                ui.label(RichText::new("موجودی").strong());
                                ui.end_row();

                                for p in &self.business_products {
                                    ui.label(RichText::new(&p.name));
                                    ui.label(RichText::new(money(p.sale_price)));
                                    ui.label(RichText::new(money(p.purchase_price)));
                                    let stock_color = if p.stock <= 3 { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(16, 185, 129) };
                                    ui.label(RichText::new(persian_digits(&p.stock.to_string())).color(stock_color).strong());
                                    ui.end_row();
                                }
                            });
                        });
                    });
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(15, 118, 110)));
        });
    }

    fn ui_purchases(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("ثبت فاکتور خرید و ورود کالا به انبار").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new("افزایش موجودی انبار همراه با به‌روزرسانی قیمت خرید و حساب تامین‌کننده.").size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(12.0);

            Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                ui.label(RichText::new("انتخاب کالا *").strong());
                egui::ComboBox::from_id_salt("purchase_product_select")
                    .selected_text(RichText::new(self.business_products.iter().find(|p| p.id == self.purchase_product_id).map(|p| p.name.clone()).unwrap_or_else(|| "انتخاب کالا...".into())))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for p in &self.business_products {
                            ui.selectable_value(&mut self.purchase_product_id, p.id, RichText::new(format!("{} (موجودی: {})", p.name, persian_digits(&p.stock.to_string()))));
                        }
                    });
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("تعداد / مقدار ورودی *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.purchase_quantity).horizontal_align(Align::Max).desired_width(ui.available_width()));
                    });

                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("قیمت خرید واحد (تومان) *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.purchase_unit_cost).horizontal_align(Align::Max).desired_width(ui.available_width()));
                    });
                });
                ui.add_space(8.0);

                ui.columns(2, |cols| {
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("مبلغ پرداخت شده (تومان)").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.purchase_paid).horizontal_align(Align::Max).desired_width(ui.available_width()));
                    });

                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("نام تامین‌کننده / شرکت").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.purchase_supplier).horizontal_align(Align::Max).desired_width(ui.available_width()));
                    });
                });
                ui.add_space(14.0);

                if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(RichText::new("ثبت فاکتور خرید و افزایش موجودی").strong().color(Color32::WHITE)).fill(Color32::from_rgb(15, 118, 110))).clicked() {
                    let qty = parse_business_number(&self.purchase_quantity);
                    let cost = parse_business_number(&self.purchase_unit_cost);
                    let paid = parse_business_number(&self.purchase_paid);

                    match (qty, cost, paid) {
                        (Some(qty), Some(cost), Some(paid)) if self.purchase_product_id > 0 => {
                            match business::record_purchase(&mut self.db, self.purchase_product_id, qty, cost, &self.purchase_supplier, &today_jalali(), paid, "") {
                                Ok(id) => {
                                    self.business_status = format!("فاکتور خرید #{id} ثبت شد و موجودی انبار افزایش یافت.");
                                    self.purchase_quantity = "1".into();
                                    let _ = self.refresh_business_products();
                                }
                                Err(e) => self.business_status = e,
                            }
                        }
                        _ => self.business_status = "لطفاً کالا و مقادیر معتبر را مشخص کنید.".into(),
                    }
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(15, 118, 110)));
        });
    }

    fn ui_parties(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("مدیریت مشتریان و تامین‌کنندگان").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new("ثبت مشخصات اشخاص و پیگیری مانده حساب و بدهکاری/بستانکاری.").size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(12.0);

            ui.columns(2, |cols| {
                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "ثبت طرف‌حساب جدید");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        ui.label(RichText::new("نام و نام خانوادگی / شرکت *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.party_name).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("شماره تماس").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.party_phone).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("نوع طرف‌حساب").strong());
                        egui::ComboBox::from_id_salt("party_kind_select")
                            .selected_text(RichText::new(match self.party_kind { 0 => "مشتری", 1 => "تامین‌کننده", _ => "هر دو" }))
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.party_kind, 0, RichText::new("مشتری"));
                                ui.selectable_value(&mut self.party_kind, 1, RichText::new("تامین‌کننده"));
                                ui.selectable_value(&mut self.party_kind, 2, RichText::new("هر دو"));
                            });
                        ui.add_space(14.0);

                        if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(RichText::new("ثبت شخص").strong().color(Color32::WHITE)).fill(Color32::from_rgb(15, 118, 110))).clicked() {
                            let kind = match self.party_kind { 0 => "customer", 1 => "supplier", _ => "both" };
                            match business::add_party(&self.db, &self.party_name, kind, &self.party_phone) {
                                Ok(_) => {
                                    self.business_status = "طرف‌حساب با موفقیت اضافه شد.".into();
                                    self.party_name.clear();
                                    self.party_phone.clear();
                                }
                                Err(e) => self.business_status = e,
                            }
                        }
                    });
                });

                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "لیست طرف‌حساب‌ها");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                        match business::list_parties(&self.db) {
                            Ok(parties) => {
                                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                    egui::Grid::new("parties_grid").striped(true).num_columns(4).spacing(Vec2::new(12.0, 8.0)).show(ui, |ui| {
                                        ui.label(RichText::new("نام").strong());
                                        ui.label(RichText::new("نوع").strong());
                                        ui.label(RichText::new("تلفن").strong());
                                        ui.label(RichText::new("مانده (تومان)").strong());
                                        ui.end_row();

                                        for party in parties {
                                            ui.label(RichText::new(&party.name));
                                            ui.label(RichText::new(match party.kind.as_str() { "customer" => "مشتری", "supplier" => "تامین‌کننده", _ => "هر دو" }));
                                            ui.label(RichText::new(persian_digits(&party.phone)));
                                            let color = if party.balance > 0 { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(16, 185, 129) };
                                            ui.label(RichText::new(money(party.balance)).color(color).strong());
                                            ui.end_row();
                                        }
                                    });
                                });
                            }
                            Err(e) => { ui.label(RichText::new(format!("خطا: {e}"))); }
                        }
                    });
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(15, 118, 110)));
        });
    }

    fn ui_expenses_reports(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new("هزینه‌های فروشگاه و گزارش سود").size(22.0).strong().color(Color32::from_rgb(15, 23, 42)));
            ui.label(RichText::new("ثبت هزینه‌های جاری کسب‌وکار و مشاهده گزارش کلی سود و زیان.").size(12.0).color(Color32::from_rgb(100, 116, 139)));
            ui.add_space(12.0);

            ui.columns(2, |cols| {
                cols[1].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "ثبت هزینه جدید فروشگاه");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        ui.label(RichText::new("دسته هزینه *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.expense_category).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("مبلغ (تومان) *").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.expense_amount).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(8.0);

                        ui.label(RichText::new("توضیحات").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.expense_note).horizontal_align(Align::Max).desired_width(ui.available_width()));
                        ui.add_space(14.0);

                        if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(RichText::new("ثبت هزینه").strong().color(Color32::WHITE)).fill(Color32::from_rgb(15, 118, 110))).clicked() {
                            if let Some(amount) = parse_business_number(&self.expense_amount) {
                                match business::record_expense(&self.db, &today_jalali(), &self.expense_category, amount, "cash", &self.expense_note) {
                                    Ok(id) => {
                                        self.business_status = format!("هزینه #{id} با موفقیت ثبت شد.");
                                        self.expense_amount.clear();
                                        self.expense_note.clear();
                                    }
                                    Err(e) => self.business_status = e,
                                }
                            } else {
                                self.business_status = "مبلغ معتبر وارد کنید.".into();
                            }
                        }
                    });
                });

                cols[0].with_layout(Layout::top_down(Align::Max), |ui| {
                    panel_title(ui, "خلاصه گزارش مالی فروشگاه");
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240))).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        if let Ok((sales, purchases, expenses, _low, gross_profit)) = business::business_totals(&self.db) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("فروش کل:").strong());
                                ui.label(RichText::new(format!("{} تومان", money(sales))).color(Color32::from_rgb(16, 185, 129)).strong());
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("سود ناخالص فروش:").strong());
                                ui.label(RichText::new(format!("{} تومان", money(gross_profit))).color(Color32::from_rgb(59, 130, 246)).strong());
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("خرید کل:").strong());
                                ui.label(RichText::new(format!("{} تومان", money(purchases))));
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("هزینه‌های جاری:").strong());
                                ui.label(RichText::new(format!("{} تومان", money(expenses))).color(Color32::from_rgb(239, 68, 68)));
                            });
                            ui.separator();

                            let net_profit = gross_profit - expenses;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("سود خالص فروشگاه:").size(15.0).strong());
                                let color = if net_profit >= 0 { Color32::from_rgb(16, 185, 129) } else { Color32::from_rgb(239, 68, 68) };
                                ui.label(RichText::new(format!("{} تومان", money(net_profit))).size(16.0).color(color).strong());
                            });
                        }

                        ui.add_space(14.0);
                        if ui.button(RichText::new(format!("{}  خروجی CSV فروش‌ها", ph::DOWNLOAD_SIMPLE))).clicked() {
                            if let Ok(csv) = business::sales_csv(&self.db) {
                                if let Ok(path) = data_path() {
                                    if let Some(parent) = path.parent() {
                                        let dir = parent.join("exports");
                                        let _ = std::fs::create_dir_all(&dir);
                                        let file = dir.join(format!("sales-export-{}.csv", today_jalali().replace('/', "-")));
                                        if std::fs::write(&file, csv).is_ok() {
                                            self.business_status = format!("فایل CSV در ذخیره شد: {}", file.display());
                                        }
                                    }
                                }
                            }
                        }
                    });
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new(&self.business_status).size(12.0).color(Color32::from_rgb(15, 118, 110)));
        });
    }
}

fn draw_nabz_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(10.0), Color32::from_rgb(15, 118, 110));
    let c = rect.center();
    let pts = [
        egui::pos2(rect.left() + size * 0.18, c.y),
        egui::pos2(rect.left() + size * 0.34, c.y),
        egui::pos2(rect.left() + size * 0.43, rect.top() + size * 0.28),
        egui::pos2(rect.left() + size * 0.56, rect.bottom() - size * 0.25),
        egui::pos2(rect.left() + size * 0.66, c.y),
        egui::pos2(rect.right() - size * 0.16, c.y),
    ];
    for pair in pts.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new((size * 0.075).max(2.0), Color32::WHITE));
    }
}

fn apply_nabz_theme(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        let mut visuals = egui::Visuals::light();
        let ink = Color32::from_rgb(15, 23, 42);
        let border = Color32::from_rgb(226, 232, 240);
        let teal = Color32::from_rgb(15, 118, 110);

        visuals.panel_fill = Color32::from_rgb(248, 250, 252);
        visuals.window_fill = Color32::WHITE;
        visuals.extreme_bg_color = Color32::WHITE;
        visuals.faint_bg_color = Color32::from_rgb(241, 245, 249);
        visuals.selection.bg_fill = Color32::from_rgb(204, 251, 241);
        visuals.selection.stroke = Stroke::new(1.0, teal);
        visuals.hyperlink_color = teal;

        visuals.widgets.noninteractive.bg_fill = Color32::WHITE;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ink);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);

        visuals.widgets.inactive.bg_fill = Color32::from_rgb(241, 245, 249);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(241, 245, 249);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, ink);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
        visuals.widgets.inactive.rounding = Rounding::same(8.0);

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(204, 251, 241);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, teal);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, teal);
        visuals.widgets.hovered.rounding = Rounding::same(8.0);

        visuals.widgets.active.bg_fill = teal;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, teal);
        visuals.widgets.active.rounding = Rounding::same(8.0);

        visuals.override_text_color = Some(ink);
        style.visuals = visuals;
        style.spacing.item_spacing = Vec2::new(10.0, 9.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.spacing.interact_size.y = 36.0;
    });
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

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.retain(|name| name != "phosphor");
        family.insert(0, "phosphor".to_owned());
    }

    let app = NabzApp::new().map_err(|e| {
        eprintln!("Nabz startup failed: {e}");
        eframe::Error::AppCreation(Box::new(std::io::Error::other(e)))
    })?;

    eframe::run_native(
        APP_NAME,
        eframe::NativeOptions::default(),
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(fonts);
            apply_nabz_theme(&cc.egui_ctx);
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

fn summary_card(ui: &mut egui::Ui, title: &str, value: i64, color: Color32) {
    Frame::none()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(226, 232, 240)))
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() / 3.0).max(120.0));
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                ui.label(RichText::new(title).size(12.0).color(Color32::from_rgb(100, 116, 139)));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} تومان", money(value)))
                        .size(18.0)
                        .strong()
                        .color(color),
                );
            });
        });
}

fn panel_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).strong().color(Color32::from_rgb(15, 23, 42)));
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
        assert!(valid_jalali("۱۴۰۳/۰۷/۰۲"));
        assert!(valid_jalali("1403/01/31"));
        assert!(valid_jalali("۱۴۰۳/۱۲/۲۹"));
    }

    #[test]
    fn rejects_invalid_jalali_dates() {
        assert!(!valid_jalali("1403/13/01"));
        assert!(!valid_jalali("1403/00/01"));
        assert!(!valid_jalali("1403/07/32"));
        assert!(!valid_jalali("1403/12/30"));
        assert!(!valid_jalali("not-a-date"));
    }

    #[test]
    fn formats_digits_in_persian() {
        assert_eq!(persian_digits("1403/07/02"), "۱۴۰۳/۰۷/۰۲");
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
}
