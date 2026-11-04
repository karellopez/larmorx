# SPDX-License-Identifier: Apache-2.0
"""One fslmaths (or fslstats) command line on files already in ``<dir>``: FSL 6.0.7.23 through
the oracle cache (``use`` mode) and the original (``<larmorx>``, ``reproduce`` mode), each in a
copy of ``<dir>``; their exit status, streams and the output image's values side by side (the
first differences), the outputs kept in ``<dir>/fsl_out`` and ``<dir>/orig_out``. For bisecting a random-testing finding's chain step by step (findings 24 to
30 of ``docs/findings/fsl-maths-original.md``; ``show_finding.py`` writes a finding's inputs).

Run (in a memory box): ``python -P compare_call.py <dir> <larmorx> <program> <args...>``; the
output image is the last argument (fslmaths).
"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep


def _image(d: Path, stem: str):
    for p in sorted(d.glob(f"{stem}.*")):
        if ".nii" in p.name or p.name.endswith((".hdr", ".hdr.gz")):
            return np.asarray(nib.load(str(p)).dataobj)
    return None


def main(argv: list[str]) -> int:
    src, exe, program, args = Path(argv[0]), argv[1], argv[2], argv[3:]
    inputs = [p for p in src.iterdir() if p.is_file()]
    with tempfile.TemporaryDirectory() as a_, tempfile.TemporaryDirectory() as b_:
        a, b = Path(a_), Path(b_)
        for p in inputs:
            shutil.copy(p, a / p.name)
            shutil.copy(p, b / p.name)
        env = {**rep._environment({}), "FSLOUTPUTTYPE": "NIFTI_GZ"}
        f = oracle_cache.run(
            [str(rep.fsldir() / "bin" / program), *args],
            env=env,
            cwd=a,
            outputs=[a],
            policy=oracle_cache.Policy(threads=1, family="fsl"),
            runner="cleanroom-maths-compare",
            inputs=[a / p.name for p in inputs],
            timeout=600,
        )
        o = subprocess.run(
            [exe, "fsl", program, *args],
            cwd=b,
            capture_output=True,
            text=True,
            env={"FSLOUTPUTTYPE": "NIFTI_GZ", "OMP_NUM_THREADS": "1",
                 "LARMORX_IMPLEMENTATION": "original", "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
        )  # fmt: skip
        print(f"FSL exit {f.returncode}: {f.stdout[:300]!r} {f.stderr[:300]!r}")
        print(f"original exit {o.returncode}: {o.stdout[:300]!r} {o.stderr[:300]!r}")
        for name, d in (("fsl_out", a), ("orig_out", b)):
            keep = src / name
            keep.mkdir(exist_ok=True)
            for p in d.iterdir():
                if p.name not in {q.name for q in inputs}:
                    shutil.copy(p, keep / p.name)
        if program == "fslmaths":
            x, y = _image(a, args[-1]), _image(b, args[-1])
            if x is not None and y is not None:
                d = np.flatnonzero(x.ravel() != y.ravel())
                print(f"{x.dtype}{x.shape}: {d.size} of {x.size} values differ")
                for i in d[:8]:
                    print(f"  [{i}] FSL {x.ravel()[i]!r} / original {y.ravel()[i]!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
