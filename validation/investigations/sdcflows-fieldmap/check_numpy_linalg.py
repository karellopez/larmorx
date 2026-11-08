# SPDX-License-Identifier: Apache-2.0
"""numpy.linalg's svd (3 x 3), eigh (4 x 4, lower) and det sign against larmorx's replica
(``larmorx_transform::numpy_linalg``, ``_core.numpy_linalg_small``), and nibabel 5.4.2's
``set_qform`` against ``_core.nibabel_qform``, bit for bit (signs of zeros included).

The matrices are the ones nibabel meets: rotations from rational quaternions (the difftest
generator's), the 48 signed axis permutations, rotations with last-bit noise, scaled and sheared
affines, random 3 x 3 matrices; and nibabel's Bar-Itzhack matrices of them plus random
symmetric 4 x 4 ones. Run with OPENBLAS_CORETYPE=Haswell in the newest environment (numpy 2.5.4,
OpenBLAS 0.3.34: LAPACK 3.12.1) or the shared one (numpy 2.3.5, OpenBLAS 0.3.30: LAPACK
3.12.0); the release is read from numpy's configuration. Prints one line per family and
``ALL OK`` or the first differences.
"""

from __future__ import annotations

import itertools
import sys
from pathlib import Path

import nibabel as nb
import numpy as np
from nibabel.quaternions import mat2quat

from larmorx import _core

sys.path.insert(0, str(Path(__file__).parent))  # stepdigest (python -P)
import stepdigest

N_RANDOM = int(sys.argv[1]) if len(sys.argv) > 1 else 3000


REL = stepdigest.numpy_lapack()


def same(a, b) -> bool:
    a = np.ascontiguousarray(a, dtype=np.float64)
    b = np.ascontiguousarray(b, dtype=np.float64)
    return a.shape == b.shape and a.tobytes() == b.tobytes()


def rot_from_quat(q):
    q = np.asarray(q, dtype=np.float64)
    q = q / np.sqrt(q @ q)
    w, x, y, z = q
    return np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    )


def matrices(rng):
    out = {
        "permutations": [],
        "rational rotations": [],
        "noisy rotations": [],
        "affine-like": [],
        "random": [],
    }
    for perm in itertools.permutations(range(3)):
        for signs in itertools.product((1.0, -1.0), repeat=3):
            m = np.zeros((3, 3))
            for i, j in enumerate(perm):
                m[i, j] = signs[i]
            out["permutations"].append(m)
    for _ in range(N_RANDOM):
        q = rng.integers(-40, 41, size=4)
        if not q.any():
            continue
        out["rational rotations"].append(rot_from_quat(q))
        r = rot_from_quat(q)
        out["noisy rotations"].append(r + rng.normal(0, 1e-9, (3, 3)) * rng.integers(0, 2, (3, 3)))
        z = rng.choice([0.5, 1.0, 1.5, 2.0, 2.4, 3.0, 3.75, 4.0])
        s = np.diag(rng.choice([0.8, 1.0, 1.2, 2.0, 3.3], 3))
        sh = np.eye(3)
        if rng.random() < 0.3:
            sh[0, 1] = rng.normal(0, 0.05)
        a = r @ s @ sh * z
        zooms = np.sqrt(np.sum(a * a, axis=0))
        out["affine-like"].append(a / zooms)
        out["random"].append(rng.normal(0, 1, (3, 3)))
    return out


def kmat(m):
    Qxx, Qyx, Qzx, Qxy, Qyy, Qzy, Qxz, Qyz, Qzz = m.flat
    return (
        np.array(
            [
                [Qxx - Qyy - Qzz, 0, 0, 0],
                [Qyx + Qxy, Qyy - Qxx - Qzz, 0, 0],
                [Qzx + Qxz, Qzy + Qyz, Qzz - Qxx - Qyy, 0],
                [Qyz - Qzy, Qzx - Qxz, Qxy - Qyx, Qxx + Qyy + Qzz],
            ]
        )
        / 3.0
    )


def main() -> int:
    rng = np.random.default_rng(20261108)
    fams = matrices(rng)
    bad = 0
    total = 0
    for fam, mats in fams.items():
        nsvd = neig = ndet = nq = 0
        first = None
        for m in mats:
            total += 1
            # det sign
            if (np.linalg.det(m) > 0) != bool(_core.numpy_linalg_small("det_positive", m)):
                ndet += 1
                first = first or ("det", m)
            # svd
            try:
                u, s, vt = np.linalg.svd(m)
                ref = (u, s, vt)
            except np.linalg.LinAlgError:
                ref = None
            got = _core.numpy_linalg_small("svd", m, REL)
            if (ref is None) != (got is None) or (
                ref is not None
                and not all(
                    same(x, y) for x, y in zip(ref, (got[0], got[1][0], got[2]), strict=True)
                )
            ):
                nsvd += 1
                first = first or ("svd", m)
            # eigh of K (of the polar factor, as nibabel) and of a random symmetric matrix
            for k in (kmat(u @ vt) if ref is not None else None, rng.normal(0, 1, (4, 4))):
                if k is None:
                    continue
                kl = np.tril(k)
                w, v = np.linalg.eigh(kl)
                g = _core.numpy_linalg_small("eigh", np.ascontiguousarray(kl), REL)
                if g is None or not (same(w, g[0][0]) and same(v, g[1])):
                    neig += 1
                    first = first or ("eigh", kl)
            # nibabel's set_qform on an affine with this rotation part
            aff = np.eye(4)
            aff[:3, :3] = m * rng.choice([1.0, 2.0, 3.5])
            aff[:3, 3] = rng.normal(0, 30, 3)
            hdr = nb.Nifti1Header()
            try:
                hdr.set_qform(aff, 1)
                want = (
                    float(hdr["pixdim"][0]),
                    np.asarray(hdr["pixdim"][1:4], dtype=np.float32),
                    np.array(
                        [hdr["quatern_b"], hdr["quatern_c"], hdr["quatern_d"]], dtype=np.float32
                    ),
                )
                # float64 quaternion of nibabel's computation (before the float32 store)
                rzs = aff[:3, :3]
                zooms = np.sqrt(np.sum(rzs * rzs, axis=0))
                r = rzs / zooms
                if not np.linalg.det(r) > 0:
                    r[:, -1] *= -1
                p_, _s, qs_ = np.linalg.svd(r)
                q64 = mat2quat(np.dot(p_, qs_))
            except Exception as exc:
                want = type(exc).__name__
                q64 = None
            try:
                g = _core.nibabel_qform(aff, REL)
                got_q = (
                    g["qfac"],
                    np.asarray(g["zooms"], dtype=np.float32),
                    np.asarray(g["quaternion"][1:], dtype=np.float32),
                )
                ok = (
                    not isinstance(want, str)
                    and want[0] == got_q[0]
                    and same(want[1], got_q[1])
                    and want[2].tobytes() == got_q[2].tobytes()
                    and same(q64, g["quaternion"])
                )
            except Exception as exc:
                ok = isinstance(want, str)
                got_q = type(exc).__name__
            if not ok:
                nq += 1
                first = first or ("qform", aff)
        bad += nsvd + neig + ndet + nq
        print(
            f"{fam}: {len(mats)} matrices, svd differ {nsvd}, eigh differ {neig}, det differ {ndet}, nibabel qform differ {nq}"
        )
        if first is not None:
            np.set_printoptions(precision=17)
            print("   first:", first[0], repr(first[1].tolist()))
    print(f"LAPACK {REL}, numpy {np.__version__}: {total} matrices")
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
