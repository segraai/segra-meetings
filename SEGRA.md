# Segra in Segra Meetings

Segra Meetings can generate notes, and answer meeting chat, with **Segra models**: private models running in TEEs, paid from the user's Segra account. It's the same account and credits as Segra Desktop and the Segra web app.

**Privacy:** the transcript goes straight from this computer to the model provider and never passes through Segra's server. The server only:
- signs the user in
- hands out short-lived provider sessions
- receives token counts for billing

## How it works

1. **Sign in** (Settings → Summary → provider "Segra" → **Sign in with Segra**). Device sign-in, the same flow as Segra Desktop:
   - The app asks Segra for a code (`POST /device/code`, `client_app: "Segra Meetings"`) and opens the approval page in the browser.
   - The user signs in to Segra on the web and approves the code.
   - The app polls (`POST /device/token`) and receives a Segra API key (`sk-segra-…`).
   - The key is stored in the local settings database (`settings.segraApiKey`) and **never sent to the webview**. On the website it shows under API keys as "Segra Meetings (hostname)", where it can be revoked.
2. **Each summary or chat request:**
   - The app trades the key for a provider session for the chosen model (`POST /session` → `{provider, endpoint, model, token, expiresAt}`), cached per model until a minute before it expires.
   - It sends the OpenAI-style request straight to `{endpoint}/chat/completions`.
   - If the provider answers 401, it mints a new session and retries once.
3. **After each response:** it reports `{inputTokens, outputTokens}` to `POST /usage`, in the background. Failures are only logged.

The models list comes from `GET /models` (public): models with a funded provider right now.

### Server

Release builds (`tauri build`) use the deployed server, `https://web.segra.ai/api/app/v1`. Debug builds (`tauri dev`) use the local dev server, `http://localhost:3000/api/app/v1` (`npm run dev` in segra-web). `SEGRA_BASE_URL` (at run time, or baked in at build time) overrides either, for example to run dev against production.

### Errors the user sees

| Situation | Message |
|---|---|
| Not signed in | "Sign in to Segra in Settings → Summary to use Segra models." |
| Key revoked or expired | "Your Segra sign-in has expired or was revoked. Sign in again…" |
| Out of credits (402) | "Your Segra balance is used up. Top up your Segra account…" |
| Provider below its reserve | "This Segra model is temporarily unavailable. Try again later, or pick another Segra model…" |

Segra models reason before answering, so `max_tokens` is left unset; with a low cap, the answer can come back empty. Reasoning text is stripped, as for other providers.

## Change log

### 2026-10-21: Segra provider

**Rust (`frontend/src-tauri/`)**
- `src/segra/mod.rs` (new):
  - base URL
  - account storage
  - the session cache
  - `generate()`: session → direct request → usage report, one retry on a 401, cancellation
  - `list_models()`
  - tests: a unit test, plus an opt-in live test (`SEGRA_TEST_KEY=… cargo test --lib segra:: -- --include-ignored`)
- `src/segra/commands.rs` (new): Tauri commands
  - `segra_get_account`
  - `segra_start_sign_in`, `segra_finish_sign_in`, `segra_cancel_sign_in`
  - `segra_sign_out`
  - `segra_list_models`
  - `segra_get_balance`
- `migrations/20261021000000_add_segra_account.sql` (new): `settings.segraApiKey`, `settings.segraUserId`.
- `src/summary/llm_client.rs`:
  - `LLMProvider::Segra` (`"segra"`)
  - `generate_summary` hands Segra off to `crate::segra::generate` before the HTTP provider match
  - provider name "Segra"
- `src/summary/service.rs`: the Segra provider reads the account key (`segra::require_api_key`) and fails with a sign-in message if it's missing.
- `src/knowledge/chat.rs`: meeting chat supports Segra the same way.
- `src/database/repositories/setting.rs`: `save_api_key` and `delete_api_key` are no-ops for `"segra"`, and `get_api_key` returns none. The key is saved only by sign-in and is kept out of the settings form and the webview.
- `src/lib.rs`: `pub mod segra;`, with the commands registered.

**Frontend (`frontend/src/`)**
- `components/SegraAccountSetup.tsx` (new): the privacy note, Sign in with Segra (code display, waiting state, reopen page, cancel), the signed-in account with credits left, and Sign out.
- `components/ModelSettingsModal.tsx`:
  - a "Segra (private models, your Segra account)" provider
  - its models loaded from Segra
  - the account panel
  - Done is disabled until signed in
- `services/configService.ts`, `contexts/ConfigContext.tsx`: `'segra'` added to the provider type, default model `0GM-1.0-35B-A3B`.

**Segra server**
- Device sign-in takes `client_app`, allow-listed to "Segra Desktop" and "Segra Meetings"; anything else falls back to Desktop, so a request can't give itself a misleading name.
- The key is named `<app> (<host>)`.
- The approval page names the requesting app. Its privacy note now also mentions meeting transcripts.

### 2026-10-21 (later): per-user wallet accounts

Segra accounts can now have their own wallet, with balances in **credits**.
- `segra_get_balance` returns `{credits, usd, status}`. The account panel shows "N credits left", "Setting up your account…" or "Balance used up", and falls back to "$X in credits left" for older accounts.
- Session errors 403 ("<model> is available after a top-up…") and 409 ("Setting up your account…") are shown exactly as Segra words them.

### Verified

- `cargo check`: clean apart from warnings that were already there. `tsc --noEmit`: clean.
- **Device sign-in against the local server:**
  - the approval page shows "Segra Meetings" on "test-host"
  - polling returns `authorization_pending` before approval, then the key
  - the key is listed as "Segra Meetings (test-host)"
- **Live Rust test** (`segra::tests::live_summary_through_segra`), with a key from that sign-in on a throwaway user (deleted afterwards):
  - models listed: `0GM-1.0-35B-A3B`, `glm-5`
  - a summary came back with Summary / Decisions / Action items ("moved from Friday to **Monday**"), reasoning stripped
  - the session was cached
  - the usage report reached Segra: 81 in / 1,255 out tokens, $0.0006
- Replaying Meetily's custom-endpoint requests against the 0GM mainnet provider (before this change): the connection test passes and a summary works. `GET /models` isn't supported by providers, so the model has to be typed in. That's why the Segra provider lists models from Segra itself.

### Still to do

- App identity: `tauri.conf.json` still has `productName: "meetily"` and `identifier: "com.meetily.ai"`, so it can clash with a real Meetily install and get the wrong taskbar icon on Linux.
- `origin` still points at `Zackriya-Solutions/meetily`. Set up a private repo, with that repo as `upstream`, the same way as Segra Desktop.
