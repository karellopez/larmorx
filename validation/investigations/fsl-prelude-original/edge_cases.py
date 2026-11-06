# SPDX-License-Identifier: Apache-2.0
"""PRELUDE's validation plan's cases (`specs/prelude.md` §11) on generated inputs, run with two
programs in both D13 modes and compared: exit status, standard output, standard error without
larmorx's own lines (those starting with ``larmorx``), and every file written (every header field
but ``descrip``, a pair's header file's length, the data bit for bit).

Written for the clean-room original against the replica (the replica's record: bit-identical to
FSL 6.0.7.23 on its cases); no FSL program runs here. Usage::

    python -P edge_cases.py <work dir> --a "<larmorx> fsl prelude" --b "<larmorx-nc> fsl prelude"
        [--only <case name prefix>] [--mode fix|reproduce|both]

One line per differing case and mode, then the counts. Inputs: seeded numpy (a wrapped quadratic
phase with a linear ramp over an ellipsoid, its magnitude with noise, ...), written with nibabel.
"""

from __future__ import annotations

import argparse
import os
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

SHAPE = (24, 20, 10)


def wrap(p: np.ndarray) -> np.ndarray:
    return (np.mod(p + np.pi, 2 * np.pi) - np.pi).astype(np.float32)


def affine(neuro: bool = False, vox=(2.0, 2.0, 3.0)) -> np.ndarray:
    a = np.diag([-vox[0] if not neuro else vox[0], vox[1], vox[2], 1.0])
    a[:3, 3] = [20.0, -18.0, -12.0]
    return a


def save(path: Path, data: np.ndarray, aff=None, dtype=None, slope=None, endian="<", **hdr):
    img = nib.Nifti1Image(data, affine(False) if aff is None else aff)
    h = img.header
    if dtype is not None:
        h.set_data_dtype(dtype)
    if slope is not None:
        h["scl_slope"], h["scl_inter"] = slope, 0.0
    for k, v in hdr.items():
        h[k] = v
    if endian == ">":
        img = nib.Nifti1Image(data, img.affine, h.as_byteswapped(">"))
    nib.save(img, path)


