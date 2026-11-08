# SPDX-License-Identifier: Apache-2.0
"""Speed of ``lx.sdcflows`` against sdcflows' Python on fMRIPrep's calls (field maps part A).

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python python -P bench_sdcflows.py \\
        [--repeats 3] [--volumes 60] [--threads 1,14] [--out results.json]

Run in the newest Python environment (``oracles/python-newest``: sdcflows 2.17.1, scikit-learn
1.9.1, SciPy 1.18.1, numpy 2.5.4), inside a memory box. Two kinds of calls:

- ``BSplineApprox`` as fMRIPrep's seven ``bs_filter`` nodes of the oracle runs call it (their
  field maps and masks), sdcflows' interface (nipype, a temporary directory) against
  ``lx.sdcflows.bspline_approx`` with the same files written, larmorx at each thread count
  (BSplineApprox has no thread option: numpy's and SciPy's BLAS at one thread,
  ``OPENBLAS_NUM_THREADS=1``, the way fMRIPrep's nodes ran);
- ``ApplyCoeffsField`` on ds005454's coefficients, registration and the first ``--volumes``
  BOLD volumes (160 x 160 x 96, int16; the whole 280-volume series would need about 50 GiB in
  sdcflows: 3.7 GiB for 20 volumes, ``tests/parity/test_memory_peak.py``), sdcflows'
  ``num_threads`` and larmorx's ``n_threads`` at each thread count.

Each call's wall time is the median of ``--repeats`` runs (after one untimed warm-up of
larmorx); every larmorx output is checked bit-identical to sdcflows' (``upstream_defects=
"reproduce"``; decompressed files). Prints one line per call and writes the JSON the benchmark
report is made from (``docs/benchmarks/sdcflows-fieldmap.md``).
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import nibabel as nb
import numpy as np

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
BIDS = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
W = "work/fmriprep_25_2_wf"
FITS = [
    ("ds005454-sub-16", "sub_16_wf/fmap_preproc_wf/wf_gre16"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds002422-sub-02", "sub_02_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00000"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00001"),
    ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00002"),
    ("ds002603-sub-01", "sub_01_wf/fmap_preproc_wf/wf_auto_00000"),
]


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def timed(fn, repeats: int) -> tuple[float, list[float], object]:
    times, out = [], None
    for _ in range(repeats):
        t0 = time.perf_counter()
        out = fn()
        times.append(time.perf_counter() - t0)
    return statistics.median(times), times, out


def _quiet() -> None:
    import logging

    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.ERROR)


def bench_fit(run: str, node: str, threads: list[int], repeats: int) -> dict | None:
    from sdcflows.interfaces.bspline import BSplineApprox

    import larmorx as lx

    _quiet()
    base = R / f"{run}-noreconall" / W / node
    fmaps = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))
    mask = base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
    if not fmaps or not mask.is_file():
        return None
    shape = nb.load(fmaps[0]).shape[:3]
    out = {"call": "BSplineApprox", "case": f"{run}/{node.rsplit('/', 1)[-1]}", "shape": shape}
    with tempfile.TemporaryDirectory() as t:

        def ref():
            d = tempfile.mkdtemp(dir=t)
            BSplineApprox(in_data=str(fmaps[0]), in_mask=str(mask)).run(cwd=d)
            return d

        out["sdcflows_s"], out["sdcflows_runs"], ref_dir = timed(ref, repeats)
        for n in threads:

            def mine(n=n):
                d = tempfile.mkdtemp(dir=t)
                lx.sdcflows.bspline_approx(
                    str(fmaps[0]), str(mask), out_dir=d, n_threads=n, upstream_defects="reproduce"
                )
                return d

            mine()  # warm-up
            out[f"larmorx_{n}_s"], out[f"larmorx_{n}_runs"], my_dir = timed(mine, repeats)
            names = sorted(p.name for p in Path(ref_dir).glob("*.nii.gz"))
            out[f"identical_{n}"] = names == sorted(
                p.name for p in Path(my_dir).glob("*.nii.gz")
            ) and all(raw(Path(ref_dir) / k) == raw(Path(my_dir) / k) for k in names)
    return out


def bench_apply(volumes: int, threads: list[int], repeats: int) -> dict | None:
    from sdcflows.interfaces.bspline import ApplyCoeffsField

    import larmorx as lx

    _quiet()
    w = R / "ds005454-sub-16-noreconall" / W / "sub_16_wf"
    coeffs = sorted(w.glob("fmap_preproc_wf/wf_gre16/bs_filter/*_coeff000.nii.gz"))
    xfm = w / "bold_task_rest_wf/bold_fit_wf/fmapreg_wf/coregister/transform0GenericAffine.mat"
    bold = BIDS / "ds005454" / "sub-16" / "func" / "sub-16_task-rest_bold.nii.gz"
    if not coeffs or not xfm.is_file() or not bold.is_file():
        return None
    out = {"call": "ApplyCoeffsField", "case": f"ds005454-sub-16, {volumes} volumes"}
    with tempfile.TemporaryDirectory() as t:
        img = nb.load(bold)
        head = Path(t) / "sub-16_task-rest_bold.nii.gz"
        sub = img.slicer[..., :volumes]
        nb.Nifti1Image(np.asanyarray(sub.dataobj), img.affine, img.header).to_filename(head)
        out["shape"] = nb.load(head).shape
        for n in threads:

            def ref(n=n):
                d = tempfile.mkdtemp(dir=t)
                ApplyCoeffsField(
                    in_data=str(head),
                    in_coeff=[str(coeffs[0])],
                    fmap2data_xfm=str(xfm),
                    ro_time=[0.0429303],
                    pe_dir=["j-"],
                    num_threads=n,
                ).run(cwd=d)
                return d

            def mine(n=n):
                d = tempfile.mkdtemp(dir=t)
                lx.sdcflows.apply_coeffs_field(
                    str(head), [str(coeffs[0])], [0.0429303], ["j-"], fmap2data_xfm=str(xfm),
                    out_dir=d, n_threads=n, upstream_defects="reproduce",
                )  # fmt: skip
                return d

            out[f"sdcflows_{n}_s"], out[f"sdcflows_{n}_runs"], ref_dir = timed(ref, repeats)
            mine()  # warm-up
            out[f"larmorx_{n}_s"], out[f"larmorx_{n}_runs"], my_dir = timed(mine, repeats)
            names = sorted(p.name for p in Path(ref_dir).glob("*.nii.gz"))
            out[f"identical_{n}"] = all(
                raw(Path(ref_dir) / k) == raw(Path(my_dir) / k) for k in names
            )
            for d in (ref_dir, my_dir):
                for p in Path(d).glob("*"):
                    p.unlink()
    return out


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--volumes", type=int, default=60)
    p.add_argument("--threads", default="1,14")
    p.add_argument("--out", default=None)
    p.add_argument("--skip-apply", action="store_true")
    args = p.parse_args()
    threads = [int(x) for x in args.threads.split(",")]
    import importlib.metadata as md

    commit = subprocess.run(
        ["git", "-C", str(Path(__file__).parent), "describe", "--always", "--dirty"],
        capture_output=True, text=True, check=False,
    ).stdout.strip()  # fmt: skip
    env = {
        "commit": commit,
        "python": sys.version.split()[0],
        "machine": platform.machine(),
        "cpus": os.cpu_count(),
        "releases": {
            n: md.version(n) for n in ("sdcflows", "scikit-learn", "scipy", "numpy", "nibabel")
        },
        "OPENBLAS_CORETYPE": os.environ.get("OPENBLAS_CORETYPE", ""),
        "OPENBLAS_NUM_THREADS": os.environ.get("OPENBLAS_NUM_THREADS", ""),
    }
    results = []
    for run, node in FITS:
        r = bench_fit(run, node, threads, args.repeats)
        if r is None:
            print(f"skipped (no oracle run here): {run} {node}")
            continue
        results.append(r)
        print(json.dumps({k: v for k, v in r.items() if not k.endswith("_runs")}), flush=True)
    if not args.skip_apply:
        r = bench_apply(args.volumes, threads, args.repeats)
        if r is not None:
            results.append(r)
            print(json.dumps({k: v for k, v in r.items() if not k.endswith("_runs")}), flush=True)
    if args.out:
        Path(args.out).write_text(json.dumps({"environment": env, "results": results}, indent=1))
    print("BENCH FINISHED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
