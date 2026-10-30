# SPDX-License-Identifier: Apache-2.0
"""sweep.py <out dir> <n>...: the 2026-10-30 trial's cases run by the replica (FSL's bits), the
clean-room original and the replica with the original's ten arithmetic choices ("all"); for each
case, which of FSL / all differ from the original in stdout, images and meshes."""

import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import nibabel as nib
import numpy as np

S = Path(os.environ["BET_PROBES"])  # the probe builds (README)
W = Path(__file__).resolve().parents[3]
IMPLS = {
    "fsl": [str(S / "target/release/larmorx-nc"), "fsl"],
    "orig": [str(S / "target-main/release/larmorx"), "fsl"],
    "all": [
        str(S / extra) if (extra := os.environ.get("ALL", "target-all/release/larmorx-nc")) else "",
        "fsl",
    ],
}


def env():
    e = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "OPENBLAS_CORETYPE": "Haswell",
        "LARMORX_WORKSPACE": os.environ.get("LARMORX_WORKSPACE", str(W.parent)),
        "PYTHONPATH": f"{W}/python:{W}/validation/src",
        "HOME": os.environ.get("HOME", ""),
    }
    return e


def materialise(n, d):
    out = subprocess.run(
        [
            "python",
            "-P",
            str(W / "validation/investigations/cleanroom-bet/night_case.py"),
            "20261030",
            str(n),
            str(d),
        ],
        capture_output=True,
        text=True,
        env=env(),
        cwd=str(S),
    )
    line = out.stdout.strip().splitlines()[-1]
    words = line.split()
    ot = words[0].split("=", 1)[1]
    import shlex

    rest = shlex.split(line)[1:]
    return ot, rest[0], rest[1:]


def run(impl, tool, args, ot, cwd):
    cwd.mkdir(parents=True, exist_ok=True)
    e = env()
    e["FSLOUTPUTTYPE"] = ot
    r = subprocess.run(
        IMPLS[impl] + [tool] + args,
        capture_output=True,
        text=True,
        env=e,
        cwd=str(cwd),
        timeout=900,
    )
    (cwd.parent / f"{impl}.stdout").write_text(r.stdout)
    (cwd.parent / f"{impl}.stderr").write_text(r.stderr)
    return r.returncode, r.stdout


def files(d):
    return sorted(p.name for p in d.iterdir() if p.is_file())


def same_file(a, b):
    if a.name.endswith(".vtk"):
        return a.read_text() == b.read_text()
    if a.name.endswith((".img", ".img.gz")):
        return True  # the pair's header carries the data
    try:
        x = np.asanyarray(nib.load(str(a)).dataobj)
        y = np.asanyarray(nib.load(str(b)).dataobj)
        return x.shape == y.shape and np.array_equal(x, y, equal_nan=True)
    except Exception:
        return a.read_bytes() == b.read_bytes()


def case(n, root):
    d = root / str(n)
    try:
        ot, tool, args = materialise(n, d / "in")
    except Exception as ex:
        return n, f"materialise failed: {ex}"
    res = {}
    for impl in IMPLS:
        res[impl] = run(impl, tool, args, ot, d / impl)
    out = []
    for impl in ("fsl", "all"):
        diffs = []
        if res[impl][1] != res["orig"][1]:
            diffs.append("stdout")
        fa, fo = files(d / impl), files(d / "orig")
        if fa != fo:
            diffs.append("files")
        for f in set(fa) & set(fo):
            if not same_file(d / impl / f, d / "orig" / f):
                diffs.append(f)
        out.append(f"{impl}:{'=' if not diffs else ','.join(sorted(diffs))}")
    return n, f"{tool} {' '.join(args[2:]) if len(args) > 2 else ''} | " + "  ".join(out)


def main():
    root = Path(sys.argv[1])
    ns = [int(x) for x in sys.argv[2:]]
    with ThreadPoolExecutor(12) as pool:
        for n, msg in pool.map(lambda k: case(k, root), ns):
            print(f"#{n}: {msg}", flush=True)


if __name__ == "__main__":
    main()
