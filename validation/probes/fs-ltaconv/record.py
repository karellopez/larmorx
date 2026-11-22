# SPDX-License-Identifier: Apache-2.0
"""lta_convert's validation record from the replica's runs: ``docs/validation/freesurfer-lta-convert.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's recorded steps (``freesurfer-replay run ... --json``, one file per
run and mode: only their verdicts and times are kept, never their paths), the probe's summaries
(``check.py --json``), the fMRIPrep-like calls on the recorded heads (``real.py --json``), the
golden values and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-ltaconv/record.py --replays <dir> --probe <a.json> ... \\
        --real <real.json> --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-lta-convert"


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
    ap.add_argument("--real", type=Path)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    replays = {}
    for mode in a.modes.split(","):
        rows = []
        for f in sorted((a.replays / f"replay-{mode}").glob("*.json")):
            rows.extend(json.loads(f.read_text(encoding="utf-8")))
        replays[mode] = {
            "steps": len(rows),
            "runs": len({r["run"] for r in rows}),
            "verdicts": dict(Counter(r["verdict"] for r in rows)),
            "seconds_recorded": round(sum(r["seconds"]["recorded"] for r in rows), 1),
            "seconds_replayed": round(sum(r["seconds"]["replayed"] for r in rows), 1),
        }
    # a summary of check.py --extra (its file named so) is the extra cases', not a seed's
    probes = [
        json.loads(p.read_text(encoding="utf-8")) | {"extra": "extra" in p.name} for p in a.probe
    ]
    real = {}
    if a.real and a.real.is_file():
        r = json.loads(a.real.read_text(encoding="utf-8"))
        real = {k: r[k] for k in ("calls", "reproduce_identical", "fix_identical")}
        real["runs"] = sorted({x["run"] for x in r["rows"]})
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"lta_convert ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "lta_convert (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/lta_convert/), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: every "
        "step recorded) and its official program ($FREESURFER_HOME/bin/lta_convert)",
        "replay": replays,
        "probe": [
            {
                "set": "extra" if p["extra"] else f"seed {p['seed']}",
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "real": real,
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `lta_convert` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-ltaconv/record.py` (queue item 3.2-09).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer lta_convert ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded steps",
        "",
        "recon-all runs the program once per head (step 4: `--inlta identity.nofile --src "
        "rawavg.mgz --trg orig.mgz --outlta rawavg2orig.lta --subject <s>`). Each recorded step "
        "replayed on the subject folder as it was before it (`python -m larmorx_validation "
        "freesurfer-replay run`), its files compared with the recorded ones:",
        "",
    ]
    for mode, r in replays.items():
        v = ", ".join(f"{n} {k}" for k, n in sorted(r["verdicts"].items()))
        lines.append(
            f"- `{mode}`: {r['steps']} steps on {r['runs']} runs: {v} ({r['seconds_replayed']} s "
            f"in all; {r['seconds_recorded']} s recorded)."
        )
    if real:
        lines += [
            "",
            "## fMRIPrep's and sMRIPrep's calls on recorded heads",
            "",
            "`validation/probes/fs-ltaconv/real.py`: niworkflows' `--inlta <lta> --outfsl <mat>`, "
            "sMRIPrep's `--inlta identity.nofile --src --trg --outlta`, the coregistration's "
            "`--inlta <lta> --outlta`, bbregister's `--inlta <lta> --outreg <dat>`, every other "
            "output, and the ITK and FSL ways back, on the heads' own transforms and volumes "
            f"({', '.join(real['runs'])}): {real['calls']} calls, `reproduce` "
            f"{real['reproduce_identical']} identical to the official program, the default "
            f"{real['fix_identical']}.",
        ]
    lines += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-ltaconv/check.py`: the command line (no argument, the version and "
        "help options, an option without its value, unknown options and spellings, words that "
        "are not options), every input format (`.lta` of each type, with and without "
        "geometries, `identity.nofile`; FSL matrices; MINC `.xfm`, inverted; `register.dat`; "
        "NiftyReg, 3D and 2D; ITK: affine, matrix-offset, 2D, a centre, with and without the "
        "last new line, unknown lines and types, a comment last, a number that is not one; voxel "
        "space; failures and missing files), volumes of several geometries and formats (oblique, "
        "45 degrees, no valid RAS, a voxel size of 0, flat, NIfTI, a damaged one, odd names), "
        "every geometry option (conform, `-dc`, `-min`, `-size`, up- and downsampling, factors "
        "that make no sense), `--invert`, `--subject`, `--sqrt`, `--ltavox2vox`, `--ltatkreg`, "
        "every output and outputs that cannot be written, FSL's output type; then seeded "
        "command lines. Exit status, both streams and every file after the run, case by case; "
        "counted apart: the cases whose result in FreeSurfer is undefined (`check.py`'s `SKIP`: "
        "a crash or an abort, a program that never ends, memory contents):",
        "",
    ]
    for p in payload["probe"]:
        name = (
            "the extra cases (Linux's names only: a dot last)" if p["set"] == "extra" else p["set"]
        )
        lines.append(
            f"- {name}: {p['cases']} cases; `reproduce` {p['identical']} identical, "
            f"{p['different']} different, {p['skipped']} counted; `fix` "
            f"{p['fix_different_with_notice']} different with a notice, "
            f"{p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/lta_convert_golden.rs`: 5 cases on "
        "volumes up to 32^3 (recon-all's step 4, talairach_avi's call, an ITK registration to an "
        "`.lta`, an FSL matrix and a `register.dat`, an `.lta` to ITK, an inverse to ITK, voxel "
        f"space, NiftyReg and MINC), on every platform: `reproduce` `{a.golden}` (the official "
        f"program's value on the same inputs, `check.py --golden`), `fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_lta_convert_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's volumes read in place, one thread; the rule: at most 10 % "
        "above FreeSurfer's peak plus 8 MiB; the output identical byte for byte):",
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
