# Nabz AI — Architecture and implementation plan

Status: approved product direction; implementation not started by this document.
Target branch: `feature/shared-core-ui-switching`.

## 1. Product decisions

- Execution modes: both cloud and local.
- Scope: all planned capabilities (Persian assistant, sales/profit analysis, inventory advisor, financial insights, anomaly alerts and recommendations).
- Both frontends (Tauri and Slint) must use one shared AI service in `crates/nabz-core`.
- AI is optional. Nabz accounting must remain fully usable with AI disabled or offline.
- Free-first: prefer provider free tiers and local inference; never promise permanently free cloud access.
- Initial release is read-only. Any future write action must require a clear user confirmation and must execute through validated accounting-core commands, never arbitrary model-generated SQL.

## 2. Architectural placement

The existing architecture document identifies `crates/nabz-core` as the shared Rust/SQLite layer and `tauri-web` and `slint-native` as independent frontends. Nabz AI belongs in that shared core.

Suggested modules (names are proposals; fit them to the actual crate layout during implementation):

- `ai/mod.rs`: public service API and shared types.
- `ai/config.rs`: provider selection, model, privacy and usage settings.
- `ai/provider.rs`: provider trait and normalized request/response types.
- `ai/providers/gemini.rs`: Gemini Developer API adapter.
- `ai/providers/openai_compatible.rs`: configurable OpenAI-compatible endpoint adapter.
- `ai/providers/ollama.rs`: local Ollama adapter (or reuse the OpenAI-compatible adapter where behavior is compatible).
- `ai/router.rs`: provider selection, fallback, timeout and retry policy.
- `ai/tools.rs`: allowlisted read-only accounting tools.
- `ai/context.rs`: minimal, explicitly selected business context.
- `ai/guardrails.rs`: data minimization, confirmation policy, output validation and audit events.
- `ai/insights.rs`: deterministic sales, profit, inventory and anomaly calculations.
- `ai/conversations.rs`: optional local conversation history and retention policy.

The exact module split may change after inspecting the current crate and dependency versions. Keep provider-specific HTTP code out of both UI crates.

## 3. Runtime flow

1. The UI sends a typed `AiRequest` to the shared service.
2. The service classifies the request and selects one or more allowlisted accounting tools.
3. Rust executes the tools against the canonical Nabz database using existing accounting services/queries.
4. The service builds a minimal context payload. It must not send the full SQLite database, customer list, phone numbers or invoice line history by default.
5. The router sends the prompt and approved context to the selected provider (or local model).
6. The provider response is validated and returned with source facts/tool results where applicable.
7. The UI displays the answer, the data period and a clear indication of whether cloud or local inference was used.

For simple numeric questions, deterministic tools should answer without an LLM where possible. The LLM is used for language understanding, explanation and synthesis—not as the source of truth.

## 4. Provider strategy

### Cloud
- First adapter: Gemini Developer API, configured by the user with their own API key.
- Additional adapters: generic OpenAI-compatible endpoint and later other providers after compatibility testing.
- Cloud access is opt-in. The setup screen must explain that selected business data leaves the device and is sent to the chosen provider.
- Free-tier availability, eligible models, quotas, data handling and terms can change. Show provider limits as provider-reported/configured information; do not guarantee availability or silently switch to a paid tier.
- Never embed a shared developer API key in the app, repository, CI variables used to ship binaries, or source code.

### Local
- First local target: Ollama at the local endpoint (default `http://127.0.0.1:11434`).
- Local mode requires the user to install Ollama and download a compatible model. Model size and quality depend on available RAM/VRAM and hardware.
- No API key is required for the local Ollama endpoint by default.
- Add a connection test and model availability check. Do not download multi-gigabyte models automatically without explicit user action.

### Router and fallback
- Modes: `cloud_only`, `local_only`, `auto`.
- In `auto`, prefer local when configured and healthy if the user has selected local-first; otherwise use the selected cloud provider, then fall back only to another provider explicitly enabled by the user.
- Never silently send private data to cloud after a local failure. If the selected privacy mode forbids cloud, fail closed and explain how to recover.
- Handle 401/403, 429, timeouts, unavailable model and network errors distinctly. Use bounded retries and respect Retry-After when available.
- Do not implement provider-account rotation, quota circumvention or anonymous key sharing.

