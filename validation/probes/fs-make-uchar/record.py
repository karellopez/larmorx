# SPDX-License-Identifier: Apache-2.0
"""mri_make_uchar's validation record from the replica's runs: ``docs/validation/freesurfer-mri-make-uchar.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the whole-program probe's summaries (``prog.py --json``, one per seed), the real calls on
the recorded heads (``real.py --json``), the memory figures of ``tests/parity/test_memory_peak.py``
(its printed lines) and the golden values, and writes the record with the commit it was made from
(a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-make-uchar/record.py --prog <prog1.json> <prog2.json> \\
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
NAME = "freesurfer-mri-make-uchar"


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
        pat = re.compile(
            r"mri_make_uchar (\S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
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
        "tool": "mri_make_uchar (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_make_uchar/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_make_uchar) and "
        "its recon-all runs' files (oracles/freesurfer-8.2.0/runs/)",
        "real": {
            "runs": real["runs"],
            "calls": real["calls"],
            "kinds": dict(calls),
            "verdicts": real["verdicts"],
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
    lines = [
        "# Validation: FreeSurfer's `mri_make_uchar` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-make-uchar/record.py` (queue item 3.2-22; "
        "the whole program).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_make_uchar ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The real calls on the recorded heads",
        "",
        "`mri_make_uchar` is no step of its own in the reference runs: `mri_nu_correct.mni` runs "
        "it last, in place, on its rescaled N3 output, which the runs do not keep (the step's "
        "recorded `nu.mgz` is its output). `validation/probes/fs-make-uchar/real.py` runs the "
        f"call on the recorded files of {len(real['runs'])} runs (3 heads, both streams) with "
        "stand-ins, with the official program and the port, comparing exit status, messages "
        "and every file written (FreeSurfer's provenance aside), one thread:",
        "",
        f"- {real['calls']} calls ({len(calls)} kinds: the call itself in place on the run's "
        "`orig.mgz` times its recorded rescale ratio as `float`, with the run's "
        "`transforms/talairach.xfm`; the run's `orig.mgz` (`uchar`); the run's `T1.mgz` with the "
        "histogram and ball files and a 60 mm ball).",
        f"- `reproduce`: **{rv.get('identical', 0)} identical**, {rv.get('different', 0)} "
        "different.",
        f"- `fix`: {rv.get('fix-identical', 0)} identical, {rv.get('fix-explained', 0)} different "
        f"with a notice, {rv.get('fix-unexplained', 0)} without.",
        f"- Time (the calls not served from the cache): official "
        f"{real['seconds'].get('official', 0):.1f} s (those run), port "
        f"{real['seconds'].get('reproduce', 0):.1f} s (`reproduce`).",
        "",
        "## The whole program against the official binary",
        "",
        "`validation/probes/fs-make-uchar/prog.py` (the open probe's command lines, "
        "`validation/investigations/freesurfer-mri-make-uchar-probe/`): every option and its "
        "values (numbers that do not scan, NaN, infinities, hexadecimal), the histogram and ball "
        "files, the output's names and formats, inputs of every type, three frames, NaNs and "
        "infinities, constant and empty volumes, odd sizes, one voxel, NIfTI, axial and "
        "sagittal volumes, transforms near the identity, far away, with a NaN, cut short, "
        "linear transform arrays, the failures; exit status, standard output and error, and "
        "every file written compared:",
        "",
    ]
    for p in payload["program"]:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(FreeSurfer reads memory it never wrote: a volume named as the transform, FS-308, "
            f"FS-504); `fix` {v.get('fix-identical', 0)} identical, "
            f"{v.get('fix-explained', 0)} different with a notice, "
            f"{v.get('fix-unexplained', 0)} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_make_uchar_golden.rs`: seven "
        "cases on 32^3 coronal volumes and an 8^3 axial one made with integer arithmetic "
        "(`mri_nu_correct.mni`'s call on a `float` volume, the histogram and ball files, a "
        "`uchar` volume, an empty ball: a NaN mapping, a volume with NaNs and infinities, the "
        f"axial volume's error per voxel, no output name), on every platform: `reproduce` "
        f"`{a.golden}`, the official program's value on the same inputs (`prog.py --golden`); "
        f"the default `{a.golden_fix}` (FS-1127, FS-1130).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_make_uchar_replica_needs_no_more_memory_"
        "than_freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above "
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
