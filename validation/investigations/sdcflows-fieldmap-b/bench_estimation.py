# SPDX-License-Identifier: Apache-2.0
"""Speed of the field maps' part B (``lx.sdcflows``' masks, ``lx.niworkflows``,
``lx.transforms.reconstruct_fieldmap_node``, ``larmorx sdcflows sdcflows``) against the
originals (sdcflows 2.17.1, niworkflows 1.15.4, fMRIPrep 25.2.7, in-process).

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \\
    PYTHONPATH=<repo>/python:<repo>/validation/src LARMORX_WORKSPACE=<workspace> \\
        python -P bench_estimation.py calls [--repeats 3] [--threads 1,14] [--out calls.json]
    ... python -P bench_estimation.py masker-nan [--out masker-nan.json]
    ... LARMORX_NC_BIN=<larmorx-nc> python -P bench_estimation.py app --datasets <name> ...
        [--scratch <folder>] [--out app.json]

Run in the newest Python environment (``oracles/python-newest``: numpy 2.5.4, SciPy 1.18.1,
scikit-image 0.26.0), inside a memory box, on a quiet machine. Three parts:

- ``calls``: the four calls of ``tests/parity/test_memory_peak.py``'s
  ``test_fieldmap_steps_need_no_more_memory_than_the_originals`` on the oracle runs' files
  (``memory_call.py``'s calls, in this process): sdcflows' ``brain_masker`` on ds002603's
  clipped magnitude, niworkflows' ``IntensityClip`` (``_advanced_clip``) on its merged
  magnitude, ``RobustAverage``'s own steps (``mc_method=None``) on ds004958's PEPOLAR series,
  fMRIPrep's ``ReconstructFieldmap`` onto ds005454's T1w grid. The originals have no thread
  option (numpy's and SciPy's BLAS at one thread, as fMRIPrep's nodes run); larmorx at each
  ``--threads`` (``upstream_defects="reproduce"``). Each time is the median of ``--repeats``
  runs after one untimed run of each implementation; every larmorx file is checked byte for
  byte (decompressed) against the original's.
- ``masker-nan``: ``brain_masker`` on random testing's case ``sdcflows-estimation:fmapb-try1:492``
  (an image whose grey closing is 0 everywhere: the random walker's system is NaN, SciPy's
  conjugate gradients run to ``maxiter``, 2 x 162,120 iterations), sdcflows against larmorx at
  one thread, once each (the files compared as above).
- ``app``: sdcflows' own BIDS app against ``larmorx sdcflows sdcflows`` on chosen sdcflows 2.17.1
  runs (``check_app.py --sections run``: one thread, ``--no-fmapless``; sdcflows' programs
  (FSL's ``topup`` and ``prelude``, AFNI's ``3dvolreg``, ANTs' N4 with correctly rounded maths)
  run fresh (``LARMORX_ORACLE_CACHE_MODE=off``), larmorx with its replicas; every derivative
  file compared): the wall time of each run as ``check_app.py`` prints it.

Prints one line per measurement and writes the JSON the benchmark report is made from
(``docs/benchmarks/sdcflows-estimation.md``).
"""

from __future__ import annotations

import argparse
import contextlib
import gzip
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or HERE.parents[4])
SDC_RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
DATA = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
sys.path.insert(0, str(HERE))

import memory_call as mc  # noqa: E402


def raw(path: Path) -> bytes:
    b = path.read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def stored(folder: Path) -> dict[str, bytes]:
    """Every file the call wrote (decompressed; a NIfTI file's four extension bytes after its
    header aside, as ``test_memory_peak``'s ``_stored``)."""
    out = {}
    for f in sorted(folder.iterdir()):
        b = raw(f)
        out[f.name] = b[:348] + b[352:] if f.name.endswith((".nii", ".nii.gz")) else b
    return out


