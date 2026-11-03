# SPDX-License-Identifier: Apache-2.0
"""FSL's Gaussian (``-s 5``) at voxel sizes not exact in binary (findings 23 of
``docs/findings/fsl-maths-original.md``): an impulse in the middle of a line of 81 voxels along y
(voxel sizes 0.8333333, 0.85 and 1 mm), FSL 6.0.7.23 through the oracle cache; then the impulse
responses against the spec's form of the kernel and other forms of its argument (glibc's
``expf``/``exp`` through ``ctypes``, as FSL's process has them; each response ``k_i / W`` with
``W`` the single-precision sum of the taps).

Run (in a memory box): ``python -P probe_gauss.py <dir>``.
"""

import ctypes
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep

f32 = np.float32
SIZES = ((0.8333333, 24), (0.85, 24), (1.0, 20))


def _fsl(d: Path) -> dict[float, np.ndarray]:
    out = {}
    for vy, n in SIZES:
        name = f"imp{str(vy).replace('.', '_')}.nii.gz"
        a = np.zeros((1, 81, 1), np.float32)
        a[0, 40, 0] = 1.0
        aff = np.diag([1.0, vy, 1.0, 1.0])
        im = nib.Nifti1Image(a, aff)
        im.header.set_zooms((1.0, np.float32(vy), 1.0))
        im.header.set_sform(aff, 1)
        im.header.set_qform(aff, 1)
        if not (d / name).exists():
            nib.save(im, str(d / name))
        res = f"s_{name}"
        p = oracle_cache.run(
            [str(rep.fsldir() / "bin" / "fslmaths"), name, "-s", "5", res],
            env=rep._environment({}),
            cwd=d,
            outputs=[d],
            policy=oracle_cache.Policy(threads=1, family="fsl"),
            runner="cleanroom-maths-extent",
            inputs=[d / name],
            timeout=600,
        )
        r = np.asarray(nib.load(str(d / res)).dataobj).reshape(-1)
        nz = np.nonzero(r)[0]
        print(f"d = {vy}: exit {p.returncode}, {len(nz)} taps ({nz.min()} to {nz.max()})")
        out[vy] = r[40 - n : 40 + n + 1]
    return out


def main(argv: list[str]) -> int:
    d = Path(argv[0])
    d.mkdir(parents=True, exist_ok=True)
    fsl = _fsl(d)
    libm = ctypes.CDLL("libm.so.6")
    libm.expf.restype, libm.expf.argtypes = ctypes.c_float, [ctypes.c_float]
    libm.exp.restype, libm.exp.argtypes = ctypes.c_double, [ctypes.c_double]

    def acc(vals):
        w = f32(0)
        for v in vals:
            w = f32(float(w) + float(v))
        return w

    for vy, n in SIZES:
        dd = f32(vy)
        big = float(dd)
        forms = {
            "spec: expf(-((i2 d) d) / 50) in single": lambda k, dd=dd: libm.expf(
                f32(-(f32(f32(k * k) * dd) * dd)) / f32(50)
            ),
            "i2 (d d) in single": lambda k, dd=dd: libm.expf(
                f32(-(f32(k * k) * f32(dd * dd))) / f32(50)
            ),
            "(i d)^2 in single": lambda k, dd=dd: libm.expf(
                f32(-(f32(f32(k * dd) * f32(k * dd)))) / f32(50)
            ),
            "i2 d d in double, expf": lambda k, b=big: libm.expf(f32(-(k * k) * b * b / 50.0)),
            "i2 d d in double, exp": lambda k, b=big: f32(libm.exp(-(k * k) * b * b / 50.0)),
            "(i d)^2 in double, exp": lambda k, b=big: f32(libm.exp(-((k * b) ** 2) / 50.0)),
        }
        row = []
        for name, fn in forms.items():
            k = np.array([fn(i) for i in range(-n, n + 1)], np.float32)
            w = acc(k)
            o = np.array([f32(v / w) for v in k], np.float32)
            row.append(f"{name}: {int((o == fsl[vy]).sum())}")
        print(f"d = {vy} ({2 * n + 1} taps; equal to FSL's): " + "; ".join(row))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
