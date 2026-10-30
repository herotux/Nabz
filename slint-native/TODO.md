# Nabz Slint migration — TODO

Updated: 2026-10-30

## Current CI / blocking issues
- [x] Inspect the failed Slint Native UI CI run.
- [x] Remove incompatible Window `min-width` / `min-height` constraints when explicit `width` / `height` are set.
- [x] Move card padding onto layout elements instead of Rectangle.
- [x] Extend tests for anonymous unpaid sale rejection and receipt overpayment rollback.
- [x] Confirm Slint Native UI CI and Rust tests succeed after the compile fix.
- [ ] Download and launch the Linux artifact on Manjaro; verify Persian glyph shaping, RTL, and keyboard input.
- [x] Validate the versioned SQLite migration commit in root Rust tests and Windows build.
- [ ] Validate the versioned SQLite migration commit in Slint-specific tests and Linux build.

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
- [x] Add multi-row invoice tests for line totals, stock updates, malformed rows, and full rollback on insufficient stock.
- [ ] Test customer/supplier/both opening balances, partial payments, discount, duplicate codes, invalid roles, overpayment, stock rollback, and purchase debts.
- [ ] Confirm database schema compatibility using a copy of an actual Tauri database; do not test on the only production database.
- [x] Add migration/version handling before schema changes (SQLite `user_version`, transactional v1 migration, idempotency test).
- [ ] Ensure dashboard balances include `customer` and `both` roles and reconcile to invoices/payments.

## Priority 2 — invoice and inventory workflows
- [x] Replace single-line invoice form with a multi-row text editor (one item per line).
- [x] Calculate each line, subtotal, invoice discount, total, and paid amount with overflow validation.
- [x] Apply stock movements in one SQLite transaction; invalid rows and insufficient stock roll back the entire invoice.
- [ ] Support invoice-level and per-line discounts without double counting.
- [x] Add invoice details view showing all lines, subtotal, discount, paid amount, balance, and notes.
- [x] Add invoice void workflow with confirmation, transactional stock reversal, and safe restrictions for later allocations.
- [x] Add invoice list search and kind/void-status filters.
- [x] Add partial return entry from selected invoice details.
- [x] Add invoice voiding with safe stock reversal.
- [x] Add transactional partial sales returns with stock restoration, return history, proportional invoice discount adjustment, and over-return prevention.
- [x] Add transactional refunds when a partial return exceeds the unpaid balance; record refund amount/method, reduce invoice paid amount, and subtract refunds from today's net receipts.
- [x] Show refund history on invoice details and block invoice voiding after returns to prevent double-restoring stock.
- [ ] Test multi-stage returns with discounts, prior receipt allocations, and refunds on different dates.

## Priority 3 — record management
- [x] Add database-backed search/filter for contacts (name/phone/type), products (name/code/barcode), and payments (contact/method/notes/date).
- [x] Add invoice search by number, contact, and notes; filter all/sales/purchases/voided invoices.
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