def make_inputs(d: Path) -> None:
    rng = np.random.default_rng(20261106)
    x, y, z = np.meshgrid(*(np.arange(n, dtype=np.float64) for n in SHAPE), indexing="ij")
    c = [(n - 1) / 2 for n in SHAPE]
    r = ((x - c[0]) / 10.5) ** 2 + ((y - c[1]) / 8.5) ** 2 + ((z - c[2]) / 4.6) ** 2
    inside = r < 1
    truth = 0.035 * (x - c[0]) ** 2 + 0.02 * (y - c[1]) ** 2 + 0.45 * x - 0.3 * z
    phase = wrap(truth + rng.normal(0, 0.05, SHAPE))
    mag = np.where(inside, 1000.0, 15.0) + rng.normal(0, 8, SHAPE)
    mag = mag.astype(np.float32)
    save(d / "ph.nii.gz", phase)
    save(d / "mag.nii.gz", mag)
    save(d / "mask.nii.gz", inside.astype(np.float32))
    save(d / "ph2pi.nii.gz", (phase + np.float32(np.pi)).astype(np.float32))
    save(d / "phneuro.nii.gz", phase, aff=affine(True))
    save(d / "magneuro.nii.gz", mag, aff=affine(True))
    # a series of three volumes, a mask series of two
    ser = np.stack([wrap(truth * (1 + 0.2 * k) + rng.normal(0, 0.05, SHAPE)) for k in range(3)], -1)
    save(d / "ph3.nii.gz", ser)
    save(d / "mag3.nii.gz", np.stack([mag] * 3, -1))
    save(d / "mask2.nii.gz", np.stack([inside, inside & (z < 7)], -1).astype(np.float32))
    # int16 phase with a slope
    save(d / "phint.nii.gz", np.round(phase / 0.001).astype(np.int16), dtype=np.int16, slope=0.001)
    # one slice, a wide image
    save(d / "ph1.nii.gz", phase[:, :, 5:6])
    save(d / "mag1.nii.gz", mag[:, :, 5:6])
    wide_shape = (70, 10, 4)
    wx = np.arange(70, dtype=np.float64)[:, None, None] * np.ones(wide_shape)
    save(d / "phwide.nii.gz", wrap(0.4 * wx + rng.normal(0, 0.05, wide_shape)))
    save(d / "magwide.nii.gz", (1000 + rng.normal(0, 8, wide_shape)).astype(np.float32))
    # complex images
    z64 = (mag * np.exp(1j * phase)).astype(np.complex64)
    save(d / "cplx.nii.gz", z64, dtype=np.complex64)
    save(d / "cplxbe.nii", z64, dtype=np.complex64, endian=">")
    save(d / "cplx128.nii.gz", z64.astype(np.complex128), dtype=np.complex128)
    save(d / "notcplx.nii.gz", phase)
    save(d / "notcplx16.nii.gz", np.round(phase * 1000).astype(np.int16), dtype=np.int16)
    # NaN values
    m = mag.copy()
    m[0, 0, 0] = np.nan
    save(d / "magnanfirst.nii.gz", m)
    m = mag.copy()
    m[12, 10, 5] = np.nan
    m[3, 3, 3] = np.nan
    save(d / "magnaninside.nii.gz", m)
    p = phase.copy()
    p[0, 0, 0] = np.nan
    save(d / "phnanfirst.nii.gz", p)
    p2 = (phase + np.float32(np.pi)).astype(np.float32)
    p2[0, 0, 0] = np.nan
    save(d / "ph2pinanfirst.nii.gz", p2)
    p = phase.copy()
    p[12, 10, 5] = np.nan
    p[13, 10, 5] = np.nan
    save(d / "phnaninside.nii.gz", p)
    # a mask of two parts (the largest kept), a magnitude of another size
    two = inside.copy()
    two[11:13, :, :] = False
    save(d / "maskgap.nii.gz", two.astype(np.float32))
    big = np.pad(mag, ((0, 2), (0, 0), (0, 0)), constant_values=15.0)
    save(d / "magbig.nii.gz", big)
    save(d / "magsmall.nii.gz", mag[:-1, :, :])
    # a range above 3 pi
    save(d / "phrange.nii.gz", (phase * 1.6).astype(np.float32))


