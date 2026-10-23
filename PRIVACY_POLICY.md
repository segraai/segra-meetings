# Privacy Policy for Segra Meetings

**Effective Date:** October 17, 2026  
**Last Updated:** October 23, 2026

At **Segra Meetings** ("we", "us", or "our"), accessible at [https://segra.ai/meetings](https://segra.ai/meetings), privacy is the foundational principle of our product. Segra Meetings is designed as a **local-first desktop application** that captures, transcribes, and organizes meetings directly on your device.

This Privacy Policy explains what information Segra Meetings accesses, how it is processed and stored, and our strict practices regarding third-party integrations, including Google user data.

---

## 1. Local-First Processing Philosophy

Segra Meetings runs locally on your computer. Under default operation:
- **Audio Recordings:** Microphone and system audio streams are captured and stored solely on your local hard drive.
- **Transcriptions:** Speech-to-text models (such as local Whisper or Parakeet) process audio directly on your device using your computer's CPU/GPU. No audio recordings are ever transmitted to our servers.
- **Meeting Notes & Transcripts:** Your meeting titles, transcripts, attendee notes, and summaries are saved in a local database stored entirely on your computer.

---

## 2. Google User Data & Calendar Integration

Segra Meetings provides an optional integration with Google Calendar to display your schedule, remind you before calls begin, and automatically name your meeting recordings.

### 2.1 Google Data Accessed
When you authenticate using "Connect Google Calendar", Segra Meetings uses OAuth 2.0 PKCE desktop authorization to request access to the following scope:
- `https://www.googleapis.com/auth/calendar.readonly`

Through this scope, the application accesses:
- **Calendar Event Metadata:** Event titles (summaries), descriptions, start and end dates/times, event status, and calendar event URLs.
- **Meeting Links:** Conference data (e.g., Google Meet, Zoom, or Microsoft Teams links).
- **Attendee Information:** Attendee names and email addresses.
- **Account Identity:** Primary email address of the authenticated Google account (used solely to indicate the active connection state in the application interface).

The application reads events from your primary calendar covering the window of **one day prior to seven days in advance**. The application **never** requests write, edit, or delete permissions, and **never** creates, alters, or deletes your Google Calendar events.

### 2.2 How We Use Google Data
Google user data is used strictly for the following client-side features:
1. **Upcoming Meeting Display:** Showing upcoming events on your application home screen.
2. **Meeting Reminders:** Displaying local desktop notifications shortly before a scheduled meeting begins.
3. **Automatic Recording Titling:** Matching active recordings to current calendar events to name your meeting notes automatically.
4. **Meeting Context:** Associating attendee lists with your meeting notes for easy reference.

### 2.3 Storage and Security of Google Data
- **On-Device Storage Only:** All retrieved calendar data and OAuth authentication tokens (access and refresh tokens) are stored **exclusively on your local computer** within the application's protected local data directory (in the local SQLite database and `meeting_assistant.json`).
- **No Developer Server Transmission:** Your calendar events and tokens are never uploaded, proxied, or transmitted to Segra servers or any third-party analytics servers.
- **Direct Connection:** All API communications occur directly between your device and Google's official endpoints (`https://www.googleapis.com`).

### 2.4 Sharing and Disclosure of Google Data
- **No Sale of Data:** We do not sell, rent, lease, or monetize Google user data under any circumstances.
- **No Advertising or Profiling:** Google user data is never shared with advertising networks, data brokers, or marketing platforms.
- **No AI Model Training:** **Google user data is never used to train, retrain, fine-tune, or improve generalized artificial intelligence or machine learning models.**

### 2.5 Data Retention, Revocation, and Deletion
- **In-App Disconnect:** You can disconnect your Google account at any time via **Settings → Calendar & Meetings → Disconnect**. When clicked, the application immediately:
  1. Revokes the authentication token directly with Google (`https://oauth2.googleapis.com/revoke`).
  2. Permanently deletes all stored tokens and cached calendar events from your local computer.
- **Google Account Permissions:** You can also revoke access externally at any time via your [Google Account Permissions Page](https://myaccount.google.com/permissions).

### 2.6 Google API Limited Use Disclosure
> **Segra Meetings' use and transfer of information received from Google APIs to any other app will adhere to the [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy), including the Limited Use requirements.**

---

## 3. Meeting Notes and AI Summarization

Segra Meetings allows you to generate summaries combining your typed notes and meeting transcripts:
- **Local Models (Default):** Summarization using built-in local models (via `llama-cpp` sidecar) or local Ollama instances processes text 100% locally on your machine.
- **Subscription CLIs:** If you use the "Use your subscription" feature (Claude Code, OpenAI Codex CLI, or Gemini CLI), prompts are sent through your own locally installed command-line tools under your respective direct account agreements.
- **External AI Providers (Optional):** If you choose to configure an external cloud API key (such as Anthropic Claude, OpenAI, or Groq), the transcript and prompt text are sent directly to that provider over encrypted HTTPS connections in accordance with that provider’s privacy terms. We do not intermediate or retain these requests.
- **Segra Models (Optional):** If you choose Segra models, summaries and meeting chat run on private models in secure enclaves (TEEs), paid from your Segra account. See section 4.

---

## 4. Segra Account (Optional)

Segra models require signing in to a Segra account. Segra's server handles sign-in, model access and billing only. **Your transcripts, notes, prompts and summaries never pass through Segra's server**: the app sends them directly to the model provider over HTTPS, using a short-lived session that Segra's server issues.

When you use a Segra account, Segra's server receives:
- **Sign-in:** a sign-in request carrying the app's name ("Segra Meetings") and your computer's name, so the access key can be listed and recognized in your Segra account. You approve the sign-in on the Segra website.
- **Model sessions:** your access key and the name of the model you chose, each time the app starts a session with a model.
- **Usage reports:** after each response, the provider, the model and the number of input and output tokens, used to charge your credits.
- **Balance checks:** requests to show your remaining credits.

The access key is stored only in the app's local database on your computer. You can sign out in the app, or revoke the key from your Segra account at any time. Your Segra account itself (email address, credits and usage history) is held by Segra and covered by Segra's own privacy policy on the Segra website.

---

## 5. Telemetry and Analytics

Segra Meetings prioritizes user privacy:
- Any crash reporting or anonymous usage metrics are **strictly opt-in and disabled by default**.
- When disabled, no telemetry is gathered or transmitted.
- Even when enabled, telemetry **never** includes audio recordings, transcripts, meeting titles, calendar events, personal notes, or identifiable account details.

---

## 6. Security

We protect your data by:
- Storing all application databases and audio files locally within your operating system's standard user-permission-restricted application directories.
- Using standard OAuth 2.0 PKCE with state verification and loopback redirection to secure token exchanges during sign-in.
- Supporting secure token revocation upon disconnection.

---

## 7. Your Rights and Choices

Apart from your optional Segra account (section 4), all user data resides locally on your computer:
- **Access & Portability:** You have direct access to your SQLite database, audio recordings, and markdown notes at all times.
- **Deletion:** You can delete individual meetings, clear transcripts, disconnect integrations, or completely wipe application data simply by uninstalling the application and deleting its application data directory.

---

## 8. Changes to This Privacy Policy

We may update this Privacy Policy from time to time to reflect product updates or legal requirements. Material changes will be noted with an updated "Last Updated" date at the top of this document, and notable updates will be surfaced in application release notes.

---

## 9. Contact Us

If you have questions or concerns regarding this Privacy Policy or your data, please contact us:

- **Email:** [support@segra.ai](mailto:support@segra.ai)  
- **Application Homepage:** [https://segra.ai/meetings](https://segra.ai/meetings)  
- **Developer & Company Website:** [https://segra.ai](https://segra.ai)
