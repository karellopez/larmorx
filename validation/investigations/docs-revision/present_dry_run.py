# SPDX-License-Identifier: Apache-2.0
"""Apply an evidence module's presentation (its ``present``) to a page's published JSON and print
what the page will show: a dry run, before the page is exported again.

    PYTHONPATH=python:validation/src python -P present_dry_run.py <module> <json> <out json>
"""

from __future__ import annotations

import copy
import importlib
import json
import sys
from pathlib import Path


def main() -> None:
    module, src, dst = sys.argv[1:4]
    mod = importlib.import_module(f"larmorx_validation.evidence.{module}")
    doc = json.loads(Path(src).read_text(encoding="utf-8"))
    out = mod.present(copy.deepcopy(doc))
    for ex in out["examples"]:
        print("=====", ex["id"], "|", ex["title"])
        print("  SHOWS:", ex["shows"]["text"])
        print("        ", [(v["port"], v["diff"], v["verdict"]) for v in ex["shows"]["verdicts"]])
        panels = {p["id"]: p for p in ex["panels"]}
        for pid in ex["order"]["panels"]:
            p = panels[pid]
            print(f"  P {pid:16} {p['port']:10} {p['label']:50} | {p.get('sub', '')}")
        diffs = {x["id"]: x for x in ex["diffs"]}
        for title, ids in ex["order"]["groups"]:
            print("  G", title)
            for i in ids:
                x = diffs[i]
                print(f"     D {i:14} {x['label']:62} | {x['role_label']} | {x['verdict']}")
        print("  default:", ex["default"]["panels"], ex["default"]["diffs"])
        print("  SUM:", ex["summary"])
        for f in ex["facts"]:
            print("  F", f["label"], ":", f["text"][:200])
        for x in ex["diffs"]:
            print("  R", x["id"], ":", (x.get("reason") or "")[:220])
    for im in out["implementations"]:
        print("IMPL", im["id"], "|", im["title"], "|", im["comparison"], "|", im["label"])
    if out.get("speed"):
        print("SPEED", out["speed"]["title"], "|", out["speed"]["caption"])
    if out.get("band"):
        print("BAND", out["band"]["title"])
    for t in out.get("tables", []):
        print("TABLE", t["id"], "|", t["title"][:200], "|", (t.get("note") or "")[:200])
    Path(dst).write_text(json.dumps(out, indent=1, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
