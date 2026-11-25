# SPDX-License-Identifier: Apache-2.0
"""mri_coreg's validation record from the replica's runs: ``docs/validation/freesurfer-mri-coreg.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded heads' comparisons (``real.py
--json``: only their verdicts, times and peaks are kept, never their paths), the golden values and
the memory figures of ``tests/parity/test_memory_peak.py`` (its printed lines), and writes the
record with the commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-coreg/record.py --probe <a.json> ... --real <real.json> \\
        --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-coreg"
RECIPES = {
    "fmriprep": "fMRIPrep's `MRICoreg` (`--s --sd --mov --lta --dof 6 --sep 4 --ftol 1e-4 "
    "--linmintol 1e-2`)",
    "bbregister": "`bbregister --init-coreg`'s (`--s --mov --regdat --reg --nthreads 1 --dof 6 "
    "--sep 4 --ftol .0001 --linmintol .01`)",
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
    ap.add_argument("--commit", help="the clean commit the runs were made from (default: HEAD's)")
    ap.add_argument(
        "--official-times",
        type=Path,
        help="a real.py summary whose official runs were fresh (the record's are served by the "
        "oracle-output cache, which keeps no time): FreeSurfer's seconds taken from it",
    )
    ap.add_argument("--prelude", type=int, default=125, help="the prelude's case count")
    a = ap.parse_args(argv)
    keys = ("seed", "family", "cases", "reproduce_equal", "reproduce_differ", "skipped",
            "skipped_but_identical", "not_ported", "fix_differ", "fix_differ_without_notice",
            "official_sha256_skip")  # fmt: skip
    probes = [
        {k: json.loads(p.read_text(encoding="utf-8")).get(k, 0) for k in keys} for p in a.probe
    ]
    real = json.loads(a.real.read_text(encoding="utf-8"))
    fresh = {}
    if a.official_times:
        for r in json.loads(a.official_times.read_text(encoding="utf-8"))["rows"]:
            fresh[(r["head"], r["recipe"])] = r["seconds"]["official"]
    for r in real["rows"]:
        if r["served"] and (r["head"], r["recipe"]) in fresh:
            r["seconds"]["official"] = fresh[(r["head"], r["recipe"])]
    rows = [
        {
            "head": r["head"],
            "recipe": r["recipe"],
            "reproduce": r["reproduce_identical"],
            "fix": r["fix_identical"],
            "seconds": r["seconds"],
            "peak_mib": r["peak_mib"],
        }
        for r in real["rows"]
    ]
    mem = re.findall(
        r"mri_coreg (\w+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)",
        a.memory.read_text(encoding="utf-8", errors="replace"),
    )
    memory = [
        {"case": c, "mode": m, "larmorx_mib": float(x), "freesurfer_mib": float(y)}
        for c, m, x, y in mem
    ]
    rec = {
        "tool": "mri_coreg",
        "upstream": "FreeSurfer 8.2.0",
        "status": "experimental",
        "queue": "3.2-17 (mri_coreg, the whole program)",
        "commit": a.commit or commit(),
        "date": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "probe": probes,
        "real": rows,
        "golden": {"reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    out = REPO / "docs" / "validation" / f"{NAME}.json"
    out.write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")

    def same(v: dict) -> str:
        return (
            "identical in every byte"
            if all(v.values())
            else ("differs: " + ", ".join(k for k, x in v.items() if not x))
        )

    md = [
        "# Validation: FreeSurfer's `mri_coreg` (replica)",
        "",
        f"Status `experimental`. Generated {rec['date']} on {rec['platform']} from commit "
        f"`{rec['commit']}` with `validation/probes/fs-coreg/record.py` (queue item 3.2-17: the "
        "whole program).",
        "",
        "- **Replica:** crates-freesurfer/larmorx-freesurfer-volumes (src/mri_coreg/) on the volume "
        "and filter functions of larmorx-freesurfer-volume and -filter, Powell's minimiser of "
        "larmorx-freesurfer-core and the RANLUX389 draws of larmorx-core's `ranlux` (the clean room "
        "of `specs/ranlux.md`), the larmorx-freesurfer program (`larmorx-freesurfer "
        "[--upstream-defects reproduce|fix] freesurfer mri_coreg ...`).",
        "- **Reference:** FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/mri_coreg) at one "
        "thread.",
        "- **Recorded steps:** neither recording stream runs `mri_coreg` (recon-all does not; the "
        "fMRIPrep oracle runs skipped recon-all): fMRIPrep's and `bbregister`'s calls are made below "
        "on the FreeSurfer reference runs' subjects with the fMRIPrep runs' BOLD references.",
        "",
        "## fMRIPrep's and bbregister's calls on recorded heads",
        "",
        "`validation/probes/fs-coreg/real.py`: the subject of each FreeSurfer reference run (its "
        "`brainmask.mgz` the reference, its `aparc+aseg.mgz` the reference's mask) and the BOLD "
        "reference fMRIPrep registered (`desc-coreg_boldref`); exit status, both streams (the times "
        "masked; larmorx's notices aside) and every file, byte for byte; times and peaks at one "
        "thread (FreeSurfer's from its fresh run of the same call the same day: the record's own "
        "official runs are served by the oracle-output cache):",
        "",
    ]
    for r in rows:
        md.append(
            f"- {r['head']}, {RECIPES[r['recipe']]}: `reproduce` {same(r['reproduce'])}; `fix` "
            f"{same(r['fix'])}; {r['seconds']['reproduce']} s against FreeSurfer's "
            f"{r['seconds']['official']} s; peak {r['peak_mib']['reproduce']} MiB against "
            f"{round(r['peak_mib']['official'], 1)}."
        )
    md += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-coreg/check.py`: the command line (no argument, the help with the "
        "settings so far, the version, unknown options, missing arguments, the checks, case-blind "
        "names), the stand-alone options (`--mat2par`, `--mat2par-vox`, `--mat2rot`, `--par2mat` with "
        "a number that does not read, `--lrrev`, `--rms` with every failure, `--landmarks` and "
        "`--landmarks-2d` with pair files and JSON point sets, `--init-reg`, "
        "`--init-reg-save-only`), every registration option on a small BOLD-like volume and a brain "
        "reference (the schemas, the dithers, the smoothing, separations, tolerances, the masks, the "
        "intensity dither volume, every output file, `--conf-ref`, the initial transforms, the "
        "brute force's limits, `--sep 0`'s division by 0, the saturation at 100), a subject's folder "
        "(`--s` with `SUBJECTS_DIR` or `--sd`, a missing subject, FreeSurfer's environment missing); "
        "then seeded pairs of every type and orientation with random options. Exit status, both "
        "streams and every file written, case by case; counted apart: the cases whose result in "
        "FreeSurfer is undefined (`check.py`'s `SKIP`: memory contents, crashes' output, runs that "
        "never end; most of them identical anyway):",
        "",
    ]
    for p in probes:
        n = p["cases"] - a.prelude
        what = "the prelude" if n == 0 else f"the prelude and {n} seeded pairs, seed {p['seed']}"
        md.append(
            f"- {what}: {p['cases']} cases; `reproduce` {p['reproduce_equal']} identical, "
            f"{p['reproduce_differ']} different, {p['skipped']} undefined "
            f"({p['skipped_but_identical']} of them identical anyway), {p['not_ported']} not ported; "
            f"`fix` {p['fix_differ'] - p['fix_differ_without_notice']} different with a notice, "
            f"{p['fix_differ_without_notice']} without."
        )
    md += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_coreg_golden.rs`: 6 cases on small "
        "volumes made with integer arithmetic (fMRIPrep's call with a reference mask and a narrowed "
        "brute force, the 12-parameter schema with every output and an initial registration, the "
        "2-D schema with the centroids and a RAS-to-RAS output, `--mat2par`, `--lrrev`, "
        "`--landmarks` with its point set), on every "
        f"platform: `reproduce` `{a.golden}` (the official program's value on the same inputs, "
        f"`check.py --golden`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_coreg_replica_needs_no_more_memory_than_freesurfer` "
        "(fMRIPrep's call on ds000228, one thread; the rule: at most 10 % above FreeSurfer's peak plus "
        "8 MiB; the registration identical byte for byte):",
        "",
    ]
    for m in memory:
        md.append(
            f"- {m['case']} ({m['mode']}): {m['larmorx_mib']} MiB, FreeSurfer {m['freesurfer_mib']} MiB."
        )
    md += [
        "",
        "## Defects",
        "",
        "FS-1313 to FS-1327 (`larmorx_core::defects`; `docs/findings/freesurfer-coreg.md`); the "
        "spec `specs/mri_coreg.md` §10.",
        "",
    ]
    (REPO / "docs" / "validation" / f"{NAME}.md").write_text("\n".join(md), encoding="utf-8")
    print(f"wrote docs/validation/{NAME}.{{json,md}} from {rec['commit']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
