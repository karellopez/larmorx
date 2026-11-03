# SPDX-License-Identifier: Apache-2.0
"""One fslmaths case end to end with the FFT's twiddle factors swapped: FSL's output (through the
oracle-output cache) against the replica's as built (correctly rounded factors) and against the
replica built with the probe feature ``probe-twiddles`` and handed Armadillo's own factors (the
file ``fft_twiddles.cc`` writes, through ``LARMORX_PROBE_FFT_TWIDDLES``). If only the factors
differ, the second replica's output is FSL's bit for bit.

    python -P validation/probes/fsl-avwutils-lib/check_fft_case.py \\
        --probe-bin <target>/release/larmorx-nc --twiddles <work>/arma_twiddles.txt \\
        [--case fft-twiddles-glibc-sincos]

The case is a permanent case of random testing (``validation/difftest/cases/fsl-maths/``). Run
inside a memory box (``scripts/orchestration/boxed.sh``). See README.md.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import permanent
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths_replica as fm
from larmorx_validation.parity.fsl_flirt_replica import _environment


def _data(d: Path) -> dict[str, bytes]:
    out = {}
    for f in sorted(d.glob("out*")):
        if f.name.endswith((".nii.gz", ".nii", ".hdr")):
            img = nib.load(str(f))
            out[f.name] = img.header.binaryblock + np.asarray(img.dataobj).tobytes()
    return out


def _replica(exe: str, args: list[str], cwd: Path, settings: dict, extra: dict) -> None:
    env = {**_environment(settings), defects.ENV: defects.REPRODUCE, **extra}
    subprocess.run([exe, "fsl", fm.PROGRAM, *args], cwd=cwd, env=env, check=True)


def _differ(a: dict[str, bytes], b: dict[str, bytes]) -> str:
    if a.keys() != b.keys():
        return f"files {sorted(a)} vs {sorted(b)}"
    bad = [k for k in a if a[k] != b[k]]
    return "identical" if not bad else f"differ: {', '.join(bad)}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--probe-bin", required=True, help="larmorx-nc built with probe-twiddles")
    ap.add_argument("--twiddles", required=True, type=Path, help="fft_twiddles.cc's output")
    ap.add_argument("--case", default="fft-twiddles-glibc-sincos")
    a = ap.parse_args()
    cases = {c.id: c for c in permanent.cases("fsl-maths", "full")}
    run = cases[f"difftest/{a.case}"].payload[1]
    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-fft-case-") as t:
        t = Path(t)
        dirs = {k: t / k for k in ("fsl", "built", "probe", "probe-own")}
        for d in dirs.values():
            d.mkdir()
        args = fm._prepare(run, dirs["fsl"])
        for k in ("built", "probe", "probe-own"):
            fm._prepare(run, dirs[k])
        oracle = fm.run_oracle(args, cwd=dirs["fsl"], settings=settings)
        built = fm.replica_binary()
        assert built is not None, "the larmorx-nc program is not built"
        _replica(str(built), args, dirs["built"], settings, {})
        _replica(
            a.probe_bin,
            args,
            dirs["probe"],
            settings,
            {"LARMORX_PROBE_FFT_TWIDDLES": str(a.twiddles.resolve())},
        )
        _replica(a.probe_bin, args, dirs["probe-own"], settings, {})
        fsl = _data(dirs["fsl"])
        print(f"case {a.case}: fslmaths {' '.join(args)} (FSL exit {oracle.code})")
        print(f"  replica as built: {_differ(fsl, _data(dirs['built']))}")
        print(f"  probe build, its own factors: {_differ(fsl, _data(dirs['probe-own']))}")
        same = _differ(fsl, _data(dirs["probe"]))
        print(f"  probe build, Armadillo's factors: {same}")
    return 0 if same == "identical" else 1


if __name__ == "__main__":
    sys.exit(main())
