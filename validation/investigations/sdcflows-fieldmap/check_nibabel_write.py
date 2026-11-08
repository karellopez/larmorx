# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' port of nibabel's scaled NIfTI writer with nibabel's own.

Runs, in this interpreter's environment (each kept one: ``oracles/python-newest`` and the
shared ``.venv``; nibabel 5.4.2 in both):

    PYTHONPATH=<repo>/python python -P check_nibabel_write.py

1. The extended-precision scalars (``_Extended``) against ``np.longdouble`` (x87 on x86-64
   Linux): subtraction, product, quotient and the rounding to float32 of 20,000 random value
   pairs (normal, tiny, huge, equal exponents, integers to 2**64).
2. ``save_nibabel(Image(data, affine, header))`` against ``Nifti1Image(data, affine, header)
   .to_filename`` with the header's data type set: the file bytes once decompressed (header
   with ``scl_slope``/``scl_inter``, and the stored voxels), or the same exception type and
   message. Float32 and float64 data into every integer type and into floats; integers into
   smaller integers (the offset-only, sign-flip and range scalings); NaN, infinities, signed
   zeros, constant arrays, all-NaN, all-zero, one voxel, 4D, NIfTI-2; 400 random arrays
   (ranges from 1e-30 to 1e30, offsets, signs).

