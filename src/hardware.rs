use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use barcoders::{
    generators::svg::SVG,
    sym::{code128::Code128, ean13::EAN13},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrinterMode {
    ThermalEscPos,
    A4,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Printer {
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanBuffer {
    value: String,
    max_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    Pending,
    Complete(String),
    Empty,
    TooLong,
}

impl Default for ScanBuffer {
    fn default() -> Self { Self::new(128) }
}

impl ScanBuffer {
    pub fn new(max_len: usize) -> Self { Self { value: String::new(), max_len } }

    /// Feed characters from a USB HID keyboard-wedge scanner; Enter completes a scan.
    pub fn feed(&mut self, ch: char) -> ScanEvent {
        match ch {
            '\r' | '\n' => {
                if self.value.is_empty() { return ScanEvent::Empty; }
                ScanEvent::Complete(std::mem::take(&mut self.value))
            }
            '\u{8}' | '\u{7f}' => { self.value.pop(); ScanEvent::Pending }
            c if c.is_ascii_graphic() && self.value.len() < self.max_len => {
                self.value.push(c); ScanEvent::Pending
            }
            c if c.is_ascii_whitespace() => ScanEvent::Pending,
            _ if self.value.len() >= self.max_len => { self.value.clear(); ScanEvent::TooLong }
            _ => ScanEvent::Pending,
        }
    }

    pub fn clear(&mut self) { self.value.clear(); }
}

/// Query CUPS for configured printers. Requires cups-client/lpstat on Linux.
pub fn list_printers() -> Result<Vec<Printer>, String> {
    let output = Command::new("lpstat").args(["-p", "-d"]).output()
        .map_err(|e| format!("اجرای lpstat ناموفق بود؛ بستهٔ CUPS نصب است؟ ({e})"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let default_name = stdout.lines()
        .find_map(|line| line.strip_prefix("system default destination: "))
        .map(str::trim).unwrap_or_default();
    let mut printers = stdout.lines().filter_map(|line| {
        let rest = line.strip_prefix("printer ")?;
        let name = rest.split_whitespace().next()?.to_string();
        Some(Printer { is_default: name == default_name, name })
    }).collect::<Vec<_>>();
    printers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(printers)
}

/// Submit a document to CUPS. Thermal mode targets a queue configured for raw ESC/POS;
/// A4 mode requests A4 media. Rasterize Persian thermal receipts when the printer
/// cannot shape and right-align Persian text itself.
pub fn print_file(path: &Path, printer: Option<&str>, mode: PrinterMode, copies: u8) -> Result<String, String> {
    if !path.is_file() { return Err(format!("فایل چاپ پیدا نشد: {}", path.display())); }
    if !(1..=20).contains(&copies) { return Err("تعداد نسخه باید بین ۱ تا ۲۰ باشد.".into()); }
    if let Some(name) = printer {
        if name.trim().is_empty() || name.chars().any(|c| c.is_control()) {
            return Err("نام چاپگر نامعتبر است.".into());
        }
    }
    let mut command = Command::new("lp");
    if let Some(name) = printer { command.args(["-d", name]); }
    command.args(["-n", &copies.to_string()]);
    match mode {
        PrinterMode::ThermalEscPos => { command.args(["-o", "raw"]); }
        PrinterMode::A4 => { command.args(["-o", "media=A4"]); }
    }
    command.arg(path);
    let output = command.output().map_err(|e| format!("اجرای lp ناموفق بود؛ بستهٔ CUPS نصب است؟ ({e})"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Create a test document without touching accounting data.
pub fn create_test_receipt() -> Result<PathBuf, String> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let path = std::env::temp_dir().join(format!("nabz-printer-test-{}-{nonce}.txt", std::process::id()));
    let body = "نبض | آزمون چاپ\n========================\nPrinter test / آزمون چاپ\nA4 / Thermal ESC-POS\n\n\n";
    std::fs::write(&path, body).map_err(|e| format!("ساخت فایل آزمون چاپ ناموفق بود: {e}"))?;
    Ok(path)
}

/// Generate a standards-based Code 128 barcode as vector SVG.
pub fn code128_svg(value: &str, height: u32) -> Result<String, String> {
    if value.is_empty() || value.len() > 128 || !value.is_ascii() || value.chars().any(char::is_control) {
        return Err("بارکد Code 128 باید ۱ تا ۱۲۸ نویسهٔ ASCII قابل‌چاپ داشته باشد.".into());
    }
    let barcode = Code128::new(value).map_err(|e| format!("کد بارکد نامعتبر است: {e}"))?;
    SVG::new(height).generate(&barcode.encode()).map_err(|e| format!("تولید SVG ناموفق بود: {e}"))
}

/// Accept EAN-13 only when its 13 digits and GS1 check digit are valid.
pub fn valid_ean13(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 13 || !bytes.iter().all(u8::is_ascii_digit) { return false; }
    let sum: u32 = bytes[..12].iter().enumerate().map(|(i, b)| {
        let digit = (b - b'0') as u32;
        digit * if i % 2 == 0 { 1 } else { 3 }
    }).sum();
    let check = ((10 - sum % 10) % 10) as u8 + b'0';
    bytes[12] == check
}

pub fn ean13_svg(value: &str, height: u32) -> Result<String, String> {
    if !valid_ean13(value) { return Err("EAN-13 باید ۱۳ رقم و رقم کنترلی معتبر داشته باشد.".into()); }
    let barcode = EAN13::new(value).map_err(|e| format!("EAN-13 نامعتبر است: {e}"))?;
    SVG::new(height).generate(&barcode.encode()).map_err(|e| format!("تولید SVG ناموفق بود: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_returns_one_code_on_enter_and_resets() {
        let mut scanner = ScanBuffer::default();
        for ch in "SKU-123".chars() { assert_eq!(scanner.feed(ch), ScanEvent::Pending); }
        assert_eq!(scanner.feed('\r'), ScanEvent::Complete("SKU-123".into()));
        assert_eq!(scanner.feed('\n'), ScanEvent::Empty);
    }

    #[test]
    fn scanner_supports_backspace_and_length_limit() {
        let mut scanner = ScanBuffer::new(4);
        for ch in "ABCD".chars() { scanner.feed(ch); }
        scanner.feed('\u{8}');
        assert_eq!(scanner.feed('E'), ScanEvent::Pending);
        assert_eq!(scanner.feed('\n'), ScanEvent::Complete("ABCE".into()));
        for ch in "ABCDE".chars() { scanner.feed(ch); }
        assert_eq!(scanner.feed('F'), ScanEvent::TooLong);
    }

    #[test]
    fn code128_generates_svg_for_product_sku() {
        let svg = code128_svg("SKU-12345", 80).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn ean13_requires_correct_checksum() {
        assert!(valid_ean13("4006381333931"));
        assert!(!valid_ean13("4006381333932"));
        assert!(!valid_ean13("123"));
        assert!(!valid_ean13("ABCDEFGHIJKLM"));
    }

    #[test]
    fn ean13_generates_svg_for_valid_gtin() {
        assert!(ean13_svg("4006381333931", 80).unwrap().contains("<svg"));
        assert!(ean13_svg("4006381333932", 80).is_err());
    }

    #[test]
    fn invalid_printer_options_are_rejected_before_invoking_cups() {
        let missing = Path::new("/definitely/not/a/real/nabz-file.pdf");
        assert!(print_file(missing, None, PrinterMode::A4, 1).is_err());
        assert!(print_file(Path::new("."), None, PrinterMode::A4, 0).is_err());
    }
}
