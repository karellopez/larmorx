# SPDX-License-Identifier: Apache-2.0
"""The site's evidence for one page, counted, so an export can be shown to lose nothing (the
user, 2026-10-24: the site never shows less than before).

``python -P site_inventory.py count <site> <tool> > before.json`` lists, per example, its panels,
comparisons, outputs, facts and the volumes and images it ships (with their bytes, from
``static/evidence/blobs/``), the page's implementations, cases with a verdict per implementation,
and the speed chart's rows and series. ``python -P site_inventory.py compare before.json
after.json`` checks that every count after is at least the count before and that every id shown
before is still shown (exit status 1 and the missing items otherwise).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def blobs(x, out: set[str]) -> None:
    if isinstance(x, dict):
        for v in x.values():
            blobs(v, out)
    elif isinstance(x, list):
        for v in x:
            blobs(v, out)
    elif isinstance(x, str) and x.endswith((".bin", ".webp", ".png", ".json.gz")) and "/" not in x:
        out.add(x)


def count(site: Path, tool: str) -> dict:
    d = json.loads((site / "static" / "evidence" / f"{tool}.json").read_text(encoding="utf-8"))
    store = site / "static" / "evidence" / "blobs"
    out: dict = {"tool": tool, "examples": {}, "implementations": {}, "cases": {}}
    for ex in d.get("examples", []):
        b: set[str] = set()
        blobs(ex, b)
        out["examples"][ex["id"]] = {
            "panels": sorted(p["id"] for p in ex.get("panels", [])),
            "diffs": sorted(x["id"] for x in ex.get("diffs", [])),
            "outputs": sorted(o["id"] for o in ex.get("outputs", [])),
            "facts": len(ex.get("facts", [])),
            "grid": ex.get("grid", {}).get("shape"),
            "view": ex.get("view", "volume"),
            "arrays": sum(len(p.get("arrays", {})) for p in ex.get("panels", [])),
            "blobs": len(b),
            "bytes": sum((store / n).stat().st_size for n in b if (store / n).is_file()),
            "verdicts": sum(1 for x in ex.get("diffs", []) if x.get("reason")),
        }
    for i in d.get("implementations", []):
        out["implementations"][i["id"]] = i["summary"]["cases"]
    for c in d.get("cases", []):
        out["cases"][c["id"]] = sorted(c["results"])
    sp = d.get("speed") or {}
    out["speed"] = {
        "rows": sorted(r["input"] for r in sp.get("rows", [])),
        "series": sorted(s["id"] for s in sp.get("series", [])),
    }
    return out


def compare(a: dict, b: dict) -> list[str]:
    bad = []
    for eid, ea in a["examples"].items():
        eb = b["examples"].get(eid)
        if eb is None:
            bad.append(f"example {eid} missing")
            continue
        for k in ("panels", "diffs", "outputs"):
            missing = set(ea[k]) - set(eb[k])
            if missing:
                bad.append(f"{eid}: {k} missing: {sorted(missing)}")
        for k in ("facts", "arrays", "blobs", "verdicts"):
            if eb[k] < ea[k]:
                bad.append(f"{eid}: {k} {ea[k]} -> {eb[k]}")
        if ea["grid"] != eb["grid"] or ea["view"] != eb["view"]:
            bad.append(f"{eid}: grid/view {ea['grid']} {ea['view']} -> {eb['grid']} {eb['view']}")
    for iid, n in a["implementations"].items():
        if b["implementations"].get(iid, -1) < n:
            bad.append(f"implementation {iid}: {n} -> {b['implementations'].get(iid)}")
    for cid, impls in a["cases"].items():
        missing = set(impls) - set(b["cases"].get(cid, []))
        if missing:
            bad.append(f"case {cid}: results missing {sorted(missing)}")
    for k in ("rows", "series"):
        missing = set(a["speed"][k]) - set(b["speed"][k])
        if missing:
            bad.append(f"speed {k} missing: {sorted(missing)}")
    return bad


def main(argv: list[str]) -> int:
    if argv[:1] == ["count"]:
        print(json.dumps(count(Path(argv[1]), argv[2]), indent=1))
        return 0
    if argv[:1] == ["compare"]:
        a, b = (json.loads(Path(p).read_text(encoding="utf-8")) for p in argv[1:3])
        bad = compare(a, b)
        for line in bad:
            print(line)
        for eid, eb in b["examples"].items():
            ea = a["examples"].get(eid, {})
            print(
                f"{eid}: panels {len(ea.get('panels', []))} -> {len(eb['panels'])}, "
                f"diffs {len(ea.get('diffs', []))} -> {len(eb['diffs'])}, "
                f"arrays {ea.get('arrays', 0)} -> {eb['arrays']}, "
                f"blobs {ea.get('blobs', 0)} -> {eb['blobs']}, "
                f"bytes {ea.get('bytes', 0):,} -> {eb['bytes']:,}"
            )
        print(
            f"cases {len(a['cases'])} -> {len(b['cases'])}; implementations "
            f"{a['implementations']} -> {b['implementations']}; speed rows "
            f"{len(a['speed']['rows'])} -> {len(b['speed']['rows'])}"
        )
        print("after >= before" if not bad else f"{len(bad)} losses")
        return 1 if bad else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
