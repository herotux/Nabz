# Nabz — UI/UX and Product TODO

> Branch: figma-exact-ui-from-jules  
> Design reference: docs/Screenshot_20261026_085826_Chrome.jpg  
> Goal: bring every screen and form to a coherent, polished Persian accounting-app experience inspired by the reference, while preserving existing accounting behavior and SQLite data.

## 1. Design system and shared UI

- [x] Add a global light theme with consistent colors, typography, spacing, borders, rounded controls, and teal selection.
- [x] Introduce a shared card/panel frame helper.
- [x] Introduce a shared page-header helper and apply it to the main pages.
- [ ] Compare the running app against the reference screenshot and tune details until the overall hierarchy, density, and alignment are close.
- [ ] Define and consistently apply design tokens for page background, surface, border, primary/secondary text, success, warning, danger, and focus states.
- [ ] Standardize primary, secondary, quiet, destructive, and disabled button styles and sizes.
- [ ] Standardize text inputs, numeric inputs, multiline fields, combo boxes, date controls, checkboxes, and validation/error styling.
- [ ] Standardize card, section, toolbar, dialog, table, badge, and empty-state components.
- [ ] Ensure Persian RTL layout throughout: alignment, label placement, icons, spacing, navigation, and reading order.
- [ ] Check Persian typography, line height, numeral consistency, and readability at typical desktop sizes.
- [ ] Make monetary values and quantities consistently formatted, with Toman units and clear digit grouping.
- [ ] Ensure long Persian text, long names, large numbers, and narrow windows do not clip or overlap.
- [ ] Add clear hover, pressed, focused, selected, disabled, loading, success, and error states where applicable.
- [ ] Use confirmation dialogs for destructive actions and concise success/error feedback for operations.
- [ ] Keep styling changes separate from business logic wherever practical.

## 2. App shell and navigation

- [ ] Align the overall window layout, content margins, navigation, toolbars, and page widths with the reference.
- [ ] Make the current page and active navigation item unmistakable.
- [ ] Group related navigation items logically and use consistent icon sizes and labels.
- [ ] Make global search placement and behavior clear; show a useful no-results state.
- [ ] Check keyboard navigation, tab order, visible focus, and shortcuts for frequently used actions.
- [ ] Verify resizing and minimum-window-size behavior; prevent controls and tables from being squeezed into unusable layouts.

## 3. Dashboard

- [x] Present four data-backed summary cards: today's sales, receivables, payables, and inventory valuation.
- [x] Add a recent-sales table and daily/status summaries.
- [x] Surface low-stock information and a route to the relevant products.
- [x] Add dashboard quick actions.
- [ ] Verify every dashboard metric against the underlying SQLite records and document its exact calculation.
- [ ] Check customer/supplier balance calculations for parties whose type is both; avoid both omissions and double counting.
- [ ] Add clear empty states when there are no sales, products, debts, or low-stock items.
- [ ] Make recent-sales rows readable and provide a clear action to open the relevant record, where supported.
- [ ] Ensure dashboard values refresh immediately after recording sales, purchases, expenses, or cashbook transactions.
- [ ] Check number formatting, date labels, status badges, card alignment, and visual hierarchy against the reference.

## 4. Point-of-sale / sales form

- [ ] Keep barcode/item-code entry prominent and make Enter-to-add behavior discoverable.
- [ ] Improve the quick-product list so products and their prices are easy to scan.
- [x] Add a clear empty-basket state and helpful guidance for adding the first item.
- [x] Allow changing each basket line's quantity.
- [x] Allow removing a basket line without clearing the entire basket.
- [ ] Show product name, quantity, unit price, line total, and invoice total with consistent formatting.
- [ ] Add customer selection, if supported by the existing data model.
- [ ] Add discount entry and a clear subtotal/discount/final-total breakdown, after defining safe calculation rules.
- [ ] Add amount-paid and payment-method fields, and show the remaining balance when partial payment is supported.
- [ ] Add an optional invoice note if the data model supports it or after a migration is designed.
- [ ] Add a review/confirmation step before finalizing a sale, especially when the basket is empty or invalid.
- [ ] Prevent accidental duplicate submission and give visible success/failure feedback.
- [ ] Preserve existing stock deduction, invoice recording, and SQLite consistency.
- [ ] Verify cancellation/clear-basket behavior does not save partial or unintended transactions.

## 5. Products form and product list

- [ ] Improve the product form's field grouping, label clarity, required-field indicators, and validation messages.
- [ ] Keep product name, barcode, purchase price, sale price, and stock clearly represented.
- [ ] Explain that barcode and SKU are generated automatically when omitted.
- [ ] Validate numeric values and prevent invalid negative prices/stock unless explicitly allowed by business rules.
- [x] Add editing and guarded deletion flows for existing products; deletion is refused when sales, purchases, or stock movements reference the product.
- [x] Improve product table readability: aligned columns, formatted prices, stock units, and clear low-stock warnings.
- [ ] Add useful product search/filtering and sorting where the current data volume warrants it.
- [x] Show a helpful empty state when no products exist.
- [ ] Ensure stock changes after sales and purchases are reflected without requiring a restart.

## 6. Purchase form and purchase history

