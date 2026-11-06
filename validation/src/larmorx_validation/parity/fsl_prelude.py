# SPDX-License-Identifier: Apache-2.0
"""Parity of PRELUDE's clean-room original (``larmorx fsl prelude``, crate larmorx-fsl,
Apache-2.0, written from ``specs/prelude.md``) with FSL 6.0.7.23's ``prelude``.

Two kinds of cases (``specs/prelude.md`` §11):

- **The replica suite's** (``fsl-prelude-replica``: fMRIPrep's seven calls, the data panel's
  phase-difference maps, every option, output type, message and failure, FSL-261 to FSL-272 in
  both modes; categories as there): that suite is run as a black box (never read; this suite
  calls its ``suite()``) with larmorx's original standing in for the replica's program (a small
  generated program on ``LARMORX_NC_BIN`` that runs ``larmorx fsl prelude`` with the original
  forced and refuses to run inside itself), so each of its cases compares the original with FSL's
  run from the oracle cache, by that suite's checks. Its texts say "the replica" where they mean
  the program under test; this suite's record says "the original".
- **The original's own** (categories ``orig-*``): the validation plan's cases on generated inputs
  (``orig-options``, ``orig-images``, ``orig-nan``, ``orig-errors``, ``orig-types``) and fMRIPrep's
  seven calls (``orig-fmriprep``), each run three ways: FSL (through the oracle cache), the
  original with ``reproduce`` and the original by default, and the replica by default (the
  ``larmorx-nc`` program, a black box: ``LARMORX_PRELUDE_REPLICA``, else ``LARMORX_NC_BIN`` as
  given, else the ``crates-nc`` build).

What the original's own cases compare:

- with ``reproduce``, against FSL: the exit status (FSL's abort, signal 6: the original exits 1
  after FSL's lines and its own), standard output, standard error without the original's
  ``larmorx`` lines (the run directory and FSL's program path replaced), the files written, and
  every file bit for bit, decompressed: a header file whole (its length, the four extension
  bytes and the extensions included), ``descrip`` left out; where FSL takes values from memory it
  never wrote (FSL-263, FSL-265) only the status, the messages and the names;
- by default, against the replica's default: the same, and the upstream defects named (the IDs
  in the larmorx lines) the same; where GLIBC-07 is named (``-c``: the correctly rounded
  arctangent against glibc's ``atan2f``) the masks bit for bit, and the unwrapped phase with the
  replica's integer offsets on at least 99.9 % of the mask's voxels (``|U - U_replica| < π``), the
  labels and the phases' units in the last place reported;
- the D13 pass: where the default's exit status, output or files differ from ``reproduce``'s, a
  defect is named;
- fMRIPrep's calls: both modes' unwrapped phase also against fMRIPrep's own output (its data bit
  for bit, its header's fields but ``descrip``).
"""

from __future__ import annotations

import bz2
import contextlib
import dataclasses
import gzip
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation import oracle_cache
from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    FAIL,
    SKIPPED,
    Case,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "fsl-prelude"
PROGRAM = "prelude"
#: The original's and the replica's thread count in the own cases.
LX_THREADS = 4
#: The replica's program as this process was given it, before the stand-in replaces it.
_GIVEN_NC = os.environ.get("LARMORX_NC_BIN")
#: Set inside the stand-in: it refuses to run inside itself.
_STANDIN_GUARD = "LARMORX_PRELUDE_STANDIN"
#: The share of the mask's voxels whose unwrapped phase must keep the replica's integer offset
#: where GLIBC-07 is named (``specs/prelude.md`` §11).
GLIBC07_SHARE = 0.999


# ------------------------------------------------------------------------------------------------
# The programs


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_PRELUDE_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_PRELUDE_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def replica_program() -> Path | None:
    """The replica's program (a black box), if built."""
    for p in (os.environ.get("LARMORX_PRELUDE_REPLICA"), _GIVEN_NC):
        if p and Path(p).is_file():
            return Path(p)
    root = Path(__file__).resolve().parents[4]
    p = root / "crates-nc" / "target" / "release" / "larmorx-nc"
    return p if p.is_file() else None


def _base_env(home: Path, settings: dict[str, str | None]) -> dict[str, str]:
    env = {
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsl_versions.fsldir()),
        "HOME": str(home),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        **fsl_versions.openblas_env(),
    }
    for k, v in settings.items():
        if v is None:
            env.pop(k, None)
        else:
            env[k] = v
    return env


@dataclasses.dataclass
class Ran:
    code: int | None
    stdout: str
    stderr: str


def run_fsl(args: list[str], cwd: Path, env: dict[str, str], never: str = "") -> Ran:
    inputs = [p for p in cwd.iterdir() if p.is_file()]
    r = oracle_cache.run(
        [str(fsl_versions.program(PROGRAM)), *args],
        env=env,
        cwd=cwd,
        outputs=[cwd],
        inputs=inputs,
        policy=oracle_cache.Policy(threads=1, family="fsl", never=never),
        runner=SUITE,
        timeout=900,
    )
    return Ran(r.returncode, r.stdout, r.stderr)


def _run(argv: list[str], cwd: Path, env: dict[str, str]) -> Ran:
    p = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=1800,
    )
    return Ran(p.returncode, p.stdout, p.stderr)