## 5. Shared request and response contract

Proposed types:

- `AiExecutionMode`: CloudOnly | LocalOnly | Auto.
- `AiProviderId`: Gemini | OpenAiCompatible | Ollama.
- `AiRequest`: user_text, conversation_id, execution_mode, privacy_mode, requested_capabilities.
- `AiResponse`: answer_markdown, provider_id, model_id, execution_location, generated_at, evidence, warnings.
- `AiEvidence`: tool_name, period, structured result summary, database snapshot timestamp.
- `AiToolCall`: allowlisted tool name plus typed arguments; never raw SQL or arbitrary filesystem/network access.

Both frontends should call the same service methods and render the same response model. Avoid duplicating prompts, provider clients, accounting queries or policy logic in Tauri and Slint.

## 6. Accounting tools and capability groups

Initial allowlist (read-only):

- Sales: sales totals by day/month/custom range, invoice count, average basket, period comparison.
- Profit: gross profit using saved historical `invoice_items.unit_cost`, returns/refunds, expenses and net result. Respect the current accounting definitions and avoid presenting gross margin as net profit.
- Inventory: on-hand quantity, moving average cost, stock value, low-stock items, no-movement items and recent stock movement.
- Contacts: customer/supplier balances and aggregate receivables/payables. Do not expose names or phone numbers unless necessary for the user’s explicit question.
- Alerts: deterministic thresholds and explainable anomaly candidates (e.g. unusual sales drop, negative/zero margin, low stock). Alerts are suggestions, not accusations or definitive fraud findings.
- Recommendations: reorder quantity suggestions based on available sales history and configured lead time/minimum stock. Mark assumptions and uncertainty.

Every tool must define input schema, authorization, SQL/query implementation, output schema, tests and evidence metadata. Use checked arithmetic and the accounting core's existing rules.

## 7. Privacy, security and safety

- Default state: AI disabled until configured.
- Separate settings for cloud permission, local permission, provider, model and data categories allowed to leave the device.
- API keys must be stored using OS credential storage where available (Linux Secret Service/libsecret, Windows Credential Manager, macOS Keychain). If secure storage is unavailable, explain the limitation and offer a user-protected configuration path; never store plaintext keys in SQLite backups or logs.
- Redact secrets, authorization headers, customer names, phone numbers and invoice-level personal data from logs.
- Do not include API keys in crash reports, diagnostics, backups, screenshots or telemetry.
- Provide a “what will be sent” preview for cloud requests and a clear cloud/local indicator in every conversation.
- No training on user data by Nabz. Provider-side processing and retention are governed by that provider's current terms; explain this before enabling cloud.
- Prompt injection in user text, notes or imported content must not expand tool permissions. Treat retrieved database text as untrusted data.
- Initial AI tools are read-only. Future writes require a typed proposal, preview/diff, explicit confirmation, validation by accounting core, and an audit record. Never let the model execute SQL or shell commands.
- AI must not silently change prices, stock, invoices, balances, payments or settings.

## 8. User experience

Settings:
- Enable/disable Nabz AI.
- Execution mode: Automatic / Cloud only / Local only.
- Provider and model selector.
- API key setup and connection test.
- Ollama endpoint and installed-model check.
- Privacy/data-sharing explanation and per-category cloud permissions.
- Usage/limit status when provider exposes it.
- Clear chat history and retention controls.

Assistant:
- Persian RTL chat interface shared in behavior across Tauri and Slint.
- Suggested prompts: “فروش این ماه چقدر بوده؟”, “سود خالص این ماه؟”, “کدام کالاها رو به اتمام‌اند؟”, “بدهی مشتریان چقدر است؟”
- Evidence panel with date range and figures used.
- Show when the result is calculated locally without an LLM.
- Clear error states and retry/provider-switch controls.