def call_inputs(call: str) -> tuple[list[str], str]:
    """The call's files (``test_memory_peak``'s ``_fieldmap_b_inputs``) and ``--inverse``."""
    mag = (
        SDC_RUNS / "ds002603-sub-01-noreconall" / "work" / "fmriprep_25_2_wf" / "sub_01_wf"
        / "fmap_preproc_wf" / "wf_auto_00000" / "magnitude_wf"
    )  # fmt: skip
    r54 = SDC_RUNS / "ds005454-sub-16-noreconall"
    inverse = ""
    if call == "masker":
        paths = [mag / "brainextraction_wf" / "clipper_post" / "clipped.nii.gz"]
    elif call == "clip":
        paths = [mag / "magmrg" / "sub-01_acq-Mm2_magnitude1_merged.nii.gz"]
    elif call == "ravg":
        paths = [
            DATA / "ds004958" / "sub-fsm64ps" / "fmap" / "sub-fsm64ps_acq-se_dir-AP_epi.nii.gz"
        ]
    else:
        fmap, func = r54 / "out" / "sub-16" / "fmap", r54 / "out" / "sub-16" / "func"
        paths = [
            fmap / "sub-16_fmapid-gre16_desc-coeff_fieldmap.nii.gz",
            fmap / "sub-16_fmapid-gre16_desc-magnitude_fieldmap.nii.gz",
            r54 / "work" / "fmriprep_25_2_wf" / "sub_16_wf" / "bold_task_rest_wf" / "bold_anat_wf"
            / "gen_ref" / "sub-16_desc-preproc_T1w_reference.nii.gz",
            func / "sub-16_task-rest_from-boldref_to-gre16_mode-image_desc-fmap_xfm.txt",
            func / "sub-16_task-rest_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt",
        ]  # fmt: skip
        inverse = "true,false"
    missing = [str(p) for p in paths if not p.is_file()]
    if missing:
        raise SystemExit(f"{call}: missing {missing}")
    extra = ["35.0", "99.98"] if call == "clip" else []
    return [str(p) for p in paths] + extra, inverse


def run_in(folder: Path, fn) -> float:
    """``fn()`` with ``folder`` (emptied) as the working directory; its wall time."""
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    old = os.getcwd()
    os.chdir(folder)
    try:
        t0 = time.perf_counter()
        fn()
        return time.perf_counter() - t0
    finally:
        os.chdir(old)


def bench_calls(repeats: int, threads: list[int], scratch: Path) -> list[dict]:
    rows = []
    for name, call in (
        ("masker-ds002603", "masker"),
        ("clip-ds002603", "clip"),
        ("ravg-ds004958", "ravg"),
        ("recon-ds005454", "recon"),
    ):
        inputs, inverse = call_inputs(call)
        orig = SimpleNamespace(call=call, inputs=inputs, inverse=inverse)
        o_dir = scratch / f"{name}-original"
        with contextlib.redirect_stdout(sys.stderr):
            run_in(o_dir, lambda o=orig: mc._original(o))  # imports, caches
            times = [run_in(o_dir, lambda o=orig: mc._original(o)) for _ in range(repeats)]
        row = {"call": name, "original": statistics.median(times), "original_runs": times}
        want = stored(o_dir)
        for n in threads:
            mine = SimpleNamespace(
                call=call, inputs=inputs, inverse=inverse, threads=n, defects="reproduce"
            )
            l_dir = scratch / f"{name}-larmorx-{n}"
            run_in(l_dir, lambda m=mine: mc._larmorx(m))
            lt = [run_in(l_dir, lambda m=mine: mc._larmorx(m)) for _ in range(repeats)]
            got = stored(l_dir)
            row[f"larmorx_{n}"] = statistics.median(lt)
            row[f"larmorx_{n}_runs"] = lt
            row[f"identical_{n}"] = bool(want) and got == want
        rows.append(row)
        print(
            f"{name}: original {row['original']:.2f} s; "
            + "; ".join(
                f"larmorx ({n}) {row[f'larmorx_{n}']:.2f} s "
                f"({row['original'] / row[f'larmorx_{n}']:.2f}x), identical {row[f'identical_{n}']}"
                for n in threads
            ),
            flush=True,
        )
    return rows


