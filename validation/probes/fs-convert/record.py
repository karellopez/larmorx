# SPDX-License-Identifier: Apache-2.0
"""mri_convert's validation record from the replica's runs: ``docs/validation/freesurfer-mri-convert.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's recorded calls (``freesurfer-replay run ... --json``, one file per
run and mode), the probe's summaries (``check.py --json``: the command line, whole conversions and
outline series), fMRIPrep's call (``fmriprep_call.py --json``), the golden value and the memory
figures of ``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the
commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-convert/record.py --replays <dir> --probe <a.json> <b.json> ... \\
        --fmriprep <call.json> --memory <memory.log> --golden <sha>
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
NAME = "freesurfer-mri-convert"
KINDS = {
    "opt": "the command line (options, checks, the parse-only report; stats tables, CSV)",
    "cnv": "whole conversions (small MGH inputs, one to three option groups, five output formats)",
    "otl": "outline (.otl) series (every path of the reader, then seeded series)",
}


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replays", type=Path, required=True, help="<dir>/replay-<mode>/<run>.json")
    ap.add_argument("--modes", default="reproduce,fix")
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--fmriprep", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    steps = {}
    for mode in a.modes.split(","):
        rows = []
        for f in sorted((a.replays / f"replay-{mode}").glob("*.json")):
            rows.extend(json.loads(f.read_text(encoding="utf-8")))
        steps[mode] = rows
    probes = [json.loads(p.read_text(encoding="utf-8")) for p in a.probe]
    call = json.loads(a.fmriprep.read_text(encoding="utf-8"))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(r"mri_convert (\w+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_convert (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_convert/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/), its "
        "official program ($FREESURFER_HOME/bin/mri_convert) and the fs-convert probe "
        "(mri_convert.cpp compiled against the unit oracle's build)",
        "replay": {
            mode: {
                "steps": len(rows),
                "reproduced": sum(r["verdict"] == "reproduced" for r in rows),
                "runs": sorted({r["run"] for r in rows}),
                "seconds_replayed": round(sum(r["seconds"]["replayed"] for r in rows), 1),
                "seconds_recorded": round(sum(r["seconds"]["recorded"] for r in rows), 1),
            }
            for mode, rows in steps.items()
        },
        "probe": [
            {
                "kind": p["kind"],
                "seed": p["seed"],
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": sum(p["reproduce_differ"].values()),
                "skipped": p["skipped_total"],
                "fix_different_with_notice": sum(p["fix_differ"].values())
                - sum(p["fix_differ_without_notice"].values()),
                "fix_different_without_notice": sum(p["fix_differ_without_notice"].values()),
            }
            for p in probes
        ],
        "fmriprep_call": call,
        "golden_sha256": a.golden,
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_convert` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-convert/record.py` (queue items 3.2-04 to "
        "3.2-07).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_convert ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## recon-all's calls, replayed",
        "",
        "Every recorded `mri_convert` step of the 12 runs (6 heads, the classic and v8 streams: "
        "the input image to `orig/001.mgz`, and `rawavg.mgz` to `orig.mgz --conform`), rebuilt "
        "and run by the replay tool (`docs/validation/freesurfer-replay.md`) at one thread:",
        "",
    ]
    for mode, v in payload["replay"].items():
        lines.append(
            f"- `{mode}`: **{v['reproduced']} of {v['steps']} steps reproduced** (the output "
            "identical to the recorded one, FreeSurfer's provenance aside, and the exit status); "
            f"{v['seconds_replayed']:.0f} s for all ({v['seconds_recorded']:.0f} s recorded, "
            "FreeSurfer at four threads)."
        )
    m = call["modes"]
    lines += [
        "",
        "## fMRIPrep's call",
        "",
        "fMRIPrep 25.2.6 runs `mri_convert` once, as its FreeSurfer licence check "
        "(`mri_convert <niworkflows' sentinel.nii.gz> out.mgz`): `validation/probes/fs-convert/"
        "fmriprep_call.py` runs it with the official program and the port: "
        + "; ".join(
            f"`{k}`: exit status {'the same' if v['status_same'] else 'different'}, standard "
            f"output {'the same' if v['stdout_same'] else 'different'}, standard error "
            f"{'the same' if v['stderr_same'] else 'different'}, the volume {v['volume']}"
            for k, v in m.items()
        )
        + ". (The port needs no licence file; the official program stops without one.)",
        "",
        "## The whole program against FreeSurfer's code",
        "",
        "`validation/probes/fs-convert/check.py` (`fsconvert`: `mri_convert.cpp`'s `main` "
        "compiled against the unit oracle's build, each case in a child process): exit status, "
        "standard output and error in their order, and every file written compared (a "
        "compressed file decompressed, the command-line records' time stamps masked); counted "
        "apart: the cases where FreeSurfer reads memory it does not own or crashes (the README's "
        "`SKIP` lists):",
        "",
    ]
    for p in payload["probe"]:
        lines.append(
            f"- {KINDS.get(p['kind'], p['kind'])}, seed {p['seed']}: {p['cases']} cases; "
            f"`reproduce` {p['identical']} identical, {p['different']} different, "
            f"{p['skipped']} counted; `fix` {p['fix_different_with_notice']} different with a "
            f"notice, {p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_convert_golden.rs`: six cases on "
        "a 32^3 input made with integer arithmetic (a conversion to NIfTI, a type change with "
        "scaling, the conformed volume cut to 32^3, a 2 mm resampling, a linear transform, an "
        "outline series), on every platform, both modes: "
        f"`{a.golden}`, the official program's value on the same inputs (`golden.py`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_convert_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above "
        "FreeSurfer's peak plus 8 MiB):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): {e['larmorx_mib']:.0f} MiB, FreeSurfer "
            f"{e['freesurfer_mib']:.0f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{NAME}: {stamp}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
