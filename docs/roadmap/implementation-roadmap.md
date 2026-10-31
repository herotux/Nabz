# Nabz — Official Implementation Roadmap

> Status: living implementation plan  
> Scope: one accounting product with a shared Rust/SQLite core and two independently distributable user interfaces (Slint and Tauri).  
> Working branch: `feature/shared-core-ui-switching`

This roadmap defines the eight major delivery phases for Nabz. The checklists are acceptance criteria and work breakdown, not claims that the work is already complete. Items must only be checked after implementation and verification.

## Product and architecture principles

- **One product, one source of truth:** Slint and Tauri are alternative frontends over the same `nabz-core` business logic and SQLite database.
- **Independent distribution:** users may install either frontend, or both, without creating separate accounting databases.
- **Safe UI migration:** switching the preferred frontend must not copy, reset, or relocate accounting data.
- **Accounting correctness first:** financial calculations, validation, stock movements, and transactions belong in Rust core APIs, not duplicated UI SQL.
- **Persian-first UX:** full RTL, correct Persian glyph shaping, Persian/Arabic digit input, Jalali dates, and practical keyboard workflows.
- **Evidence-based completion:** compilation alone does not prove a feature works; tests and target-platform verification are required.
- **Privacy by default:** AI is optional, disabled by default, and must never expose accounting data without explicit consent.

## Phase 1 — Stabilize architecture and shared core

**Goal:** make the shared Rust core the single authoritative implementation for persistence and accounting operations.

### Work breakdown
- [ ] Audit current Slint and Tauri database access and enumerate duplicated SQL/business rules.
- [ ] Move accounting commands, validation, calculations, and read models into `nabz-core`.
- [ ] Keep database path resolution, connection setup, schema versioning, and migrations in the shared core.
- [ ] Replace frontend-specific SQL with typed core APIs.
- [ ] Define stable request/result/error types usable by both frontends.
- [ ] Add cross-frontend integration tests: write through shared APIs and read the same data through the other frontend.
- [x] Add a persisted preferred-UI setting and a safe UI-switch flow; successful switches remember the target, and failed launches roll back the preference.
- [ ] Define behavior for simultaneous app launches, SQLite locking, and write conflicts.
- [ ] Add backup-before-migration and verified restore behavior.
- [ ] Ensure uninstalling either frontend does not delete shared user data.

### Exit criteria
- Both frontends use the same core APIs for all implemented accounting workflows.
- No duplicated accounting SQL/business rules remain in the UI layers for migrated workflows.
- Cross-frontend tests prove both interfaces see the same database state.
- UI switching preserves the database and user settings.
- Migration, backup, restore, and concurrent access behavior are tested.

## Phase 2 — Complete the accounting system

**Goal:** provide reliable end-to-end accounting workflows with auditable, transaction-safe financial behavior.

### Work breakdown
- [ ] Complete customer, supplier, and combined contact roles, including opening balances.
- [ ] Complete multi-line purchase and sales invoices with line and invoice discounts.
- [ ] Complete cash/card/other payment recording and partial settlement.
- [ ] Complete payment allocation, outstanding balances, and overpayment handling.
- [ ] Complete purchase and sales returns, invoice cancellation, refunds, and reversal rules.
- [ ] Ensure every stock and financial mutation is atomic and rolls back on failure.
- [ ] Implement and verify moving weighted-average inventory costing.
- [ ] Preserve historical unit cost snapshots on invoice lines and use correct historical values for returns/reversals.
- [ ] Implement expenses and income entries where required by the accounting model.
- [ ] Add account statements, customer/supplier balances, cash flow, sales/purchase summaries, and profit/loss reports.
- [ ] Reconcile dashboard totals and reports against source transactions.
- [ ] Add edge-case tests for discounts, partial payments, multi-stage returns, refunds, date boundaries, rounding, and rollback.

### Exit criteria
- Core accounting workflows pass unit and integration tests, including negative and boundary cases.
- Stock quantities, cost values, invoice balances, and payment allocations reconcile.
- Returns and cancellations preserve historical financial correctness.
- Reports reconcile to the underlying ledger/transaction records.

## Phase 3 — Complete UI and UX

