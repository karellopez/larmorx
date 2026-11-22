# SPDX-License-Identifier: Apache-2.0
"""mris_calc's validation record from the replica's runs: ``docs/validation/freesurfer-mris-calc.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the whole-program probe's summaries (``prog.py --json``, one per seed), the real calls on
the recorded heads (``real.py --json``), the memory figures of ``tests/parity/test_memory_peak.py``
(its printed lines) and the golden values, and writes the record with the commit it was made from
(a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-calc/record.py --prog <prog1.json> <prog2.json> \\
        --real <real.json> --memory <memory.log> --golden <sha> --golden-fix <sha>
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import subprocess
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
NAME = "freesurfer-mris-calc"


def commit() -> str:
    head = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "--short=12", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", str(REPO), "status", "--porcelain", "--untracked-files=no"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return head + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--prog", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    progs = [json.loads(p.read_text(encoding="utf-8")) for p in a.prog]
    real = json.loads(a.real.read_text(encoding="utf-8"))
    calls = Counter(r["call"] for r in real["rows"])
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(r"mris_calc (\S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {
                    "case": m[1],
                    "mode": m[2],
                    "larmorx_mib": float(m[3]),
                    "freesurfer_mib": float(m[4]),
                }
            )
    payload = {
        "name": NAME,
        "tool": "mris_calc (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mris_calc/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mris_calc) and "
        "its recon-all runs' files (oracles/freesurfer-8.2.0/runs/)",
        "real": {
            "runs": real["runs"],
            "calls": real["calls"],
            "kinds": dict(calls),
            "verdicts": real["verdicts"],
            "recorded": real.get("recorded", {}),
            "seconds": real["seconds"],
        },
        "program": [
            {"seed": p["seed"], "cases": p["cases"], "verdicts": p["verdicts"]} for p in progs
        ],
        "golden_sha256": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    rv = real["verdicts"]
    rec = real.get("recorded", {})
    lines = [
        "# Validation: FreeSurfer's `mris_calc` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-calc/record.py` (queue item 3.2-21; the "
        "whole program, its curvature files included, so queue item 3.5-37 adds only checks).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mris_calc ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The real calls on the recorded heads",
        "",
        "`mris_calc` is no step of its own in the reference runs (`mri_nu_correct.mni`, recon-all's "
        "vertex areas and `label-cortex` run it; their recorded steps check it when those "
        "scripts are ported). `validation/probes/fs-calc/real.py` runs their calls on the "
        f"recorded files of {len(real['runs'])} runs (3 heads, both streams) with the official "
        "program and the port, comparing exit status, messages and every file written "
        "(FreeSurfer's provenance aside), one thread:",
        "",
        f"- {real['calls']} calls ({len(calls)} kinds: `mri_nu_correct.mni`'s rescale with each "
        "run's recorded ratio on a copy of its `orig.mgz` (N3's output is not kept), the vertex "
        "areas of each hemisphere (`add`, then `div 2` in place), `label-cortex`'s mask product "
        "on a surface overlay).",
        f"- `reproduce`: **{rv.get('identical', 0)} identical**, {rv.get('different', 0)} "
        f"different, {rv.get('reproduce-skip', 0)} counted; the vertex areas equal the runs' "
        f"own `?h.area.mid` in {rec.get('reproduce-identical', 0)} of "
        f"{sum(v for k, v in rec.items() if k.startswith('reproduce'))} (and in "
        f"{rec.get('fix-identical', 0)} with the default).",
        f"- `fix`: {rv.get('fix-identical', 0)} identical, {rv.get('fix-explained', 0)} different "
        f"with a notice, {rv.get('fix-unexplained', 0)} without.",
        f"- Time (the calls not served from the cache): official "
        f"{real['seconds'].get('official', 0):.1f} s (those run), port "
        f"{real['seconds'].get('reproduce', 0):.1f} s (`reproduce`).",
        "",
        "## The whole program against the official binary",
        "",
        "`validation/probes/fs-calc/prog.py` (the open probe's command lines, "
        "`validation/investigations/freesurfer-mris-calc-probe/`): every option, every action "
        "on curvature files with NaN, infinite and denormal values, on numbers and on volumes, "
        "the statistics, the output's names, labels, the failures; exit status, standard output "
        "and error, and every file written compared:",
        "",
    ]
    for p in payload["program"]:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(FreeSurfer reads memory it never wrote or writes outside its mask: FS-167, "
            f"FS-1124, FS-1125); `fix` {v.get('fix-identical', 0)} identical, "
            f"{v.get('fix-explained', 0)} different with a notice, "
            f"{v.get('fix-unexplained', 0)} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mris_calc_golden.rs`: ten cases on "
        "32^3 volumes and 1,000-value curvature files made with integer arithmetic (the rescale, "
        "a square root of negative values: SSE's default NaN, a volume over another with zeros, "
        "NaN operands added, `atan2`, `bcor`, the statistics, `norm` in a label, `ascii`, "
        f"`andbw`), on every platform: `reproduce` `{a.golden}`, the official program's value on "
        f"the same inputs (`prog.py --golden`); the default `{a.golden_fix}` (the correctly "
        "rounded `atan2f`, `pow` and `log10`, GLIBC-07; FS-17's `andbw`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mris_calc_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above "
        "FreeSurfer's peak plus 8 MiB):",
        "",
    ]
    for m in memory:
        lines.append(
            f"- {m['case']} ({m['mode']}): {m['larmorx_mib']:.0f} MiB, FreeSurfer "
            f"{m['freesurfer_mib']:.0f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {a.out / NAME}.json and .md from {stamp}")
    return 1 if stamp.endswith("-dirty") else 0


if __name__ == "__main__":
    raise SystemExit(main())
