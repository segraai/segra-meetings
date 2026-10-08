#!/usr/bin/env bash
# Builds a standalone test copy that runs side by side with an installed Meetily:
# separate app identifier (own data dir + single-instance lock) and its own display name.
#
#   ./scripts/build-test-copy.sh            # name: MeetingsYo
#   APP_NAME="Other" ./scripts/build-test-copy.sh
#
# Output: ../target/release/meetily (not bundled, not installed)
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="${APP_NAME:-MeetingsYo}"
IDENTIFIER="${IDENTIFIER:-com.meetily.ai.dev}"

export NEXT_PUBLIC_APP_NAME="$APP_NAME"
export MEETILY_APP_NAME="$APP_NAME"

CONFIG=$(cat <<EOF
{
  "identifier": "$IDENTIFIER",
  "productName": "$APP_NAME",
  "build": { "beforeBuildCommand": "./node_modules/.bin/next build" }
}
EOF
)

FEATURES=()
if [ -n "${TAURI_GPU_FEATURE:-}" ]; then
  FEATURES=(-- --features "$TAURI_GPU_FEATURE")
fi

./node_modules/.bin/tauri build --no-bundle --config "$CONFIG" "${FEATURES[@]}"

# First run: seed the test copy with a copy of the real database and settings,
# and share the downloaded models instead of re-downloading them
SRC="$HOME/.local/share/com.meetily.ai"
DST="$HOME/.local/share/$IDENTIFIER"
if [ ! -d "$DST" ] && [ -d "$SRC" ]; then
  mkdir -p "$DST"
  cp "$SRC/meeting_minutes.sqlite" "$DST/" 2>/dev/null || true
  cp "$SRC"/*.json "$DST/" 2>/dev/null || true
  ln -s "$SRC/models" "$DST/models"
  echo "Seeded $DST from $SRC"
fi

echo
echo "Run it with: $(cd .. && pwd)/target/release/meetily"
