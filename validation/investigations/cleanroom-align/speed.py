# SPDX-License-Identifier: Apache-2.0
"""A short before/after timing of the paths this part changed: the head-motion correction's
`mutualinfo` and `normmi` costs and its `-sinc_final` and `-nn_final` (FSL-59), 3dTshift's
`-no_detrend` and a header with slice code 2, on ds000005's first BOLD run (64 x 64 x 34 x 240), at
one and at eight threads, each the best of `--repeat` runs (wall time). Two builds of the
`larmorx` command line are compared on the same inputs.

    python -P speed.py <work> --before <larmorx> --after <larmorx> [--repeat 3]
"""

from __future__ import annotations

import gzip
import os
import subprocess
import sys
import time
from pathlib import Path

HOME = Path.home()
DATA = HOME / ".cache" / "larmorx-testdata" / "data"
BOLD = DATA / "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"
REF = (
    DATA
    / "openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_boldref.nii.gz"
)


def code2_copy(work: Path) -> Path:
    """The BOLD run with header slice code 2 over every slice (dim_info slice axis 3)."""
    import numpy as np

    out = work / "bold_code2.nii"
    raw = bytearray(gzip.decompress(BOLD.read_bytes()))
    nz = int(np.frombuffer(bytes(raw[40:56]), "<i2")[3])
    raw[39] = 48  # dim_info: slice axis 3
    raw[122] = 2  # slice_code: sequential decreasing
    raw[74:76] = np.int16(0).tobytes()  # slice_start
    raw[120:122] = np.int16(nz - 1).tobytes()  # slice_end
    raw[132:136] = np.float32(2.0 / nz).tobytes()  # slice_duration (TR 2 s)
    out.write_bytes(bytes(raw))
    return out


def best(argv: list[str], env: dict[str, str], cwd: Path, repeat: int) -> float:
    times = []
    for _ in range(repeat):
        t = time.perf_counter()
        p = subprocess.run(argv, env=env, cwd=cwd, capture_output=True, text=True)
        times.append(time.perf_counter() - t)
        if p.returncode != 0:
            raise SystemExit(f"{argv}: exit {p.returncode}: {p.stderr[-400:]}")
    return min(times)


def main(argv: list[str]) -> int:
    work = Path(argv.pop(0)).resolve()
    opts = {"--before": "", "--after": "", "--repeat": "3"}
    while argv:
        k = argv.pop(0)
        opts[k] = argv.pop(0)
    work.mkdir(parents=True, exist_ok=True)
    code2 = code2_copy(work)
    hmc = ["mri", "hmc", "-in", str(BOLD), "-reffile", str(REF), "-out", "o", "-plots"]
    cases = {
        "hmc -cost normmi": [*hmc, "-cost", "normmi"],
        "hmc -cost mutualinfo": [*hmc, "-cost", "mutualinfo"],
        "hmc -nn_final": [*hmc, "-nn_final"],
        "hmc -sinc_final": [*hmc, "-sinc_final"],
        "3dTshift -no_detrend": [
            "afni",
            "3dTshift",
            "-linear",
            "-no_detrend",
            "-tpattern",
            "alt+z",
            "-prefix",
            "t.nii",
            str(BOLD),
        ],
        "3dTshift header code 2": ["afni", "3dTshift", "-prefix", "t2.nii", str(code2)],
    }
    print(f"{'case':26s} {'mode':9s} thr  before    after")
    for name, args in cases.items():
        modes = ("fix", "reproduce") if "final" in name or "code 2" in name else ("reproduce",)
        for mode in modes:
            for threads in ("1", "8"):
                env = {
                    "HOME": str(HOME),
                    "PATH": "/usr/bin:/bin",
                    "FSLOUTPUTTYPE": "NIFTI_GZ",
                    "OMP_NUM_THREADS": threads,
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                    "LARMORX_IMPLEMENTATION": "original",
                }
                row = []
                for who in ("--before", "--after"):
                    d = work / who.strip("-")
                    d.mkdir(exist_ok=True)
                    for f in d.iterdir():
                        if f.is_file():
                            f.unlink()
                    row.append(best([opts[who], *args], env, d, int(opts["--repeat"])))
                print(f"{name:26s} {mode:9s} {threads:>3s}  {row[0]:7.2f}  {row[1]:7.2f}")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("OPENBLAS_CORETYPE", "Haswell")
    sys.exit(main(sys.argv[1:]))