CASES: list[tuple[str, str]] = [
    ("basic", "-p ph -a mag -o o -v"),
    ("n2", "-p ph -a mag -o o -n 2 -v"),
    ("n3", "-p ph -a mag -o o -n 3"),
    ("n24", "-p ph -a mag -o o -n 24 -v"),
    ("t0", "-p ph -a mag -o o -t 0 -v"),
    ("t300", "-p ph -a mag -o o -t 300 --savemask=m"),
    ("mask", "-p ph -a mag -m mask -o o -v --savemask=m"),
    ("mask-t", "-p ph -a mag -m mask -t 0.2 -o o --savemask=m"),
    ("maskgap", "-p ph -a mag -m maskgap -o o -v --savemask=m -l lab"),
    ("slices", "-p ph -a mag -o o -s -v --savemask=m -l lab -r raw"),
    ("labelslices", "-p ph -a mag -o o --labelslices -v -l lab"),
    ("force3d-slices", "-p ph -a mag -o o -s -f -v"),
    ("ramps", "-p ph -a mag -o o --removeramps -v -r raw -l lab --savemask=m"),
    ("ramps-slices", "-p ph -a mag -o o -s --removeramps -v -r raw -l lab --savemask=m"),
    ("ramps-labelslices", "-p ph -a mag -o o --labelslices --removeramps -r raw"),
    ("ramps-noverbose", "-p ph -a mag -o o --removeramps -r raw"),
    ("one-slice", "-p ph1 -a mag1 -o o -v -l lab"),
    ("one-slice-ramps", "-p ph1 -a mag1 -o o --removeramps -v -r raw"),
    ("one-slice-s", "-p ph1 -a mag1 -o o -s -v"),
    ("wide", "-p phwide -a magwide -o o -v -l lab"),
    ("wide-f", "-p phwide -a magwide -o o -f -v -l lab"),
    ("neuro", "-p phneuro -a magneuro -o o -v --savemask=m -l lab -r raw"),
    ("neuro-ramps", "-p phneuro -a magneuro -o o --removeramps -r raw"),
    ("int16-slope", "-p phint -a mag -o o -v -r raw"),
    ("rewrap", "-p ph2pi -a mag -o o -v -r raw"),
    ("series", "-p ph3 -a mag3 -o o -v --savemask=m -l lab -r raw"),
    ("series-start", "-p ph3 -a mag3 -o o --start=1 -v"),
    ("series-start-end", "-p ph3 -a mag3 -o o --start=1 --end=1 -r raw"),
    ("series-mask2", "-p ph3 -a mag3 -m mask2 -o o -v --savemask=m"),
    ("series-short-mag", "-p ph3 -a mag -o o -v --savemask=m -l lab"),
    ("end-before-start", "-p ph3 -a mag3 -o o --start=2 --end=1 -r raw"),
    ("end-before-start-outside", "-p ph3 -a mag3 -o o --start=4 --end=1"),
    ("complex", "-c cplx -o o -v --savemask=m -l lab -r raw"),
    ("complex-be", "-c cplxbe -o o -v"),
    ("complex128", "-c cplx128 -o o -v"),
    ("complex-float32", "-c notcplx -o o -v"),
    ("complex-int16", "-c notcplx16 -o o"),
    ("complex-as-phase", "-p cplx -a mag -o o"),
    ("complex-as-mask", "-p ph -a mag -m cplx -o o"),
    ("nan-mag-first", "-p ph -a magnanfirst -o o -v --savemask=m"),
    ("nan-mag-inside", "-p ph -a magnaninside -o o -v --savemask=m"),
    ("nan-mag-first-s", "-p ph -a magnanfirst -o o -s -v --savemask=m"),
    ("nan-phase-first", "-p phnanfirst -a mag -o o -v"),
    ("nan-phase-first-2pi", "-p ph2pinanfirst -a mag -o o -v -r raw"),
    ("nan-phase-inside", "-p phnaninside -a mag -o o -v --savemask=m -l lab -r raw"),
    ("nan-phase-inside-s", "-p phnaninside -a mag -o o -s -v -l lab"),
    ("nan-phase-inside-ramps", "-p phnaninside -a mag -o o --removeramps -v -r raw"),
    ("size-big", "-p ph -a magbig -o o -v --savemask=m"),
    ("size-small", "-p ph -a magsmall -o o -v --savemask=m"),
    ("size-mask", "-p ph -a mag -m magsmall -o o --savemask=m"),
    ("range", "-p phrange -a mag -o o -v"),
    ("grot", "-p ph -a mag -o o -v --removeramps"),
    (
        "long-names",
        "--phase=ph --abs=mag --unwrap=o --numphasesplit=5 --thresh=100 --rawphase=raw --labels=lab --verbose",
    ),
    ("u-grouped", "-p ph -a mag -u o -fvn 4"),
    (
        "type-pair",
        "-p ph -a mag -o o -l lab",
    ),
    ("type-nifti2", "-p ph -a mag -o o --savemask=m"),
    ("type-unset", "-p ph -a mag -o o -v --savemask=m"),
    ("type-bogus", "-p ph -a mag -o o -v --savemask=m"),
]

ENV: dict[str, dict[str, str | None]] = {
    "type-pair": {"FSLOUTPUTTYPE": "NIFTI_PAIR"},
    "type-nifti2": {"FSLOUTPUTTYPE": "NIFTI2_GZ"},
    "type-unset": {"FSLOUTPUTTYPE": None},
    "type-bogus": {"FSLOUTPUTTYPE": "bogus"},
}