- [ ] Group product, quantity, unit cost, amount paid, and supplier fields into a clear purchase-entry flow.
- [ ] Make required and optional fields explicit and validate quantities/prices before saving.
- [ ] Add an editable purchase date with the Jalali date picker.
- [ ] Add an optional purchase note.
- [ ] Make the supplier field/select consistent with the parties data and prevent inconsistent supplier names where possible.
- [ ] Show the calculated purchase total and remaining payable amount before saving.
- [ ] Improve purchase-history readability and provide filtering/search where supported.
- [ ] Confirm that purchase recording updates stock, supplier balances, and financial totals correctly.
- [ ] Add clear success/error feedback and protect against duplicate submissions.

## 7. Parties (customers and suppliers)

- [ ] Improve the party form and list for customer, supplier, and combined (both) types.
- [ ] Make name, phone, and party type clearly labeled and validate phone input without rejecting valid local formats.
- [ ] Add edit and delete actions with safeguards for parties linked to invoices or balances.
- [ ] Improve the balance column and clearly distinguish receivable versus payable balances.
- [ ] Add search/filtering by name, phone, and party type where practical.
- [ ] Consider optional address, national/company identifier, and notes only after confirming requirements and designing appropriate persistence.
- [ ] Ensure a party of type both is handled consistently in dashboards, balances, sales, and purchases.
- [ ] Provide empty, loading, validation, and error states.

## 8. Cashbook / income and expense transactions

- [ ] Make income/expense type, amount, category, Jalali date, and note visually distinct and easy to complete.
- [ ] Improve category selection and clarify which categories apply to income versus expense.
- [ ] Validate amount and date and provide field-specific error messages.
- [ ] Improve the transaction list with consistent date/amount formatting and clear income-versus-expense distinction.
- [ ] Add edit/delete actions if compatible with accounting integrity, with confirmation for destructive changes.
- [ ] Ensure the date picker opens, closes, and applies a selected date predictably.
- [ ] Verify that saved transactions immediately update reports and balances.

## 9. Operating expenses form and reports

- [ ] Improve expense category, amount, and note field layout and validation.
- [ ] Add an editable expense date.
- [ ] Replace the hardcoded cash payment method with a selector only after supported payment methods and persistence are defined.
- [ ] Make the report's sales, gross profit, purchases, operating expenses, and net result easy to distinguish.
- [ ] Document and verify report formulas against the accounting rules; do not silently change financial calculations during UI work.
- [ ] Format all report values consistently and clearly identify date ranges.
- [ ] Add appropriate empty states and useful filtering/date-range controls where feasible.
- [ ] Verify report totals against sample SQLite data and edge cases.

## 10. Shared tables, forms, dialogs, and feedback

- [ ] Standardize table headers, row spacing, alignment, separators, numeric columns, and selected-row styling.
- [ ] Keep table actions discoverable without cluttering every row.
- [ ] Standardize form widths, field spacing, label alignment, required indicators, and validation placement.
- [ ] Ensure Tab/Shift+Tab and keyboard activation work for common controls.
- [ ] Standardize confirmation dialogs, delete warnings, cancel actions, and close behavior.
- [ ] Provide actionable error messages rather than silent failures or raw implementation errors.
- [ ] Provide success feedback after saves, purchases, sales, backups, and printer tests.
- [ ] Ensure disabled buttons communicate why an action is unavailable where practical.
- [ ] Check text contrast and avoid relying on color alone to communicate status.

## 11. Search, date picker, printer, and backup

- [ ] Verify global search behavior and make matching results and no-results feedback clear.
- [ ] Improve Jalali date picker layout, month/year/day controls, selected-date emphasis, and Today action.
- [ ] Ensure Jalali dates are displayed consistently across forms, tables, and reports.
- [ ] Improve printer selection, refresh, A4 test, and thermal ESC/POS test controls and status feedback.
- [ ] Clearly distinguish printer-not-found, connection/test failure, and successful test states.
- [ ] Make backup location and completion/failure feedback clear.
- [ ] Verify backup does not block the UI unnecessarily and that failures preserve existing data.

## 12. Responsive behavior and accessibility

- [ ] Test the app at small, medium, and large window sizes and on high-DPI displays.
- [ ] Prevent clipped dialogs, inaccessible bottom buttons, overlapping fields, and horizontal overflow where avoidable.
- [ ] Ensure important actions remain reachable when the window is resized.
- [ ] Check keyboard-only use, focus order, and focus visibility.
- [ ] Check readable font sizes, contrast, and status text.
- [ ] Ensure icons have adjacent text/tooltips when their meaning is not obvious.

## 13. Quality assurance and acceptance

- [ ] Run formatting, linting, and the full test suite after each substantial UI batch.
- [ ] Confirm CI passes on figma-exact-ui-from-jules.
- [ ] Exercise every form with valid data, missing required fields, invalid numeric input, cancellation, and save failures.
- [ ] Verify create/read/update/delete flows wherever those actions exist.
- [ ] Verify sales, purchases, expenses, cashbook, stock, balances, and reports against SQLite data before and after operations.
- [ ] Test empty database, large tables, long Persian text, and unusually large amounts.
- [ ] Compare screenshots of every page against the reference/design direction, not just the dashboard.
- [ ] Check that no UI change deletes, resets, or silently migrates user data.
- [ ] Update this checklist as tasks are completed; only mark an item complete after implementation and verification.

## Guardrails

- Preserve the existing accounting behavior and SQLite records.
- Do not invent dashboard values or display placeholder data as real financial data.
- Do not mark a feature complete merely because its UI control exists; verify its behavior.
- For new persisted fields (discounts, customer links, payment methods, notes, etc.), inspect the schema and migration strategy before implementation.
- Prioritize a consistent, polished experience across all existing pages before adding nonessential features.
