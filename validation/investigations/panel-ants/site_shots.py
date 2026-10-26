# SPDX-License-Identifier: Apache-2.0
"""Before/after photographs and checks of the ANTs tools' pages of the documentation site (the
data panel's re-validation, part a), through the site's own review script
(``tools/review/shoot.mjs``, headless Chrome): each page whole at 1440 and 390 pixels wide, light
and dark, and once, at 1440 light, the number check (``LX_EVIDENCE.check()``: every shown number
against its JSON) and the viewers' check (``LX_EVIDENCE.checkViewers()``), printed as JSON.

    python -P site_shots.py <site checkout> <base url> <out dir> [page ...]

The site is served (``tools/build.py --serve --port N``) at ``<base url>``; pages default to the
seven ANTs pages (``tools/ants-*/``).
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

PAGES = [
    "tools/ants-programs/",
    "tools/ants-n4/",
    "tools/ants-denoise/",
    "tools/ants-atropos/",
    "tools/ants-ai/",
    "tools/ants-apply-transforms/",
    "tools/ants-registration/",
]
EVAL = (
    "(async () => { for (const e of document.querySelectorAll('.ev-example')) { e.scrollIntoView(); "
    "await new Promise(r => setTimeout(r, 2500)); } const v = await LX_EVIDENCE.checkViewers(); "
    "const c = await LX_EVIDENCE.check(); return JSON.stringify({examples: "
    "document.querySelectorAll('.ev-example').length, viewers: v.length, viewers_bad: "
    "v.filter(x => !x.ok).map(x => x.diff + ':' + x.output), checked: c.checked, agree: c.agree, "
    "disagree: c.disagree.slice(0, 5), failed: document.querySelectorAll('.ev-failed').length}); })()"
)


def shoot(site: Path, url: str, out: Path, name: str, *extra: str) -> str:
    p = subprocess.run(
        ["node", "tools/review/shoot.mjs", "--url", url, "--out", str(out), "--name", name, *extra],
        cwd=site,
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    return p.stdout + p.stderr


def main() -> int:
    site, base, out = Path(sys.argv[1]), sys.argv[2].rstrip("/") + "/", Path(sys.argv[3])
    pages = sys.argv[4:] or PAGES
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    for page in pages:
        name = page.strip("/").replace("/", "-")
        text = shoot(site, base + page, out / "checks", name, "--sizes", "1440x900",
                     "--themes", "light", "--eval", EVAL)  # fmt: skip
        line = next((ln for ln in text.splitlines() if ln.startswith("eval ")), text[-300:])
        payload = line.split(": ", 1)[1] if ": " in line else line
        try:
            value = json.loads(payload)
            results[page] = json.loads(value) if isinstance(value, str) else value
        except json.JSONDecodeError:
            results[page] = payload
        shoot(site, base + page, out, name, "--sizes", "1440x900,390x844",
              "--themes", "light,dark", "--full", "1")  # fmt: skip
        print(page, json.dumps(results[page]), flush=True)
    (out / "checks.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
