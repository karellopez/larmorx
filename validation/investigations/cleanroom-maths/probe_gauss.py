# SPDX-License-Identifier: Apache-2.0
"""FSL's Gaussian (``-s 5``) at voxel sizes not exact in binary (findings 23 of
``docs/findings/fsl-maths-original.md``): an impulse in the middle of a line of 81 voxels along y
(voxel sizes 0.8333333, 0.85 and 1 mm), FSL 6.0.7.23 through the oracle cache; then the impulse
responses against the spec's form of the kernel and other forms of its argument (glibc's
``expf``/``exp`` through ``ctypes``, as FSL's process has them).

Each form is put through §10.2's separable filter as the spec states it: the response at voxel
``j`` is ``k_t / W_j`` in single precision, ``W_j`` the single-precision sum, in the taps' order,
of the taps whose voxel is inside the line (near the line's ends a part of the kernel only). With
``<larmorx>``, the original's own response is compared too.

Run (in a memory box): ``python -P probe_gauss.py <dir> [<larmorx>]``.
"""

import ctypes
import itertools
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep

f32 = np.float32
f64 = np.float64
SIGMA = 5.0
SIZES = ((0.8333333, 24), (0.85, 24), (1.0, 20))
LINE = 81
CENTRE = 40


def _impulse(d: Path, vy: float) -> str:
    name = f"imp{str(vy).replace('.', '_')}.nii.gz"
    a = np.zeros((1, LINE, 1), np.float32)
    a[0, CENTRE, 0] = 1.0
    aff = np.diag([1.0, vy, 1.0, 1.0])
    im = nib.Nifti1Image(a, aff)
    im.header.set_zooms((1.0, np.float32(vy), 1.0))
    im.header.set_sform(aff, 1)
    im.header.set_qform(aff, 1)
    if not (d / name).exists():
        nib.save(im, str(d / name))
    return name


def _fsl(d: Path) -> dict[float, np.ndarray]:
    out = {}
    for vy, _n in SIZES:
        name = _impulse(d, vy)
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
        out[vy] = r
    return out


def _larmorx(d: Path, exe: str) -> dict[float, np.ndarray]:
    out = {}
    for vy, _n in SIZES:
        name = _impulse(d, vy)
        res = f"lx_{name}"
        subprocess.run(
            [exe, "fsl", "fslmaths", name, "-s", "5", res],
            cwd=d,
            check=True,
            env={
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                "OMP_NUM_THREADS": "1",
                "LARMORX_IMPLEMENTATION": "original",
            },
        )
        out[vy] = np.asarray(nib.load(str(d / res)).dataobj).reshape(-1)
    return out


def _forms(t, s, dd) -> dict:
    """The exponent's candidate forms in the precision ``t``."""
    two = t(2)
    return {
        "-(i2 d) d / (2 s s)": lambda i: -((t(i * i) * dd) * dd) / (two * s * s),
        "-i2 (d d) / (2 s s)": lambda i: -(t(i * i) * (dd * dd)) / (two * s * s),
        "-(i d)^2 / (2 s s)": lambda i: -((t(i) * dd) * (t(i) * dd)) / (two * s * s),
        "-0.5 ((i d) / s)^2": lambda i: t(-0.5) * ((t(i) * dd) / s) ** 2,
        "-0.5 (i / (s / d))^2": lambda i: t(-0.5) * (t(i) / (s / dd)) ** 2,
        "-i2 / (2 (s/d)^2)": lambda i: -t(i * i) / (two * (s / dd) * (s / dd)),
        "-(i d)^2 * (1 / (2 s s))": lambda i: -((t(i) * dd) * (t(i) * dd)) * (t(1) / (two * s * s)),
    }


def _response(k: np.ndarray, centre: int) -> np.ndarray:
    """§10.2's separable filter of the impulse with the line of taps ``k`` (single precision)."""
    n = len(k) // 2
    out = np.zeros(LINE, np.float32)
    for j in range(LINE):
        w = f32(0)
        for t in range(-n, n + 1):
            if 0 <= j + t < LINE:
                w = f32(f64(w) + f64(k[t + n]))
        t = centre - j
        if -n <= t <= n:
            num = k[t + n]
            out[j] = f32(num / w) if abs(f64(w)) > 1e-12 else num
    return out


VOX3 = (0.85, 0.8333333, 0.83)
N3 = 27


