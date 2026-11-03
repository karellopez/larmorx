# SPDX-License-Identifier: Apache-2.0
"""The output geometry of fslmaths' operations that change the grid along x (``-subsamp2``,
``-subsamp2offc``, the reductions along X) and along y and z (``-Ymax``, ``-Zmax``), for inputs
in either orientation (a positive determinant: FSL holds x mirrored) and with either matrix
unset (qform or sform code 0): FSL 6.0.7.23 through the oracle cache, and the original
(``<larmorx>``), header field by field, with the hypothesis of findings 24 evaluated: FSL maps
both matrices, the unset ones too (the qform from its quaternion fields), through its mirrored
frame, ``M' = M · F(n) · S · F(n')``, where ``F(n)`` mirrors x over ``n`` voxels for an image
FSL mirrors (else the identity) and ``S`` is the operation's map of the new voxel to the old.

Run (in a memory box): ``python -P probe_geometry.py <dir> <larmorx>``.
"""

import itertools
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep

#: header variants: (name, qform, qform code, sform, sform code)
_OBL = np.array(
    [[1.9, 0.2, -0.3, -20.25], [-0.15, 2.4, 0.4, 31.5], [0.25, -0.35, 2.9, -7.75], [0, 0, 0, 1]]
)
_RAD = np.diag([-2.0, 2.5, 3.0, 1.0])
_RAD[:3, 3] = (40.5, -12.0, 6.25)
_NEU = np.diag([2.0, 2.5, 3.0, 1.0])
_NEU[:3, 3] = (-40.5, -12.0, 6.25)
_SHEAR = _NEU.copy()
_SHEAR[0, 1] = 0.3
_OBL_RAD = _OBL @ np.diag([-1.0, 1.0, 1.0, 1.0])
_OBL2 = np.array(
    [[2.1, -0.37, 0.29, 17.3], [0.31, 1.7, -0.53, -29.9], [-0.23, 0.47, 3.1, 11.1], [0, 0, 0, 1]]
)
VARIANTS = (
    ("oblique-both", _OBL, 1, _OBL2, 1),
    ("oblique-radio", _OBL_RAD, 1, _OBL_RAD, 1),
    ("oblique-qform-only", _OBL, 1, _OBL2, 0),
    ("oblique-sform-only", _OBL, 0, _OBL2, 1),
    ("neuro-both", _NEU, 1, _NEU, 1),
    ("neuro-sform-only", _NEU, 0, _NEU, 1),
    ("neuro-qform-only", _NEU, 1, _NEU, 0),
    ("radio-both", _RAD, 1, _RAD, 1),
    ("radio-sform-only", _RAD, 0, _RAD, 1),
    ("oblique-sheared", _OBL, 1, _SHEAR, 2),
    ("no-codes", _NEU, 0, _NEU, 0),
)
OPS = (("-subsamp2",), ("-subsamp2offc",), ("-Xmax",), ("-Ymax",), ("-Zmax",))
SIZES = ((5, 4, 2), (4, 5, 3), (7, 6, 4))
FIELDS = ("qform_code", "sform_code", "quatern_b", "quatern_c", "quatern_d", "qoffset_x",
          "qoffset_y", "qoffset_z", "srow_x", "srow_y", "srow_z", "pixdim")  # fmt: skip


def _image(d: Path, name: str, shape, q, qc, s, sc) -> str:
    a = (np.arange(np.prod(shape), dtype=np.float32) + 1).reshape(shape, order="F")
    im = nib.Nifti1Image(a, None)
    im.header.set_qform(q, int(qc))
    im.header.set_sform(s, int(sc))
    path = d / f"{name}.nii.gz"
    if not path.exists():
        nib.save(im, str(path))
    return path.name


def _fsl(d: Path, words: list[str], inp: str) -> Path:
    p = oracle_cache.run(
        [str(rep.fsldir() / "bin" / "fslmaths"), *words],
        env=rep._environment({}),
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="cleanroom-maths-geometry",
        inputs=[d / inp],
        timeout=600,
    )
    if p.returncode != 0:
        raise RuntimeError(f"FSL {' '.join(words)}: exit {p.returncode}: {p.stderr}")
    return d / f"{words[-1]}.nii.gz"


_ENV = {"FSLOUTPUTTYPE": "NIFTI_GZ", "OMP_NUM_THREADS": "1", "LARMORX_IMPLEMENTATION": "original"}


def _ours(d: Path, exe: str, words: list[str]) -> Path:
    subprocess.run(
        [exe, "fsl", "fslmaths", *words],
        cwd=d,
        check=False,
        capture_output=True,
        env=_ENV,
    )
    return d / f"{words[-1]}.nii.gz"