One line per group (and per failure); exit status 1 if any differs.
"""

import gzip
import sys
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np

from larmorx.image import Image
from larmorx.io.nibabel_writer import _Extended, save_nibabel

FAILS: list[str] = []
INT_TYPES = ("int8", "uint8", "int16", "uint16", "int32", "uint32", "int64", "uint64")


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def check_extended(rng: np.random.Generator) -> None:
    ld = np.longdouble
    bad = 0
    n = 0
    for _ in range(20000):
        kind = rng.integers(0, 5)
        if kind == 0:
            a, b = rng.standard_normal(2) * 10.0 ** rng.integers(-40, 40, 2)
        elif kind == 1:
            a = rng.standard_normal() * 10.0 ** rng.integers(-300, 300)
            b = rng.standard_normal() * 10.0 ** rng.integers(-300, 300)
        elif kind == 2:
            a = float(np.float32(rng.standard_normal() * 1000))
            b = float(np.nextafter(np.float32(a), np.float32(np.inf)))
        elif kind == 3:
            a, b = (float(np.float32(v)) for v in rng.standard_normal(2) * 1e6)
        else:
            a = int(rng.integers(-(2**62), 2**62)) * 3
            b = int(rng.integers(1, 2**62))
        for op in ("sub", "mul", "div"):
            xa, xb = _Extended.of(a), _Extended.of(b)
            na, nb_ = ld(a), ld(b)
            if op == "sub":
                x, r = xa - xb, na - nb_
            elif op == "mul":
                x, r = xa * xb, na * nb_
            else:
                if b == 0:
                    continue
                x, r = xa / xb, na / nb_
            n += 1
            with np.errstate(over="ignore", under="ignore"):
                want = np.float32(r)
            got = x.to_float32()
            if want.tobytes() != got.tobytes():
                bad += 1
                if bad <= 5:
                    print(f"   {op} {a!r} {b!r}: want {want!r} got {got!r}")
    report(f"extended precision vs np.longdouble ({n} operations)", bad == 0, f"{bad} differ")


def write_both(data: np.ndarray, out_dtype: str, version: int = 1, affine=None) -> str | None:
    """None if both files are identical (or both raise the same), else a description."""
    affine = np.diag([2.0, 2.0, 3.0, 1.0]) if affine is None else affine
    klass = nb.Nifti2Image if version == 2 else nb.Nifti1Image
    with tempfile.TemporaryDirectory() as tmp:
        p_nb = Path(tmp) / "nb.nii"
        p_lx = Path(tmp) / "lx.nii"
        err_nb = err_lx = None
        img = klass(data, affine, dtype=np.dtype(out_dtype))
        lx_img = Image.from_nibabel(img)
        try:
            with np.errstate(all="ignore"):
                img.to_filename(p_nb)
        except Exception as e:  # the exception is the result
            err_nb = (type(e).__name__, str(e))
        try:
            with np.errstate(all="ignore"):
                save_nibabel(lx_img, p_lx)
        except Exception as e:  # the exception is the result
            err_lx = (type(e).__name__, str(e))
        if err_nb or err_lx:
            return None if err_nb == err_lx else f"nibabel {err_nb}, larmorx {err_lx}"
        a, b = raw(p_nb), raw(p_lx)
        if a == b:
            return None
        hk = nb.Nifti2Header if version == 2 else nb.Nifti1Header
        n = hk.sizeof_hdr
        h_nb, h_lx = hk(a[:n]), hk(b[:n])
        diff = [k for k in h_nb.keys() if h_nb[k].tobytes() != h_lx[k].tobytes()]  # noqa: SIM118
        return f"header fields {diff}; voxels {'same' if a[n:] == b[n:] else 'differ'}"


def group(name: str, cases: list[tuple[np.ndarray, str]], version: int = 1) -> None:
    bad = []
    for i, (data, out) in enumerate(cases):
        d = write_both(data, out, version)
        if d is not None:
            bad.append(f"case {i} ({data.dtype}->{out}, shape {data.shape}): {d}")
    report(f"{name} ({len(cases)} cases)", not bad, "; ".join(bad[:3]))


def main() -> int:
    rng = np.random.default_rng(20261108)
    check_extended(rng)

    shape = (5, 4, 3)
    base = rng.standard_normal(shape)
    specials = []
    for dt in ("float32", "float64"):
        x = base.astype(dt)
        for out in (*INT_TYPES, "float32", "float64"):
            specials.append((x * 100, out))
            specials.append((x * 100 + 1000, out))
            specials.append((np.abs(x) * 1e4, out))
            specials.append((-np.abs(x) * 3, out))
    group("float data into every type", specials)

    edge = []
    for dt in ("float32", "float64"):
        f = base.astype(dt) * 50
        nanf = f.copy()
        nanf[0, 0, 0] = np.nan
        inff = f.copy()
        inff[1, 1, 1] = np.inf
        inff[2, 2, 2] = -np.inf
        zeros_signed = np.zeros(shape, dt)
        zeros_signed[0] = -0.0
        pos_nan = np.abs(f) + 1
        pos_nan[1, 2, 0] = np.nan
        neg_zero_min = np.abs(f)
        neg_zero_min.flat[0] = -0.0
        for out in ("int16", "uint8", "int8", "uint16", "int32"):
            edge += [
                (nanf, out),
                (inff, out),
                (np.full(shape, 7.25, dt), out),
                (np.full(shape, -3.5, dt), out),
                (np.full(shape, np.nan, dt), out),
                (np.zeros(shape, dt), out),
                (zeros_signed, out),
                (pos_nan, out),
                (neg_zero_min, out),
                (np.array([[[12.5]]], dt), out),
                (f.reshape(5, 4, 3, 1), out),
                (np.stack([f, f * 2], -1), out),
                (np.where(f > 0, np.nan, f), out),
                (np.full(shape, np.inf, dt), out),
            ]
        edge.append((np.array([-3e38, 3e38, 0.0], "float64").reshape(3, 1, 1), "int16"))
        edge.append((np.array([-1e300, 1e300], "float64").reshape(2, 1, 1), "int16"))
    group("special values and shapes", edge)
    group("NIfTI-2", [(c, o) for c, o in edge[:20]], version=2)

    ints = []
    for dt, scale, off in (
        ("int32", 100000, 0),
        ("int32", 30000, 40000),
        ("int64", 2**40, 0),
        ("int64", 1000, -(2**33)),
        ("uint16", 60000, 0),
        ("int16", 32000, 0),
        ("uint32", 3_000_000_000, 0),
        ("int64", 2**61, 2**61),
    ):
        v = (rng.random(shape) * scale + off).astype(dt)
        for out in INT_TYPES:
            ints.append((v, out))
            ints.append((-np.abs(v.astype("int64")).astype("int64"), out))
        ints.append((v, "float32"))
    ints.append((np.array([-5, -1, 0], "int32").reshape(3, 1, 1), "uint8"))
    ints.append((np.array([-300, -1], "int32").reshape(2, 1, 1), "uint8"))
    group("integer data into integers", ints)

    rand = []
    for _ in range(400):
        dt = ("float32", "float64")[int(rng.integers(0, 2))]
        mag = 10.0 ** rng.uniform(-30, 30)
        off = rng.standard_normal() * mag * 10.0 ** rng.uniform(-3, 3) * int(rng.integers(0, 2))
        v = (rng.standard_normal((6, 5, 4)) * mag + off).astype(dt)
        if rng.random() < 0.2:
            v.flat[int(rng.integers(0, v.size))] = np.nan
        if rng.random() < 0.2:
            v = np.abs(v)
        out = INT_TYPES[int(rng.integers(0, len(INT_TYPES)))]
        rand.append((v, out))
    group("random arrays", rand)

    print(f"ALL {'OK' if not FAILS else 'FAIL'}: {len(FAILS)} groups differ")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
