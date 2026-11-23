# SPDX-License-Identifier: Apache-2.0
"""mri_robust_register's validation record from the replica's runs: ``docs/validation/freesurfer-mri-robust-register.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``), the recorded heads' comparisons (``real.py
--json``: only their verdicts, times and peaks are kept, never their paths), the golden values and
the memory figures of ``tests/parity/test_memory_peak.py`` (its printed lines), and writes the
record with the commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-robust-register/record.py --probe <a.json> ... \\
        --real <real.json> ... --memory <memory.log> --golden <official sha> --golden-fix <sha>
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
NAME = "freesurfer-mri-robust-register"


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def summary_rows(path: Path | None) -> list[dict]:
    """A real.py summary's verdicts, times and peaks (never its paths)."""
    if path is None:
        return []
    d = json.loads(path.read_text(encoding="utf-8"))
    return [
        {
            "head": r["head"],
            "reproduce": r["reproduce_identical"],
            "fix": r["fix_identical"],
            "seconds": r["seconds"],
            "peak_mib": r["peak_mib"],
        }
        for r in d["rows"]
    ]


def with_times(rows: list[dict], path: Path | None) -> list[dict]:
    """``rows`` with the official seconds of ``path``'s rows whose official run was not served."""
    if path is None:
        return rows
    d = json.loads(path.read_text(encoding="utf-8"))
    t = {r["head"]: r["seconds"]["official"] for r in d["rows"] if not r.get("served")}
    for r in rows:
        if r["head"] in t:
            r["seconds"] = r["seconds"] | {"official": t[r["head"]]}
    return rows


#: The paths the port does not run yet (counted as ``not ported``, never compared); relay 47.
NOT_PORTED_YET = (
    "the Powell registration's `MI`, `NMI`, `ECC`, `SCR` and `SB` costs, its half-way outputs, "
    "`--debug`, `--test`"
)


