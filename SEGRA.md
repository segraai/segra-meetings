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

### App identity

`productName` is "Segra Meetings" and `identifier` is `ai.segra.meetings` (`tauri.conf.json`); the Rust package and `package.json` are `segra-meetings`. The app keeps its data in the identifier's folder (on Linux `~/.local/share/ai.segra.meetings`). Builds before 2026-10-23 used `com.meetily.ai`; their data doesn't move over by itself. Some internal names still say Meetily on purpose: the recordings folder (`meetily-recordings`), the templates and models folders, `MEETILY_*` build variables, the `Meetily/<version>` User-Agent, and credits to the upstream project.

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

### 2026-10-23: renamed from Meetily to Segra Meetings

Installers and the app were still called "meetily" (`com.meetily.ai`), which could clash with a real Meetily install. What changed (see [App identity](#app-identity)):
- `tauri.conf.json`: `productName` `meetily` → `Segra Meetings`, `identifier` `com.meetily.ai` → `ai.segra.meetings` (matches Segra Desktop's `ai.segra.desktop`). Installers are now named "Segra Meetings".
- Package names `meetily` → `segra-meetings`: `frontend/src-tauri/Cargo.toml` (with `Cargo.lock`) and `frontend/package.json`. The built binary is `target/<profile>/segra-meetings`; the bundled one was already `segra_meetings` (`mainBinaryName`).
- Text users see now says Segra Meetings: the speech-recognition error ("Restart Segra Meetings"), the Google Calendar sign-in pages, and the meeting chat assistant's system prompt. Also the build message in `build.rs`.
- `scripts/build-test-copy.sh`: test copies default to `ai.segra.meetings.dev` and seed from `~/.local/share/ai.segra.meetings`.
- Left as Meetily on purpose: the recordings, templates and models folder names (renaming them would split recordings and re-download models), `MEETILY_*` build variables, the `Meetily/<version>` User-Agent, the upstream credit on the About page, and the import screens for old Meetily installs.

Moving an existing install over (done by hand on the dev machine, the only one that had used it):
- The new identifier keeps data in a new folder, and nothing moves it automatically. With the app closed, the old folder was copied: `cp -a ~/.local/share/com.meetily.ai ~/.local/share/ai.segra.meetings` (3.7 GB: meetings database, settings, the Segra sign-in, downloaded models). On macOS the folders are under `~/Library/Application Support/`, on Windows under `%APPDATA%`.
- Once the new build opens with everything in place, delete `~/.local/share/com.meetily.ai`.
- The old Linux package is named `meetily`, so the new one installs alongside it: remove it with `sudo apt remove meetily`.

### 2026-10-24: local first, Segra second

Summaries stay on the computer by default: onboarding downloads the transcription engine and the built-in summary model, as upstream did. In Settings → Summary, Built-in AI is listed first and Segra second. While Built-in AI is selected, a note offers Segra for more powerful models or when the built-in engine is slow, with a "Use Segra models" button (`ModelSettingsModal.tsx`); the Segra panel's text says the same (`SegraAccountSetup.tsx`).

### 2026-10-25: live transcription by default, recording in the Transcript tab

- Transcription is live during the meeting by default again, as in upstream Meetily (`default_transcribe_after_meeting` is `false`). "After the meeting" stays in Settings → Recording for computers where live transcription is laggy. A saved choice is kept.
- The Recording tab is gone: the recording plays from a bar at the top of the Transcript tab (`RecordingPlayer`, passed to `TranscriptPanel` as `player`), so it can be played while reading. With no saved audio the bar is hidden.
- On Windows the built-in summary engine (`llama-helper`) is built with Vulkan, like transcription already was, so it runs on a dedicated GPU when there is one and on the CPU otherwise. It now reads the GPU's free memory to decide how much of the model fits (`dedicated_gpu_free_gb`); integrated GPUs are left out. Before, it was CPU-only on Windows.
- Settings → Transcription says Parakeet only understands European languages and points to Local Whisper for others; an empty transcript says the same.
- The "Writing your notes" placeholder after a recording only covers the wait before the summary starts, and gives up after 15 seconds (`AUTO_SUMMARY_WAIT_MS`). Before, an empty transcript (for example Korean speech with Parakeet, which only knows European languages) left it showing forever.

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

- `origin` still points at `Zackriya-Solutions/meetily`. Set up a private repo, with that repo as `upstream`, the same way as Segra Desktop.