def run_original(args: list[str], cwd: Path, env: dict[str, str], mode: str) -> Ran:
    e = dict(env)
    e[defects.ENV] = mode
    e["OMP_NUM_THREADS"] = str(LX_THREADS)
    e["LARMORX_IMPLEMENTATION"] = "original"
    for k in ("PYTHONPATH", "VIRTUAL_ENV"):
        if k in os.environ:
            e[k] = os.environ[k]
    return _run([*original_command(), "fsl", PROGRAM, *args], cwd, e)


def run_replica(program: Path, args: list[str], cwd: Path, env: dict[str, str]) -> Ran:
    e = dict(env)
    e[defects.ENV] = "fix"
    e["OMP_NUM_THREADS"] = str(LX_THREADS)
    return _run([str(program), "fsl", PROGRAM, *args], cwd, e)


# ------------------------------------------------------------------------------------------------
# The own cases' inputs (seeded numpy; plain arithmetic and numpy's wrap)

SHAPE = (24, 20, 10)


def _wrap(p: np.ndarray) -> np.ndarray:
    return (np.mod(p + np.pi, 2 * np.pi) - np.pi).astype(np.float32)


def _affine(neuro: bool = False, vox=(2.0, 2.0, 3.0)) -> np.ndarray:
    a = np.diag([-vox[0] if not neuro else vox[0], vox[1], vox[2], 1.0])
    a[:3, 3] = [20.0, -18.0, -12.0]
    return a


def _save(path: Path, data: np.ndarray, aff=None, dtype=None, slope=None, endian="<") -> None:
    import nibabel as nib

    img = nib.Nifti1Image(data, _affine(False) if aff is None else aff)
    h = img.header
    if dtype is not None:
        h.set_data_dtype(dtype)
    if slope is not None:
        h["scl_slope"], h["scl_inter"] = slope, 0.0
    if endian == ">":
        img = nib.Nifti1Image(data, img.affine, h.as_byteswapped(">"))
    nib.save(img, path)


def make_inputs(d: Path) -> None:
    """The own cases' inputs: a wrapped quadratic phase with a linear ramp over an ellipsoid, its
    magnitude with noise, and the variations the cases name."""
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(20261106)
    x, y, z = np.meshgrid(*(np.arange(n, dtype=np.float64) for n in SHAPE), indexing="ij")
    c = [(n - 1) / 2 for n in SHAPE]
    r = ((x - c[0]) / 10.5) ** 2 + ((y - c[1]) / 8.5) ** 2 + ((z - c[2]) / 4.6) ** 2
    inside = r < 1
    truth = 0.035 * (x - c[0]) ** 2 + 0.02 * (y - c[1]) ** 2 + 0.45 * x - 0.3 * z
    phase = _wrap(truth + rng.normal(0, 0.05, SHAPE))
    mag = (np.where(inside, 1000.0, 15.0) + rng.normal(0, 8, SHAPE)).astype(np.float32)
    _save(d / "ph.nii.gz", phase)
    _save(d / "mag.nii.gz", mag)
    _save(d / "mask.nii.gz", inside.astype(np.float32))
    _save(d / "ph2pi.nii.gz", (phase + np.float32(np.pi)).astype(np.float32))
    _save(d / "phneuro.nii.gz", phase, aff=_affine(True))
    _save(d / "magneuro.nii.gz", mag, aff=_affine(True))
    ser = np.stack(
        [_wrap(truth * (1 + 0.2 * k) + rng.normal(0, 0.05, SHAPE)) for k in range(3)], -1
    )
    _save(d / "ph3.nii.gz", ser)
    _save(d / "mag3.nii.gz", np.stack([mag] * 3, -1))
    _save(d / "mask2.nii.gz", np.stack([inside, inside & (z < 7)], -1).astype(np.float32))
    _save(d / "phint.nii.gz", np.round(phase / 0.001).astype(np.int16), dtype=np.int16, slope=0.001)
    _save(d / "ph1.nii.gz", phase[:, :, 5:6])
    _save(d / "mag1.nii.gz", mag[:, :, 5:6])
    wide_shape = (70, 10, 4)
    wx = np.arange(70, dtype=np.float64)[:, None, None] * np.ones(wide_shape)
    _save(d / "phwide.nii.gz", _wrap(0.4 * wx + rng.normal(0, 0.05, wide_shape)))
    _save(d / "magwide.nii.gz", (1000 + rng.normal(0, 8, wide_shape)).astype(np.float32))
    z64 = (mag * np.exp(1j * phase)).astype(np.complex64)
    _save(d / "cplx.nii.gz", z64, dtype=np.complex64)
    _save(d / "cplxbe.nii", z64, dtype=np.complex64, endian=">")
    _save(d / "cplx128.nii.gz", z64.astype(np.complex128), dtype=np.complex128)
    _save(d / "notcplx.nii.gz", phase)
    _save(d / "notcplx16.nii.gz", np.round(phase * 1000).astype(np.int16), dtype=np.int16)
    m = mag.copy()
    m[0, 0, 0] = np.nan
    _save(d / "magnanfirst.nii.gz", m)
    m = mag.copy()
    m[12, 10, 5] = np.nan
    m[3, 3, 3] = np.nan
    _save(d / "magnaninside.nii.gz", m)
    p = phase.copy()
    p[0, 0, 0] = np.nan
    _save(d / "phnanfirst.nii.gz", p)
    p2 = (phase + np.float32(np.pi)).astype(np.float32)
    p2[0, 0, 0] = np.nan
    _save(d / "ph2pinanfirst.nii.gz", p2)
    p = phase.copy()
    p[12, 10, 5] = np.nan
    p[13, 10, 5] = np.nan
    _save(d / "phnaninside.nii.gz", p)
    two = inside.copy()
    two[11:13, :, :] = False
    _save(d / "maskgap.nii.gz", two.astype(np.float32))
    _save(d / "magbig.nii.gz", np.pad(mag, ((0, 2), (0, 0), (0, 0)), constant_values=15.0))
    _save(d / "magsmall.nii.gz", mag[:-1, :, :])
    _save(d / "phrange.nii.gz", (phase * 1.6).astype(np.float32))


