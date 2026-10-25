# SPDX-License-Identifier: Apache-2.0
"""The site's checks and review photographs for the random-testing panels, through the site's
own review script (``tools/review/shoot.mjs``, headless Chrome): on every tool page and the
Validation page, the number check (``LX_EVIDENCE.check()``: every shown number against its
JSON) and the viewers' check (``LX_EVIDENCE.checkViewers()``), then photographs of the random
testing panel at 1440 and 390 pixels wide, light and dark.

    python -P validation/investigations/difftest/site_checks.py <site checkout> <base url> <out dir>

The site is served (``tools/build.py --serve``) at ``<base url>``.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

EVAL = (
    "(async () => { for (const e of document.querySelectorAll('.ev-example')) { e.scrollIntoView(); "
    "await new Promise(r => setTimeout(r, 2500)); } const v = await LX_EVIDENCE.checkViewers(); "
    "const c = await LX_EVIDENCE.check(); return JSON.stringify({viewers: v.length, viewers_bad: "
    "v.filter(x => !x.ok).map(x => x.diff + ':' + x.output), checked: c.checked, agree: c.agree, "
    "disagree: c.disagree.slice(0, 5), failed: document.querySelectorAll('.ev-failed').length}); })()"
)


def shoot(site: Path, url: str, out: Path, name: str, *extra: str) -> str:
    p = subprocess.run(
        ["node", "tools/review/shoot.mjs", "--url", url, "--out", str(out), "--name", name, *extra],
        cwd=site,
        capture_output=True,
        text=True,
        timeout=1200,
        check=False,
    )
    return p.stdout + p.stderr


def main() -> int:
    site, base, out = Path(sys.argv[1]), sys.argv[2].rstrip("/") + "/", Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    pages = ["validation/"] + [
        f"tools/{p.stem}/" for p in sorted((site / "content" / "tools").glob("*.md")) if p.stem != "index"
    ]
    results = {}
    for page in pages:
        name = page.strip("/").replace("/", "-")
        text = shoot(site, base + page, out / "checks", name, "--sizes", "1440x900", "--themes", "light", "--eval", EVAL)
        line = next((ln for ln in text.splitlines() if ln.startswith("eval ")), text[-300:])
        payload = line.split(": ", 1)[1] if ": " in line else line
        try:
            value = json.loads(payload)
            results[page] = json.loads(value) if isinstance(value, str) else value
        except json.JSONDecodeError:
            results[page] = payload
        panel = "#random-testing-summary" if page == "validation/" else "#random-testing-panel"
        shoot(site, base + page, out, f"{name}-random-testing", "--sizes", "1440x900,390x844", "--themes", "light,dark", "--select", panel)
        print(page, results[page], flush=True)
    (out / "checks.json").write_text(json.dumps(results, indent=1) + "\n", encoding="utf-8")
    bad = {
        p: r
        for p, r in results.items()
        if not isinstance(r, dict) or r.get("disagree") or r.get("viewers_bad") or r.get("failed")
    }
    print("ALL CHECKS AGREE" if not bad else f"PROBLEMS: {bad}")
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main())
