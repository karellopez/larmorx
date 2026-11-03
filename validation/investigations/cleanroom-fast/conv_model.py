# SPDX-License-Identifier: Apache-2.0
"""The bias field's convolution (``specs/fast.md`` §4.5, §8.5) modelled in numpy against FSL's
``_bias``: from the clean-room original's per-voxel ``W`` and ``R`` (dumped by the development
hook of ``dump.patch``), the x pass of a 5-tap kernel (``-l 5`` on ``t1_half``: one tap along y
and z) computed in several ways, each turned into the bias field as the original does
(``q = R_s / W_s``, the mask's blocked mean subtracted, ``1 / expf(-b)``) and compared with FSL's
(with correctly rounded maths, written by ``files_cr.py``). It showed that the normalised taps
stay in double precision (the findings' "The bias field's kernel"). A development tool of FAST's
clean-room original (relay 2, 2026-11-03).

    # in a scratch copy of the worktree with dump.patch applied, the binary built:
    LXFAST_DUMP=<dump> LARMORX_FAST_BIN=<binary> python -P files_cr.py <run> \\
        "-l 5 -b -I 0 -O 0 -W 1 --nopve -o seg @t1_half.nii.gz"
    python -I conv_model.py <dump> <run>
"""

from __future__ import annotations

import sys

import nibabel as nib
import numpy as np

f32 = np.float32


def main() -> None:
    dump, run = sys.argv[1], sys.argv[2]
    img = nib.load(f"{run}/fsl/t1_half.nii.gz")
    nx, ny, nz = img.shape[:3]
    w, r = (np.fromfile(f"{dump}/{n}.f32", dtype="<f4") for n in ("w", "r"))
    fb = np.asarray(nib.load(f"{run}/fsl/seg_bias.nii.gz").dataobj)
    flip = np.linalg.det(img.affine[:3, :3]) > 0

    def frame(v: np.ndarray) -> np.ndarray:
        a = v.reshape(nz, ny, nx).transpose(2, 1, 0)
        return a[::-1] if flip else a

    mask = frame(w) > 0
    internal_mask = (mask[::-1] if flip else mask).transpose(2, 1, 0).reshape(-1)
    block = max(int(np.sqrt(nx * ny * nz)), 10000) + 1

    def bias_of(rs: np.ndarray, ws: np.ndarray) -> np.ndarray:
        with np.errstate(invalid="ignore", divide="ignore"):
            q = np.where(mask, (frame(rs) / frame(ws)).astype(f32), f32(0))
        qi = (q[::-1] if flip else q).transpose(2, 1, 0).reshape(-1)[internal_mask]
        total = 0.0
        for k in range(0, len(qi), block):
            part = 0.0
            for x in qi[k : k + block].astype(np.float64):
                part += x
            total += part
        b = np.where(mask, q - f32(total / len(qi)), f32(0)).astype(f32)
        e = np.exp(-b.astype(np.float64)).astype(f32)
        return np.where(mask, (f32(1) / e).astype(f32), f32(1))

    def conv_x(v: np.ndarray, k: np.ndarray, mode: str = "single-sum") -> np.ndarray:
        a = v.reshape(nz, ny, nx)
        rr = len(k) // 2
        out = np.zeros(a.shape, f32)
        acc = np.zeros(a.shape, np.float64)
        order = range(len(k)) if mode != "reversed" else range(len(k) - 1, -1, -1)
        for j in order:
            off = j - rr
            src = np.zeros_like(a)
            if off >= 0:
                src[..., : nx - off] = a[..., off:]
            else:
                src[..., -off:] = a[..., : nx + off]
            if mode == "single-products":
                out = (out + (f32(k[j]) * src).astype(f32)).astype(f32)
                continue
            prod = np.float64(k[j]) * src.astype(np.float64)
            if mode == "double-sum":
                acc += prod
            else:
                out = (out.astype(np.float64) + prod).astype(f32)
        return (acc.astype(f32) if mode == "double-sum" else out).reshape(-1)

    sigma = np.float64(f32(0.51 * 5 / 2.0))
    raw = np.array([np.exp(-(j * j) / (2 * sigma * sigma)) for j in range(-2, 3)]).astype(f32)
    total = np.float64(raw.sum(dtype=f32))
    taps_double = raw.astype(np.float64) * (1.0 / total)
    taps_single = taps_double.astype(f32)
    rows = [
        ("taps in single, single sum", taps_single, "single-sum"),
        ("taps in single, double sum", taps_single, "double-sum"),
        ("taps in single, reversed", taps_single, "reversed"),
        ("taps in single, single products", taps_single, "single-products"),
        ("unnormalised taps", raw.astype(np.float64), "single-sum"),
        ("taps in double, single sum", taps_double, "single-sum"),
    ]
    for name, k, mode in rows:
        b = bias_of(conv_x(r, k, mode), conv_x(w, k, mode))
        print(f"{name:34s} identical to FSL in {np.mean(b[mask] == fb[mask]):.4%} of the mask")


if __name__ == "__main__":
    main()
