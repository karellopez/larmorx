# SPDX-License-Identifier: Apache-2.0
"""mri_cc's validation record from the replica's runs: ``docs/validation/freesurfer-mri-cc.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's ``seg2cc`` steps (the text output of
``validation/investigations/freesurfer-mri-cc-replay/replay.sh``, one file per mode), the open
probe's summaries (``validation/investigations/freesurfer-mri-cc-probe/compare.py --json``, one
per seed, the first with ``--heads``), the white-matter path on the recorded heads
(``real.py --json``), the memory figures of ``tests/parity/test_memory_peak.py`` (its printed
lines) and the golden values, and writes the record with the commit it was made from (a clean
one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-cc/record.py --replay-reproduce <out> --replay-fix <out> \\
        --probe <seed1.json> <seed2.json> --real <real.json> --memory <memory.log> \\
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
NAME = "freesurfer-mri-cc"
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
    """The ``seg2cc`` steps of one mode's replay output: verdicts and times."""
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
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probes = [probe(json.loads(p.read_text(encoding="utf-8"))) for p in a.probe]
    rep = {"reproduce": replays(a.replay_reproduce), "fix": replays(a.replay_fix)}
    real = json.loads(a.real.read_text(encoding="utf-8"))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"(mri_cc(?: -a 0)? \S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
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
        "tool": "mri_cc (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_cc/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_cc) and "
        "its recon-all runs' recorded seg2cc steps (oracles/freesurfer-8.2.0/runs/)",
        "replay": rep,
        "real": {
            "runs": real["runs"],
            "calls": real["calls"],
            "kinds": dict(Counter(r["call"] for r in real["rows"])),
            "verdicts": real["verdicts"],
        },
        "program": probes,
        "golden_sha256": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_cc` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-cc/record.py` (queue item 3.2-27; the "
        "whole program: the aseg path recon-all runs and the white-matter path, `-a 0`).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_cc ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded steps",
        "",
        "recon-all's `seg2cc` (both streams: `mri_cc -aseg aseg.auto_noCCseg.mgz -o aseg.auto.mgz "
        "-lta transforms/cc_up.lta <subject>`) replayed with FreeSurfer's own script and the port "
        "as its `mri_cc` (`validation/investigations/freesurfer-mri-cc-replay/`; every file "
        "compared, FreeSurfer's provenance and the log's dates, times and memory figures aside):",
        "",
    ]
    for mode, r in rep.items():
        v = r["verdicts"]
        lines.append(
            f"- `{mode}`: {r['steps']} steps of {len(r['runs'])} runs, **{v.get('REPRODUCED', 0)} "
            f"reproduced**, {r['steps'] - v.get('REPRODUCED', 0)} not; the whole `seg2cc` step at "
            f"one thread {r['seconds']['replayed']:.0f} s (recorded {r['seconds']['recorded']:.0f} "
            "s)."
        )
    rv = real["verdicts"]
    lines += [
        "",
        "## The white-matter path on the recorded heads",
        "",
        "`validation/probes/fs-cc/real.py`: each run's `wm.mgz` written as a COR folder `wm` by "
        "the official `mri_convert -ot cor`, its `talairach.xfm` beside it; `-a 0` and `-a 0 -d 7 "
        "-t 3`; the official program against the port, messages and `cc_org.mgz` compared: "
        f"{real['calls']} calls on {len(real['runs'])} runs, `reproduce` **{rv.get('identical', 0)} "
        f"identical**, {rv.get('different', 0)} different; `fix` {rv.get('fix-identical', 0)} "
        f"identical, {rv.get('fix-explained', 0)} different with a notice (FS-1210: the seed search "
        f"finds no slice), {rv.get('fix-unexplained', 0)} without.",
        "",
        "## The whole program against the official binary (the open probe)",
        "",
        "`validation/investigations/freesurfer-mri-cc-probe/compare.py` with the replica as the "
        "candidate: every option, given last, by its first letter, its value forms, names of 1,023 "
        "and 1,024 bytes, the subjects folder from `-sdir` and the environment, missing inputs, "
        "read-only output folders; the aseg path on small segmentations and on a conformed phantom "
        "head with each option; the white-matter path on small volumes (NaNs, an unidentified "
        "`wm`, voxels of 0.45 mm) and on the phantom (a NIfTI file, a COR folder, voxels of 1.4 "
        "mm); exit status, standard output and error and every file written compared:",
        "",
    ]
    for p in probes:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(FreeSurfer reads memory past the volume or its tables: FS-53, FS-1201, FS-1206); "
            f"`fix` {v.get('fix-identical', 0)} identical, {v.get('fix-explained', 0)} different "
            f"with a notice, {v.get('fix-unexplained', 0)} without."
        )
        if p["heads"]:
            h = p["heads_verdicts"]
            lines.append(
                f"  With `--heads`, recon-all's call and the white-matter path on the six "
                f"recorded heads' two streams ({p['heads']} calls): `reproduce` "
                f"{h.get('identical', 0)} identical, {h.get('different', 0)} different; `fix` "
                f"{h.get('fix-identical', 0)} identical, {h.get('fix-explained', 0)} with a "
                f"notice, {h.get('fix-unexplained', 0)} without."
            )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_cc_golden.rs`: twelve cases on "
        "32^3 subjects made with integer arithmetic (the white-matter path with other divisions "
        "and thickness and with NaNs kept, without `wm` (FS-1205) and without the transform, more "
        "than 50 divisions (FS-1211), the aseg path with `-m 1`, the callosum present, `-lh`, a "
        "name of 1,024 bytes (FS-1212), no subjects folder, a norm that does not read), on every "
        f"platform: `reproduce` `{a.golden}`, the official program's value on the same inputs "
        f"(`prog.py --golden`); the default `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py` (`MRI_CC_CASES`: recon-all's `seg2cc` call; "
        "`MRI_CC_WM_CASES`: `-a 0`; ds000005 sub-01's v8 files, one thread; the rule: at most 10 % "
        "above FreeSurfer's peak plus 8 MiB):",
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