def _flip(n: int, mirrored: bool) -> np.ndarray:
    f = np.eye(4)
    if mirrored:
        f[0, 0], f[0, 3] = -1.0, n - 1.0
    return f


def _map(op: str, axis_sizes) -> np.ndarray:
    """The operation's map of a new voxel to the old (``S``)."""
    s = np.eye(4)
    if op.startswith("-subsamp2"):
        s[:3, :3] *= 2
        if op == "-subsamp2offc":
            s[:3, 3] = 0.5
    return s


def _times(m: np.ndarray, a: np.ndarray) -> np.ndarray:
    """``m · a`` with ``m``'s entries in single precision (the header's matrices as FSL holds
    them), the products and sums in double, the result rounded to single precision."""
    return (m.astype(np.float32).astype(float) @ a).astype(np.float32)


def _hypothesis(h, op: str, shape, new_shape) -> dict[str, np.ndarray]:
    """Findings 24: a matrix that is set maps through the mirrored frame and the operation,
    ``M · F(n) · S · F(n')``; one that is unset (code 0) through the mirroring only,
    ``M · F(n) · F(n')``; the product from the header's matrices in single precision, in
    double, rounded to single precision once (the remaining misses: the last bit of an offset,
    counted in units in the last place)."""
    s = np.asarray(h.get_sform(coded=False), float)
    q = np.asarray(h.get_qform(coded=False), float)
    # the orientation of the matrix FSL uses (the sform when set, else the qform; neither:
    # radiological)
    best = s if h["sform_code"] > 0 else q if h["qform_code"] > 0 else None
    mirrored = best is not None and np.linalg.det(best[:3, :3]) > 0
    f, f2 = _flip(shape[0], mirrored), _flip(new_shape[0], mirrored)
    op_map = _map(op, shape)

    def mapped(m: np.ndarray, code: int) -> np.ndarray:
        return _times(m, f @ op_map @ f2 if code > 0 else f @ f2)

    return {"sform": mapped(s, h["sform_code"]), "qform": mapped(q, h["qform_code"])}


def main(argv: list[str]) -> int:
    d = Path(argv[0])
    d.mkdir(parents=True, exist_ok=True)
    exe = argv[1]
    differ = hyp_bad = total = worst_ulp = 0
    for (vname, q, qc, s, sc), op, shape in itertools.product(VARIANTS, OPS, SIZES):
        name = f"{vname}_{'x'.join(map(str, shape))}"
        inp = _image(d, name, shape, q, qc, s, sc)
        tag = f"{name}{op[0]}".replace("-", "_")
        words = [inp, *op, f"f_{tag}"]
        fo = nib.load(str(_fsl(d, words, inp)))
        lo = nib.load(str(_ours(d, exe, [inp, *op, f"o_{tag}"])))
        hf, hl = fo.header, lo.header
        bad = [k for k in FIELDS if not np.array_equal(hf[k], hl[k])]
        total += 1
        if bad:
            differ += 1
        h_in = nib.load(str(d / inp)).header
        hyp = _hypothesis(h_in, op[0], shape, fo.shape)
        fs = np.asarray(hf.get_sform(coded=False), float)
        fq = np.asarray(hf.get_qform(coded=False), float)
        ok_s = np.array_equal(hyp["sform"][:3], fs[:3].astype(np.float32))
        ok_q = np.array_equal(hyp["qform"][:3, 3], fq[:3, 3].astype(np.float32))
        if not (ok_s and ok_q):
            hyp_bad += 1
            for got, want in ((hyp["sform"][:3], fs[:3]), (hyp["qform"][:3, 3], fq[:3, 3])):
                a = np.asarray(got, np.float32).view(np.int32).astype(np.int64)
                b = np.asarray(want, np.float32).view(np.int32).astype(np.int64)
                worst_ulp = max(worst_ulp, int(np.abs(a - b).max()))
        if bad or not (ok_s and ok_q):
            print(f"{name} {op[0]}: FSL shape {fo.shape}; differ: {bad}; hypothesis sform "
                  f"{'ok' if ok_s else 'NO'}, qform {'ok' if ok_q else 'NO'}")  # fmt: skip
            for k in bad:
                print(f"    {k}: FSL {hf[k]} / original {hl[k]}")
            if not ok_q:
                print(f"    FSL qform translation {fq[:3, 3]}, hypothesis {hyp['qform'][:3, 3]}")
            if not ok_s:
                print(f"    FSL sform translation {fs[:3, 3]}, hypothesis {hyp['sform'][:3, 3]}")
    print(
        f"{total} calls: {differ} differ from FSL; the hypothesis misses {hyp_bad} "
        f"(by at most {worst_ulp} units in the last place)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
