# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's speed: AFNI 26.3.05's program against larmorx's replica (the standalone
``larmorx afni 3dvolreg`` binary, ``LARMORX_IMPLEMENTATION=replica``, and the Python entry point
``python -m larmorx afni 3dvolreg``) on fMRIPrep's calls (niworkflows' RobustAverage:
``-Fourier -twopass -zpad 4 -1Dfile -1Dmatrix_save -maxdisp1D``) on the parity suite's inputs
(``oracles/inputs/afni-volreg/fmriprep/``, made by ``python -m larmorx_validation parity
afni-volreg --implementation replica``). One thread each (3dvolreg has no OpenMP loop; the
replica is single-threaded on this path too); each call ``--repeats`` times, the median wall
time; the outputs checked byte for byte (``reproduce``) on the first run.

Usage: speed.py [--repeats 3] [--labels ds000005-run-01 ...] [--json out.json]
Environment: LARMORX_WORKSPACE (default: the repository's parent), LARMORX_BIN (default
``target/release/larmorx``).
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
WS = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = WS / "oracles/afni-26.3.05/bin/3dvolreg"
LARMORX = Path(os.environ.get("LARMORX_BIN", REPO / "target/release/larmorx"))
INPUTS = WS / "oracles/inputs/afni-volreg/fmriprep"
LABELS = ("ds000005-run-01", "ds003592-echo-1", "ds005454", "ds004958", "pepolar-ds005040")
CALL = ["-Fourier", "-twopass", "-zpad", "4", "-prefix", "o.nii.gz", "-1Dfile", "o.1D",
        "-1Dmatrix_save", "o.aff12.1D", "-maxdisp1D", "o_md.1D"]  # fmt: skip


def run(cmd: list[str], cwd: Path, env: dict[str, str]) -> float:
    for f in cwd.iterdir():
        f.unlink()
    t0 = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True)
    dt = time.perf_counter() - t0
    if p.returncode != 0:
        raise SystemExit(f"{cmd[0]} failed: {p.stderr.decode(errors='replace')[-500:]}")
    return dt


def outputs(d: Path) -> dict[str, bytes]:
    import gzip

    out = {}
    for f in sorted(d.iterdir()):
        raw = f.read_bytes()
        if f.name.endswith(".gz"):
            raw = gzip.decompress(raw)
        if f.name.endswith("_md.1D") or f.name.endswith("_md_delt.1D"):
            raw = b"\n".join(x for x in raw.split(b"\n") if not x.startswith(b"# 3dvolreg"))
        out[f.name] = raw
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--labels", nargs="+", default=list(LABELS))
    ap.add_argument("--json")
    a = ap.parse_args()
    base = {
        "PATH": "/usr/bin:/bin",
        "OMP_NUM_THREADS": "1",
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_NOEXT": "YES",
        "LARMORX_IMPLEMENTATION": "replica",
        "LARMORX_UPSTREAM_DEFECTS": "reproduce",
    }
    rows = []
    with tempfile.TemporaryDirectory(prefix="lx-volreg-speed_") as t:
        home = Path(t) / "home"
        home.mkdir()
        env = dict(base, HOME=str(home))
        py_env = dict(env, PYTHONPATH=f"{REPO / 'python'}")
        for label in a.labels:
            src = INPUTS / f"{label}_sliced.nii.gz"
            if not src.is_file():
                print(f"{label}: {src} missing (run the parity suite first)", file=sys.stderr)
                continue
            import nibabel as nib

            shape = nib.load(src).shape
            times: dict[str, list[float]] = {}
            dirs = {}
            for name, cmd, e in (
                ("AFNI", [str(AFNI)], env),
                ("larmorx", [str(LARMORX), "afni", "3dvolreg"], env),
                (
                    "python -m larmorx",
                    [sys.executable, "-m", "larmorx", "afni", "3dvolreg"],
                    py_env,
                ),
            ):
                d = Path(t) / f"{label}-{len(dirs)}"
                d.mkdir()
                dirs[name] = d
                times[name] = [run([*cmd, *CALL, str(src)], d, e) for _ in range(a.repeats)]
            ref = outputs(dirs["AFNI"])
            same = {n: outputs(d) == ref for n, d in dirs.items() if n != "AFNI"}
            med = {n: statistics.median(v) for n, v in times.items()}
            rows.append({"label": label, "shape": list(shape), "seconds": med, "identical": same,
                         "runs": times})  # fmt: skip
            print(f"{label} {shape}: AFNI {med['AFNI']:.2f} s, larmorx {med['larmorx']:.2f} s "
                  f"({med['AFNI'] / med['larmorx']:.1f}x), python -m larmorx "
                  f"{med['python -m larmorx']:.2f} s; outputs identical: {same}")  # fmt: skip
            for d in dirs.values():
                shutil.rmtree(d)
    if a.json:
        Path(a.json).write_text(json.dumps(rows, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