def probe_set(p: dict) -> str:
    """A check.py summary's name: its seed, or the Powell registration's family (relay 47)."""
    name = "the Powell family" if p.get("family") == "powell" else f"seed {p['seed']}"
    return name + (" (the decided `cabs` and `csqrt` preloaded)" if p.get("preload") else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--real", type=Path, nargs="+", required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument(
        "--official-times",
        type=Path,
        nargs="*",
        default=[],
        help="earlier real.py summaries whose official runs were not served (their times)",
    )
    ap.add_argument(
        "--preload",
        type=Path,
        help="real.py --preload's summary (relay 45: the official program with the decided cabs "
        "and csqrt)",
    )
    ap.add_argument(
        "--robent",
        type=Path,
        help="real.py --extra '--cost ROBENT ...''s summary (relay 45: the entropy images)",
    )
    ap.add_argument(
        "--robent-times",
        type=Path,
        help="an earlier --robent summary whose official runs were not served (their times)",
    )
    ap.add_argument("--golden", required=True)
    ap.add_argument("--golden-fix", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    probes = [json.loads(p.read_text(encoding="utf-8")) for p in a.probe]
    timed: dict[tuple[str, bool], float] = {}
    for p in a.official_times:
        d = json.loads(p.read_text(encoding="utf-8"))
        outputs = "outputs" in p.name
        for r in d["rows"]:
            if not r.get("served") and r["seconds"]["official"]:
                timed[(r["head"], outputs)] = r["seconds"]["official"]
    real = []
    for p in a.real:
        d = json.loads(p.read_text(encoding="utf-8"))
        outputs = "outputs" in p.name
        for r in d["rows"]:
            real.append(
                {
                    "head": r["head"],
                    "outputs": outputs,
                    "reproduce": r["reproduce_identical"],
                    "fix": r["fix_identical"],
                    "seconds": r["seconds"]
                    | {"official": timed.get((r["head"], outputs), r["seconds"]["official"])},
                    "peak_mib": r["peak_mib"],
                }
            )
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_robust_register ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "mri_robust_register (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_robust_register/) on "
        "larmorx-freesurfer-robust (fs.robust), the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official program ($FREESURFER_HOME/bin/"
        "mri_robust_register) on the recorded heads (oracles/freesurfer-8.2.0/runs/) and on "
        "synthetic cases",
        "recorded_steps": "none: recon-all runs the program only as `mri_robust_register "
        "-all-info` (its version line in recon-all.log); fMRIPrep's call is compared on the "
        "recorded heads instead",
        "real_heads": real,
        "probe": [
            {
                "set": probe_set(p),
                "decided_complex_preloaded": p.get("preload", False),
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "not_ported": p.get("not_ported", 0),
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "golden_sha256": {"official_and_reproduce": a.golden, "fix": a.golden_fix},
        "decided_complex": summary_rows(a.preload),
        "entropy_images": with_times(summary_rows(a.robent), a.robent_times),
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_robust_register` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-robust-register/record.py` (queue item "
        "3.2-13).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_robust_register ...`).",
        f"- **Reference:** {payload['reference']}.",
        f"- **Recorded steps:** {payload['recorded_steps']}.",
        "",
        "## fMRIPrep's call on the recorded heads",
        "",
        "`validation/probes/fs-robust-register/real.py`: `--satit --iscale`, each head's "
        "`T1.mgz` onto its `rawavg.mgz` (sMRIPrep's `RobustRegister`); with `--outputs` also "
        "`--mapmov`, `--weights` and every half-way output. Exit status, both streams (the time "
        "taken masked; larmorx's notices aside) and every file, byte for byte; times and peaks "
        "at one thread:",
        "",
    ]
    for r in real:
        files = "every file identical" if r["reproduce"]["files"] else "FILES DIFFER"
        so = "standard output identical" if r["reproduce"]["stdout"] else "standard output differs"
        ff = (
            "the same with `fix`"
            if all(r["fix"].values())
            else "`fix`: " + ", ".join(k for k, v in r["fix"].items() if not v) + " differ"
        )
        lines.append(
            f"- {r['head']}{' (every output)' if r['outputs'] else ''}: `reproduce` {files}, "
            f"{so}; {ff}; {r['seconds']['reproduce']} s against FreeSurfer's "
            f"{r['seconds']['official']} s; peak {r['peak_mib']['reproduce']} MiB "
            f"against {r['peak_mib']['official']:.1f}."
        )
    lines += [
        "",
        "ds004173-sub-603309's standard output (and its half-way maps on standard error) differ "
        "in the last two printed digits of the final voxel transform's translations (2e-13 of "
        "210); the transform files are identical. The cause is the project's decided `hypot` "
        "(the user's D4: correctly rounded in both modes, also inside the complex `cabs` and "
        "`csqrt` of the matrix square roots), where the official program takes glibc's: the "
        "official build's own product on the port's matrices gives the port's numbers "
        "(`v2v_official.py`), and the official program with the decided `cabs` and `csqrt` "
        "preloaded (`decided_complex.sh`, `real.py --preload`) gives the port's output:",
        "",
    ]
    for r in payload["decided_complex"]:
        same = all(r["reproduce"].values())
        lines.append(
            f"- {r['head']} (the decided `cabs` and `csqrt` preloaded): "
            f"{'identical in every byte' if same else 'DIFFERS'} with `reproduce`"
            f"{' and `fix`' if all(r['fix'].values()) else ''}."
        )
    if payload["entropy_images"]:
        lines += [
            "",
            "The entropy images (`--cost ROBENT --entmov em.mgz --entdst ed.mgz`, the default "
            "radius 5, after fMRIPrep's arguments; `real.py --extra`):",
            "",
        ]
    for r in payload["entropy_images"]:
        same = all(r["reproduce"].values())
        lines.append(
            f"- {r['head']}: `reproduce` {'identical in every byte' if same else 'DIFFERS'}"
            f"{', the same with `fix`' if all(r['fix'].values()) else ''}; "
            f"{r['seconds']['reproduce']} s against FreeSurfer's {r['seconds']['official']} s; "
            f"peak {r['peak_mib']['reproduce']} MiB against {r['peak_mib']['official']:.1f}."
        )
    lines += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-robust-register/check.py`: the command line (no argument, the "
        "help, the version, unknown options and costs, an option's missing argument (FS-1070), "
        "each check), unreadable and multi-frame inputs, images below 16 voxels, masks, initial "
        "transforms (LTA, MINC, unreadable, missing), the intensity scale's files (numbers, "
        "words, empty, too large, a folder, missing), every model (rigid, affine, isotropic "
        "scale, translation, intensity only), the saturation (`--satit`, `--sat`), every "
        "iteration and resolution option, double precision, float images, no symmetry, "
        "`--uchar`, white backgrounds, Sobel images, every output (LTA, MINC and register.dat "
        "transforms, `--vox2vox`, the mapped and header-moved images, the weights, the half-way "
        "outputs, the intensity scale's file, outputs that cannot be written), the entropy "
        "images (`--cost ROBENT`: byte, short and float images, masks, 2-D images, every "
        "entropy option, a radius below 0, FS-1074); then seeded pairs "
        "of head-like images of every type, extent and orientation with random options; and "
        "the Powell registration's family (`--family powell`: 40 cases of the `LS`, `NCC`, `TB`, "
        "`LNCC` and `SAD` costs with random models, intensity scales, symmetry or not, full "
        "images, tolerances, subsampling, 2-D pairs and outputs). Every Powell evaluation with "
        "symmetry takes the matrix square roots, so the official program runs with the decided "
        "`cabs` and `csqrt` preloaded (`--preload`, `decided_complex.sh`: the user's D4; without "
        "it 4 of the 40 differ in their last printed digits, glibc's `hypot`). Exit "
        "status, both streams and every file written, case by case; counted apart: the cases "
        "whose result in FreeSurfer is undefined (`check.py`'s `SKIP`) and the paths not ported "
        f"yet ({NOT_PORTED_YET}):",
        "",
    ]
    for p in payload["probe"]:
        lines.append(
            f"- {p['set']}: {p['cases']} cases; `reproduce` {p['identical']} identical, "
            f"{p['different']} different, {p['skipped']} undefined, {p['not_ported']} not ported; "
            f"`fix` {p['fix_different_with_notice']} different with a notice, "
            f"{p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_robust_register_golden.rs`: 5 "
        "cases on 32^3 images made with integer arithmetic (fMRIPrep's call with every output, "
        "an affine registration with its half-way outputs, a short source onto a float target "
        "of anisotropic voxels, the entropy images of a short source and a byte target, the "
        "Powell registration with `--cost NCC`), on "
        "every platform: `reproduce` "
        f"`{a.golden}` (the official program's value on the same inputs, `check.py --golden`), "
        f"`fix` `{a.golden_fix}`.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_robust_register_replica_needs_no_more_memory_"
        "than_freesurfer` (fMRIPrep's call on ds000228 pixar008, one thread; the rule: at most "
        "10 % above FreeSurfer's peak plus 8 MiB; the transform identical byte for byte):",
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
