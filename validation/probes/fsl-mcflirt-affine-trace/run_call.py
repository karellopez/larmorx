# SPDX-License-Identifier: Apache-2.0
"""One of fMRIPrep's mcflirt calls (an oracle run's ``bold_hmc_wf/mcflirt/command.txt``), run
outside the container with FSL 6.0.7.17 at ``~/fsl``.

``compare``: mcflirt twice, mcflirt with the CORE-MATH preload and the replica
(``LARMORX_UPSTREAM_DEFECTS=reproduce``), then the ``.mat`` files of each against the first run.

``trace``: mcflirt with ``-report`` and an LD_PRELOAD interposer of this directory
(``libaff_trace.so``: every ``decompose_aff`` call; ``libcost_trace.so``: every cost
evaluation), its standard error in ``<out>/trace.txt``.

Usage::

    python -P run_call.py compare <run> <node substring> <out> [--bin larmorx-nc]
    python -P run_call.py trace <run> <node substring> <out> --preload libaff_trace.so

``<run>`` is a directory name under ``oracles/fmriprep-25.2.6/`` (for example
``ds002422-sub-02-noreconall``), the substring picks the call (for example ``task_rest_wf``).
The workspace is ``LARMORX_WORKSPACE`` or found from this file's location.
"""

from __future__ import annotations

import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
WS = Path(os.environ.get("LARMORX_WORKSPACE", HERE.parents[3]))
FSL = Path.home() / "fsl"
PRELOAD = WS / "oracles/core-math-preload/libcrmath.so"


def call_args(run: str, sub: str) -> list[str]:
    """The call's arguments with the container's mounts mapped to this machine's paths."""
    rdir = WS / "oracles/fmriprep-25.2.6" / run
    cmd = next(
        c for c in sorted(rdir.glob("work/**/bold_hmc_wf/mcflirt/command.txt")) if sub in str(c)
    )
    info = (rdir / "RUN_INFO.txt").read_text(encoding="utf-8")
    bids = next(ln.split(": ", 1)[1] for ln in info.splitlines() if ln.startswith("BIDS input"))
    argv = cmd.read_text(encoding="utf-8").split()[1:]
    mounts = {
        "/data/": bids + "/",
        "/out/": str(rdir / "out") + "/",
        "/work/": str(rdir / "work") + "/",
    }
    args = []
    for i, a in enumerate(argv):
        if i > 0 and argv[i - 1] == "-out":
            args.append(Path(a).name)
            continue
        for k, v in mounts.items():
            if a.startswith(k):
                a = v + a[len(k) :]
        args.append(a)
    return args


def fsl_env() -> dict[str, str]:
    """A clean environment: FSL is never on the PATH."""
    return {
        "HOME": str(Path.home()),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(FSL),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
    }


def compare(run: str, sub: str, out: Path, exe: Path) -> int:
    args = call_args(run, sub)
    env = fsl_env()
    runs = {
        "mcflirt": ([str(FSL / "bin/mcflirt"), *args], env),
        "mcflirt-2": ([str(FSL / "bin/mcflirt"), *args], env),
        "mcflirt-crmath": ([str(FSL / "bin/mcflirt"), *args], dict(env, LD_PRELOAD=str(PRELOAD))),
        "replica": (
            [str(exe), "fsl", "mcflirt", *args],
            dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce", OMP_NUM_THREADS="1"),
        ),
    }

    def go(item: tuple[str, tuple[list[str], dict[str, str]]]) -> tuple[str, int]:
        label, (a, e) = item
        d = out / label
        d.mkdir(parents=True, exist_ok=True)
        p = subprocess.run(a, cwd=d, env=e, capture_output=True, text=True, check=False)
        (d / "stderr.txt").write_text(p.stderr, encoding="utf-8")
        return label, p.returncode

    with ThreadPoolExecutor(max_workers=4) as ex:
        for label, code in ex.map(go, runs.items()):
            print(label, "exit", code)
    mats = {
        label: {p.name: p.read_bytes() for p in sorted((out / label).glob("*.mat/MAT_*"))}
        for label in runs
    }
    ref = mats["mcflirt"]
    for label in list(runs)[1:]:
        diff = [k for k in ref if mats[label].get(k) != ref[k]]
        print(f"{label} vs mcflirt: {len(diff)} of {len(ref)} MAT files differ; first {diff[:5]}")
    return 0


def trace(run: str, sub: str, out: Path, preload: Path) -> int:
    args = [*call_args(run, sub), "-report"]
    env = dict(fsl_env(), LD_PRELOAD=str(preload.resolve()))
    out.mkdir(parents=True, exist_ok=True)
    with open(out / "trace.txt", "w", encoding="utf-8") as f:
        p = subprocess.run(
            [str(FSL / "bin/mcflirt"), *args],
            cwd=out,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=f,
            check=False,
        )
    print("exit", p.returncode)
    return p.returncode


def main() -> int:
    if len(sys.argv) < 5 or sys.argv[1] not in ("compare", "trace"):
        print(__doc__)
        return 2
    mode, run, sub, out = sys.argv[1], sys.argv[2], sys.argv[3], Path(sys.argv[4])

    def opt(name: str, default: Path) -> Path:
        return Path(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default

    if mode == "compare":
        return compare(
            run, sub, out, opt("--bin", WS / "larmorx/crates-nc/target/release/larmorx-nc")
        )
    return trace(run, sub, out, opt("--preload", HERE / "libaff_trace.so"))


if __name__ == "__main__":
    sys.exit(main())
