# SPDX-License-Identifier: Apache-2.0
"""Where a run of the port's mri_robust_register spends its time: every line of its standard output
with the seconds since the start and since the line before (relay 49, the Powell costs' speed).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

Runs ``crates-freesurfer/target/release/larmorx-freesurfer freesurfer mri_robust_register <args>``
(built beforehand: ``real.py`` builds it) in ``--dir`` at one thread, and prints the lines whose gap
is at least ``--min`` seconds, then the total; ``--official`` runs FreeSurfer 8.2.0's program instead
(``$FREESURFER_HOME/bin``, ``FS_DISABLE_LANG=1``, no diagnostic level; ``--preload`` its
``LD_PRELOAD``). Run it in a memory box:

    scripts/orchestration/boxed.sh 8 python -P validation/probes/fs-robust-register/timeline.py \\
        --dir <folder> -- --mov <head>/T1.mgz --dst <head>/rawavg.mgz --lta reg.lta --cost LS
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))
CLI = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dir", type=Path, required=True)
    ap.add_argument("--min", type=float, default=0.5)
    ap.add_argument("--official", action="store_true")
    ap.add_argument("--preload", type=Path)
    ap.add_argument("args", nargs=argparse.REMAINDER)
    a = ap.parse_args(argv)
    args = a.args[1:] if a.args[:1] == ["--"] else a.args
    a.dir.mkdir(parents=True, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
    argv = [str(CLI), "freesurfer", "mri_robust_register", *args]
    if a.official:
        from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env
        from larmorx_validation.oracle_cache.store import workspace

        ws = workspace()
        assert ws is not None, "no workspace with oracles/"
        exe = ws / FS_HOME / "bin" / "mri_robust_register"
        env = {**env, "PATH": f"{exe.parent}:/usr/bin:/bin", "FS_DISABLE_LANG": "1",
               **freesurfer_env()}  # fmt: skip
        if a.preload:
            env["LD_PRELOAD"] = str(a.preload.resolve())
        if any(k.startswith("DIAG") for k in env) or any("gdiagno" in x for x in args):
            raise SystemExit("a diagnostic level would print the licence file")
        argv = [str(exe), *args]
    t0 = time.monotonic()
    last = t0
    p = subprocess.Popen(
        argv,
        cwd=a.dir,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        stdin=subprocess.DEVNULL,
    )
    assert p.stdout is not None
    for raw in p.stdout:
        now = time.monotonic()
        if now - last >= a.min:
            line = raw.decode("utf-8", "replace").rstrip("\n")
            print(f"{now - t0:8.1f} +{now - last:7.1f}  {line[:120]}", flush=True)
        last = now
    p.wait()
    print(f"total {time.monotonic() - t0:.1f} s, status {p.returncode}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
