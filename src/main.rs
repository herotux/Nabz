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
}

impl NabzApp {
    fn new() -> Result<Self, String> {
        let path = data_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
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
        };
        app.reload().map_err(|e| e.to_string())?;
        Ok(app)
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
                                        ui.label("تاریخ شمسی (سال/ماه/روز)");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.date)
                                                .hint_text("۱۴۰۵/۰۷/۰۲")
                                                .desired_width(f32::INFINITY),
                                        );
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
                    ui.label(
                        RichText::new("نبض • نسخهٔ آغازین ۰٫۱ • داده‌ها روی دستگاه شما می‌مانند")
                            .size(11.0)
                            .color(Color32::from_rgb(130, 140, 155)),
                    );
                });
            });

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
