# SPDX-License-Identifier: Apache-2.0
"""mri_label2vol's validation record from the replica's runs: ``docs/validation/freesurfer-mri-label2vol.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded calls on the recorded heads
(``real.py --json``), the golden values and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone). The program has no recorded
step of its own (``defect2seg`` runs it in both streams, ``seg2cc`` for heads that are not
conformed), so ``real.py`` stands for the replays.

    python -P validation/probes/fs-label2vol/record.py --probe <a.json> ... --real <real.json> \\
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
NAME = "freesurfer-mri-label2vol"


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
        real = {
            k: r[k]
            for k in ("calls", "reproduce_identical", "fix_identical",
                      "official_image_is_recorded", "with_a_record")
        }  # fmt: skip
        real["runs"] = sorted({x["run"] for x in r["rows"]})
        real["kinds"] = dict(Counter(x["call"] for x in r["rows"]))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_label2vol ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_label2vol (FreeSurfer 8.2.0): every option and path but --new-aseg2vol and --fill-ribbon",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_label2vol/), fs.surf (defects2seg.rs), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_label2vol) "
        "and its recon-all runs (oracles/freesurfer-8.2.0/runs/: defect2seg's calls)",
        "probe": [
            {
                "set": f"seed {p['seed']}" if p["seed"] is not None else "golden",
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "skipped_why": {
                    k: v
                    for k, v in p["reproduce_notices"].items()
                    if k in ("CRASH", "MEMORY", "HANG")
                },
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "real": real,
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
        "not_ported_yet": "--new-aseg2vol (MRIseg2SegPVF) and --fill-ribbon (MRIsurf2VolOpt): "
        "part 2 of queue item 3.2-40; both end with a message, status 1",
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_label2vol` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-label2vol/record.py` (queue item 3.2-40 "
        "part 1, track A relay 56: every option, message, exit status and failure case but "
        "`--new-aseg2vol` and `--fill-ribbon`, which end with a message, status 1: part 2).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_label2vol ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
    ]
    if real:
        kinds = ", ".join(f"{k} {n}" for k, n in sorted(real["kinds"].items()))
        lines += [
            "## The recorded calls, on the recorded heads",
            "",
            "`validation/probes/fs-label2vol/real.py`: `defect2seg`'s two calls per run (the left "
            "hemisphere's defects into `orig.mgz` with the cortex label, the right's merged into "
            "the left's volume) on the six classic and six v8 runs, and, on the v8 runs, `seg2cc`'s form (`--seg "
            "<seg> --temp <vol> --regheader`: the head's `aseg.mgz` onto its native grid "
            "`rawavg.mgz` and back onto `norm.mgz`), each run's own files "
            f"({', '.join(real['runs'])}; by kind: {kinds}): {real['calls']} calls, `reproduce` "
            f"{real['reproduce_identical']} identical to the official program (exit status, both "
            f"streams, every file byte for byte, the command lines a volume records masked), the "
            f"default {real['fix_identical']}; the official defect volume's header and voxels equal "
            f"to the run's recorded `mri/surface.defects.mgz` in {real['official_image_is_recorded']} "
            f"of {real['with_a_record']} (its colour table's random colours left out: recon-all "
            "seeds them from the clock, the probe with `FREESURFER_SEED=1234`).",
            "",
        ]
    lines += [
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-label2vol/check.py`: the command line (no argument, the version "
        "and help options, unknown words, every value-taking option without its value, the case "
        "rules, `--debug`), every check and refused combination, templates of four voxel types "
        "and five grids (one of a NaN voxel size), registrations by `register.dat`, `.lta`, "
        "headers, the identity, inverted and singular; volume labels (NaN, infinite and huge "
        "coordinates, scanner coordinates with and without `--tkr-template`, an empty and a "
        "broken label), several labels with `--hits`, `--fillthresh`, `--labvoxvol`, "
        "`--offset` (to the int range's end), `--label-stat`, `--stat-thresh`, `--pvf`; surface "
        "labels projected along the normals (`abs`, `frac`, `none`, an infinite stop, NaN, a "
        "step of 2e-5 that passes 32,767 hits in one voxel, points beyond the surface); an "
        "annotation with its colour table (offsets, hits, a projection, no labels, a short "
        "file); segmentations of three grids by headers, a registration or the identity, with "
        "`--fillthresh`, `--upsample`, `--ttype`, `--ttype+head`, `--hits`, `--pvf`, "
        "`--aparc+aseg`; `--defects` into templates of three types (merged and not, a label, "
        "no volume geometry, NaN and infinite defect numbers, negative ones, numbers near the "
        "int range, a short overlay, offsets and merge flags that are no numbers, a negative "
        "offset, missing files); then seeded command lines. Each case in one folder; exit "
        "status, both streams and every file written, case by case; counted apart: FreeSurfer's "
        "crashes (CRASH: statuses 134, 136, 139), results it takes from memory contents (MEMORY: "
        "FS-53, FS-249, FS-1252, FS-1256, FS-1258, FS-1259) and a projection that never ends "
        "(HANG: FS-1257):",
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
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_label2vol_golden.rs`: 9 cases "
        "on volumes up to 12^3 and a surface of 66 vertices (`defect2seg`'s two calls, defect "
        "numbers with NaN and infinities from their bits, `seg2cc`'s segmentation onto another "
        "grid and upsampled with its hits, two labels with their hits, a label with NaN, "
        "infinite and huge coordinates through a `register.dat` with its stat volume, a "
        "projected surface label, an annotation), on every platform: "
        f"`reproduce` `{a.golden}` (the official program's value on the same inputs, `check.py "
        f"--golden`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_label2vol_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's v8 files read in place, one thread; the rule: at most 10 "
        "% above FreeSurfer's peak plus 8 MiB; the output identical, its command lines masked):",
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
