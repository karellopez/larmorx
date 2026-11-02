# SPDX-License-Identifier: Apache-2.0
"""Runs the FLIRT replica (``larmorx-nc fsl flirt``) on ``fsl_runs.py``'s named runs, in both
D13 modes, next to FSL's outputs (``<out>/<name>/``), and prints for each: the exit statuses,
whether ``reproduce`` gives FSL's matrix and standard error (the replica's ``larmorx-nc:`` lines
left out), the defects named, and the default's matrix's RMS deviation (80 mm) from FSL's and
from a reference run's (``--against <name>``: ``quat`` against ``default12``, FSL-90).

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \\
        python -P validation/investigations/flirt-spec-answers/replica_runs.py <out> name ... \\
        [--against name]

Run ``fsl_runs.py <out> <name>`` first (FSL's run, from the oracle-output cache).
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt_replica as rep

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import fsl_runs  # noqa: E402


def _mat(p: Path) -> np.ndarray | None:
    return np.loadtxt(p) if p.is_file() else None


def _rms(a: np.ndarray | None, b: np.ndarray | None) -> str:
    if a is None or b is None:
        return "-"
    return f"{rep._rms80(a, b):.4g} mm"


def run(out: Path, name: str, against: str | None) -> None:
    args, _ = fsl_runs.RUNS[name]
    args = tuple(a[2:] if a.startswith("@@") else a for a in args)
    fsl_dir = out / name
    fsl_err = (fsl_dir / "stderr.txt").read_text(encoding="utf-8")
    fsl_mat = _mat(fsl_dir / "out.mat")
    res = {}
    for mode in (defects.REPRODUCE, defects.FIX):
        d = out / f"{name}.replica-{mode}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        for f in fsl_dir.iterdir():
            if f.suffix in (".nii", ".gz", ".mat", ".sch") and f.name != "out.mat":
                shutil.copy(f, d / f.name)
        program = fsl_runs.PROGRAM.get(name, "flirt")
        argv = rep._prepare(rep.Run(program, args), d)
        r = rep.run_replica(program, argv, cwd=d, settings={}, mode=mode)
        (d / "stdout.txt").write_text(r.stdout, encoding="utf-8")
        (d / "stderr.txt").write_text(r.stderr, encoding="utf-8")
        res[mode] = (r, _mat(d / "out.mat"))
    (rr, rmat), (fr, fmat) = res[defects.REPRODUCE], res[defects.FIX]
    same_err = rep._clean(rr.stderr, out, "flirt") == rep._clean(fsl_err, out, "flirt")
    same_mat = (rmat is None and fsl_mat is None) or (
        rmat is not None and fsl_mat is not None and np.array_equal(rmat, fsl_mat)
    )
    ref = _mat(out / against / "out.mat") if against else None
    print(
        f"{name}: reproduce exit {rr.code}, FSL's matrix {same_mat}, FSL's stderr {same_err}, "
        f"named {defects.notices_in(rr.stderr)}; fix exit {fr.code}, named "
        f"{defects.notices_in(fr.stderr)}, from FSL's {_rms(fmat, fsl_mat)}"
        + (
            f"; against {against}: FSL's {_rms(fsl_mat, ref)}, fix {_rms(fmat, ref)}"
            if against
            else ""
        )
    )
    for ln in fr.stderr.splitlines():
        if ln.startswith(rep.REPLICA_PREFIX):
            print("   fix:", ln[:300])


def main() -> None:
    argv = sys.argv[1:]
    against = None
    if "--against" in argv:
        i = argv.index("--against")
        against = argv[i + 1]
        del argv[i : i + 2]
    out = Path(argv[0])
    for name in argv[1:]:
        run(out, name, against)


if __name__ == "__main__":
    main()
