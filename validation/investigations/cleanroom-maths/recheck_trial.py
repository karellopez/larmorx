# SPDX-License-Identifier: Apache-2.0
"""Every finding of a random-testing trial of fslmaths' and fslstats' clean-room originals
replayed with the current build (``difftest --replay``: the shrunk recipe, FSL's runs from the
oracle cache), ``--jobs`` at a time; one JSON line per finding in ``<out.jsonl>`` and the counts
by verdict and by the remaining differences' signatures and operations (the triage of
``validation/difftest/triage.toml``'s ``fsl-maths-original`` and ``fsl-stats-original`` entries,
``docs/validation/fsl-maths.md``).

Run inside a memory box (each replay holds at most about 1 GiB):
``python -P recheck_trial.py <night> <out.jsonl> [--jobs 8] [--merge <earlier.jsonl>]`` (with
``--merge``, only the findings that still differ there are replayed), with ``LARMORX_MATHS_BIN`` the
build and ``LARMORX_DIFFTEST_CACHE_MODE=use``.
"""

import collections
import concurrent.futures as cf
import json
import os
import re
import subprocess
import sys
from pathlib import Path

_LINE = re.compile(r"^(fsl-(?:maths|stats)-original) \(original\): (\w+)\s*(.*)$")


def _replay(f: Path) -> dict:
    p = subprocess.run(
        [sys.executable, "-m", "larmorx_validation", "difftest", "--replay", str(f)],
        capture_output=True,
        text=True,
        env=os.environ.copy(),
        timeout=900,
    )
    lines = (p.stdout + p.stderr).splitlines()
    verdict, detail = "unclear", (p.stdout + p.stderr)[-400:]
    for k, line in enumerate(lines):
        m = _LINE.match(line.strip())
        if m:
            verdict = m.group(2)
            detail = " ".join([m.group(3), *[x.strip() for x in lines[k + 1 : k + 3]]])[:600]
    d = json.loads(f.read_text(encoding="utf-8"))
    rec = d.get("shrunk") or d["recipe"]
    ops = [g["name"] for g in rec["groups"] if g["name"] not in ("input", "output")]
    return {
        "finding": f.name,
        "tool": d["tool"],
        "case": f"{d['tool']}:{d['seed']}:{d['n']}",
        "trial_signature": d.get("signature"),
        "verdict": verdict,
        "detail": detail,
        "ops": ops,
        "args": " ".join(" ".join(g["args"]) for g in rec["groups"]),
    }


def main(argv: list[str]) -> int:
    night, out = Path(argv[0]), Path(argv[1])
    jobs = int(argv[argv.index("--jobs") + 1]) if "--jobs" in argv else 8
    files = sorted((night / "findings").rglob("*.json"))
    merged: dict[str, dict] = {}
    if "--merge" in argv:
        # replay only the findings that still differ in an earlier re-check, keep the others
        old = Path(argv[argv.index("--merge") + 1])
        for line in old.read_text(encoding="utf-8").splitlines():
            r = json.loads(line)
            merged[f"{r['tool']}/{r['finding']}"] = r
        files = [
            f
            for f in files
            if merged.get(f"{f.parent.name}/{f.name}", {}).get("verdict") not in ("agree", "notice")
        ]
    rows = []
    with cf.ThreadPoolExecutor(jobs) as ex:
        for r in ex.map(_replay, files):
            merged[f"{r['tool']}/{r['finding']}"] = r
    rows = [merged[k] for k in sorted(merged)]
    with out.open("w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")
    by = collections.Counter((r["tool"], r["verdict"]) for r in rows)
    print(f"{len(rows)} findings replayed")
    for k, n in sorted(by.items()):
        print(f"  {k[0]} {k[1]}: {n}")
    rest = collections.Counter(
        (r["tool"], r["detail"].split(":")[0][:90]) for r in rows if r["verdict"] == "differ"
    )
    for k, n in rest.most_common(40):
        print(f"  [{n}] {k[0]}: {k[1]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