@dataclasses.dataclass(frozen=True)
class Own:
    """An own case: its arguments (input names without their extension, in the run directory),
    its settings, whether FSL takes values from memory it never wrote, its tier."""

    args: str
    settings: tuple[tuple[str, str | None], ...] = ()
    undefined: str = ""
    tier: str = "standard"


_UNDEF_263 = "FSL-263: the imaginary parts (and a narrower type's values past its data) from memory FSL never wrote"
_UNDEF_265 = "FSL-265: every value from memory past an empty buffer"

#: (category, name) -> case. The validation plan's cases (§11) the replica suite's generated
#: inputs do not reach in the same form, with the outputs of every kind.
OWN: dict[tuple[str, str], Own] = {
    ("orig-options", "basic"): Own("-p ph -a mag -o o -v", tier="smoke"),
    ("orig-options", "n2"): Own("-p ph -a mag -o o -n 2 -v"),
    ("orig-options", "n3"): Own("-p ph -a mag -o o -n 3"),
    ("orig-options", "n24"): Own("-p ph -a mag -o o -n 24 -v"),
    ("orig-options", "t0"): Own("-p ph -a mag -o o -t 0 -v"),
    ("orig-options", "t300"): Own("-p ph -a mag -o o -t 300 --savemask=m"),
    ("orig-options", "mask"): Own("-p ph -a mag -m mask -o o -v --savemask=m"),
    ("orig-options", "mask-t"): Own("-p ph -a mag -m mask -t 0.2 -o o --savemask=m"),
    ("orig-options", "maskgap"): Own("-p ph -a mag -m maskgap -o o -v --savemask=m -l lab"),
    ("orig-options", "slices"): Own("-p ph -a mag -o o -s -v --savemask=m -l lab -r raw"),
    ("orig-options", "labelslices"): Own("-p ph -a mag -o o --labelslices -v -l lab"),
    ("orig-options", "force3d-slices"): Own("-p ph -a mag -o o -s -f -v"),
    ("orig-options", "ramps"): Own("-p ph -a mag -o o --removeramps -v -r raw -l lab --savemask=m", tier="smoke"),
    ("orig-options", "ramps-slices"): Own("-p ph -a mag -o o -s --removeramps -v -r raw -l lab --savemask=m"),
    ("orig-options", "ramps-labelslices"): Own("-p ph -a mag -o o --labelslices --removeramps -r raw"),
    ("orig-options", "ramps-noverbose"): Own("-p ph -a mag -o o --removeramps -r raw"),
    ("orig-options", "grot"): Own("-p ph -a mag -o o -v --removeramps"),
    ("orig-options", "long-names"): Own(
        "--phase=ph --abs=mag --unwrap=o --numphasesplit=5 --thresh=100 --rawphase=raw --labels=lab --verbose"
    ),
    ("orig-options", "u-grouped"): Own("-p ph -a mag -u o -fvn 4"),
    ("orig-images", "one-slice"): Own("-p ph1 -a mag1 -o o -v -l lab"),
    ("orig-images", "one-slice-ramps"): Own("-p ph1 -a mag1 -o o --removeramps -v -r raw"),
    ("orig-images", "one-slice-s"): Own("-p ph1 -a mag1 -o o -s -v"),
    ("orig-images", "wide"): Own("-p phwide -a magwide -o o -v -l lab"),
    ("orig-images", "wide-f"): Own("-p phwide -a magwide -o o -f -v -l lab"),
    ("orig-images", "neuro"): Own("-p phneuro -a magneuro -o o -v --savemask=m -l lab -r raw"),
    ("orig-images", "neuro-ramps"): Own("-p phneuro -a magneuro -o o --removeramps -r raw"),
    ("orig-images", "int16-slope"): Own("-p phint -a mag -o o -v -r raw"),
    ("orig-images", "rewrap"): Own("-p ph2pi -a mag -o o -v -r raw"),
    ("orig-images", "series"): Own("-p ph3 -a mag3 -o o -v --savemask=m -l lab -r raw"),
    ("orig-images", "series-start"): Own("-p ph3 -a mag3 -o o --start=1 -v"),
    ("orig-images", "series-start-end"): Own("-p ph3 -a mag3 -o o --start=1 --end=1 -r raw"),
    ("orig-images", "series-mask2"): Own("-p ph3 -a mag3 -m mask2 -o o -v --savemask=m"),
    ("orig-images", "series-short-mag"): Own("-p ph3 -a mag -o o -v --savemask=m -l lab"),
    ("orig-images", "complex"): Own("-c cplx -o o -v --savemask=m -l lab -r raw", tier="smoke"),
    ("orig-images", "complex-be"): Own("-c cplxbe -o o -v"),
    ("orig-images", "complex128"): Own("-c cplx128 -o o -v", undefined=_UNDEF_265),
    ("orig-images", "complex-float32"): Own("-c notcplx -o o -v", undefined=_UNDEF_263),
    ("orig-images", "complex-int16"): Own("-c notcplx16 -o o", undefined=_UNDEF_263),
    ("orig-images", "size-big"): Own("-p ph -a magbig -o o -v --savemask=m"),
    ("orig-images", "size-small"): Own("-p ph -a magsmall -o o -v --savemask=m"),
    ("orig-images", "size-mask"): Own("-p ph -a mag -m magsmall -o o --savemask=m"),
    ("orig-nan", "mag-first"): Own("-p ph -a magnanfirst -o o -v --savemask=m"),
    ("orig-nan", "mag-inside"): Own("-p ph -a magnaninside -o o -v --savemask=m"),
    ("orig-nan", "mag-first-s"): Own("-p ph -a magnanfirst -o o -s -v --savemask=m"),
    ("orig-nan", "phase-first"): Own("-p phnanfirst -a mag -o o -v"),
    ("orig-nan", "phase-first-2pi"): Own("-p ph2pinanfirst -a mag -o o -v -r raw"),
    ("orig-nan", "phase-inside"): Own("-p phnaninside -a mag -o o -v --savemask=m -l lab -r raw"),
    ("orig-nan", "phase-inside-s"): Own("-p phnaninside -a mag -o o -s -v -l lab"),
    ("orig-nan", "phase-inside-ramps"): Own("-p phnaninside -a mag -o o --removeramps -v -r raw"),
    ("orig-errors", "end-before-start"): Own("-p ph3 -a mag3 -o o --start=2 --end=1 -r raw"),
    ("orig-errors", "end-before-start-outside"): Own("-p ph3 -a mag3 -o o --start=4 --end=1"),
    ("orig-errors", "complex-as-phase"): Own("-p cplx -a mag -o o"),
    ("orig-errors", "complex-as-mask"): Own("-p ph -a mag -m cplx -o o"),
    ("orig-errors", "range"): Own("-p phrange -a mag -o o -v"),
    ("orig-errors", "no-dir"): Own("-p ph -a mag -o nodir/o -v"),
    ("orig-types", "pair"): Own("-p ph -a mag -o o -l lab --savemask=m -r raw", (("FSLOUTPUTTYPE", "NIFTI_PAIR"),)),
    ("orig-types", "pair-gz"): Own("-p ph -a mag -o o -l lab", (("FSLOUTPUTTYPE", "NIFTI_PAIR_GZ"),)),
    ("orig-types", "pair-zst"): Own("-p ph -a mag -o o --savemask=m", (("FSLOUTPUTTYPE", "NIFTI_PAIR_ZST"),)),
    ("orig-types", "nifti2-pair-bz2"): Own("-p ph -a mag -o o -l lab", (("FSLOUTPUTTYPE", "NIFTI2_PAIR_BZ2"),)),
    ("orig-types", "nifti2-gz"): Own("-p ph -a mag -o o --savemask=m", (("FSLOUTPUTTYPE", "NIFTI2_GZ"),)),
    ("orig-types", "nifti-zst"): Own("-p ph -a mag -o o -r raw", (("FSLOUTPUTTYPE", "NIFTI_ZST"),)),
    ("orig-types", "unset"): Own("-p ph -a mag -o o -v --savemask=m", (("FSLOUTPUTTYPE", None),)),
    ("orig-types", "bogus"): Own("-p ph -a mag -o o -v --savemask=m", (("FSLOUTPUTTYPE", "bogus"),)),
}  # fmt: skip