def run(program: str, case: str, args: str, inputs: Path, d: Path, mode: str):
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for f in inputs.iterdir():
        os.symlink(f, d / f.name)
    env = {
        "PATH": "/usr/bin:/bin",
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "LARMORX_UPSTREAM_DEFECTS": mode,
        "OMP_NUM_THREADS": "4",
        # larmorx's own program runs its original, never a replica it finds
        "LARMORX_IMPLEMENTATION": "original",
    }
    for k, v in ENV.get(case, {}).items():
        if v is None:
            env.pop(k, None)
        else:
            env[k] = v
    r = subprocess.run(
        [*shlex.split(program), *shlex.split(args)], cwd=d, env=env, capture_output=True, text=True
    )
    err = [ln for ln in r.stderr.splitlines() if not ln.startswith("larmorx")]
    files = {p.name: p for p in d.iterdir() if not p.is_symlink()}
    return r.returncode, r.stdout, err, files, r.stderr


def _plain(p: Path) -> bytes:
    """A file's bytes, decompressed when gzip-compressed."""
    import gzip

    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def compare_files(fa: dict, fb: dict) -> list[str]:
    out = []
    if set(fa) != set(fb):
        out.append(f"files {sorted(set(fa) ^ set(fb))}")
    for name in sorted(set(fa) & set(fb)):
        if not any(name.endswith(e) for e in (".nii", ".nii.gz", ".hdr")):
            if fa[name].read_bytes() != fb[name].read_bytes():
                out.append(f"{name} bytes")
            continue
        ra_bytes, rb_bytes = _plain(fa[name]), _plain(fb[name])
        if name.endswith((".hdr", ".hdr.gz")) and len(ra_bytes) != len(rb_bytes):
            out.append(f"{name} header file {len(ra_bytes)} vs {len(rb_bytes)} bytes")
        ia, ib = nib.load(fa[name]), nib.load(fb[name])
        for k in ia.header:
            if k == "descrip":
                continue
            if np.asarray(ia.header[k]).tobytes() != np.asarray(ib.header[k]).tobytes():
                out.append(f"{name} header {k}")
        ra, rb = np.asanyarray(ia.dataobj), np.asanyarray(ib.dataobj)
        if ra.shape != rb.shape or ra.dtype != rb.dtype:
            out.append(f"{name} data {ra.shape}/{ra.dtype} vs {rb.shape}/{rb.dtype}")
        elif ra.tobytes() != rb.tobytes():
            x = np.frombuffer(ra.tobytes(), np.uint32 if ra.itemsize == 4 else np.uint8)
            y = np.frombuffer(rb.tobytes(), np.uint32 if rb.itemsize == 4 else np.uint8)
            out.append(f"{name} data: {int(np.sum(x != y))} values differ")
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--a", required=True)
    ap.add_argument("--b", required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("--mode", default="both")
    ap.add_argument("--show", action="store_true", help="print both programs' standard error")
    a = ap.parse_args()
    inputs = a.work / "inputs"
    if not inputs.is_dir():
        inputs.mkdir(parents=True)
        make_inputs(inputs)
    modes = ["fix", "reproduce"] if a.mode == "both" else [a.mode]
    same = differ = 0
    for case, args in CASES:
        if not case.startswith(a.only):
            continue
        for mode in modes:
            ra = run(a.a, case, args, inputs, a.work / "a" / mode / case, mode)
            rb = run(a.b, case, args, inputs, a.work / "b" / mode / case, mode)
            problems = []
            if ra[0] != rb[0]:
                problems.append(f"exit {ra[0]} vs {rb[0]}")
            if ra[1] != rb[1]:
                problems.append("stdout")
            if ra[2] != rb[2]:
                problems.append("stderr")
            problems += compare_files(ra[3], rb[3])
            if problems:
                differ += 1
                print(f"{case} [{mode}] (exit {ra[0]}): " + "; ".join(problems[:6]))
            else:
                same += 1
            if a.show:
                print(f"--- {case} [{mode}] a:\n{ra[4]}--- b:\n{rb[4]}")
    print(f"{same} identical, {differ} differ")
    return 0


if __name__ == "__main__":
    sys.exit(main())
