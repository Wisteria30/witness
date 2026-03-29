#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import yaml


def cargo_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.search(r'(?m)^version = "([^"]+)"$', text)
    if not match:
        raise SystemExit(f"could not read version from {path}")
    return match.group(1)


def main() -> int:
    root = Path(__file__).resolve().parents[1]

    plugin = json.loads((root / '.claude-plugin' / 'plugin.json').read_text(encoding='utf-8'))
    market = json.loads((root / '.claude-plugin' / 'marketplace.json').read_text(encoding='utf-8'))
    hooks = json.loads((root / 'hooks' / 'hooks.json').read_text(encoding='utf-8'))
    del hooks

    for path in list((root / 'policy').glob('*.yml')) + list((root / 'rules').glob('*.yml')) + [root / 'sgconfig.yml']:
        yaml.safe_load(path.read_text(encoding='utf-8'))

    cargo = cargo_version(root / 'Cargo.toml')
    plugin_version = plugin.get('version')
    market_plugins = market.get('plugins', [])
    if not market_plugins:
        raise SystemExit('marketplace.json has no plugins entry')
    market_version = market_plugins[0].get('version')

    versions = {cargo, plugin_version, market_version}
    if len(versions) != 1:
        raise SystemExit(
            f'version mismatch: Cargo.toml={cargo} plugin.json={plugin_version} marketplace.json={market_version}'
        )

    print(f'metadata ok: version {cargo}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
