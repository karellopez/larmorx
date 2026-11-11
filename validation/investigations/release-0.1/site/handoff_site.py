# SPDX-License-Identifier: Apache-2.0
"""The first release's changes to the site's shared files (relay 5b, 2026-11-11), applied to a
site checkout: `site.yml`'s navigation (Install) and documentation list (Installation, Why Rust
and Python), and `data/status.yml`'s planned release item. Each replacement must apply exactly
once, or nothing is written. The same changes are in the release part's HANDOFF-site.toml, for
`scripts/orchestration/apply_handoff.py --site`.

Usage: python -P handoff_site.py <site checkout>
"""

import sys
from pathlib import Path

SITE_YML = [
    (
        '  - { title: Status, url: "/#status" }\n  - { title: Validation, url: "/validation/" }\n',
        '  - { title: Status, url: "/#status" }\n'
        '  - { title: Install, url: "/installation/" }\n'
        '  - { title: Validation, url: "/validation/" }\n',
    ),
    (
        '  - { title: Overview, url: "/" }\n  - { title: Validation, url: "/validation/" }\n',
        '  - { title: Overview, url: "/" }\n'
        '  - { title: Installation, url: "/installation/" }\n'
        '  - { title: Why Rust and Python, url: "/why-rust-and-python/" }\n'
        '  - { title: Validation, url: "/validation/" }\n',
    ),
]
STATUS_YML = [
    (
        "  - eco: [larmorx]\n    title: Releases on PyPI\n    text: for all six platforms.\n",
        "  - eco: [larmorx]\n    title: Release 0.1.0\n    text: the final release, once the"
        " release candidate 0.1.0rc1 installs cleanly on all six platforms.\n",
    ),
]


def replaced(text: str, pairs: list[tuple[str, str]], name: str) -> str:
    for old, new in pairs:
        if text.count(old) != 1:
            raise SystemExit(f"{name}: not found exactly once: {old!r}")
        text = text.replace(old, new)
    return text


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        raise SystemExit(__doc__)
    site = Path(argv[0])
    files = {"site.yml": SITE_YML, "data/status.yml": STATUS_YML}
    new = {
        rel: replaced((site / rel).read_text(encoding="utf-8"), pairs, rel)
        for rel, pairs in files.items()
    }
    for rel, text in new.items():
        (site / rel).write_text(text, encoding="utf-8")
    print("site.yml: nav + docs; data/status.yml: the planned release item")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