def fmriprep_calls(root: Path | None = None) -> list[tuple[str, Path, list[str]]]:
    """fMRIPrep 25.2.6's prelude calls: (run/node, the node folder, the arguments with the
    container's ``/work`` mapped and the output in the run directory)."""
    ws = fsl_versions.workspace()
    root = root or (ws / "oracles" / "fmriprep-25.2.6" if ws else None)
    if root is None or not root.is_dir():
        return []
    out = []
    for cmd in sorted(root.glob("*/work/**/phdiff_wf/prelude/command.txt")):
        run = cmd.relative_to(root).parts[0]
        work = root / run / "work"
        words = [w.replace("=/work/", f"={work}/") for w in shlex.split(cmd.read_text())[1:]]
        words = [w if not w.startswith("--unwrap=") else "--unwrap=out.nii.gz" for w in words]
        out.append((f"{run}/{cmd.parent.parent.parent.name}", cmd.parent, words))
    return out


def _inputs_dir() -> Path:
    ws = fsl_versions.workspace()
    d = (
        (ws / "oracles" / "inputs" / SUITE)
        if ws
        else Path(tempfile.gettempdir()) / "lx-fsl-prelude-inputs_"
    )
    if not (d / "phrange.nii.gz").is_file():
        make_inputs(d)
    return d


# ------------------------------------------------------------------------------------------------
# Comparison


def _decompressed(path: Path) -> bytes:
    name = path.name
    if name.endswith(".gz"):
        return gzip.decompress(path.read_bytes())
    if name.endswith(".bz2"):
        return bz2.decompress(path.read_bytes())
    if name.endswith(".zst"):
        zstd = fsl_versions.fsldir() / "bin" / "zstd"
        return subprocess.run(
            [str(zstd), "-dcq", str(path)], capture_output=True, check=True
        ).stdout
    return path.read_bytes()


