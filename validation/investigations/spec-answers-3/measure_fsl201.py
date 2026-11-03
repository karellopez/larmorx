# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL-201 (specs/topup.md §4.6, spec writer, 2026-11-03), rule D measured: topup with FSL's
movement derivatives (the replica's `reproduce`, bit-identical to FSL) against the cost's exact
derivatives (the replica's default), on the cases that estimate the movements.

For each case: FSL's run through the oracle cache (`use` mode: the replica's `reproduce` run is
checked against it, its movement file byte for byte), then the replica in both modes with `-v`
and `--fout` added where the case lacks them. Printed per case: the final cost of each mode and
their relative difference (negative: the exact derivatives end lower), the field's difference
inside the head (the first volume above 10 % of its 99th percentile; Hz: 99th percentile and
largest) and the movements' largest difference (mm, rad). A summary counts the cases where the
exact derivatives end lower, equal or higher.

Usage (inside a memory box; the larmorx-nc program built from crates-nc):
  measure_fsl201.py <work dir> case <suite case> ...
  measure_fsl201.py <work dir> fmriprep
  measure_fsl201.py <work dir> fmriprep-fsl-config
  measure_fsl201.py <work dir> args <name> -- <topup arguments>
  measure_fsl201.py <work dir> finding <finding JSON> ...
"""

from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup_replica as T

COST = re.compile(r"Cost = (\S+)")


def final_cost(stdout: str) -> float | None:
    found = COST.findall(stdout)
    return float(found[-1]) if found else None


def option(args: list[str], name: str) -> str | None:
    for a in args:
        if a.startswith(name + "="):
            return a.split("=", 1)[1]
    return None


def find(d: Path, stem: str) -> Path | None:
    if (d / stem).is_file():
        return d / stem
    for ext in (".nii.gz", ".nii", ".hdr"):
        p = d / (stem + ext)
        if p.exists():
            return p
    return None


def measure(
    name: str, run, args: list[str], work: Path, node, fmriprep: bool = False
) -> dict | None:
    base = work / re.sub(r"[^A-Za-z0-9_.-]", "_", name)
    if base.exists():
        shutil.rmtree(base)
    dirs = {}
    for who in ("fsl", "fix", "reproduce"):
        dirs[who] = base / who
        dirs[who].mkdir(parents=True)
    if fmriprep:
        for who in dirs:
            T._prepare_fmriprep(node, dirs[who])
        a = list(args)
    else:
        a = T._prepare(run, dirs["fsl"], node)
        for who in ("fix", "reproduce"):
            T._prepare(run, dirs[who], node)
    settings = dict(run.settings)
    fsl = T._oracle(run, a, dirs["fsl"], settings)
    extra = []
    if "-v" not in a and "--verbose" not in a:
        extra.append("-v")
    field = option(a, "--fout")
    if field is None:
        field = "fsl201_field"
        extra.append(f"--fout={field}")
    out = {}
    for who, mode in (("fix", defects.FIX), ("reproduce", defects.REPRODUCE)):
        r = T.run_replica(
            run.program, a + extra, cwd=dirs[who], settings=settings, mode=mode, threads=1
        )
        out[who] = r
        if r.code != 0:
            print(f"{name}: the replica ({who}) exits {r.code}: {r.stderr[-300:]}")
            return None
    mov = {}
    for who in dirs:
        found = sorted(dirs[who].glob("*_movpar.txt"))
        mov[who] = found[0].read_bytes() if found else None
    if mov["fix"] is None:
        print(f"{name}: no movement file")
        return None
    same = mov["fsl"] == mov["reproduce"]
    cf, cr = final_cost(out["fix"].stdout), final_cost(out["reproduce"].stdout)
    ff = np.asarray(nib.load(str(find(dirs["fix"], field))).dataobj, float)
    fr = np.asarray(nib.load(str(find(dirs["reproduce"], field))).dataobj, float)
    img = np.asarray(nib.load(str(dirs["fsl"] / option(a, "--imain"))).dataobj, float)
    first = img[..., 0] if img.ndim == 4 else img
    head = first > 0.1 * np.percentile(first, 99)
    dfield = np.abs(ff - fr)[head]
    mf = np.loadtxt(sorted(dirs["fix"].glob("*_movpar.txt"))[0], ndmin=2)
    mr = np.loadtxt(sorted(dirs["reproduce"].glob("*_movpar.txt"))[0], ndmin=2)
    named = "FSL-201" in out["fix"].stderr
    res = {
        "case": name,
        "reproduce = FSL (movements)": same,
        "FSL-201 named": named,
        "cost FSL": cr,
        "cost exact": cf,
        "relative": (cf - cr) / cr if cf is not None and cr else None,
        "field p99 Hz": float(np.percentile(dfield, 99)) if dfield.size else 0.0,
        "field max Hz": float(dfield.max()) if dfield.size else 0.0,
        "translations mm": float(np.abs(mf[:, :3] - mr[:, :3]).max()),
        "rotations rad": float(np.abs(mf[:, 3:] - mr[:, 3:]).max()),
    }
    rel = res["relative"]
    print(
        f"{name}: cost FSL {cr} exact {cf} ({rel:+.3%}); field p99 {res['field p99 Hz']:.3g} "
        f"max {res['field max Hz']:.3g} Hz; movements {res['translations mm']:.3g} mm "
        f"{res['rotations rad']:.3g} rad; reproduce = FSL: {same}; FSL-201 named: {named}"
        if rel is not None
        else f"{name}: no cost line"
    )
    return res


def main() -> None:
    work = Path(sys.argv[1])
    kind = sys.argv[2]
    work.mkdir(parents=True, exist_ok=True)
    results = []
    if kind == "case":
        cases = T._all_cases()
        for name in sys.argv[3:]:
            tier, desc, run = cases[name]
            results.append(measure(name, run, list(run.args), work, T._source_node()))
    elif kind == "fmriprep":
        for runname, node in T.fmriprep_calls():
            results.append(measure_fmriprep(runname, node, work))
    elif kind == "fmriprep-fsl-config":
        # fMRIPrep's images and encoding file with FSL's own b02b0.cnf (nine levels, the
        # movements estimated in the first five): topup as FSL's guide runs it
        import os

        cfg = Path(os.path.expanduser("~/fsl-6.0.7.23/etc/flirtsch/b02b0.cnf"))
        for runname, node in T.fmriprep_calls():
            args = [
                f"--config={cfg}" if a.startswith("--config=") else a for a in T.fmriprep_args(node)
            ]
            args = [
                a for a in args if not a.startswith(("--logout", "--jacout", "--dfout", "--rbmout"))
            ]
            run = T.Run(tuple(args), timeout=3600)
            results.append(measure(f"fsl-config/{runname}", run, args, work, node, fmriprep=True))
    elif kind == "args":
        # converged runs: <name> -- <topup arguments> (`@name` the suite's derived inputs)
        i = sys.argv.index("--")
        run = T.Run(tuple(sys.argv[i + 1 :]), timeout=3600)
        results.append(measure(sys.argv[3], run, list(run.args), work, T._source_node()))
    else:
        from larmorx_validation.difftest.tools import fsl_topup as dt

        for path in sys.argv[3:]:
            doc = json.loads(Path(path).read_text(encoding="utf-8"))
            c = dt.case(doc["recipe"], "replica")
            run, at = c.payload[1]
            assert at is None
            results.append(measure(Path(path).stem, run, list(run.args), work, None))
    done = [r for r in results if r and r["relative"] is not None]
    lower = sum(r["relative"] < -1e-5 for r in done)
    higher = sum(r["relative"] > 1e-5 for r in done)
    print(
        f"{len(done)} cases: the exact derivatives end lower in {lower}, higher in {higher}, "
        f"equal (to the six digits of -v) in {len(done) - lower - higher}"
    )
    (work / f"summary-{kind}.json").write_text(json.dumps(results, indent=1), encoding="utf-8")


def measure_fmriprep(runname: str, node: Path, work: Path) -> dict | None:
    """fMRIPrep's call (sdcflows' b02b0.cnf: --estmov=0, so FSL-201 cannot reach it): both
    modes must be identical and name nothing."""
    base = work / f"fmriprep-{runname}"
    if base.exists():
        shutil.rmtree(base)
    res = {}
    for who, mode in (("fix", defects.FIX), ("reproduce", defects.REPRODUCE)):
        d = base / who
        d.mkdir(parents=True)
        T._prepare_fmriprep(node, d)
        r = T.run_replica(
            "topup", T.fmriprep_args(node) + ["-v"], cwd=d, settings={}, mode=mode, threads=1
        )
        res[who] = (r.code, final_cost(r.stdout), "FSL-201" in r.stderr, d)
    same = all(
        (res["fix"][3] / f.name).read_bytes() == f.read_bytes()
        for f in sorted(res["reproduce"][3].iterdir())
        if f.is_file() and f.suffix in (".gz", ".txt", ".nii")
    )
    print(
        f"fmriprep {runname}: exit {res['fix'][0]}/{res['reproduce'][0]}; cost {res['fix'][1]} / "
        f"{res['reproduce'][1]}; FSL-201 named {res['fix'][2]}; every output identical: {same}"
    )
    return {"case": f"fmriprep/{runname}", "relative": None, "identical": same}


if __name__ == "__main__":
    main()