def bench_masker_nan(scratch: Path) -> dict:
    import numpy as np
    from larmorx_validation.difftest import core, inputs, tools
    from sdcflows.utils.tools import brain_masker

    import larmorx as lx

    r = core.make_recipe(tools.get("sdcflows-estimation"), "fmapb-try1", 492)
    src = str(inputs.path(inputs.key(inputs.register(r["inputs"]), "in")))
    o_dir, l_dir = scratch / "masker-nan-original", scratch / "masker-nan-larmorx"
    t_o = run_in(o_dir, lambda: brain_masker(src, str(o_dir / "c_mask.nii.gz")))

    def mine() -> None:
        import warnings

        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            lx.sdcflows.brain_masker(
                src,
                str(l_dir / "c_mask.nii.gz"),
                numpy_version=np.__version__,
                upstream_defects="reproduce",
                n_threads=1,
            )

    t_l = run_in(l_dir, mine)
    row = {
        "case": "sdcflows-estimation:fmapb-try1:492",
        "original": t_o,
        "larmorx_1": t_l,
        "identical_1": stored(o_dir) == stored(l_dir) and bool(stored(o_dir)),
        "build": hashlib_of(Path(lx._core.__file__)),
    }
    print(
        f"masker-nan: sdcflows {t_o:.2f} s; larmorx (1) {t_l:.3f} s, identical "
        f"{row['identical_1']}",
        flush=True,
    )
    return row


def hashlib_of(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def bench_app(datasets: list[str], scratch: Path) -> list[dict]:
    rows = []
    for name in datasets:
        env = dict(os.environ)
        env["LARMORX_ORACLE_CACHE_MODE"] = "off"
        p = subprocess.run(
            [
                sys.executable,
                "-P",
                str(HERE / "check_app.py"),
                "--sections",
                "run",
                "--datasets",
                name,
                "--scratch",
                str(scratch / f"app-{name}"),
            ],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        times = dict(
            re.findall(r"^INFO \S+ (sdcflows|larmorx): (\d+) s, exit 0$", p.stdout, flags=re.M)
        )
        row = {
            "dataset": name,
            "sdcflows": float(times["sdcflows"]) if "sdcflows" in times else None,
            "larmorx_1": float(times["larmorx"]) if "larmorx" in times else None,
            "identical": p.returncode == 0 and "ALL OK" in p.stdout,
            "checks": [ln for ln in p.stdout.splitlines() if re.match(r"^\S.*: \d+ cases", ln)],
        }
        rows.append(row)
        print(
            f"app {name}: sdcflows {row['sdcflows']} s; larmorx {row['larmorx_1']} s; "
            f"every file identical {row['identical']}",
            flush=True,
        )
        if not row["identical"]:
            print(p.stdout[-3000:], p.stderr[-3000:], sep="\n", file=sys.stderr)
    return rows


def machine() -> dict:
    import numpy
    import scipy

    load = os.getloadavg()
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpus": os.cpu_count(),
        "load_average_at_start": [round(x, 2) for x in load],
        "numpy": numpy.__version__,
        "scipy": scipy.__version__,
        "OPENBLAS_CORETYPE": os.environ.get("OPENBLAS_CORETYPE", ""),
        "OPENBLAS_NUM_THREADS": os.environ.get("OPENBLAS_NUM_THREADS", ""),
        "commit": subprocess.run(
            ["git", "-C", str(REPO), "describe", "--always", "--dirty", "--abbrev=12"],
            capture_output=True,
            text=True,
            check=False,
        ).stdout.strip(),
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("part", choices=("calls", "masker-nan", "app"))
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--threads", default="1,14")
    p.add_argument("--datasets", nargs="*", default=["ds005752-sub-ON01016-direct"])
    p.add_argument("--scratch", default=None)
    p.add_argument("--out", default=None)
    args = p.parse_args()
    out: dict = {"machine": machine(), "part": args.part}
    with tempfile.TemporaryDirectory(prefix="lx-bench-estimation-") as t:
        scratch = Path(args.scratch) if args.scratch else Path(t)
        scratch.mkdir(parents=True, exist_ok=True)
        if args.part == "calls":
            out["repeats"] = args.repeats
            out["rows"] = bench_calls(
                args.repeats, [int(x) for x in args.threads.split(",")], scratch
            )
        elif args.part == "masker-nan":
            out["rows"] = [bench_masker_nan(scratch)]
        else:
            out["rows"] = bench_app(args.datasets, scratch)
    if args.out:
        Path(args.out).write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    ok = all(v for r in out["rows"] for k, v in r.items() if k.startswith("identical"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
