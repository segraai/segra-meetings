# Google requirements for Segra Meetings

Everything needed to let people sign in with Google for the Calendar connection: the
one-time Google Cloud setup, what "Testing" and "In production" mean, and how to get the
app verified. Written 2026-10-17; Google renames console pages often, so look for the
closest match if a label differs.

## What the app needs from Google

| Item | Value |
| --- | --- |
| API | Google Calendar API |
| Permission (scope) | `https://www.googleapis.com/auth/calendar.readonly` |
| Scope category | **Sensitive** (not restricted): verification is free and needs no security audit |
| OAuth client type | Desktop app |
| Sign-in flow | System browser, loopback redirect to `http://127.0.0.1:<random port>`, PKCE (S256), `state` check, `access_type=offline` |
| What is read | Primary calendar only, events from 1 day ago to 7 days ahead: title, description, start/end, location, video meeting link, event link, attendee names and emails |
| Where it is stored | Only on the user's computer (app data folder): events in the local database, tokens in `meeting_assistant.json` |
| Disconnect | Revokes the token with Google (`oauth2.googleapis.com/revoke`) and deletes stored tokens and events |

Code: `frontend/src-tauri/src/meeting_assistant/google_calendar.rs` (sign-in, sync,
disconnect), `settings.rs` (token storage), `frontend/src/components/CalendarSettings.tsx`
(the settings screen).

## 1. One-time Google Cloud setup

1. Open [Google Cloud Console](https://console.cloud.google.com/) and create a project
   (e.g. "Segra Meetings").
2. **APIs & Services → Library** → search **Google Calendar API** → **Enable**.
3. **OAuth consent screen** (shown as "Google Auth Platform" in newer consoles):
   - User type / audience: **External**.
   - Fill the branding fields from the table in section 3.
   - **Data access / Scopes**: add `.../auth/calendar.readonly`.
   - While in Testing: add your Google account (and anyone you share with) under
     **Test users**.
4. **Credentials → Create credentials → OAuth client ID** → type **Desktop app** → copy the
   **client ID** and **client secret**.
5. In Segra Meetings: **Settings → Calendar & Meetings** → paste both → **Connect Google
   Calendar**.

A Desktop app's client secret is not treated as confidential by Google (it ships inside
apps), so giving it to people you share the app with is acceptable.

## 2. Publishing status: Testing vs In production

Set on the consent screen ("Audience" page). You can switch to production at any time
with **Publish app**; it takes effect immediately and needs no approval.

| | Testing | In production, not verified | In production, verified |
| --- | --- | --- | --- |
| Who can sign in | Only listed test users (max 100) | Anyone | Anyone |
| Sign-in lasts | **7 days**, then must reconnect | Until revoked | Until revoked |
| Warning shown | "Google hasn't verified this app" → Continue | Stronger warning: **Advanced → Go to Segra Meetings (unsafe)** | None |
| User limit | 100 test users | **100 users lifetime** (sensitive scope) | No cap |
| Cost / effort | None | None | Free; branding + scope review |

Recommendation: publish to production now (fixes the 7-day reconnects), and apply for
verification once the website is live and before sharing beyond a handful of people.

## 3. Consent screen fields

Must match across the consent screen, the website and the app; Google checks.

| Field | Value |
| --- | --- |
| App name | Segra Meetings |
| User support email | `support@segra.ai` |
| App logo | 120×120 px PNG of the Segra "S" mark (`frontend/src-tauri/icons/icon_128x128.png` resized). Adding a logo triggers brand verification. |
| Application home page | `https://segra.ai/meetings` (or wherever the homepage copy is published) |
| Privacy policy link | `https://segra.ai/meetings/privacy` |
| Terms of service link | `https://segra.ai/meetings/terms` (optional) |
| Authorized domains | `segra.ai` |
| Developer contact email | `support@segra.ai` |

Domain ownership: verify `segra.ai` in [Google Search Console](https://search.google.com/search-console)
with the same Google account that owns the Cloud project (or add that account as an owner).

## 4. Website requirements

The homepage and privacy policy copy is in the Claude doc "Segra Meetings website copy".
Requirements Google checks:

- **Homepage** on the verified domain, publicly reachable without login, describing what
  the app does, and linking to the privacy policy.
- **Privacy policy** on the same domain, covering: what Google data is accessed, how it is
  used, stored and shared, how users delete it or revoke access, and the Limited Use
  statement below (word for word is safest).
- Homepage, privacy policy and consent screen all use the same app name.

Limited Use statement (already in the policy copy):

> Segra Meetings' use and transfer of information received from Google APIs adheres to the
> [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy),
> including the Limited Use requirements.

## 5. Verification submission

Start from the consent screen: **Verification Center** / **Prepare for verification**.

### Scope justification (paste into the form)

> Segra Meetings is a desktop app that records and summarizes the user's meetings on their
> own computer. It uses calendar.readonly to read the user's primary calendar from one day
> ago to seven days ahead so it can (1) list upcoming meetings in the app, (2) remind the
> user shortly before a meeting starts, (3) name each recording after its calendar event,
> and (4) show the meeting's attendees. The app never creates, edits or deletes events.
> Event data and tokens are stored only on the user's device and are not sent to our
> servers. A narrower scope is not sufficient because the reminders and recording names
> need event titles, times, attendees and meeting links, which calendar.readonly provides
> and which calendar.freebusy does not.

### Demo video (unlisted YouTube link)

Record the screen in English with the browser address bar visible. About 2 minutes:

1. Open Segra Meetings → **Settings → Calendar & Meetings**.
2. Click **Connect Google Calendar**. Show the Google consent screen: the app name "Segra
   Meetings" and the requested permission ("See and download any calendar you can access
   using your Google Calendar"). Show the URL bar, including the `client_id`.
3. Approve. Show the app now "Connected as <email>".
4. Show the Home screen listing upcoming events from the calendar.
5. Show a meeting reminder notification for an upcoming event.
6. Start and stop a recording during a calendar event; show the meeting is named after the
   event.
7. Click **Disconnect** in Settings and show the events disappear.

### Timeline

- Brand verification (name, logo, domain): usually 2–3 business days.
- Sensitive scope review: often 1–3 weeks, with email back-and-forth. Answer from the same
  account and keep the app in production meanwhile.

Not required for this scope: a third-party security assessment (CASA), which only applies
to restricted scopes such as full Gmail or Drive access.

## 6. After verification: build the client into the app

Today every user pastes a client ID and secret. Once the client is verified, ship it with
the app so people only click **Connect Google Calendar**:

- Embed the verified client ID and secret at build time (e.g. an env var read by
  `google_calendar.rs`), keep the paste-your-own fields as an advanced option.
- Hide the "One-time setup: create a Google OAuth client" instructions when the built-in
  client is present.
- Rotating the secret later means shipping a new build, so keep the Cloud project's owners
  limited.

## Checklist

- [ ] Cloud project created, Calendar API enabled
- [ ] Consent screen filled (section 3), `calendar.readonly` added
- [ ] Desktop OAuth client created, app connected
- [ ] Published to production (stops 7-day expiry)
- [ ] `segra.ai` verified in Search Console
- [ ] Homepage and privacy policy live on segra.ai, linked from the consent screen
- [ ] Logo uploaded, support and developer emails set
- [ ] Demo video recorded and uploaded (unlisted)
- [ ] Verification submitted with the scope justification
- [ ] After approval: client built into the app (section 6)

## References

- [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy)
- [OAuth 2.0 for desktop apps](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Google Cloud Console credentials](https://console.cloud.google.com/apis/credentials)
- [Google Search Console](https://search.google.com/search-console)
