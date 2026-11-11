# SPDX-License-Identifier: Apache-2.0
"""Speed of the ``dist`` profile against ``release``: the same calls with each build's extension.

The release files are built with the ``dist`` profile (fat LTO, one codegen unit, symbols
stripped); everyday builds use ``release``. This compares the two on real calls, at one thread and
at eight: fMRIPrep's antsRegistration of the BOLD reference to the field map's magnitude
(ds005003's ``fmapreg_wf/coregister``), its N4 of the T1w (ds000005's ``inu_n4``), its 3dvolreg
(RobustAverage, ds000005 run 1), larmorx's one-shot BOLD resampler (ds000005 run 1 onto its
boldref, with seeded motion and a field map) and sdcflows' B-spline fit (ds005454's field map).
Each build's outputs are hashed too: the two builds must give the same bytes.

Every measurement runs in a fresh process (``worker``) that imports ``larmorx`` from the build's
package tree; the builds alternate in each round (ABBA), so a drift of the machine falls on both.
Each worker runs every call once untimed (a warm-up), then times it ``--repeats`` times; a call
whose first run takes more than ``HEAVY`` seconds is timed by that run alone. The times are
medians over the rounds and repeats.

Usage (``scripts/orchestration/env.sh``'s variables set; inside a memory box, nothing else
running on the machine):

    python -P dist_speed.py run --tree release=DIR --tree dist=DIR [--threads 1,8]
        [--rounds 3] [--repeats 3] [--calls name,...] --out results.json

``DIR`` holds a ``larmorx/`` package (the repository's ``python/larmorx`` with that build's
``_core.abi3.so``). Prints a Markdown table: per call and thread count each build's median, the
ratio dist / release and whether the outputs are the same.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = HERE.parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or REPO.parent)
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
VOLREG_INPUT = (
    WORKSPACE / "oracles/inputs/afni-volreg-original/fmriprep/ds000005-run-01_sliced.nii.gz"
)
VOLREG = ["-Fourier", "-twopass", "-zpad", "4", "-prefix", "o.nii.gz", "-1Dfile", "o.1D"]
VOLREG += ["-1Dmatrix_save", "o.aff12.1D", "-maxdisp1D", "o_md.1D"]
BSPLINE_NODE = "ds005454-sub-16-noreconall/work/fmriprep_25_2_wf/sub_16_wf/fmap_preproc_wf/wf_gre16"
DS5 = "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"
BOLDREF = (
    "openneuro-derivatives/ds000005-fmriprep/sub-01/func/"
    "sub-01_task-mixedgamblestask_run-1_boldref.nii.gz"
)
#: The calls, in the order they run.
CALLS = ("ants-registration", "n4", "3dvolreg", "resample-series", "bspline-fit")
#: Calls faster than this (seconds, one thread) are timed --repeats times per worker.
HEAVY = 5.0


def _digest(paths: list[Path]) -> str:
    h = hashlib.sha256()
    for p in sorted(paths):
        b = p.read_bytes()
        h.update(p.name.encode() + b"\0" + (gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b))
    return h.hexdigest()


def _node(program: str, run: str, norm: str):  # type: ignore[no-untyped-def]
    from larmorx_validation.parity import fmriprep_nodes

    for c in fmriprep_nodes.calls(program):
        if c.run == run and c.norm == norm and not c.missing():
            return c
    raise SystemExit(f"no {program} call {run}:{norm} with all its inputs")


def _cli(program_path: list[str], args: list[str], env: dict[str, str]):  # type: ignore[no-untyped-def]
    """A function running ``larmorx <program_path> <args>`` in-process in a fresh directory."""
    import contextlib
    import io

    from larmorx.cli import run

    def call(work: Path) -> list[Path]:
        mapped = [re.sub(r"\{out:([^}]+)\}", lambda m: str(work / m[1]), a) for a in args]
        os.environ.update(env)
        here = Path.cwd()
        os.chdir(work)  # outputs named without a folder (``--output transform``) go there
        try:
            with (
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(io.StringIO()),
            ):
                code = run(["larmorx", *program_path, *mapped])
        finally:
            os.chdir(here)
        if code != 0:
            raise RuntimeError(f"larmorx {' '.join(program_path)} failed ({code})")
        return [p for p in work.iterdir() if p.is_file()]

    return call


def make_call(name: str, threads: int, tmp: Path):  # type: ignore[no-untyped-def]
    """The call ``name`` at ``threads``: a function of a fresh work directory returning its
    output files."""
    if name == "ants-registration":
        c = _node(
            "antsRegistration",
            "ds005003-sub-RS001-noreconall-syn",
            "bold_wf/bold_fit_wf/fmapreg_wf/coregister",
        )
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        return _cli(["ants", "antsRegistration"], list(c.args), env)
    if name == "n4":
        c = _node(
            "N4BiasFieldCorrection",
            "ds000005-sub-01-noreconall",
            "anat_fit_wf/brain_extraction_wf/inu_n4",
        )
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        return _cli(["ants", "N4BiasFieldCorrection"], list(c.args), env)
    if name == "3dvolreg":
        return _cli(
            ["afni", "3dvolreg"], [*VOLREG, str(VOLREG_INPUT)], {"OMP_NUM_THREADS": str(threads)}
        )
    if name == "resample-series":
        import larmorx_testdata as td
        import nibabel as nib
        import numpy as np
        from larmorx_validation.parity.resample_series import _fieldmap_file, _motion_file

        import larmorx as lx

        src_path = td.get(DS5)
        src = nib.load(src_path)
        data = np.asanyarray(src.dataobj)
        image = lx.Image(data, src.affine, lx.load(src_path, mmap=True).header)
        ref = td.get(BOLDREF)
        hmc = _motion_file(src_path, 7, tmp / "hmc.txt")
        fmap = nib.load(_fieldmap_file(Path(ref), 3, tmp / "fmap.nii.gz")).get_fdata(dtype="f4")

        def resample(work: Path) -> list[Path]:
            out = lx.transforms.resample_series(
                image, ref, [str(hmc)], fieldmap=fmap, pe_dir="j", ro_time=0.0312,
                jacobian=True, n_threads=threads,
            )  # fmt: skip
            path = work / "resampled.raw"
            path.write_bytes(np.ascontiguousarray(out.data).tobytes())
            return [path]

        return resample
    if name == "bspline-fit":
        import larmorx as lx

        base = RUNS / BSPLINE_NODE
        fmap = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))[0]
        mask = base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"

        def fit(work: Path) -> list[Path]:
            lx.sdcflows.bspline_approx(
                str(fmap), str(mask), out_dir=str(work), n_threads=threads,
                upstream_defects="reproduce",
            )  # fmt: skip
            return [p for p in work.iterdir() if p.is_file()]

        return fit
    raise SystemExit(f"unknown call {name}")


def worker(args: argparse.Namespace) -> int:
    import larmorx

    tree = Path(larmorx.__file__).resolve().parents[1]
    if tree != args.tree.resolve():
        raise SystemExit(f"larmorx came from {tree}, not {args.tree}")
    with tempfile.TemporaryDirectory(prefix="lx-dist-speed-") as name:
        tmp = Path(name)
        for call_name in args.calls.split(","):
            fn = make_call(call_name, args.threads, tmp)
            seconds, digest = [], None
            for k in range(1 + args.repeats):
                work = Path(tempfile.mkdtemp(dir=tmp))
                start = time.perf_counter()
                outputs = fn(work)
                elapsed = time.perf_counter() - start
                d = _digest(outputs)
                if digest not in (None, d):
                    raise SystemExit(f"{call_name}: outputs differ between runs of one build")
                digest = d
                if k > 0:
                    seconds.append(elapsed)
                if k == 0 and elapsed > HEAVY:  # heavy: the warm-up run is the timed one
                    seconds.append(elapsed)
                    break
            record = {"call": call_name, "threads": args.threads, "seconds": seconds}
            print(json.dumps({**record, "digest": digest}), flush=True)
    return 0


def run(args: argparse.Namespace) -> int:
    trees = dict(t.split("=", 1) for t in args.tree)
    names = list(trees)
    threads = [int(t) for t in args.threads.split(",")]
    results: list[dict] = []
    for r in range(args.rounds):
        order = names if r % 2 == 0 else names[::-1]
        for build in order:
            for n in threads:
                env = {
                    **os.environ,
                    "PYTHONPATH": os.pathsep.join([trees[build], str(REPO / "validation" / "src")]),
                    "OPENBLAS_NUM_THREADS": "1",
                }
                cmd = [sys.executable, "-P", str(HERE), "worker", "--tree", trees[build]]
                cmd += ["--threads", str(n), "--calls", args.calls, "--repeats", str(args.repeats)]
                out = subprocess.run(
                    cmd, env=env, check=True, capture_output=True, text=True, encoding="utf-8"
                ).stdout
                for line in out.splitlines():
                    if line.startswith("{"):
                        results.append({**json.loads(line), "build": build, "round": r})
                print(f"round {r + 1}/{args.rounds} {build} {n} threads done", file=sys.stderr)
    Path(args.out).write_text(json.dumps(results, indent=1), encoding="utf-8")
    print("| Call | Threads | " + " | ".join(names) + " | dist / release | Same outputs |")
    print("|---|---|" + "---|" * len(names) + "---|---|")
    for call in args.calls.split(","):
        for n in threads:
            rows = {
                b: [x for x in results if (x["call"], x["threads"], x["build"]) == (call, n, b)]
                for b in names
            }
            med = {b: statistics.median([s for x in rows[b] for s in x["seconds"]]) for b in names}
            digests = {x["digest"] for b in names for x in rows[b]}
            ratio = (
                med["dist"] / med["release"] if {"dist", "release"} <= set(med) else float("nan")
            )
            cells = " | ".join(f"{med[b]:.3f} s" for b in names)
            print(
                f"| {call} | {n} | {cells} | {ratio:.3f} | {'yes' if len(digests) == 1 else 'NO'} |"
            )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="mode", required=True)
    w = sub.add_parser("worker")
    w.add_argument("--tree", type=Path, required=True)
    w.add_argument("--threads", type=int, required=True)
    w.add_argument("--calls", default=",".join(CALLS))
    w.add_argument("--repeats", type=int, default=3)
    r = sub.add_parser("run")
    r.add_argument("--tree", action="append", required=True, help="name=DIR (release, dist)")
    r.add_argument("--threads", default="1,8")
    r.add_argument("--rounds", type=int, default=3)
    r.add_argument("--repeats", type=int, default=3)
    r.add_argument("--calls", default=",".join(CALLS))
    r.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    return worker(args) if args.mode == "worker" else run(args)


if __name__ == "__main__":
    sys.exit(main())