## 9. Implementation phases

### Phase 0 — repository and compatibility audit
- Verify current branch layout, Rust edition/MSRV, shared-core public API and both frontend integration points.
- Confirm provider HTTP/TLS dependencies compatible with all supported Linux builds.
- Record exact feature flags and CI jobs before adding dependencies.
- No changes to the production accounting schema in this phase.

### Phase 1 — provider-neutral foundation
- Add shared AI request/response/config types and provider trait.
- Add disabled-by-default configuration and secret-store abstraction.
- Add mock provider and deterministic tests.
- Ensure both frontends can call a no-op/status API without network dependencies.

### Phase 2 — local Ollama
- Implement local connection test, model discovery/selection and chat completion.
- Add timeouts, cancellation and clear model-not-installed errors.
- Test with a small model on a developer machine; no hardware performance promise until measured.

### Phase 3 — cloud Gemini and generic endpoint
- Implement Gemini adapter and configurable OpenAI-compatible adapter.
- Add explicit cloud consent, key storage, request redaction, quota/error mapping and no-silent-fallback behavior.
- Verify provider terms and active free-tier limits at release time.

### Phase 4 — read-only accounting tools
- Implement sales, profit, inventory, contact-balance and alert tools in Rust.
- Prefer deterministic answers for exact totals.
- Add fixture-based tests for returns, refunds, expenses, moving-average cost and date ranges.
- Ensure historical sales use saved unit cost and not today's purchase price.

### Phase 5 — all assistant capabilities
- Add Persian intent routing, conversational explanations, evidence references, sales comparisons, inventory advice and anomaly explanations.
- Add explicit assumptions and confidence/coverage notes where history is incomplete.
- Keep the first release read-only.

### Phase 6 — Tauri and Slint UI
- Build matching settings and assistant experiences in both frontends.
- Both frontends call the same nabz-core API and display identical provider/privacy state.
- Test UI switching while preserving the same database, settings and conversation history.

### Phase 7 — CI, privacy and release hardening
- Add unit, integration, mock-provider and provider-contract tests.
- Add Linux CI coverage for both frontend builds; test without network for core accounting.
- Verify keys never appear in repository, binary defaults, logs or backup.
- Test offline mode, 429/quota exhaustion, invalid key, missing Ollama, timeout and provider outage.
- Publish only after CI passes and a real local install test is completed.

## 10. Acceptance criteria

- Nabz runs normally with AI disabled and without internet.
- Tauri and Slint use the same shared AI service and same configuration/database.
- Local Ollama chat works without a cloud key when a compatible model is installed.
- Cloud use is explicitly enabled and shows which data is sent.
- No shared API key is shipped with Nabz.
- A quota/network failure never triggers an undisclosed paid request or cloud data transfer.
- Exact financial answers are generated by tested Rust accounting tools.
- AI cannot execute SQL or mutate accounting data in the initial release.
- Responses identify provider/model/location and evidence period.
- CI verifies both UI integrations and core behavior.

## 11. Risks and decisions to revisit

- Free cloud tiers can change or prohibit certain production/commercial usage; re-check current terms before each release.
- Local model speed and Persian quality depend on hardware and model choice; publish measured recommendations only.
- The existing crate layout and frontend build targets must be confirmed before finalizing dependencies.
- Conversation history schema should be added only after retention, backup and privacy behavior are agreed; keep it separate from accounting tables.
- Do not make AI a dependency of invoice posting, inventory updates or any critical accounting workflow.

## 12. Reference documentation

- Gemini API pricing and free-tier conditions: https://ai.google.dev/gemini-api/docs/pricing
- Gemini API rate limits: https://ai.google.dev/gemini-api/docs/rate-limits
- Ollama API introduction: https://github.com/ollama/ollama/blob/main/docs/api/introduction.mdx
- Ollama OpenAI compatibility: https://github.com/ollama/ollama/blob/main/docs/api/openai-compatibility.mdx
- Shared Nabz architecture: [shared-core-two-uis.md](shared-core-two-uis.md)
