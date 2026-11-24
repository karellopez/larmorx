# SPDX-License-Identifier: Apache-2.0
"""mri_segment's validation record from the replica's runs: ``docs/validation/freesurfer-mri-segment.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's ``mri_segment`` steps (the replay tool's text output, one file per
mode: ``freesurfer-replay run <run> mri_segment`` for each recorded run), the open probe's
summaries (``validation/investigations/freesurfer-mri-segment-probe/compare.py --json``, one per
seed; the heads' cases with ``--heads`` in one of them or a file of their own), the memory figures
of ``tests/parity/test_memory_peak.py`` (its printed lines) and the golden values, and writes the
record with the commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-segment/record.py --replay-reproduce <out> --replay-fix <out> \\
        --probe <seed1.json> <seed2.json> <heads.json> --memory <memory.log> \\
        --golden <sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-segment"
STEP = re.compile(
    r"^(\S+) (\S+) \((\w+), (\d+) thread\): (\w+); exit (\d+) recorded, (\d+) replayed; "
    r"([\d.]+) s recorded, ([\d.]+) s now$",
    re.M,
)


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


def replays(path: Path) -> dict:
    """The ``mri_segment`` steps of one mode's replay output: verdicts and times."""
    rows = [m.groups() for m in STEP.finditer(path.read_text(encoding="utf-8"))]
    return {
        "runs": sorted({r[0] for r in rows}),
        "steps": len(rows),
        "verdicts": dict(Counter(r[4] for r in rows)),
        "seconds": {
            "recorded": round(sum(float(r[7]) for r in rows), 1),
            "replayed": round(sum(float(r[8]) for r in rows), 1),
        },
    }


def probe(p: dict) -> dict:
    synth = [r for r in p["rows"] if r["case"] < 1000]
    heads = [r for r in p["rows"] if r["case"] >= 1000]

    def count(rows: list) -> dict:
        return dict(Counter(v for r in rows for v in r["verdict"].values()))

    return {
        "seed": p["seed"],
        "cases": len(synth),
        "verdicts": count(synth),
        "heads": len(heads),
        "heads_verdicts": count(heads),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replay-reproduce", type=Path, required=True)
    ap.add_argument("--replay-fix", type=Path, required=True)
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probes = [probe(json.loads(p.read_text(encoding="utf-8"))) for p in a.probe]
    rep = {"reproduce": replays(a.replay_reproduce), "fix": replays(a.replay_fix)}
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(r"(mri_segment \S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
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
        "tool": "mri_segment (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_segment/) on "
        "larmorx-freesurfer-filter (src/polv.rs, classify.rs, nonpar.rs), the larmorx-freesurfer "
        "program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_segment) and "
        "its recon-all runs' recorded mri_segment steps (oracles/freesurfer-8.2.0/runs/)",
        "replay": rep,
        "program": probes,
        "golden_sha256": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_segment` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-segment/record.py` (queue item 3.2-31; the "
        "whole program).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_segment ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded steps",
        "",
        "recon-all's call (both streams: `mri_segment -wsizemm 13 -mprage antsdn.brain.mgz "
        "wm.seg.mgz`) replayed by the replay tool on each recorded run (`wm.seg.mgz` and "
        "`segment.dat` compared, FreeSurfer's provenance aside):",
        "",
    ]
    for mode, r in rep.items():
        v = r["verdicts"]
        lines.append(
            f"- `{mode}`: {r['steps']} steps of {len(r['runs'])} runs, **{v.get('REPRODUCED', 0)} "
            f"reproduced**, {r['steps'] - v.get('REPRODUCED', 0)} not; at one thread "
            f"{r['seconds']['replayed']:.0f} s in all (recorded {r['seconds']['recorded']:.0f} s)."
        )
    lines += [
        "",
        "## The whole program against the official binary (the open probe)",
        "",
        "`validation/investigations/freesurfer-mri-segment-probe/compare.py` with the replica as "
        "the candidate: every option, given last, by its first letter, its value forms; inputs of "
        "bytes, `float` with NaNs and infinities, shorts, another extent, not coronal, all 0 and "
        "constant; masks and segmentations of the input's extent and of others, a segmentation "
        "that loses its white matter, `-keep`, unwritable outputs; exit status, standard output "
        "and error (the time masked) and every file written compared:",
        "",
    ]
    for p in probes:
        v = p["verdicts"]
        if p["cases"]:
            lines.append(
                f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
                f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} "
                "counted (FreeSurfer reads or writes memory it does not own or never wrote: "
                "FS-498, FS-1216 to FS-1218, FS-1222, FS-1223, FS-1228, FS-1231); `fix` "
                f"{v.get('fix-identical', 0)} identical, {v.get('fix-explained', 0)} different "
                f"with a notice, {v.get('fix-unexplained', 0)} without."
            )
        if p["heads"]:
            h = p["heads_verdicts"]
            lines.append(
                f"- With `--heads`, five options recon-all does not give (`-noauto`, `-seg`, "
                f"`-fillbg -fillv`, `-thicken 0`, `-keep`) on two recorded heads ({p['heads']} "
                f"calls): `reproduce` {h.get('identical', 0)} identical, "
                f"{h.get('different', 0)} different; `fix` {h.get('fix-identical', 0)} "
                f"identical, {h.get('fix-explained', 0)} with a notice, "
                f"{h.get('fix-unexplained', 0)} without."
            )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_segment_golden.rs`: eleven cases "
        "on 32^3 volumes made with integer arithmetic (recon-all's call, the defaults, a `float` "
        "input with NaNs and infinities by their bits, `-noauto -thicken 0`, `-seg`, `-fillbg "
        "-fillv` (FS-1227), `-thickenonly`, `-keep`, `-b 0.5 -polvlen 2` (FS-1220), an output "
        "into a missing folder (FS-1226), an option without its value (FS-1219)), on every "
        f"platform: `reproduce` `{a.golden}`, the official program's value on the same inputs "
        f"(`prog.py --golden`); the default `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py` (`MRI_SEGMENT_CASES`: recon-all's call and `-seg`'s "
        "path on ds000005 sub-01's classic files, one thread; the rule: at most 10 % above "
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
