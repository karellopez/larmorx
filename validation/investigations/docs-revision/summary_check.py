# SPDX-License-Identifier: Apache-2.0
"""Does the Validation page's summary (``static/evidence/summary.json``) agree with every
evidence page's JSON? For each of its implementations taken from an evidence JSON, the cases
and verdict counts must equal that JSON's ``implementations[*].summary`` (the implementation
matched by its record). A page exported again without the summary made again shows here.

    python -P summary_check.py <site>

Prints one line per difference and the count; exit status 1 when any.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main() -> int:
    ev = Path(sys.argv[1]) / "static" / "evidence"
    summary = json.loads((ev / "summary.json").read_text(encoding="utf-8"))
    bad = checked = 0
    for t in summary["tools"]:
        page = t["page"].strip("/").split("/")[-1]
        f = ev / f"{page}.json"
        if not f.is_file():
            continue
        doc = json.loads(f.read_text(encoding="utf-8"))
        for im in t["implementations"]:
            if im.get("source") != "evidence":
                continue
            ims = [i for i in doc["implementations"] if i["record"] == im["record"]]
            if len(ims) != 1:
                print(f"{page}: {im['record']}: {len(ims)} implementations of its JSON name it")
                bad += 1
                continue
            s = ims[0]["summary"]
            checked += 1
            if (s["cases"], s["verdicts"]) != (im["cases"], im["verdicts"]):
                print(
                    f"{page}: {im['record']}: summary.json {im['cases']} {im['verdicts']}, "
                    f"its evidence {s['cases']} {s['verdicts']}"
                )
                bad += 1
    print(f"{checked} implementations checked, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
