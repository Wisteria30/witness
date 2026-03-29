#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: scripts/release.sh <version>" >&2
  exit 1
fi

VERSION="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

python3 - <<'PY' "$ROOT" "$VERSION"
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
version = sys.argv[2]

cargo = root / "Cargo.toml"
text = cargo.read_text(encoding="utf-8")
text = re.sub(r'(?m)^version = "[^"]+"$', f'version = "{version}"', text, count=1)
cargo.write_text(text, encoding="utf-8")

plugin = root / ".claude-plugin" / "plugin.json"
plugin_data = json.loads(plugin.read_text(encoding="utf-8"))
plugin_data["version"] = version
plugin.write_text(json.dumps(plugin_data, indent=2) + "\n", encoding="utf-8")

market = root / ".claude-plugin" / "marketplace.json"
market_data = json.loads(market.read_text(encoding="utf-8"))
market_data["plugins"][0]["version"] = version
market.write_text(json.dumps(market_data, indent=2) + "\n", encoding="utf-8")
PY

echo "code-guardrails: version synced to $VERSION"
