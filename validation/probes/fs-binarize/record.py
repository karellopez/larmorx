# SPDX-License-Identifier: Apache-2.0
"""mri_binarize's validation record from the replica's runs: ``docs/validation/freesurfer-mri-binarize.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the whole-program probe's summaries (``prog.py --json``, one per seed), the real calls on
the recorded heads (``real.py --json``), the memory figures of ``tests/parity/test_memory_peak.py``
(its printed lines) and the golden values, and writes the record with the commit it was made from
(a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-binarize/record.py --prog <prog1.json> <prog2.json> \\
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
NAME = "freesurfer-mri-binarize"


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
        pat = re.compile(r"mri_binarize (\S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
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
        "tool": "mri_binarize (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_binarize/) and "
        "larmorx-freesurfer-deform (src/tess.rs), the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_binarize) and "
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
        "# Validation: FreeSurfer's `mri_binarize` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-binarize/record.py` (queue item 3.2-19).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_binarize ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The real calls on the recorded heads",
        "",
        "`mri_binarize` is no step of its own in the reference runs (recon-all, "
        "`mri_nu_correct.mni`, `mri_mcadura_seg`, `mri_vsinus_seg`, `label-cortex` and `seg2cc` "
        "run it; their recorded steps check it when those scripts are ported). "
        "`validation/probes/fs-binarize/real.py` runs their calls on the recorded files of "
        f"{len(real['runs'])} runs (3 heads, both streams) with the official program and the "
        "port, comparing exit status, messages and every file written (FreeSurfer's provenance "
        "aside), one thread:",
        "",
        f"- {real['calls']} calls ({len(calls)} kinds: the white matter's mask of the segmentation, "
        "the counts of `wm.mgz`'s edits, the background in place, `mri_nu_correct.mni`'s ones "
        "and mask, the cortex and its inverse, the crop around the centre, the corpus "
        "callosum, `label-cortex`'s entorhinal labels and the white matter of the v8 stream's "
        "segmentation, a hippocampus's surface and its topology fix).",
        f"- `reproduce`: **{rv.get('identical', 0)} identical**, {rv.get('different', 0)} "
        f"different, {rv.get('reproduce-skip', 0)} counted (the topology fix of a hippocampus "
        "that is already correct: FreeSurfer writes a volume it has freed, FS-1021).",
        f"- `fix`: {rv.get('fix-identical', 0)} identical, {rv.get('fix-explained', 0)} different "
        f"with a notice (FS-1022), {rv.get('fix-unexplained', 0)} without.",
        f"- Time (the calls not served from the cache): official {real['seconds'].get('official', 0):.0f} s "
        f"(those run), port {real['seconds'].get('reproduce', 0):.0f} s (`reproduce`).",
        "",
        "## The whole program against the official binary",
        "",
        "`validation/probes/fs-binarize/prog.py` (the open probe's command lines, "
        "`validation/investigations/freesurfer-mri-binarize-probe/`): every option, the three "
        "stand-alone options, the failures; exit status, standard output and error, and every "
        "file written compared:",
        "",
    ]
    for p in payload["program"]:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(FreeSurfer reads memory it does not own, writes past its lists, crashes there or "
            "runs out of address space: FS-53, FS-1020, FS-1021, FS-1023, FS-1025, FS-1026, "
            f"FS-1027); `fix` {v.get('fix-identical', 0)} identical, {v.get('fix-explained', 0)} "
            f"different with a notice, {v.get('fix-unexplained', 0)} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_binarize_golden.rs`: seven cases "
        "on 32^3 inputs made with integer arithmetic (a match with its count file, a threshold "
        "in a mask merged as `uchar`, replacements, a dilation, an erosion and the bounding box, a "
        "smoothed surface, the topology fix of a shape with a handle, the bounding box without a "
        "pad), on every platform: "
        f"`reproduce` `{a.golden}`, the official program's value on the same inputs (`prog.py "
        f"--golden`); the default `{a.golden_fix}` (the topology fix keeps its first voxel, "
        "FS-1022; the box without a pad keeps the mask's last voxels, FS-269).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_binarize_replica_needs_no_more_memory_than_"
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
