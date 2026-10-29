# Nabz Slint migration — TODO

Updated: 2026-10-29

## Current CI / blocking issues
- [x] Inspect the failed Slint Native UI CI run.
- [x] Remove incompatible Window `min-width` / `min-height` constraints when explicit `width` / `height` are set.
- [x] Move card padding onto layout elements instead of Rectangle.
- [x] Extend tests for anonymous unpaid sale rejection and receipt overpayment rollback.
- [ ] Confirm latest Slint CI succeeds after the compile fix.
- [ ] Download and launch the Linux artifact on Manjaro; verify Persian glyph shaping, RTL, and keyboard input.

## Implemented in the native prototype
- [x] Native Slint window, Persian dashboard and section navigation.
- [x] Read summary figures and records from SQLite.
- [x] Create/list contacts.
- [x] Create/list products and stock levels.
- [x] Single-item sale/purchase invoice with transaction, stock mutation, paid sale receipt, and walk-in full-payment rule.
- [x] Receipt/payment entry with contact role validation and FIFO invoice allocation.
- [x] Store name and currency settings.
- [x] Parse Persian/Arabic digits for numeric fields.
- [x] Basic transactional/accounting unit tests.

## Priority 1 — accounting correctness and schema compatibility
- [ ] Compare every Slint accounting rule with the tested Tauri implementation; port all edge-case tests.
- [ ] Test customer/supplier/both opening balances, partial payments, discount, duplicate codes, invalid roles, overpayment, stock rollback, and purchase debts.
- [ ] Confirm database schema compatibility using a copy of an actual Tauri database; do not test on the only production database.
- [ ] Add migration/version handling before any schema changes.
- [ ] Ensure dashboard balances include `customer` and `both` roles and reconcile to invoices/payments.

## Priority 2 — invoice and inventory workflows
- [ ] Replace single-line invoice form with a dynamic multi-row invoice editor.
- [ ] Calculate each line, subtotal, discount, total, paid amount, and remaining balance safely.
- [ ] Apply all stock movements atomically; roll back the whole invoice on any invalid row or insufficient stock.
- [ ] Support invoice-level and per-line discounts without double counting.
- [ ] Add invoice details view and customer walk-in rules.
- [ ] Add returns/voiding with safe stock and ledger reversal rules.

## Priority 3 — record management
- [ ] Add search/filter for contacts, products, invoices, and payments.
- [ ] Add edit/delete flows with confirmations and accounting-safe restrictions.
- [ ] Add barcode lookup and USB HID scanner workflow.
- [ ] Add keyboard-first navigation and clear validation errors.

## Priority 4 — backup, print and settings
- [ ] Implement consistent SQLite backup/restore with validation and atomic restore.
- [ ] Add invoice print layouts for A4/A5 and thermal printers.
- [ ] Add barcode/label generation and printing.
- [ ] Finish store profile, currency, numbering, and date settings.
- [ ] Validate Jalali date display/entry.

## Priority 5 — release readiness
- [ ] Build and upload AppImage / deb from the Slint project (not Tauri or egui).
- [ ] Verify artifact size and runtime dependencies on Manjaro.
- [ ] Benchmark startup, idle RAM and CPU against the Tauri version on the target laptop.
- [ ] Test on Raspberry Pi hardware or an appropriate target image.
- [ ] Update the main README and release workflow only after native UI and accounting checks pass.

## Guardrails
- Keep `tauri-web-prototype` and `main` unchanged during the migration.
- Never claim a workflow passed until the exact current commit has a green run.
- Do not use a real accounting database for tests; use an isolated copy and keep a backup.
- Do not treat successful compilation as proof of correct accounting or Persian RTL.
