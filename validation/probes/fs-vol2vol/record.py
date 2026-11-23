# SPDX-License-Identifier: Apache-2.0
"""mri_vol2vol's validation record from the replica's runs: ``docs/validation/freesurfer-mri-vol2vol.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded calls and fMRIPrep's on the
recorded heads (``real.py --json``), the golden values and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone). The program has no recorded
step of its own (the v8 stream's scripts run it), so ``real.py`` stands for the replays.

    python -P validation/probes/fs-vol2vol/record.py --probe <a.json> ... --real <real.json> \\
        --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-vol2vol"


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probes = [json.loads(p.read_text(encoding="utf-8")) for p in a.probe]
    real = {}
    if a.real and a.real.is_file():
        r = json.loads(a.real.read_text(encoding="utf-8"))
        real = {k: r[k] for k in ("calls", "reproduce_identical", "fix_identical")}
        real["runs"] = sorted({x["run"] for x in r["rows"]})
        real["kinds"] = dict(Counter(re.sub(r"-(lh|rh)$", "", x["call"]) for x in r["rows"]))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_vol2vol ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_vol2vol (FreeSurfer 8.2.0), part 1: the linear paths",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_vol2vol/), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_vol2vol) and "
        "its recon-all runs (oracles/freesurfer-8.2.0/runs/: the v8 stream's scripts' calls)",
        "probe": [
            {
                "set": f"seed {p['seed']}" if p["seed"] is not None else "golden",
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "skipped_why": {
                    k: v for k, v in p["reproduce_notices"].items() if k in ("CRASH", "NOTPORTED")
                },
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "real": real,
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
        "not_ported_yet": "the morphs (--morph, --inv-morph, --m3z), --gcam, --spm-warp, "
        "--map-point, --disp, --cost: part 2 (relay 54)",
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_vol2vol` (replica, part 1)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-vol2vol/record.py` (queue item 3.2-38, "
        "part 1: every option's reading and checks and every linear path; "
        f"{payload['not_ported_yet']}).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_vol2vol ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
    ]
    if real:
        kinds = ", ".join(f"{k} {n}" for k, n in sorted(real["kinds"].items()))
        lines += [
            "## The recorded calls and fMRIPrep's, on the recorded heads",
            "",
            "`validation/probes/fs-vol2vol/real.py`: the v8 stream's nine calls per head "
            "(`fs-synthmorph-reg`'s check, `mri_mcadura_seg`'s priors, crops and segmentations, "
            "`mri_vsinus_seg`'s prior and segmentation) on each head's own files (the scripts' "
            "temporary inputs made again by the official programs, or the head's volumes of the "
            "same kind), and fMRIPrep's `--mov aseg.mgz --targ <T1w> --lta <lta> --interp nearest` "
            f"({', '.join(real['runs'])}; by kind: {kinds}): {real['calls']} calls, `reproduce` "
            f"{real['reproduce_identical']} identical to the official program (exit status, both "
            f"streams, every file byte for byte), the default {real['fix_identical']}.",
            "",
        ]
    lines += [
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-vol2vol/check.py`: the command line (no argument, the version and "
        "help options anywhere, unknown words, every value-taking option without its values, the "
        "prefix and case rules), every check, every registration (`--regheader`, `--xfm`, "
        "`--reg` with a register.dat and an `.lta`, `--lta`, `--lta-inv`, `--fsl` with one and "
        "two matrices, `--s`, their failures), every method, `--inv`, every type and format, "
        "multi-frame and one-slice volumes, a segmentation with its colour table, `--precision`, "
        "`--keep-precision`, `--kernel`, `--delta`, the fills and `--downsample`, the voxel shift "
        "map, `--no-resample` and the slice operations, `--mul`, `--save-reg`, `--reg-final`, "
        "`--rot`, `--trans`, `--shear`, `--seed`, `--fstarg`, `--tal`, `--mni152reg`, `--crop`, "
        "`--soap`; then seeded command lines. Each case in one folder with its own FreeSurfer, "
        "subjects and FSL folders; exit status, both streams and every file written, case by "
        "case; counted apart: FreeSurfer's crashes and the paths of part 2:",
        "",
    ]
    for p in payload["probe"]:
        why = ", ".join(f"{k} {n}" for k, n in sorted(p["skipped_why"].items()))
        lines.append(
            f"- {p['set']}: {p['cases']} cases; `reproduce` {p['identical']} identical, "
            f"{p['different']} different, {p['skipped']} counted ({why}); `fix` "
            f"{p['fix_different_with_notice']} different with a notice, "
            f"{p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_vol2vol_golden.rs`: 8 cases on "
        "volumes up to 32^3 (the v8 scripts' four forms, fMRIPrep's, the voxel-to-RAS rewrite "
        "with `--save-reg` from a register.dat, a moving volume with NaN voxels by trilinear and "
        f"cubic resampling), on every platform: `reproduce` `{a.golden}` (the official program's "
        f"value on the same inputs, `check.py --golden`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_vol2vol_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's v8 volumes read in place, one thread; the rule: at most "
        "10 % above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): {e['larmorx_mib']:.1f} MiB, FreeSurfer "
            f"{e['freesurfer_mib']:.1f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{NAME}: {stamp}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
