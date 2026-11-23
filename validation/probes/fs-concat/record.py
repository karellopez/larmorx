# SPDX-License-Identifier: Apache-2.0
"""mri_concat's validation record from the replica's runs: ``docs/validation/freesurfer-mri-concat.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded calls on the recorded heads
(``real.py --json``), the golden values and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone). The program has no recorded
step of its own (``pctsurfcon`` and ``mri_mcadura_seg`` run it), so ``real.py`` stands for the
replays.

    python -P validation/probes/fs-concat/record.py --probe <a.json> ... --real <real.json> \\
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
NAME = "freesurfer-mri-concat"


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
            for k in ("calls", "reproduce_identical", "fix_identical", "official_is_recorded",
                      "with_a_record")
        }  # fmt: skip
        real["runs"] = sorted({x["run"] for x in r["rows"]})
        real["kinds"] = dict(Counter(re.sub(r"-(lh|rh)$", "", x["call"]) for x in r["rows"]))
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_concat ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_concat (FreeSurfer 8.2.0): every option and path",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_concat/), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_concat) and "
        "its recon-all runs (oracles/freesurfer-8.2.0/runs/: pctsurfcon's and mri_mcadura_seg's "
        "calls)",
        "probe": [
            {
                "set": f"seed {p['seed']}" if p["seed"] is not None else "golden",
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "skipped_why": {
                    k: v for k, v in p["reproduce_notices"].items() if k in ("CRASH", "MEMORY")
                },
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "real": real,
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
        "not_ported_yet": "nothing",
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_concat` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-concat/record.py` (queue item 3.2-39, "
        "track A relay 55: every option, message, exit status and failure case).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_concat ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
    ]
    if real:
        kinds = ", ".join(f"{k} {n}" for k, n in sorted(real["kinds"].items()))
        lines += [
            "## The recorded calls, on the recorded heads",
            "",
            "`validation/probes/fs-concat/real.py`: `pctsurfcon`'s percent contrast per "
            "hemisphere (`<wm> <gm> --paired-diff-norm --mul 100`, its samples made again by the "
            "official `mri_vol2surf` with the run's own commands) on the six classic and six v8 "
            "runs, and `mri_mcadura_seg`'s `--sum --ctab mca-dura.ctab` of two segmentations (the "
            "head's `aseg.mgz` taken onto each hemisphere's crop and back by the official "
            "programs) on the v8 runs "
            f"({', '.join(real['runs'])}; by kind: {kinds}): {real['calls']} calls, `reproduce` "
            f"{real['reproduce_identical']} identical to the official program (exit status, both "
            f"streams, every file byte for byte), the default {real['fix_identical']}; the "
            f"official percent contrast equal to the run's recorded file in "
            f"{real['official_is_recorded']} of {real['with_a_record']} (the remade inputs are the "
            "run's).",
            "",
        ]
    lines += [
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-concat/check.py`: the command line (no argument, the version and "
        "help options, unknown words, every value-taking option without its value, the case "
        "rules, `--debug`), every refused combination, inputs of four voxel types, one to six "
        "frames, formats MGZ, MGH, NIfTI and gzipped NIfTI, segmentations with and without a "
        "colour table, zeros, NaN and infinite voxels, surface overlays, another grid, a cut "
        "file; `--no-check` with fitting and unfitting inputs; every reduction and option "
        "(`--combine`, `--prune`, the normalisations, the matrices `--mtx`, `--gmean`, `--asl`, "
        "the pairs, the means, median, sums, `--fnorm`, `--tar1`, `--std`, `--var`, `--max`, "
        "`--max-index` with `-prune` and `-add`, `--conjunct`, `--min`, `--sort`, `--vote`, "
        "`--first-non-zero`, `--mul`, `--div`, `--add`, `--cumsum`, `--scm`, `--pca`, `--rep`, "
        "`--rms`, `--keep-datatype`, `--ctab`, `--rusage`) with edge values, weights, masks of "
        "both sizes, `--in-frame`, `--zconcat` and `--f` lists; then seeded command lines. Each "
        "case in one folder; exit status, both streams and every file written, case by case; "
        "counted apart: FreeSurfer's crashes (CRASH: statuses 134, 136, 139, and FS-564's and "
        "FS-13's, where the port ends with an error in both modes) and results it takes from "
        "memory contents or leaves undefined (MEMORY: FS-53, FS-249, FS-1196, FS-1243, "
        "FS-1247):",
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
        "`validation/probes/fs-concat/long_names.sh` (paths too long for macOS's limit, run on "
        "Linux by hand): `--f` list names of 2,000 to 2,008 bytes run normally, 2,009 bytes and "
        "more abort when the command line has been read (FS-1197); lists of more than 400,000 "
        "words crash (FS-1198).",
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_concat_golden.rs`: 15 cases on "
        "volumes up to 32^3 (`pctsurfcon`'s and `mri_mcadura_seg`'s calls, a concatenation into "
        "NIfTI, the frame mean and standard deviation, weighted frames' maximum index, a matrix "
        "of frames, a masked vote, the first frame not zero, NaN made by `--mul +nan` and by NaN "
        "weights, an input with NaN and infinite voxels, the principal components with their "
        "matrix files, `--zconcat`, a list file's median), on every platform: "
        f"`reproduce` `{a.golden}` (the official program's value on the same inputs, `check.py "
        f"--golden`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_concat_replica_needs_no_more_memory_than_"
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
