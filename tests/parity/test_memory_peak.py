# SPDX-License-Identifier: Apache-2.0
"""larmorx needs no more memory than the original program for the same call (user,
2026-10-25: "we should fix the memory problem"): each ANTs image program and larmorx's command
(``python -m larmorx ants <program>``, what the ``larmorx`` console script runs) on the same
mid-size case, each process's peak resident memory from ``/usr/bin/time -v``, larmorx in both
D13 modes; its output bit-identical to ANTs' in ``reproduce`` (N4BiasFieldCorrection's and
DenoiseImage's to ANTs with correctly rounded maths preloaded, the build their records compare
with; ``validation/investigations/difftest/memory_pair.py`` measures any call by hand).

The allowance is a constant, not a ratio: ``OVERHEAD_MIB`` for the Python interpreter and the
gzip writer's two blocks per worker (a 16-voxel call needs 20 MiB in larmorx, 18 in ANTs; what
grows with the image costs larmorx no more than ANTs; ``docs/findings/ants-image-programs.md``,
``ants-n4.md`` and ``ants-denoise.md``, "Memory").
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

nib = pytest.importorskip("nibabel")
np = pytest.importorskip("numpy")
programs = pytest.importorskip("larmorx_validation.parity.ants_programs")

#: The constant larmorx may need beyond ANTs (MiB).
OVERHEAD_MIB = 8.0

CASES = [
    ("ResampleImageBySpacing", "128x128x64", "2,2,2", "3 {in} {out} 0.5 0.5 0.5"),
    ("ResampleImage", "128x128x64", "2,2,2", "3 {in} {out} 0.8x0.8x0.8 0 0"),
    ("ThresholdImage", "256x256x128", "1,1,1", "3 {in} {out} 20 60"),
    ("SmoothImage", "256x256x128", "1,1,1", "3 {in} 1.5 {out}"),
    ("MultiplyImages", "256x256x128", "1,1,1", "3 {in} 2.5 {out}"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} m {in} {in}"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} ME {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} GD {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} GE {in} 1"),
    ("ImageMath", "256x256x128", "1,1,1", "3 {out} TruncateImageIntensity {in} 0.01 0.999 256"),
    (
        "N4BiasFieldCorrection",
        "128x128x96",
        "1.5,1.5,1.5",
        "-d 3 -i {in} -o {out} -s 2 -c [20x20,0]",
    ),
    (
        "N4BiasFieldCorrection",
        "128x128x96",
        "1.5,1.5,1.5",
        "-d 3 -i {in} -o {out} -s 4 -c [20x20,0]",
    ),
    ("DenoiseImage", "96x96x64", "2,2,2", "-d 3 -i {in} -o {out}"),
]

#: The programs whose records compare with ANTs built with correctly rounded maths (their
#: ``expf``, ``logf`` and ``exp``): ANTs runs with CORE-MATH preloaded for them.
CORRECTLY_ROUNDED = {"N4BiasFieldCorrection", "DenoiseImage"}


def _peak(cmd: list[str], env: dict[str, str]) -> tuple[int, float]:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, check=False
    )
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    assert m, p.stderr[-400:]
    return p.returncode, int(m.group(1)) / 1024


@pytest.mark.parametrize(("program", "size", "spacing", "args"), CASES)
def test_larmorx_needs_no_more_memory_than_ants(tmp_path, program, size, spacing, args):
    if programs.ants_bin() is None or not (programs.ants_bin() / program).is_file():
        pytest.skip("the ANTs 2.6.5 oracle was not found")
    if not Path("/usr/bin/time").is_file() or shutil.which("python") is None:
        pytest.skip("/usr/bin/time was not found")
    shape = [int(x) for x in size.split("x")]
    n = int(np.prod(shape))
    data = ((np.arange(n, dtype=np.int64) * 7919) % 1000).astype(np.float32) / 10.0
    inp = tmp_path / "in.nii.gz"
    nib.save(
        nib.Nifti1Image(
            data.reshape(shape, order="F"),
            np.diag([*(float(s) for s in spacing.split(",")), 1.0]),
        ),
        str(inp),
    )
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "OMP_NUM_THREADS": "1"}
    words = args.split()

    def argv(out: Path) -> list[str]:
        return [w.format(**{"in": inp, "out": out}) for w in words]

    a_out = tmp_path / "ants.nii.gz"
    ants_env = dict(env)
    if program in CORRECTLY_ROUNDED:
        lib = programs.crmath_preload()
        if lib is None:
            pytest.skip("libcrmath.so (scripts/build_crmath_preload.sh) was not found")
        ants_env["LD_PRELOAD"] = str(lib)
    code, ants_mib = _peak([str(programs.ants_bin() / program), *argv(a_out)], ants_env)
    assert code == 0
    for mode in ("reproduce", "fix"):
        out = tmp_path / f"larmorx-{mode}.nii.gz"
        code, mib = _peak(
            [sys.executable, "-m", "larmorx", "ants", program, *argv(out)],
            {**env, "LARMORX_UPSTREAM_DEFECTS": mode},
        )
        assert code == 0
        assert mib <= ants_mib + OVERHEAD_MIB, (
            f"{program} ({mode}): larmorx {mib:.0f} MiB, ANTs {ants_mib:.0f} MiB"
        )
        if mode == "reproduce":
            a = np.asarray(nib.load(a_out).dataobj)
            b = np.asarray(nib.load(out).dataobj)
            assert a.tobytes() == b.tobytes(), program