def _parts(path: Path) -> tuple[bytes, bytes]:
    """An image file's header bytes (a pair's header file whole; a single file's up to its data)
    with ``descrip`` blanked, and its data bytes (empty for a pair's header file)."""
    b = _decompressed(path)
    two = len(b) >= 4 and int.from_bytes(b[:4], "little") == 540
    lo = 240 if two else 148
    if ".nii" in path.name:
        vox = (
            int(np.frombuffer(b[168:176], "<i8")[0])
            if two
            else int(np.frombuffer(b[108:112], "<f4")[0])
        )
        head, data = b[:vox], b[vox:]
    else:
        head, data = b, b""
    return head[:lo] + bytes(80) + head[lo + 80 :], data


def _is_header(name: str) -> bool:
    return ".nii" in name or ".hdr" in name


def _written(d: Path, before: set[str]) -> dict[str, Path]:
    return {
        str(p.relative_to(d)): p
        for p in sorted(d.rglob("*"))
        if p.is_file() and str(p.relative_to(d)) not in before
    }


def _own_lines(text: str) -> list[str]:
    return [ln for ln in text.splitlines() if ln.startswith("larmorx") or ln.endswith("(larmorx)")]


def _messages(text: str, d: Path) -> str:
    """The text without larmorx's own lines, the run directory and FSL's program replaced."""
    keep = [
        ln
        for ln in text.splitlines()
        if not ln.startswith("larmorx") and not ln.endswith("(larmorx)")
    ]
    out = "\n".join(keep).replace(str(d), "<dir>")
    return out.replace(str(fsl_versions.program(PROGRAM)), PROGRAM)


def _named(text: str) -> set[str]:
    """The upstream defects larmorx's own lines name."""
    return {m for ln in _own_lines(text) for m in re.findall(r"\b(?:FSL|GLIBC)-\d+\b", ln)}


def _first_difference(a: str, b: str) -> str:
    xs, ys = a.splitlines(), b.splitlines()
    for k, (p, q) in enumerate(zip(xs, ys, strict=False)):
        if p != q:
            return f"line {k + 1}: {p[:150]!r} / {q[:150]!r}"
    return f"{len(xs)} vs {len(ys)} lines"


def _f32_ulps(x: bytes, y: bytes) -> int:
    a = np.frombuffer(x, "<i4").astype(np.int64)
    b = np.frombuffer(y, "<i4").astype(np.int64)
    a = np.where(a < 0, -(a & 0x7FFFFFFF), a)
    b = np.where(b < 0, -(b & 0x7FFFFFFF), b)
    return int(np.max(np.abs(a - b))) if a.size else 0


def _compare_runs(
    checks: CheckList, label: str, a: Ran, da: Path, fa: dict[str, Path], b: Ran, db: Path, fb: dict[str, Path],
    *, values: bool = True, glibc07: bool = False,
) -> bool:  # fmt: skip
    """``b`` (larmorx's run) against ``a``: status, messages, files; returns whether every check
    passed. ``values``: compare the files' bytes; ``glibc07``: the unwrapped phase by its integer
    offsets (the arctangent's last bits named)."""
    before = len(checks.checks)
    checks(f"{label}: exit status", a.code == b.code, detail=f"{a.code} / {b.code}")
    if values:
        # (where FSL takes values from memory it never wrote, -v's counts depend on them)
        checks(f"{label}: standard output", _messages(a.stdout, da) == _messages(b.stdout, db),
               detail=_first_difference(_messages(a.stdout, da), _messages(b.stdout, db)))  # fmt: skip
        ea, eb = _messages(a.stderr, da), _messages(b.stderr, db)
        checks(f"{label}: standard error", ea == eb, detail=_first_difference(ea, eb))
    checks(f"{label}: files written", set(fa) == set(fb), detail=f"{sorted(set(fa) ^ set(fb))}")
    if values:
        for name in sorted(set(fa) & set(fb)):
            if not _is_header(name) and ".img" not in name:
                checks(f"{label}: {name}", fa[name].read_bytes() == fb[name].read_bytes())
                continue
            if ".img" in name:
                x, y = _decompressed(fa[name]), _decompressed(fb[name])
                ha = hb = b""
            else:
                (ha, x), (hb, y) = _parts(fa[name]), _parts(fb[name])
                checks(f"{label}: {name} header", ha == hb, detail=f"{len(ha)} / {len(hb)} bytes")
            stem = name.split(".")[0]
            data_name = f"{label}: {name} data" if x or y else ""
            if not data_name:
                continue
            if glibc07 and stem in ("o", "raw", "out", "lab"):
                _glibc07_check(checks, label, name, stem, x, y, fa, fb)
            else:
                checks(data_name, x == y, detail=f"{len(x)} / {len(y)} bytes"
                       + (f"; up to {_f32_ulps(x, y)} ulps" if len(x) == len(y) and len(x) % 4 == 0 else ""))  # fmt: skip
    return all(c.passed for c in checks.checks[before:])


