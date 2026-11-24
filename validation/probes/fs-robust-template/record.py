# SPDX-License-Identifier: Apache-2.0
"""mri_robust_template's validation record from the replica's runs: ``docs/validation/freesurfer-mri-robust-template.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded calls' comparisons (``real.py
--json``: only their verdicts, times and peaks are kept, never their paths), the golden values and
the memory figures of ``tests/parity/test_memory_peak.py`` (its printed lines), and writes the
record with the commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-robust-template/record.py --probe <a.json> ... \\
        --real <real.json> --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-robust-template"
NOT_PORTED_YET = (
    "the affine mean space (`--affine` without `--fixtp`: the matrices' geometric mean, "
    "`MyMatrix::GeometricMean`'s matrix logarithm and exponential; queue 3.2-15)"
)
PRELOAD = (
    "the official program with the decided `acos`, `cabs` and `csqrt` preloaded (`decided.sh`: "
    "the project's decision for glibc's `acos` and the user's D4 for `hypot`)"
)
RECIPES = {
    "fmriprep": "fMRIPrep's call (`--satit --fixtp --inittp 1 --iscale --noit --subsample 200 --lta`)",
    "reconall": "recon-all's motion correction (the same with `--average 1 --iscaleout`)",
    "longitudinal": "sMRIPrep's `--longitudinal` call (no `--fixtp`, no `--noit`: the mean space)",
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
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path, required=True)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    a = ap.parse_args(argv)
    probes = []
    for p in a.probe:
        d = json.loads(p.read_text(encoding="utf-8"))
        probes.append({k: d[k] for k in ("seed", "family", "preload", "cases", "reproduce_equal",
                       "reproduce_differ", "skipped", "not_ported", "fix_differ",
                       "fix_differ_without_notice", "official_sha256_skip")})  # fmt: skip
    real = json.loads(a.real.read_text(encoding="utf-8"))
    rows = [
        {
            "run": r["run"],
            "recipe": r["recipe"],
            "inputs": r["inputs"],
            "reproduce": r["reproduce_identical"],
            "fix": r["fix_identical"],
            "seconds": r["seconds"],
            "peak_mib": r["peak_mib"],
        }
        for r in real["rows"]
    ]
    mem = re.findall(
        r"mri_robust_template (\w+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)",
        a.memory.read_text(encoding="utf-8", errors="replace"),
    )
    memory = [
        {"case": c, "mode": m, "larmorx_mib": float(x), "freesurfer_mib": float(y)}
        for c, m, x, y in mem
    ]
    rec = {
        "tool": "mri_robust_template",
        "upstream": "FreeSurfer 8.2.0",
        "status": "experimental",
        "queue": "3.2-14 (part 1 of mri_robust_template)",
        "commit": commit(),
        "date": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "not_ported_yet": NOT_PORTED_YET,
        "probe": probes,
        "real": rows,
        "golden": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    out = REPO / "docs" / "validation" / f"{NAME}.json"
    out.write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")

    def same(v: dict) -> str:
        return "identical in every byte" if all(v.values()) else (
            "differs: " + ", ".join(k for k, x in v.items() if not x)
        )

    md = [
        "# Validation: FreeSurfer's `mri_robust_template` (replica)",
        "",
        f"Status `experimental`. Generated {rec['date']} on {rec['platform']} from commit "
        f"`{rec['commit']}` with `validation/probes/fs-robust-template/record.py` (queue item 3.2-14, "
        "part 1 of the program).",
        "",
        "- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_template/) on "
        "larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program (`larmorx-freesurfer "
        "[--upstream-defects reproduce|fix] freesurfer mri_robust_template ...`).",
        "- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/"
        "mri_robust_template) on fMRIPrep's recorded inputs and on synthetic cases, at one thread, "
        f"{PRELOAD}.",
        "- **Recorded steps:** recon-all runs the program only as `mri_robust_template -all-info` "
        "on the recorded heads (one T1w each); fMRIPrep's `StructuralReference` ran it in the "
        "fMRIPrep oracle runs of ds004173 (three T1w) and ds005454 (two), compared below.",
        f"- **Not ported yet:** {NOT_PORTED_YET}: it ends with `... is not ported yet`, status 1.",
        "",
        "## fMRIPrep's recorded inputs",
        "",
        "`validation/probes/fs-robust-template/real.py`: each fMRIPrep oracle run's N4-corrected T1w "
        "images (its `anat_template_wf/n4_correct`), each recipe; exit status, both streams (the "
        "time taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks "
        "at one thread:",
        "",
    ]
    for r in rows:
        md.append(
            f"- {r['run']}, {RECIPES[r['recipe']]}, {r['inputs']} inputs: `reproduce` "
            f"{same(r['reproduce'])}; `fix` {same(r['fix'])}; {r['seconds']['reproduce']} s against "
            f"FreeSurfer's {r['seconds']['official']} s; peak {r['peak_mib']['reproduce']} MiB against "
            f"{round(r['peak_mib']['official'], 1)}."
        )
    md += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-robust-template/check.py`: the command line (no argument, the help, "
        "the version, unknown options, the checks, every option's missing argument: FS-1080), "
        "inputs that do not read or have several frames, masks (missing, too few, of several frames), "
        "initial transforms (`identity.nofile`, unreadable, without geometry) and intensity files "
        "(numbers, words, empty, a folder), the paths of the template (fMRIPrep's and recon-all's "
        "calls on two and three inputs, the random target with and without `--seed`, `--inittp` 0, "
        "inside and past the inputs, `--fixtp`, the mean space, the global iterations, every average, "
        "every model, `--nomulti`, `--leastsquares`, double precision, subsampling, `--highit`, float "
        "images, `--finalnearest`, `--cras`, `--frobnorm-thresh`, `--res-thresh`, `--debug`), inputs "
        "of other voxel sizes, orientations, types and 2-D, empty images, one 4-D input with its mask, "
        "every output (LTAs, `--vox2vox`, the mapped inputs, one 4-D image, the moved headers, "
        "intensity files, weights, `--conform`, outputs that cannot be written), the test mode; then "
        "seeded sets of two to four head-like images of every type, extent and orientation (MGZ and "
        "NIfTI) with random options. Exit status, both streams and every file written, case by "
        "case; counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s "
        "`SKIP`) and the paths not ported yet:",
        "",
    ]
    for p in probes:
        md.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {p['reproduce_equal']} identical, "
            f"{p['reproduce_differ']} different, {p['skipped']} undefined, {p['not_ported']} not "
            f"ported; `fix` {p['fix_differ'] - p['fix_differ_without_notice']} different with a "
            f"notice, {p['fix_differ_without_notice']} without."
        )
    md += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_template_golden.rs`: 6 cases "
        "on 32^3 images made with integer arithmetic (fMRIPrep's call on three inputs, recon-all's "
        "on two, the rigid mean space of two, one global iteration of three with weights, an input "
        "with NaN voxels built from their bits, a 4-D input's one `--mapmov` image), on every "
        f"platform: `reproduce` `{a.golden}` (the official program's value on the same inputs, "
        f"`check.py --golden --preload`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_robust_template_replica_needs_no_more_memory_than_freesurfer` "
        "(fMRIPrep's call on ds005454's two T1w, one thread; the rule: at most 10 % above "
        "FreeSurfer's peak plus 8 MiB; the template and transforms identical byte for byte):",
        "",
    ]
    for m in memory:
        md.append(
            f"- {m['case']} ({m['mode']}): {m['larmorx_mib']} MiB, FreeSurfer {m['freesurfer_mib']} MiB."
        )
    md += [
        "",
        "Findings: `docs/findings/freesurfer-robust-template.md` (closed to clean rooms); the spec: "
        "`specs/mri_robust_template.md`; defects FS-1080 to FS-1098 (`docs/findings/upstream-defects.md`).",
        "",
    ]
    (REPO / "docs" / "validation" / f"{NAME}.md").write_text("\n".join(md), encoding="utf-8")
    print(f"wrote docs/validation/{NAME}.json and .md from {rec['commit']}")
    return 1 if rec["commit"].endswith("-dirty") else 0


if __name__ == "__main__":
    raise SystemExit(main())