**Goal:** deliver a consistent, fast, Persian-first experience across Slint and Tauri, aligned with the approved Nabz design.

### Work breakdown
- [ ] Implement the approved dashboard structure and visual hierarchy.
- [ ] Complete all application pages and forms using a shared navigation and interaction model.
- [ ] Keep the sidebar on the right for Persian RTL layouts.
- [ ] Apply consistent typography, spacing, colors, cards, tables, dialogs, and form states.
- [ ] Verify Persian/Arabic glyph shaping, bidirectional text, phone numbers, currency, and numeric fields.
- [ ] Implement Jalali date display, selection, filtering, and entry consistently.
- [ ] Add global search across relevant records.
- [ ] Add keyboard-first navigation, shortcuts, focus order, and accessible validation.
- [ ] Provide clear empty, loading, success, warning, and error states.
- [ ] Keep Slint and Tauri behavior and information architecture consistent while respecting each toolkit's native strengths.
- [ ] Test UI responsiveness and practical performance on the target Manjaro laptop.

### Exit criteria
- All planned pages and forms are implemented and usable.
- RTL and Persian text are visually and functionally verified on target platforms.
- Critical workflows can be completed efficiently by keyboard and mouse.
- No major visual or behavioral divergence remains between the two frontends.

## Phase 4 — Complete inventory and shop-floor operations

**Goal:** make everyday product and inventory management practical for a small shop.

### Work breakdown
- [ ] Complete product categories, units, codes, and product metadata.
- [ ] Add safe archive/soft-delete flows with audit history.
- [ ] Prevent hard deletion of records referenced by invoices, invoice lines, stock movements, or payments.
- [ ] Complete stock adjustments and stock movement history with reasons.
- [ ] Add barcode lookup and duplicate-barcode validation.
- [ ] Support USB HID barcode scanners as keyboard input.
- [ ] Add fast product entry and invoice item selection workflows.
- [ ] Verify price changes do not rewrite historical invoice prices or costs.
- [ ] Add low-stock and inventory valuation views.

### Exit criteria
- Product and stock operations are auditable and transaction-safe.
- Barcode scanning works with common USB HID scanners.
- Historical invoices and inventory valuation remain correct after product edits.
- Referenced records cannot be destructively deleted.

## Phase 5 — Printing, settings, backup, and restore

**Goal:** support real shop operations, physical documents, and reliable data protection.

### Work breakdown
- [ ] Complete store profile, logo, address, contact details, and currency settings.
- [ ] Complete invoice numbering, date, and document preferences.
- [ ] Add print preview and invoice layouts for A4 and A5.
- [ ] Add receipt layouts for common thermal printer widths.
- [ ] Add barcode label generation and printing.
- [ ] Verify printer selection, page sizing, margins, RTL text, and totals.
- [ ] Implement consistent SQLite backup using a safe snapshot mechanism.
- [ ] Support manual and scheduled backups with clear destination and retention settings.
- [ ] Validate backups before restore and restore atomically.
- [ ] Add recovery guidance for interrupted updates and failed restores.
- [ ] Ensure schema migrations create a verified pre-migration backup when required.

### Exit criteria
- Printed documents are legible, correctly formatted, and financially accurate.
- Backups can be created and validated without corrupting an active database.
- Restore is atomic and tested against valid, invalid, and interrupted backup scenarios.
- Store settings persist consistently across both frontends.

## Phase 6 — Performance, packaging, and multi-platform release

**Goal:** provide independently installable, reliable Slint and Tauri builds for supported platforms.

### Work breakdown
- [ ] Produce separate Slint and Tauri release artifacts from the same shared core.
- [ ] Build Linux packages, prioritizing AppImage and deb; evaluate Flatpak where practical.
- [ ] Build and verify Windows installers/packages for both frontends as supported.
- [ ] Build Raspberry Pi packages for the intended architecture and operating-system baseline.
- [ ] Verify runtime dependencies and minimum supported system libraries.
- [ ] Test installation, upgrade, downgrade policy, launch, and uninstall behavior.
- [ ] Verify both frontends use the same canonical database and configuration paths.
- [ ] Ensure uninstalling one frontend preserves shared data and the other frontend.
- [ ] Benchmark startup time, idle and working RAM, CPU, and common workflow latency.
- [ ] Test on real Manjaro hardware and Raspberry Pi hardware or a representative target.
- [ ] Document supported platforms, installation, updates, backups, and troubleshooting.