def _fsl3(d: Path) -> np.ndarray:
    """FSL's ``-kernel gauss 1 -fmeanu`` on an impulse of 27³ voxels of sizes ``VOX3`` (the
    direct sum: 11 x 11 x 11 taps): its response is the normalised kernel."""
    name = "imp3d.nii.gz"
    a = np.zeros((N3, N3, N3), np.float32)
    a[13, 13, 13] = 1.0
    aff = np.diag([*VOX3, 1.0])
    if not (d / name).exists():
        im = nib.Nifti1Image(a, aff)
        im.header.set_zooms(tuple(np.float32(v) for v in VOX3))
        im.header.set_sform(aff, 1)
        im.header.set_qform(aff, 1)
        nib.save(im, str(d / name))
    res = "g1_imp3d.nii.gz"
    p = oracle_cache.run(
        [str(rep.fsldir() / "bin" / "fslmaths"), name, "-kernel", "gauss", "1", "-fmeanu", res],
        env=rep._environment({}),
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="cleanroom-maths-extent",
        inputs=[d / name],
        timeout=600,
    )
    print(f"3D: exit {p.returncode}")
    return np.asarray(nib.load(str(d / res)).dataobj)


def _forms3(fsl3: np.ndarray, expf) -> None:
    """The 3D kernel's exponent: each axis' term ``i² (d d)`` in single precision, the three
    added in each order; normalised by its double sum rounded to single (findings 7)."""
    dd = [f32(f32(v) * f32(v)) for v in VOX3]
    s = f32(1.0)
    den = f32(f32(2) * s) * s
    n = 5
    orders = {
        "(x + y) + z": lambda a, b, c: f32(f32(a + b) + c),
        "x + (y + z)": lambda a, b, c: f32(a + f32(b + c)),
        "(z + y) + x": lambda a, b, c: f32(f32(c + b) + a),
        "(x + z) + y": lambda a, b, c: f32(f32(a + c) + b),
    }
    block = fsl3[13 - n : 13 + n + 1, 13 - n : 13 + n + 1, 13 - n : 13 + n + 1]
    for name, add in orders.items():
        k = np.zeros((2 * n + 1,) * 3, np.float32)
        for i, j, m in itertools.product(range(-n, n + 1), repeat=3):
            a, b, c = (f32(f32(t * t) * q) for t, q in zip((i, j, m), dd, strict=True))
            k[i + n, j + n, m + n] = expf(f32(-add(a, b, c)) / den)
        w = f32(sum(f64(v) for v in k.transpose(2, 1, 0).reshape(-1)))
        kn = (k / w).astype(np.float32)
        eq = int((kn == block).sum())
        print(f"3D gauss 1 at {VOX3}: {name}: {eq} of {kn.size} taps equal to FSL's")


def main(argv: list[str]) -> int:
    d = Path(argv[0])
    d.mkdir(parents=True, exist_ok=True)
    fsl = _fsl(d)
    lx = _larmorx(d, argv[1]) if len(argv) > 1 else {}
    libm = ctypes.CDLL("libm.so.6")
    libm.expf.restype, libm.expf.argtypes = ctypes.c_float, [ctypes.c_float]
    libm.exp.restype, libm.exp.argtypes = ctypes.c_double, [ctypes.c_double]
    exps = {
        "expf": lambda q: f32(libm.expf(f32(q))),
        "exp": lambda q: f32(libm.exp(f64(q))),
    }

    for vy, n in SIZES:
        if vy in lx:
            eq = int((lx[vy] == fsl[vy]).sum())
            print(f"d = {vy}: the original's response equal to FSL's in {eq} of {LINE} voxels")
        best = []
        for prec, ex in itertools.product(("single", "double"), exps):
            t = f32 if prec == "single" else f64
            args = _forms(t, t(SIGMA), t(f32(vy)))
            for form, arg in args.items():
                k = np.array([exps[ex](arg(i)) for i in range(-n, n + 1)], np.float32)
                for norm in ("as built", "normalised first"):
                    kk = k
                    if norm == "normalised first":
                        w = f32(0)
                        for v in k:
                            w = f32(f64(w) + f64(v))
                        kk = np.array([f32(v / w) for v in k], np.float32)
                    o = _response(kk, CENTRE)
                    eq = int((o == fsl[vy]).sum())
                    best.append((eq, f"{form}, {prec}, {ex}, {norm}"))
        best.sort(reverse=True)
        print(f"d = {vy} ({2 * n + 1} taps; voxels equal to FSL's of {LINE}):")
        for eq, name in best[:6]:
            print(f"   {eq}: {name}")
    _forms3(_fsl3(d), exps["expf"])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