def _glibc07_check(checks, label, name, stem, x, y, fa, fb) -> None:
    """GLIBC-07 (§11): the unwrapped phase keeps the replica's integer offsets on 99.9 % of the
    mask's voxels; the raw phase's and the labels' differences reported."""
    if stem == "lab":
        a, b = np.frombuffer(x, "<i4"), np.frombuffer(y, "<i4")
        n = int(np.sum(a != b)) if a.size == b.size else -1
        checks(f"{label}: {name} (labels; GLIBC-07)", True, threshold="reported", value=n,
               detail=f"{n} of {a.size} labels differ")  # fmt: skip
        return
    a, b = np.frombuffer(x, "<f4"), np.frombuffer(y, "<f4")
    if a.size != b.size:
        checks(f"{label}: {name} data", False, detail=f"{a.size} / {b.size} values")
        return
    if stem == "raw":
        n = int(np.sum(a.view("<i4") != b.view("<i4")))
        checks(f"{label}: {name} (raw phase; GLIBC-07)", True, threshold="reported", value=n,
               detail=f"{n} of {a.size} values differ, up to {_f32_ulps(x, y)} ulps")  # fmt: skip
        return
    m = (a != 0) | (b != 0)
    same = np.abs(a.astype(np.float64) - b.astype(np.float64)) < np.pi
    share = float(np.sum(same & m) / max(1, int(np.sum(m))))
    checks(f"{label}: {name} (the replica's integer offsets; GLIBC-07)", share >= GLIBC07_SHARE,
           metric="share of the mask's voxels", value=round(share, 6), threshold=GLIBC07_SHARE,
           detail=f"{int(np.sum(m & ~same))} of {int(np.sum(m))} voxels move by a cycle; "
           f"{int(np.sum(a.view('<i4') != b.view('<i4')))} differ in their bits")  # fmt: skip


def _fmriprep_check(checks: CheckList, label: str, node: Path, out: Path) -> None:
    import nibabel as nib

    ref_path = next(node.glob("*_unwrapped.nii.gz"), None)
    if ref_path is None or not out.is_file():
        checks(f"{label}: fMRIPrep's own output", False, detail="missing")
        return
    ref, got = nib.load(ref_path), nib.load(out)
    ra, rb = np.asanyarray(ref.dataobj), np.asanyarray(got.dataobj)
    same = ra.dtype == rb.dtype and ra.tobytes() == rb.tobytes()
    fields = [k for k in ref.header if k != "descrip" and np.asarray(ref.header[k]).tobytes() != np.asarray(got.header[k]).tobytes()]  # fmt: skip
    checks(
        f"{label}: fMRIPrep's own unwrapped phase",
        same and not fields,
        detail=f"header fields differing: {fields}",
    )


# ------------------------------------------------------------------------------------------------
# The own cases


def _scratch() -> Path:
    base = Path(tempfile.gettempdir())
    return Path(tempfile.mkdtemp(prefix="lx-prelude_", dir=base))


def _place(base: Path, name: str, inputs: list[Path]) -> tuple[Path, set[str]]:
    d = base / name
    d.mkdir()
    for f in inputs:
        shutil.copy2(f, d / f.name)
    return d, {p.name for p in d.iterdir()}


def _run_own(args: list[str], settings: dict[str, str | None], inputs: list[Path], undefined: str,
             checks: CheckList, fmriprep_node: Path | None = None) -> None:  # fmt: skip
    replica = replica_program()
    base = _scratch()
    try:
        home = base / "home"
        home.mkdir()
        env = _base_env(home, settings)
        dfsl, bf = _place(base, "fsl", inputs)
        drep, br = _place(base, "reproduce", inputs)
        dfix, bx = _place(base, "fix", inputs)
        fsl = run_fsl(args, dfsl, env, never=undefined)
        org_r = run_original(args, drep, env, "reproduce")
        org_f = run_original(args, dfix, env, "fix")
        ffsl, frep, ffix = _written(dfsl, bf), _written(drep, br), _written(dfix, bx)
        # FSL reads memory it does not own: the original refuses with reproduce (§9.1)
        if "FSL reads memory it does not own" in org_r.stderr:
            checks("reproduce: refused where FSL reads memory it does not own", org_r.code == 1)
            raise Outcome(
                EXPECTED, "FSL reads memory it does not own here; the original refuses (D13)"
            )
        signal = fsl.code is not None and (fsl.code < 0 or fsl.code == 134)
        if signal:
            checks(
                "reproduce: FSL's abort, the original exits 1",
                org_r.code == 1,
                detail=f"{fsl.code} / {org_r.code}",
            )
            checks(
                "reproduce: the original names FSL's abort",
                any(ln.endswith("(larmorx)") for ln in org_r.stderr.splitlines()),
            )
            org_r = dataclasses.replace(org_r, code=fsl.code)
        ok = _compare_runs(
            checks, "reproduce", fsl, dfsl, ffsl, org_r, drep, frep, values=not undefined
        )
        # the default against the replica's default (bit for bit; GLIBC-07 by its offsets)
        named_fix = _named(org_f.stderr)
        if replica is None:
            raise Outcome(
                SKIPPED,
                "the replica's program is not built (LARMORX_PRELUDE_REPLICA, LARMORX_NC_BIN, crates-nc)",
            )
        dreplica, bp = _place(base, "replica", inputs)
        rep_f = run_replica(replica, args, dreplica, env)
        frf = _written(dreplica, bp)
        _compare_runs(checks, "default against the replica's", rep_f, dreplica, frf, org_f, dfix, ffix,
                      glibc07="GLIBC-07" in named_fix)  # fmt: skip
        # the same defects named (GLIBC-07 only by the original: the replica computes glibc's atan2f)
        named_rep = _named(rep_f.stderr)
        checks(
            "default: the upstream defects named",
            named_rep == named_fix - {"GLIBC-07"},
            detail=f"replica {sorted(named_rep)} / original {sorted(named_fix)}",
        )

        # D13: a difference between the modes is named
        differs = org_f.code != org_r.code or _messages(org_f.stdout, dfix) != _messages(org_r.stdout, drep) or set(ffix) != set(frep) or any(
            _decompressed(ffix[n]) != _decompressed(frep[n]) for n in set(ffix) & set(frep) if ".hdr" not in n
        )  # fmt: skip
        if differs:
            checks(
                "D13: the default's difference from reproduce is named",
                bool(named_fix),
                detail=f"named {sorted(named_fix)}",
            )
        if fmriprep_node is not None:
            _fmriprep_check(checks, "reproduce", fmriprep_node, drep / "out.nii.gz")
            _fmriprep_check(checks, "default", fmriprep_node, dfix / "out.nii.gz")
        if ok and fsl.code not in (0, None):
            raise Outcome(
                BOTH_ERROR,
                f"both stop (exit {fsl.code}): {(_messages(fsl.stderr, dfsl).strip().splitlines() or [''])[-1][:200]}",
            )
        if undefined and ok:
            checks("reproduce: values not compared", True, threshold="reported", detail=undefined)
    finally:
        shutil.rmtree(base, ignore_errors=True)