### Exit criteria
- Each frontend has independently installable release artifacts for its declared targets.
- Install/update/uninstall tests preserve shared user data.
- Runtime behavior is verified on actual target systems, not inferred from CI compilation.
- Performance and dependency requirements are documented.

## Phase 7 — Add privacy-first Nabz AI

**Goal:** provide optional Persian-language assistance and financial analysis without compromising accounting integrity or user privacy.

### Work breakdown
- [ ] Finalize provider-neutral routing and configuration for Ollama, Gemini, and OpenAI-compatible APIs.
- [ ] Implement local Ollama integration and cloud provider integrations behind explicit settings.
- [ ] Store credentials using an operating-system secret store; never embed API keys in source, binaries, or CI artifacts.
- [ ] Add Persian chat UX and explainable financial summaries.
- [ ] Expose only narrowly scoped, read-only accounting tools in the first release.
- [ ] Keep all financial calculations and authoritative results in Rust core; AI may interpret and explain, not calculate ledger truth.
- [ ] Require explicit consent before any cloud request containing user data.
- [ ] Ensure Local-only mode never sends data to a cloud provider.
- [ ] Do not silently fall back from local to cloud or from free to paid services.
- [ ] Minimize and redact data; never send the full database or customer-identifying information by default.
- [ ] Separate AI chat history/settings from accounting records.
- [ ] Add tests for provider failures, quotas, timeouts, privacy modes, and consent enforcement.

### Exit criteria
- AI is disabled by default and has clear local/cloud controls.
- Privacy and consent rules are enforced by code and tests.
- Initial AI tools are read-only and cannot mutate accounting data.
- Provider failures and quota exhaustion never trigger unauthorized paid or cloud fallback.

## Phase 8 — Final verification and stable release

**Goal:** validate the complete product and publish a stable release with trustworthy documentation.

### Work breakdown
- [ ] Run the full test suite for core, Slint, Tauri, and cross-frontend integration.
- [ ] Run release builds for all declared target platforms and both frontends.
- [ ] Test fresh install, upgrade from supported older versions, and database migration.
- [ ] Test backup/restore, interrupted migration, crash recovery, and disk-full scenarios.
- [ ] Test simultaneous launch and database locking behavior.
- [ ] Verify accounting reconciliation with realistic but synthetic datasets.
- [ ] Perform security and privacy review, including AI consent and secret storage.
- [ ] Perform Persian RTL, Jalali date, keyboard, printing, and barcode acceptance tests.
- [ ] Fix release-blocking defects and record known limitations.
- [ ] Update user and developer documentation.
- [ ] Publish versioned release notes, checksums, and verified artifacts.
- [ ] Define post-release bug reporting and maintenance process.

### Exit criteria
- All release-blocking tests pass on the exact release commit.
- Both frontends pass installation and runtime acceptance tests on supported targets.
- Accounting, migration, backup, restore, privacy, and data integrity checks pass.
- Release notes and known limitations accurately describe the delivered product.

## Definition of done for every phase

A phase is complete only when all of the following are true:

1. The scoped functionality is implemented in the repository; a design, stub, or compile-only placeholder is not completion.
2. Relevant unit, integration, regression, and acceptance tests pass.
3. Required frontend and platform builds pass on the exact commit being evaluated.
4. Runtime behavior is verified where hardware, OS libraries, RTL rendering, printers, scanners, or packaging can affect correctness.
5. The corresponding detailed TODO items and documentation are updated.
6. Known limitations and deferred work are explicitly recorded.
7. No completion claim is made solely from successful compilation or an unrelated workflow run.

## Current execution policy

- Execute phases in order unless a dependency-safe task is explicitly identified.
- Keep work on `feature/shared-core-ui-switching` until the migration is ready for review.
- Do not modify `main` or `tauri-web-prototype` as part of this roadmap work.
- Keep the detailed task checklist in `slint-native/TODO.md`; this document is the high-level source of truth.
- When implementation status changes, update both this roadmap and the detailed TODO in the same change where practical.
