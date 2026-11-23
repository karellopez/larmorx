# SPDX-License-Identifier: Apache-2.0
"""mri_mask's validation record from the replica's runs: ``docs/validation/freesurfer-mri-mask.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of the recorded steps (``freesurfer-replay run ... --json``, one file per run
and mode), the whole-program probe's summaries (``prog.py --json``, one per seed), the calls on
recorded heads beyond the steps (``real.py --json``), the memory
figures of ``tests/parity/test_memory_peak.py`` (its printed lines) and the golden values, and
writes the record with the commit it was made from (a clean one: a record stamped ``-dirty`` is
redone).

    python -P validation/probes/fs-mask/record.py --replay-reproduce <dir> --replay-fix <dir> \\
        --prog <prog1.json> <prog2.json> --real <real.json> --memory <memory.log> --golden <sha> \\
        --golden-fix <sha>
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
NAME = "freesurfer-mri-mask"


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


def replays(folder: Path) -> dict:
    """The steps of every run's replay JSON in ``folder``: verdicts, threads, times."""
    rows = []
    for p in sorted(folder.glob("*.json")):
        rows += json.loads(p.read_text(encoding="utf-8"))
    return {
        "runs": sorted({r["run"] for r in rows}),
        "steps": len(rows),
        "verdicts": dict(Counter(r["verdict"] for r in rows)),
        "threads": sorted({r["threads"] for r in rows}),
        "seconds": {
            "recorded": round(sum(r["seconds"]["recorded"] for r in rows), 1),
            "replayed": round(sum(r["seconds"]["replayed"] for r in rows), 1),
        },
        "files": dict(Counter(f["verdict"] for r in rows for f in r["files"])),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replay-reproduce", type=Path, required=True)
    ap.add_argument("--replay-fix", type=Path, required=True)
    ap.add_argument("--prog", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    progs = [json.loads(p.read_text(encoding="utf-8")) for p in a.prog]
    rep = {"reproduce": replays(a.replay_reproduce), "fix": replays(a.replay_fix)}
    real = json.loads(a.real.read_text(encoding="utf-8"))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(r"mri_mask (\S+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
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
        "tool": "mri_mask (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_mask/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_mask) and "
        "its recon-all runs' recorded steps (oracles/freesurfer-8.2.0/runs/)",
        "replay": rep,
        "real": {
            "runs": real["runs"],
            "calls": real["calls"],
            "kinds": dict(Counter(r["call"] for r in real["rows"])),
            "verdicts": real["verdicts"],
        },
        "program": [
            {"seed": p["seed"], "cases": p["cases"], "verdicts": p["verdicts"]} for p in progs
        ],
        "golden_sha256": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_mask` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-mask/record.py` (queue item 3.2-26; "
        "the whole program).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_mask ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded steps",
        "",
        "recon-all's `mri_mask -T 5 brain.mgz brainmask.mgz brain.finalsurfs.mgz` (both streams) "
        "and the v8 stream's two `-oval 1 -invert` calls (its MCA-dura and venous-sinus masks) on "
        "the reference runs, replayed by `python -m larmorx_validation freesurfer-replay run <run> "
        "mri_mask mri_mask:2 mri_mask:3` with the port (every file compared, FreeSurfer's "
        "provenance aside):",
        "",
    ]
    for mode, r in rep.items():
        v = r["verdicts"]
        lines.append(
            f"- `{mode}`: {r['steps']} steps of {len(r['runs'])} runs, **{v.get('reproduced', 0)} "
            f"reproduced**, {r['steps'] - v.get('reproduced', 0)} not; files: "
            + ", ".join(f"{n} {k}" for k, n in sorted(r["files"].items()))
            + f"; {r['seconds']['replayed']:.0f} s at {r['threads']} thread(s) (recorded "
            f"{r['seconds']['recorded']:.0f} s)."
        )
    rv = real["verdicts"]
    lines += [
        "",
        "## Calls on the recorded heads beyond the steps",
        "",
        "`validation/probes/fs-mask/real.py`: in the classic runs `-xform` with the run's GCA "
        "morph (`talairach.m3z`: the run's `brainmask.mgz` onto its `norm.mgz` moved into the "
        "atlas by the official `mri_convert -at`; with and without `-T 5 -dilate 1`); in the v8 "
        "runs the two calls of recon-all's scripts whose inputs the runs keep (`T1.mgz` masked by "
        "`synthstrip.mgz`; `-bb 3` on `orig.mgz` into a NIfTI file); the official program against "
        f"the port, every file compared: {real['calls']} calls on {len(real['runs'])} runs, "
        f"`reproduce` **{rv.get('identical', 0)} identical**, {rv.get('different', 0)} different; "
        f"`fix` {rv.get('fix-identical', 0)} identical, {rv.get('fix-explained', 0)} different "
        f"with a notice, {rv.get('fix-unexplained', 0)} without.",
    ]
    lines += [
        "",
        "## The whole program against the official binary",
        "",
        "`validation/probes/fs-mask/prog.py` (the open probe's command lines, "
        "`validation/investigations/freesurfer-mri-mask-probe/`): every option, given last, its "
        "value forms; the inputs' types, NaNs, masks of every kind, geometries that differ, "
        "segmentations with and without a colour table, every way of `-BBEQ`, the crops, the "
        "transforms of every kind and their failures, the singular transforms of FS-1135 with each "
        "later option, the outputs' formats; exit status, standard output and error, and every "
        "file written compared:",
        "",
    ]
    for p in payload["program"]:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(a transform file FreeSurfer crashes on where both modes end with an error: FS-307, "
            f"FS-309, FS-310); `fix` {v.get('fix-identical', 0)} identical, "
            f"{v.get('fix-explained', 0)} different with a notice, "
            f"{v.get('fix-unexplained', 0)} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_mask_golden.rs`: fourteen cases "
        "on 32^3 inputs made with integer arithmetic and a 16 x 16 x 32 pair (recon-all's `-T 5`, "
        "`-invert`, a transform with a threshold and a dilation, `-transfer` with the deletion "
        "edits kept, `-BB`, `-BBEQ`, a crop to a field of view, a mask with NaNs and infinities "
        "with `-oval nan`, a NaN threshold, `-lh -no_cerebellum`, `-samseg`, an erosion with "
        "background noise, `-BBEQ` without a box, recon-all's spelling `-bb 0`), on every "
        "platform: `reproduce` "
        f"`{a.golden}`, the official program's value on the same inputs (`prog.py --golden`); the "
        f"default `{a.golden_fix}` (FS-01, FS-1133, FS-269).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_mask_replica_needs_no_more_memory_than_"
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