# ------------------------------------------------------------------------------------------------
# The replica suite's cases, the original standing in


_STANDIN = """import os, subprocess, sys
if os.environ.get({guard!r}):
    sys.exit("the stand-in for the replica's program was called from inside itself")
argv = sys.argv[1:]
if argv[:1] == ["--version"]:
    print({version!r})
    sys.exit(0)
if argv[:2] != ["fsl", "prelude"]:
    sys.exit("the stand-in runs fsl prelude only")
env = dict(os.environ)
env[{guard!r}] = "1"
env.pop("LARMORX_NC_BIN", None)
env["LARMORX_IMPLEMENTATION"] = "original"
env["PYTHONPATH"] = os.pathsep.join(p for p in ({path!r}, env.get("PYTHONPATH", "")) if p)
p = subprocess.run({command!r} + argv, env=env, stderr=subprocess.PIPE, check=False)
for ln in p.stderr.decode("utf-8", "replace").splitlines(keepends=True):
    if ln.startswith("larmorx: upstream defect"):
        ln = "larmorx-nc: upstream defect" + ln[len("larmorx: upstream defect"):]
    elif ln.startswith("larmorx: error: prelude:"):
        ln = "larmorx-nc: error: prelude:" + ln[len("larmorx: error: prelude:"):]
    elif ln.rstrip("\\n") == "larmorx fsl prelude: FSL aborts here (signal 6); exit status 1 (larmorx)":
        ln = "larmorx-nc: prelude aborts here (signal 6); the replica exits with status 1\\n"
    sys.stderr.write(ln)
sys.exit(p.returncode)
"""


def _standin() -> Path:
    """A program standing in for the replica's: ``<it> fsl prelude <args>`` runs larmorx's
    original (its own lines given the replica's prefixes, which the replica suite leaves out of
    its comparison with FSL); it refuses to run inside itself."""
    d = Path(tempfile.mkdtemp(prefix="lx-prelude-standin_"))
    script = d / "standin.py"
    import larmorx

    package = str(Path(larmorx.__file__).resolve().parents[1])
    script.write_text(
        _STANDIN.format(
            guard=_STANDIN_GUARD,
            command=original_command(),
            path=package,
            version=f"larmorx-nc {larmorx.__version__}",
        ),
        encoding="utf-8",
    )
    exe = d / "larmorx-nc"
    exe.write_text(
        f'#!/bin/sh\nexec {shlex.quote(sys.executable)} -I {shlex.quote(str(script))} "$@"\n',
        encoding="utf-8",
    )
    exe.chmod(0o755)
    return exe


_WORDS = (
    ("the replica's", "the original's"),
    ("The replica's", "The original's"),
    ("the replica", "the original"),
    ("The replica", "The original"),
    ("larmorx-nc", "larmorx"),
)


def _as_original(text: str) -> str:
    for a, b in _WORDS:
        text = text.replace(a, b)
    return text


@contextlib.contextmanager
def _standing_in():
    previous = os.environ.get("LARMORX_NC_BIN")
    os.environ["LARMORX_NC_BIN"] = str(_standin_program())
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop("LARMORX_NC_BIN", None)
        else:
            os.environ["LARMORX_NC_BIN"] = previous


_STANDIN_PROGRAM: list[Path] = []


def _standin_program() -> Path:
    if not _STANDIN_PROGRAM or not _STANDIN_PROGRAM[0].is_file():
        _STANDIN_PROGRAM[:] = [_standin()]
    return _STANDIN_PROGRAM[0]


_BASE: list[Suite] = []


def _replica_suite() -> Suite:
    """The replica suite, run as a black box (its module is never read)."""
    if not _BASE:
        from larmorx_validation.parity import fsl_prelude_replica as rep

        with _standing_in():
            _BASE.append(rep.suite())
    return _BASE[0]


def _run_replica_case(case: Case, checks: CheckList) -> None:
    base = _replica_suite()
    inner = CheckList()
    try:
        with _standing_in():
            base.run_case(case, inner)
    except Outcome as o:
        raise Outcome(o.status, _as_original(o.reason)) from None
    except Exception as e:  # the black box's failure: its message, never its traceback
        raise Outcome(
            FAIL, _as_original(f"the replica suite's case raised {type(e).__name__}: {e}")[:2000]
        ) from None
    finally:
        for c in inner.checks:
            checks.add(
                dataclasses.replace(c, name=_as_original(c.name), detail=_as_original(c.detail))
            )


