<div align="center">
    <img src="docs/segra-meetings-logo-120.png" width="120" alt="Segra Meetings" />
    <h1>Segra Meetings</h1>
    <p>Private meeting notes that stay on your computer.</p>
    <p><a href="https://segra.ai/meetings">segra.ai/meetings</a></p>
</div>

Segra Meetings records your calls, transcribes them on your own computer, and turns the transcript and your own notes into meeting notes. Recordings, transcripts and notes are stored locally. Nothing goes to the cloud unless you choose a cloud model for summaries.

Segra Meetings is based on [Meetily](https://github.com/Zackriya-Solutions/meetily) by Zackriya Solutions (MIT). See [Credits](#credits).

## Features

- **Local transcription** with Whisper or Parakeet models, with GPU acceleration where available
- **Microphone and system audio** captured together and mixed for the recording
- **Your own notes while recording**, with the live transcript underneath; summaries treat your notes as the most important context
- **Meeting notes from your choice of model:**
  - local: Ollama or a built-in local model
  - Segra models: private models running in secure enclaves (TEEs), paid from your Segra account; the transcript goes straight to the model provider, not through Segra's server
  - your own API key: Claude, OpenAI, Groq, OpenRouter, or any OpenAI-compatible endpoint
  - your own subscription, through the Claude Code, Codex or Gemini command-line tools
- **Google Calendar (read-only, optional):** upcoming meetings on the home screen, a reminder with "Join & record", and recordings named after the calendar event
- **Call detection** that can offer to record, or record automatically, and stops when the call ends
- **Search and "Ask your meetings":** search titles, notes and transcripts, or ask a question across past meetings
- **Folders**, pause and resume, import of existing audio files, and re-transcription with a different model

## Installation

Builds are published at [segra.ai/meetings](https://segra.ai/meetings) when available. Until then, build from source.

### Build from source

Requirements: Rust, Node.js and pnpm, plus the platform dependencies listed in [docs/BUILDING.md](docs/BUILDING.md) (Linux: [docs/building_in_linux.md](docs/building_in_linux.md)).

```bash
cd frontend
pnpm install --frozen-lockfile
./node_modules/.bin/next build
./node_modules/.bin/tauri build --no-bundle -c '{"build":{"beforeBuildCommand":""}}'
```

The app is built at `target/release/segra_meetings`. GPU options are described in [docs/GPU_ACCELERATION.md](docs/GPU_ACCELERATION.md).

**Google Calendar:** to build in a Google OAuth client, put `{"client_id": "...", "client_secret": "..."}` in `frontend/src-tauri/google-oauth.local.json` (git-ignored), or set `SEGRA_GOOGLE_CLIENT_ID` and `SEGRA_GOOGLE_CLIENT_SECRET`. Without one, users enter their own client in Settings. See [docs/google-requirements.md](docs/google-requirements.md).

**Segra models:** release builds use `https://web.segra.ai`, dev builds a local Segra server on port 3000; `SEGRA_BASE_URL` overrides either, at build time or run time. How sign-in and billing work is described in [SEGRA.md](SEGRA.md).

## Architecture

A single desktop app built with [Tauri](https://tauri.app/): a Rust core for audio capture, transcription, storage and model calls, and a Next.js interface. See [docs/architecture.md](docs/architecture.md).

## Privacy

See the [Privacy Policy](PRIVACY_POLICY.md) and [Terms of Service](TERMS_OF_SERVICE.md).

## License

MIT. See [LICENSE.md](LICENSE.md). The original Meetily copyright notice is kept as the license requires.

## Credits

- [Meetily](https://github.com/Zackriya-Solutions/meetily) by Zackriya Solutions and its contributors, the project this app is built on. The git history shows their work.
- [Whisper.cpp](https://github.com/ggerganov/whisper.cpp), [Screenpipe](https://github.com/mediar-ai/screenpipe) and [transcribe-rs](https://crates.io/crates/transcribe-rs), from which Meetily borrowed code.
- **NVIDIA** for the **Parakeet** model, and [istupakov](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx) for its ONNX conversion.
