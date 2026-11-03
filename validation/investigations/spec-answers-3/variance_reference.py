# SPDX-License-Identifier: Apache-2.0
"""FSL-202, rule D's second measurement: does the corrected single-channel class variance
bring FAST's outputs closer to the intended maths, and is it as stable as FSL's own
computation? Spec answers for topup, FAST and FLIRT, relay 2b (2026-11-03).

``variance_measure.py`` (relay 2) showed how far the corrected variance moves FAST's outputs.
This script asks the two questions that decide between rule A and rule C:

1. **Closer to the intended maths?** A reference run: FAST with the class statistics (the
   weighted mean and variance of each class, in the domain FAST computes them in) computed in
   double throughout (double products, double sums, the variance about the double mean in two
   passes), each rounded once to ``float`` where FAST keeps them, everything else as FSL. The
   distance of FSL's outputs to it against the distance of the corrected variant's
   (``LXNC_FAST_VARIANCE=double``: the mean's square from the double mean, the rest as FSL).
   Each distance is taken four ways (either run with the C library's ``exp``/``log`` or with
   correctly rounded ones, against the reference with either), so one chaotic sample does not
   decide.
2. **As stable?** Each computation's own last-bit band: its outputs with the C library's
   ``exp``, ``log``, ``expf``, ``logf`` against its outputs with correctly rounded ones (the
   perturbation that gives FSL's band, FSL under the CORE-MATH preload).

Per case: FSL 6.0.7.23 as built and under the CORE-MATH preload (the oracle cache), and the
FAST replica (bit-identical to FSL) built with the measurement hooks in ``reproduce`` mode:
FSL's variance with correctly rounded maths (``rep-cr``: it must equal FSL under the preload
bit for bit, which shows the hook is the same perturbation), the corrected variant and the
reference, each with both maths. Metrics as ``variance_measure.py``'s (``compare``): the label
images' voxels that differ inside the first run's mask and smallest per-class Dice, the
partial volumes' largest 99.9th percentile and largest absolute difference.

    LARMORX_NC_BIN=<the hooked larmorx-nc> PYTHONPATH=python:validation/src \\
        boxed.sh 24 python -P validation/investigations/spec-answers-3/variance_reference.py \\
        run <out dir> <case>          # one case (run the seven side by side, one thread each)
    python -P variance_reference.py summary <out dir>

The hooks (FSL-licensed replica code, so they live in the replica crate:
``crates-nc/larmorx-nc-fast/dev-patches/fsl202_reference.patch``, a superset of relay 2's
``variance_variant.patch``; a clean-room implementer does not open it): the variants
``LXNC_FAST_VARIANCE=double|twopass|reference`` and ``LXNC_CRMATH=1`` (correctly rounded
``exp``, ``log``, ``expf``, ``logf`` everywhere in the replica). Apply it in a checkout, build
``larmorx-nc-cli`` (release), copy the binary out, revert the patch and rebuild.
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))

import variance_measure as vm
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast_replica as rep

CASES = [
    "fmriprep/ds000005-sub-01-noreconall",
    "fmriprep/ds000228-sub-pixar008-noreconall",
    "fmriprep/ds005003-sub-RS001-noreconall",
    "verbose/half",
    "opt/bias-outputs",
    "opt/2d",
    "opt/big",
]
#: The replica's runs: directory, variance variant (None: FSL's), correctly rounded maths.
REPLICA_RUNS = {
    "rep-cr": (None, True),
    "double": ("double", False),
    "double-cr": ("double", True),
    "ref": ("reference", False),
    "ref-cr": ("reference", True),
}
#: The comparisons: name, the first run (whose mask counts), the second.
PAIRS = {
    "sanity rep-cr vs fsl-cr": ("fsl-cr", "rep-cr"),
    "band fsl": ("fsl", "fsl-cr"),
    "band double": ("double", "double-cr"),
    "band ref": ("ref", "ref-cr"),
    "fsl to ref": ("ref", "fsl"),
    "fsl-cr to ref": ("ref", "fsl-cr"),
    "fsl to ref-cr": ("ref-cr", "fsl"),
    "fsl-cr to ref-cr": ("ref-cr", "fsl-cr"),
    "double to ref": ("ref", "double"),
    "double-cr to ref": ("ref", "double-cr"),
    "double to ref-cr": ("ref-cr", "double"),
    "double-cr to ref-cr": ("ref-cr", "double-cr"),
}


def run_case(out_dir: Path, name: str) -> None:
    assert os.environ.get("LARMORX_NC_BIN"), "LARMORX_NC_BIN: the hooked replica"
    with tempfile.TemporaryDirectory(prefix="lx-fast-ref_") as t_:
        t = Path(t_)
        names = ["fsl", "fsl-cr", *REPLICA_RUNS]
        dirs = {k: t / k for k in names}
        for d in dirs.values():
            d.mkdir()
        args, settings, never = vm._setup(name, list(dirs.values()))
        p = rep.run_oracle(args, cwd=dirs["fsl"], settings=settings, never=never)
        q = rep.run_oracle(
            args,
            cwd=dirs["fsl-cr"],
            settings={**settings, "LD_PRELOAD": vm._preload()},
            never=never,
        )
        exits = {"fsl": p.code, "fsl-cr": q.code}
        stats = {"fsl": vm._class_lines(p.stdout), "fsl-cr": vm._class_lines(q.stdout)}
        for k, (variant, cr) in REPLICA_RUNS.items():
            env = dict(settings)
            if variant:
                env["LXNC_FAST_VARIANCE"] = variant
            if cr:
                env["LXNC_CRMATH"] = "1"
            r = rep.run_replica(
                rep.PROGRAM, args, cwd=dirs[k], settings=env, mode=defects.REPRODUCE, threads=1
            )
            exits[k] = r.code
            stats[k] = vm._class_lines(r.stdout)
        row: dict = {"args": " ".join(args), "exit": exits}
        for pair, (a, b) in PAIRS.items():
            row[pair] = vm.compare(dirs[a], dirs[b])
        if any(stats.values()):
            row["class_stats"] = stats
    (out_dir / (name.replace("/", "__") + ".json")).write_text(
        json.dumps(row, indent=1), encoding="utf-8"
    )
    print(f"== {name}: exit {exits}", flush=True)


def metrics(cmp: dict) -> dict:
    """``_seg`` and ``_mixeltype`` voxels that differ; the partial volumes' largest 99.9th
    percentile and largest difference."""
    out = {"seg": 0, "mix": 0, "pve_p999": 0.0, "pve_max": 0.0, "pve_vox": 0}
    for key, v in cmp.items():
        if not isinstance(v, dict):
            continue
        if key.endswith("_seg"):
            out["seg"] = v["voxels_differ"]
        elif key.endswith("_mixeltype"):
            out["mix"] = v["voxels_differ"]
        elif "_pve_" in key:
            out["pve_p999"] = max(out["pve_p999"], v["p999"])
            out["pve_max"] = max(out["pve_max"], v["max"])
            out["pve_vox"] = max(out["pve_vox"], v["voxels_differ"])
    return out


def summary(out_dir: Path) -> None:
    rows = {}
    for name in CASES:
        f = out_dir / (name.replace("/", "__") + ".json")
        if f.is_file():
            rows[name] = json.loads(f.read_text(encoding="utf-8"))
    closer = stable = 0
    for name, row in rows.items():
        print(f"== {name}  ({row['args']})  exits {row['exit']}")
        m = {pair: metrics(row[pair]) for pair in PAIRS}
        for pair in PAIRS:
            x = m[pair]
            print(
                f"   {pair:24s} seg {x['seg']:5d}  mix {x['mix']:5d}  "
                f"pve p99.9 {x['pve_p999']:.4f} max {x['pve_max']:.4f} vox {x['pve_vox']}"
            )
        fsl = [m[p] for p in PAIRS if p.startswith("fsl") and "ref" in p]
        dbl = [m[p] for p in PAIRS if p.startswith("double") and "ref" in p]

        def mean(xs: list[dict], k: str) -> float:
            return sum(x[k] for x in xs) / len(xs)

        fm, dm = mean(fsl, "seg") + mean(fsl, "mix"), mean(dbl, "seg") + mean(dbl, "mix")
        bf, bd = m["band fsl"], m["band double"]
        c = dm < fm
        s = bd["seg"] + bd["mix"] <= bf["seg"] + bf["mix"]
        closer += c
        stable += s
        print(
            f"   -> to the reference (mean of 4, seg+mix): FSL {fm:.1f}, corrected {dm:.1f}"
            f" ({'closer' if c else 'not closer'}); band seg+mix: FSL {bf['seg'] + bf['mix']},"
            f" corrected {bd['seg'] + bd['mix']} ({'no wider' if s else 'wider'})"
        )
        st = row.get("class_stats")
        if st:
            for k, v in st.items():
                print(f"   stats {k:9s} " + "  ".join(f"{a:.6g}/{b:.6g}" for a, b in v))
    print(f"closer on {closer} of {len(rows)}; band no wider on {stable} of {len(rows)}")


def main() -> None:
    cmd, out_dir = sys.argv[1], Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)
    if cmd == "run":
        for name in sys.argv[3:] or CASES:
            run_case(out_dir, name)
    else:
        summary(out_dir)


if __name__ == "__main__":
    main()