# ------------------------------------------------------------------------------------------------
# The suite


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = [
        Case(c.id, c.category, _as_original(c.description), ("replica-suite", c))
        for c in _replica_suite().cases(tier)
    ]
    for (cat, name), own in OWN.items():
        if in_tier(own.tier, tier):
            out.append(Case(f"{cat}/{name}", cat, f"`{own.args}`" + (f" ({', '.join(f'{k}={v}' for k, v in own.settings)})" if own.settings else ""), ("own", (cat, name))))  # fmt: skip
    if in_tier("standard", tier):
        for name, _node, words in fmriprep_calls():
            out.append(Case(f"orig-fmriprep/{name}", "orig-fmriprep", f"fMRIPrep 25.2.6's call on {name}: {' '.join(Path(w.split('=', 1)[-1]).name if '=' in w else w for w in words)}", ("fmriprep", name)))  # fmt: skip
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "replica-suite":
        _run_replica_case(payload, checks)
        return
    if not fsl_versions.installed():
        raise Outcome(SKIPPED, f"FSL is not installed in {fsl_versions.fsldir()}")
    if kind == "own":
        own = OWN[payload]
        d = _inputs_dir()
        _run_own(
            shlex.split(own.args), dict(own.settings), sorted(d.iterdir()), own.undefined, checks
        )
        return
    found = {n: (node, words) for n, node, words in fmriprep_calls()}
    if payload not in found:
        raise Outcome(SKIPPED, f"fMRIPrep's oracle run {payload} is not here")
    node, words = found[payload]
    _run_own(words, {}, [], "", checks, fmriprep_node=node)


def _highlights(results) -> list[str]:
    own = [r for r in results if r.category.startswith("orig-")]
    rep = [r for r in results if not r.category.startswith("orig-")]

    def count(rs, status):
        return sum(r.status == status for r in rs)

    files = [
        c
        for r in own
        for c in r.checks
        if c.name.startswith("reproduce: ") and (" data" in c.name or " header" in c.name)
    ]
    dflt = [
        c
        for r in own
        for c in r.checks
        if c.name.startswith("default against") and (" data" in c.name or " header" in c.name)
    ]
    g07 = [
        c for r in own for c in r.checks if "GLIBC-07" in c.name and c.threshold == GLIBC07_SHARE
    ]
    out = [
        f"- **The replica suite's {len(rep)} cases, the original standing in for the replica's program:** "
        f"{count(rep, 'pass')} pass, {count(rep, BOTH_ERROR)} where both refuse, {count(rep, EXPECTED)} expected, "
        f"{count(rep, FAIL)} failures (FSL's runs from the oracle cache under that suite's keys).",
        f"- **The original's own {len(own)} cases:** {count(own, 'pass')} pass, {count(own, BOTH_ERROR)} where both refuse, "
        f"{count(own, EXPECTED)} expected, {count(own, FAIL)} failures, {count(own, SKIPPED)} skipped.",
        f"  - with `reproduce` against FSL: {sum(c.passed for c in files)} of {len(files)} headers and data bit-identical;",
        f"  - by default against the replica's default: {sum(c.passed for c in dflt)} of {len(dflt)} headers and data "
        f"bit-identical, and {sum(c.passed for c in g07)} of {len(g07)} unwrapped phases where GLIBC-07 is named with the "
        "replica's integer offsets on at least 99.9 % of the mask's voxels.",
    ]
    return out


def suite() -> Suite:
    import larmorx

    d = fsl_versions.fsldir()
    return Suite(
        name=SUITE,
        title="PRELUDE (clean-room original)",
        tool=f"`larmorx fsl prelude` (larmorx {larmorx.__version__}, crate larmorx-fsl: the clean-room original "
        "written from specs/prelude.md, Apache-2.0), as a program (`python -m larmorx`, or `LARMORX_PRELUDE_BIN`)",
        reference=f"prelude from FSL {fsl_versions.version_of(d)} ({fsl_versions.package_versions(d, ('fsl-fugue', 'fsl-newimage'))}), "
        f"the conda installation in `~/{d.name}`; the replica (`larmorx-nc fsl prelude`) by default as a black box",
        thresholds=[
            (
                "Exit status",
                "FSL's (FSL's abort, signal 6: the original exits 1 after FSL's lines and its own)",
            ),
            (
                "Standard output and error",
                "identical (the run directory and FSL's program replaced; larmorx's own lines left out)",
            ),
            ("Files written", "the same names"),
            (
                "`reproduce` against FSL",
                "every file bit for bit, decompressed: headers whole (the extension bytes included) but `descrip`, data; values FSL takes from memory it never wrote not compared (FSL-263, FSL-265)",
            ),
            (
                "The default against the replica's",
                "the same, and the same upstream defects named; where GLIBC-07 is named, the unwrapped phase with the replica's integer offsets on at least 99.9 % of the mask's voxels, the raw phase and the labels reported",
            ),
            (
                "D13 pass",
                "a defect named where the default's status, output or files differ from `reproduce`'s",
            ),
            (
                "fMRIPrep's calls",
                "both modes' unwrapped phase bit for bit with fMRIPrep's own output (header fields but `descrip`)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        highlights=_highlights,
        notes=(
            "The replica suite's cases are run with its own checks, as a black box: larmorx's original stands in for the "
            "replica's program, and FSL's runs are served from the oracle cache under that suite's keys. The original's own "
            "cases compare it with FSL in `reproduce` mode, and its default with the replica's default (the replica shares the "
            "corrected default and is bit-identical to FSL where no defect is named)."
        ),
    )


__all__ = ["OWN", "fmriprep_calls", "make_inputs", "suite"]
