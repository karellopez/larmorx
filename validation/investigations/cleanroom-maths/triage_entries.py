# SPDX-License-Identifier: Apache-2.0
"""The ``[[difference]]`` entries of ``validation/difftest/triage.toml`` for the trial of
fslmaths' and fslstats' clean-room originals (``oracles/difftest/2026-11-03-fsl-maths-original``):
one entry per distinct signature of the trial (the failed checks' names, as the trial's summary
groups its differences), its status and explanation from the triage categories of its findings
(``triage_counts.py``'s rules on the re-check with the final build), so that a later night's
difference with the same checks finds what was done about them.

Usage: ``python -P triage_entries.py <night> <recheck.jsonl>`` (prints the TOML text).
"""

from __future__ import annotations

import collections
import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _counts_module():
    spec = importlib.util.spec_from_file_location("triage_counts", HERE / "triage_counts.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


#: A category's status in triage.toml (difftest.md, "Triage").
def status_of(cat: str) -> str:
    if cat.startswith(("larmorx mistake, fixed", "fixed:")):
        return "fixed"
    if cat.startswith("harness"):
        return "harness"
    if cat.startswith("register"):
        return "register"
    if cat.startswith("open"):
        return "open"
    return "explained"  # explained, and expected (measured, no mistake, no defect)


#: The order in which a signature's statuses decide its entry's status.
ORDER = ("open", "register", "fixed", "harness", "explained")


def _q(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def main(argv: list[str]) -> int:
    night, rows_path = Path(argv[0]), Path(argv[1])
    tc = _counts_module()
    groups: dict[tuple[str, tuple[str, ...]], collections.Counter] = collections.defaultdict(
        collections.Counter
    )
    example: dict[tuple[str, tuple[str, ...]], tuple[int, str]] = {}
    for line in rows_path.read_text(encoding="utf-8").splitlines():
        if not line:
            continue
        r = json.loads(line)
        f = night / "findings" / r["tool"] / r["finding"]
        d = json.loads(f.read_text(encoding="utf-8"))
        rec = d.get("shrunk") or d["recipe"]
        dtype = (rec.get("inputs", {}).get("img") or {}).get("dtype", "")
        if r["verdict"] in ("agree", "notice"):
            cat = tc._fixed(d, r["ops"], r["tool"], dtype)
        else:
            cat = tc._left(r)
        key = (r["tool"], tuple(d["signature"]))
        groups[key][cat] += 1
        n = int(d["n"])
        if key not in example or n < example[key][0]:
            example[key] = (n, r["finding"].removesuffix(".json"))
    out = []
    for (tool, sig), cats in sorted(
        groups.items(), key=lambda kv: (kv[0][0], -sum(kv[1].values()))
    ):
        statuses = {status_of(c) for c in cats}
        status = next(s for s in ORDER if s in statuses)
        total = sum(cats.values())
        parts = "; ".join(f"{c} ({n})" for c, n in cats.most_common())
        expl = (
            f"Trial 2026-11-03-fsl-maths-original (seed fsl-maths-trial), {total} finding(s) with "
            f"these checks, each re-run with the part's final build and triaged by category "
            f"(validation/investigations/cleanroom-maths/trial_triage.json): {parts}. "
            f"Reproducer: {example[(tool, sig)][1]}."
        )
        out.append("[[difference]]")
        out.append(f"tool = {_q(tool)}")
        out.append('implementation = "original"')
        out.append("signature = [" + ", ".join(_q(s) for s in sig) + "]")
        out.append(f"status = {_q(status)}")
        out.append(f"explanation = {_q(expl)}")
        if status == "register":
            out.append('defect = "FSL-75"')
        if status in ("fixed", "explained", "open"):
            out.append('link = "docs/findings/fsl-maths-original.md"')
        out.append("")
    print("\n".join(out).rstrip("\n"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
