# SPDX-License-Identifier: Apache-2.0
"""Parity of FAST's clean-room original (``larmorx fsl fast``, crate larmorx-fsl, Apache-2.0,
written from ``specs/fast.md``) with FSL 6.0.7.23's ``fast``.

The cases are the replica suite's (``fsl_fast_replica``: fMRIPrep's segmentation calls of the
oracle runs, the options, the odd inputs, the errors, the priors, several channels), run here
against the original. Every FSL run goes through the oracle-output cache with the replica
suite's keys and policies, so the runs that suite made are served from it.

What is compared (``specs/fast.md`` §11), each output against FSL's inside FSL's mask (its
``_seg`` above 0) unless said:

- the exit status (where FSL aborts, the original exits 1), the files written, standard output
  and error line by line with the numbers in them to 4 significant digits (the original's
  ``larmorx`` lines left out);
- each image's header: the grid, the voxel sizes, the forms and their codes, the data type;
- ``_seg``, ``_seg_<n>``, ``_pveseg``, ``_mixeltype``: per-class Dice at least 0.999 and at least
  99.9 % of the mask's voxels identical, every voxel outside the mask identical;
- ``_pve_<n>``: mean absolute difference at most 1e-3, its 99.9th percentile at most 0.02, each
  class's total within 0.1 %;
- ``_prob_<n>``: mean absolute difference at most 1e-3 (outside the mask FSL writes memory it
  never set, FSL-96: not compared);
- ``_bias``, ``_restore``: the relative difference at most 1e-4 at the 99.9th percentile,
  identical outside the mask;
- the D13 fix pass: a defect named exactly when the default's outputs differ from ``reproduce``;
- the default against the replica's default (``larmorx-nc``, ``LARMORX_NC_BIN``, run as a
  separate program; skipped when it is not built), with the same thresholds: both correct the
  same defects (FSL-202's variance among them), so there the band of a last-bit change is the
  narrow one of the corrected arithmetic (``specs/fast.md`` §11), not FSL's.

Besides the replica suite's cases, the original's own (``original/``, the clean-room catch-up,
2026-11-05): the lines of standard output at FAST's aborts and before the images are read
(§7), the loops without a class (``-n 0``), NaN voxels in ``_restore`` (§4.1.1); their derived
inputs in ``<workspace>/oracles/inputs/fsl-fast/``.
"""

from __future__ import annotations

import contextlib
import dataclasses
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast_replica as rep
from larmorx_validation.parity.fsl_mcflirt_replica import Completed, _files, _is_image
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    Check,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "fsl-fast"
_NUMBER = re.compile(r"[-+]?(?:nan|inf|(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)")
#: The thresholds of §11.
DICE, SAME = 0.999, 0.999
PVE_MEAN, PVE_P999, PVE_TOTAL = 1e-3, 0.02, 1e-3
PROB_MEAN = 1e-3
BIAS_REL = 1e-4
LABELS = ("_seg", "_pveseg", "_mixeltype")


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_FAST_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_FAST_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int = 4,
) -> Completed:
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    env = _environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads)
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k, v in os.environ.items():
        # LXM_*: the development switches of validation/investigations/cleanroom-fast/*.patch
        if k in ("PYTHONPATH", "VIRTUAL_ENV") or k.startswith("LXM_"):
            env[k] = v
    p = subprocess.run(
        [*original_command(), "fsl", "fast", *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=7200,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def _own(text: str) -> str:
    """The text without the original's own lines (notices, refusals, ``(larmorx)`` lines)."""
    keep = [
        ln
        for ln in text.split("\n")
        if not ln.startswith(("larmorx: ", "larmorx fsl ")) and not ln.endswith("(larmorx)")
    ]
    return "\n".join(keep)


def _numbers_close(want: str, got: str) -> tuple[bool, str]:
    """Whether two texts are the same apart from their numbers, and those agree to 4
    significant digits; the first difference otherwise."""
    wl, gl = want.split("\n"), got.split("\n")
    if len(wl) != len(gl):
        return False, f"{len(wl)} lines, the original {len(gl)}"
    for k, (a, b) in enumerate(zip(wl, gl, strict=True)):
        if a == b:
            continue
        if _NUMBER.sub("#", a) != _NUMBER.sub("#", b):
            return False, f"line {k + 1}: {a[:90]!r} / {b[:90]!r}"
        for x, y in zip(_NUMBER.findall(a), _NUMBER.findall(b), strict=True):
            fx, fy = float(x), float(y)
            if np.isnan(fx) and np.isnan(fy):
                continue
            if fx == fy:
                continue
            if not (np.isfinite(fx) and np.isfinite(fy)):
                return False, f"line {k + 1}: {x} / {y}"
            if abs(fx - fy) > 5e-4 * max(abs(fx), abs(fy)):
                return False, f"line {k + 1}: {x} / {y}"
    return True, ""


def _load(path: Path):
    """An image and its values: gzip and plain files through nibabel, zstd and bzip2 ones (FSL
    6.0.7.23's ``NIFTI_ZST`` and ``*_BZ2`` output types) decompressed first, pairs side by
    side."""
    import nibabel as nib

    from larmorx_validation.parity.fsl_mcflirt_replica import _open_stream

    name = path.name
    if name.endswith((".hdr.zst", ".hdr.bz2")):
        data_file = path.with_name(name.replace(".hdr.", ".img."))
        with tempfile.TemporaryDirectory(prefix="lx-fast-pair_") as t:
            for src, ext in ((path, ".hdr"), (data_file, ".img")):
                with _open_stream(src) as f:
                    (Path(t) / f"x{ext}").write_bytes(f.read())
            img = nib.load(Path(t) / "x.hdr")
            return img, np.asarray(img.dataobj).copy()
    if name.endswith((".zst", ".bz2")):
        with _open_stream(path) as f:
            raw = f.read()
        cls = nib.Nifti2Image if raw[:4] == b"\x1c\x02\x00\x00" else nib.Nifti1Image
        img = cls.from_bytes(raw)
        return img, np.asarray(img.dataobj)
    try:
        img = nib.load(str(path))
    except ValueError:
        if not name.endswith((".nii", ".nii.gz")):
            raise
        # a quaternion whose b, c, d squares sum to a little more than 1 (FSL writes such NIfTI-2
        # headers): nibabel refuses to make its affine; the header and values read without it
        import gzip
        import io
        import types

        raw = (gzip.open if name.endswith(".gz") else open)(path, "rb").read()
        cls = nib.Nifti2Header if raw[:4] == b"\x1c\x02\x00\x00" else nib.Nifti1Header
        hdr = cls.from_fileobj(io.BytesIO(raw))
        return types.SimpleNamespace(header=hdr), np.asarray(hdr.data_from_fileobj(io.BytesIO(raw)))
    return img, np.asarray(img.dataobj)


def _same_header(a, b) -> str:
    """The header fields of the grid that differ (empty: none)."""
    ha, hb = a.header, b.header
    out = []
    for k in ("dim", "pixdim", "qform_code", "sform_code", "datatype", "xyzt_units"):
        if not np.array_equal(np.asarray(ha[k]), np.asarray(hb[k])):
            out.append(k)
    for k in ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z"):
        if not np.allclose(float(ha[k]), float(hb[k]), atol=1e-6, equal_nan=True):
            out.append(k)
    for k in ("srow_x", "srow_y", "srow_z"):
        if not np.allclose(np.asarray(ha[k], float), np.asarray(hb[k], float), atol=1e-6):
            out.append(k)
    return ", ".join(out)


def _kind(name: str) -> str:
    base = re.sub(r"\.(nii|hdr|img)(\.(gz|zst|bz2))?$", "", name)
    for label in LABELS:
        if base.endswith(label):
            return "label"
    if re.search(r"_seg_\d+$", base):
        return "label"
    if re.search(r"_pve_\d+$", base):
        return "pve"
    if re.search(r"_prob_\d+$", base):
        return "prob"
    if re.search(r"_(bias|restore)(_\d+)?$", base):
        return "field"
    return "other"


def _mask_of(d: Path) -> np.ndarray | None:
    seg = next((p for p in sorted(d.iterdir()) if re.search(r"_seg\.(nii|hdr)", p.name)), None)
    if seg is None:
        return None
    return _load(seg)[1] > 0


def compare_images(checks: CheckList, a: Path, b: Path) -> None:
    """Every image FSL wrote against the original's (§11's metrics)."""
    mask = _mask_of(a)
    worst: dict[str, list[str]] = {"label": [], "pve": [], "prob": [], "field": [], "header": []}
    ok = {k: True for k in worst}
    for fa in sorted(a.iterdir()):
        if not _is_image(fa.name) or re.search(r"\.img(\.(gz|zst|bz2))?$", fa.name):
            continue
        fb = b / fa.name
        if not fb.is_file():
            continue
        ia, x = _load(fa)
        ib, y = _load(fb)
        h = _same_header(ia, ib)
        if h:
            ok["header"] = False
            worst["header"].append(f"{fa.name}: {h}")
        if x.shape != y.shape:
            continue
        kind = _kind(fa.name)
        m = mask if mask is not None and mask.shape == x.shape else np.ones(x.shape, bool)
        if kind == "label":
            xs, ys = x[m], y[m]
            same = float(np.mean(xs == ys)) if xs.size else 1.0
            dices = []
            for c in np.union1d(np.unique(xs), np.unique(ys)):
                xa, ya = xs == c, ys == c
                s = int(xa.sum() + ya.sum())
                dices.append(2 * int((xa & ya).sum()) / s if s else 1.0)
            dice = min(dices) if dices else 1.0
            outside = bool(np.array_equal(x[~m], y[~m]))
            good = same >= SAME and dice >= DICE and outside
            ok["label"] &= good
            worst["label"].append(
                f"{fa.name}: identical {same:.5f}, Dice {dice:.5f}"
                + ("" if outside else ", differs outside the mask")
            )
        elif kind in ("pve", "prob"):
            d = np.abs(x[m].astype(np.float64) - y[m].astype(np.float64))
            d = np.where(np.isnan(d) & np.isnan(x[m]) & np.isnan(y[m]), 0.0, d)
            mean = float(np.mean(d)) if d.size else 0.0
            if kind == "pve":
                p999 = float(np.percentile(d, 99.9)) if d.size else 0.0
                tx, ty = (
                    float(np.sum(x[m], dtype=np.float64)),
                    float(np.sum(y[m], dtype=np.float64)),
                )
                total = abs(tx - ty) / max(abs(tx), 1e-30) if tx or ty else 0.0
                outside = bool(np.array_equal(x[~m], y[~m]))
                good = mean <= PVE_MEAN and p999 <= PVE_P999 and total <= PVE_TOTAL and outside
                worst["pve"].append(
                    f"{fa.name}: mean {mean:.2g}, p99.9 {p999:.2g}, total {total:.2g}"
                    + ("" if outside else ", differs outside the mask")
                )
            else:
                good = mean <= PROB_MEAN
                worst["prob"].append(f"{fa.name}: mean {mean:.2g}")
            ok[kind] &= bool(good) and not np.isnan(mean)
        elif kind == "field":
            xs, ys = x[m].astype(np.float64), y[m].astype(np.float64)
            both_nan = np.isnan(xs) & np.isnan(ys)
            rel = np.abs(xs - ys) / np.maximum(np.abs(xs), 1e-30)
            rel = np.where(both_nan, 0.0, rel)
            p = float(np.percentile(rel, 99.9)) if rel.size else 0.0
            outside = bool(np.array_equal(x[~m], y[~m], equal_nan=True))
            good = (p <= BIAS_REL or (np.isnan(p) and False)) and outside
            ok["field"] &= bool(good)
            worst["field"].append(
                f"{fa.name}: rel p99.9 {p:.2g}" + ("" if outside else ", differs outside the mask")
            )
    names = {
        "label": "Segmentations (_seg, _seg_n, _pveseg, _mixeltype): Dice and identical voxels",
        "pve": "Partial volumes (_pve_n): mean and p99.9 differences, totals",
        "prob": "Probabilities (_prob_n): mean difference",
        "field": "Bias field and restored image: relative difference (p99.9)",
        "header": "Image headers: the same grid, forms and data types",
    }
    thresholds = {
        "label": f"Dice >= {DICE}, identical >= {SAME:.1%}, outside identical",
        "pve": f"mean <= {PVE_MEAN}, p99.9 <= {PVE_P999}, total within {PVE_TOTAL:.1%}",
        "prob": f"mean <= {PROB_MEAN}",
        "field": f"<= {BIAS_REL}",
        "header": "the same",
    }
    for k, lines in worst.items():
        if lines or k == "header":
            checks.add(
                Check(
                    names[k],
                    ok[k],
                    metric="per file",
                    value=str(len(lines)) if k == "header" else "; ".join(lines[:1]),
                    threshold=thresholds[k],
                    detail="; ".join(lines[:6]),
                )
            )


def crmath_preload() -> Path | None:
    """The workspace's CORE-MATH preload (correctly rounded ``exp``, ``log``, ``expf``,
    ``logf``), which gives FSL's program the last-bit band of §11."""
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    ws = workspace()
    p = ws / "oracles" / "core-math-preload" / "libcrmath.so" if ws is not None else None
    return p if p is not None and p.is_file() else None


def compare(
    checks: CheckList,
    oracle: Completed,
    ours: Completed,
    a: Path,
    b: Path,
    before: set[str],
    band=None,
) -> None:
    """The original's run (``b``) against FSL's (``a``). ``band()``, when given, runs FSL with
    correctly rounded maths (§11's band of a last-bit change) and returns its run and
    directory: a stream or image check that fails against FSL passes when the original agrees
    with that run within the same thresholds (FSL's own last bits explain the difference), and
    says so."""
    code_ok = ours.code == oracle.code or (oracle.code < 0 and ours.code == 1)
    checks(
        "Exit status",
        code_ok,
        detail=f"FSL {oracle.code}, the original {ours.code}",
    )
    fa = {k for k in _files(a, set()) if k not in before}
    fb = {k for k in _files(b, set()) if k not in before}
    checks(
        "Files written",
        fa == fb,
        detail=f"FSL only {sorted(fa - fb)[:4]}, original only {sorted(fb - fa)[:4]}",
    )
    own_out = ours.stdout.replace(str(b), "{cwd}")
    so, why_o = _numbers_close(oracle.stdout.replace(str(a), "{cwd}"), own_out)
    se, why_e = _numbers_close(
        oracle.stderr.replace(str(a), "{cwd}"), _own(ours.stderr.replace(str(b), "{cwd}"))
    )
    images = CheckList()
    compare_images(images, a, b)
    banded: tuple[Completed, Path] | None = None
    if band is not None and (not so or not all(c.passed for c in images.checks)):
        banded = band()
    if not so and banded is not None:
        ok, _why = _numbers_close(banded[0].stdout.replace(str(banded[1]), "{cwd}"), own_out)
        if ok:
            so, why_o = (
                True,
                f"within FSL's last-bit band (identical to FSL with correctly rounded maths to 4 digits); against FSL: {why_o}",
            )
    checks("Standard output (numbers to 4 significant digits)", so, detail=why_o)
    checks("Standard error (the original's own lines left out)", se, detail=why_e)
    alt: dict[str, Check] = {}
    if banded is not None:
        other = CheckList()
        compare_images(other, banded[1], b)
        alt = {c.name: c for c in other.checks}
    for c in images.checks:
        k = alt.get(c.name)
        if not c.passed and k is not None and k.passed:
            c = dataclasses.replace(
                c,
                passed=True,
                detail=f"within FSL's last-bit band: against FSL with correctly rounded maths {k.detail}; against FSL {c.detail}",
            )
        checks.add(c)


#: The checks of :func:`against_replica_default` start with this.
REPLICA_DEFAULT = "Default against the replica's default"

#: Cases whose default lies beyond the thresholds from the replica's default, with the measured
#: reason; their other checks must pass (the case is then an expected divergence).
EXPECTED_DEFAULT: dict[str, str] = {
    "multi/s2-manualseg-cycle": (
        "Both defaults correct FSL-98's class order (a cycle here), so FSL's own band (FSL with "
        "correctly rounded maths) is not this path's; several channels have no FSL-202 "
        "(one channel's), so their band is FSL's own size, not the narrow one of the corrected "
        "variance. The difference is the correctly rounded functions only: with glibc's "
        "functions the original's default gives the replica's default's labels exactly "
        "(validation/investigations/cleanroom-fast/glibc_default.py); FSL against FSL with "
        "correctly rounded maths on the same case, on FAST's uncorrected order: _mixeltype Dice "
        "0.99941. In reproduce FSL's labels exactly."
    ),
}

#: Corrections whose notice changes no compared value (FSL-96: values FSL never writes, outside
#: the mask): a default that names only these computes FSL's arithmetic, with its own functions.
_NOTHING_COMPARED = frozenset({"FSL-96"})


def against_replica_default(
    checks: CheckList,
    prepare,
    args: list[str],
    settings,
    ours_dir: Path,
    ours: Completed,
    before: set[str],
    band=None,
) -> None:
    """§11: the original's default run (in ``ours_dir``) against the replica's default (the same
    corrections), with the thresholds of FSL's comparison; each check's name says what it
    compares. Nothing when the replica's program is not built.

    The two defaults differ by the original's correctly rounded functions against the replica's
    glibc ones (with glibc's, the original's default is the replica's on every single-channel case
    of the suite: ``validation/investigations/cleanroom-fast/glibc_default.py``). With one channel
    FSL-202's variance makes that band narrow (``specs/fast.md`` §4.4), and no band is allowed.
    Several channels have no FSL-202: their band is FSL's own. So ``band`` (FSL with correctly
    rounded maths, :func:`_band_of`) is given only when the default corrects nothing compared
    (several channels, only :data:`_NOTHING_COMPARED` named): that run is then the default's
    arithmetic with the default's functions, and a check that fails against the replica's default
    passes when the original's default agrees with it within the same thresholds, saying so."""
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary

    if replica_binary() is None:
        return
    with tempfile.TemporaryDirectory(prefix="lx-fast-rd_") as name:
        tmp = Path(name)
        prepare(tmp)
        r = rep.run_replica("fast", args, cwd=tmp, settings=settings, mode=defects.FIX)
        label = REPLICA_DEFAULT
        checks(
            f"{label}: exit status",
            r.code == ours.code,
            detail=f"replica {r.code}, the original {ours.code}",
        )
        fa = {k for k in _files(tmp, set()) if k not in before}
        fb = {k for k in _files(ours_dir, set()) if k not in before}
        checks(
            f"{label}: files written",
            fa == fb,
            detail=f"replica only {sorted(fa - fb)[:4]}, original only {sorted(fb - fa)[:4]}",
        )
        if r.code == 0 and ours.code == 0:
            own_out = ours.stdout.replace(str(ours_dir), "{cwd}")
            so, why = _numbers_close(r.stdout.replace(str(tmp), "{cwd}"), own_out)
            images = CheckList()
            compare_images(images, tmp, ours_dir)
            banded = None
            if band is not None and (not so or not all(c.passed for c in images.checks)):
                banded = band()
            within = "within the correctly rounded functions' band (several channels, nothing corrected: FSL with correctly rounded maths is the default's arithmetic with its functions)"
            if not so and banded is not None:
                ok, _ = _numbers_close(banded[0].stdout.replace(str(banded[1]), "{cwd}"), own_out)
                if ok:
                    so, why = True, f"{within}; against the replica's default: {why}"
            checks(f"{label}: standard output (numbers to 4 significant digits)", so, detail=why)
            alt: dict[str, Check] = {}
            if banded is not None:
                other = CheckList()
                compare_images(other, banded[1], ours_dir)
                alt = {c.name: c for c in other.checks}
            for c in images.checks:
                k = alt.get(c.name)
                if not c.passed and k is not None and k.passed:
                    c = dataclasses.replace(
                        c,
                        passed=True,
                        detail=f"{within}: against FSL with correctly rounded maths {k.detail}; against the replica's default {c.detail}",
                    )
                checks.add(dataclasses.replace(c, name=f"{label}: {c.name}"))


def fix_pass(
    checks: CheckList,
    prepare,
    args: list[str],
    settings,
    reproduced_dir: Path,
    reproduced: Completed,
    band=None,
) -> None:
    """D13: the original's default run against its ``reproduce`` run, and against the replica's
    default (:func:`against_replica_default`)."""
    from larmorx_validation.parity.fsl_mcflirt_replica import _change

    with tempfile.TemporaryDirectory(prefix="lx-fast-fix_") as name:
        tmp = Path(name)
        prepare(tmp)
        before = set(_files(tmp, set()))
        r = run_original(args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        x, y = dict(_files(reproduced_dir, set())), dict(_files(tmp, set()))
        if set(x) != set(y):
            others.append(f"files {sorted(set(x) ^ set(y))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted((set(x) & set(y)) - before):
            if _is_image(key):
                change = change + _change(y[key], x[key])
            elif x[key].read_bytes() != y[key].read_bytes():
                others.append(key)
        defects.add_fix_check(
            checks,
            entries,
            change,
            other_changes="; ".join(others),
            silent_ok=rep.SILENT,
            # FAST names no GLIBC-07 (an iterative fit; defects.UNNAMED_GLIBC)
            unnamed_ok=defects.UNNAMED_GLIBC,
        )
        allowed = _channels(args) > 1 and set(entries) <= _NOTHING_COMPARED
        against_replica_default(
            checks, prepare, args, settings, tmp, r, before, band if allowed else None
        )


def _channels(args: list[str]) -> int:
    """``-S``'s value on a command line (1 when not given)."""
    for k, a in enumerate(args):
        if a in ("-S", "--channels") and k + 1 < len(args):
            try:
                return int(args[k + 1], 0)
            except ValueError:
                return 1
        if a.startswith("--channels="):
            try:
                return int(a.split("=", 1)[1], 0)
            except ValueError:
                return 1
    return 1


def _band_of(stack: contextlib.ExitStack, prepare, args, settings, never: str, unwritten=()):
    """The band's run (:func:`compare`, :func:`against_replica_default`), made only when a check
    needs it, once: FSL with the CORE-MATH preload through the oracle cache (the preload's
    content is part of the key)."""
    made: list[tuple[Completed, Path] | None] = []

    def band() -> tuple[Completed, Path] | None:
        if made:
            return made[0]
        lib = crmath_preload()
        if lib is None:
            made.append(None)
            return None
        c = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="lx-fast-cr_")))
        prepare(c)
        p = rep.run_oracle(args, cwd=c, settings={**settings, "LD_PRELOAD": str(lib)}, never=never)
        if unwritten:
            rep._clear_unwritten(CheckList(), c, unwritten)
        made.append((p, c))
        return p, c

    return band


#: The original's own derived inputs (``<workspace>/oracles/inputs/fsl-fast/``, from the replica
#: suite's), named ``%<name>`` in a case's arguments; the marker of their version.
OWN_INPUTS = "fsl-fast"
OWN_VERSION = "DONE-v2"


def own_inputs() -> Path:
    """The original's own inputs, made once (deterministic, from the replica suite's inputs):
    ``t1_nan_noneg`` (``t1_nan``'s five NaN voxels and no negative value: §4.1's rule skipped,
    so the NaN voxels stay NaN), ``t1_nan_offset`` (``t1_half`` whose background is -40, so its
    2nd percentile is negative and subtracting the minimum puts the background back at 0 (the
    suite's ``t1_offset`` shifts every voxel, so its background lands above 0 and FAST stops),
    with the same NaN voxels: the rule's first branch, NaN minus the minimum)."""
    import nibabel as nib

    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    src = rep.inputs()
    ws = workspace()
    d = (ws / "oracles" / "inputs" / OWN_INPUTS) if ws is not None else src.parent / OWN_INPUTS
    if (d / OWN_VERSION).is_file():
        return d
    d.mkdir(parents=True, exist_ok=True)
    nan = nib.load(src / "t1_nan.nii.gz")
    x = np.asarray(nan.dataobj, dtype=np.float32)
    holes = np.isnan(x)
    noneg = np.where(x < 0, np.float32(0), x).astype(np.float32)
    nib.save(nib.Nifti1Image(noneg, nan.affine, nan.header), d / "t1_nan_noneg.nii.gz")
    half = nib.load(src / "t1_half.nii.gz")
    z = np.asarray(half.dataobj, dtype=np.float32).copy()
    z[z <= 0] = np.float32(-40.0)
    z[holes] = np.float32(np.nan)
    nib.save(nib.Nifti1Image(z, half.affine, half.header), d / "t1_nan_offset.nii.gz")
    (d / OWN_VERSION).write_text("t1_nan_noneg, t1_nan_offset\n", encoding="utf-8")
    return d


def _prepare(run: rep.Run, tmp: Path) -> list[str]:
    """The replica suite's preparation (``@name`` its derived inputs), with the original's own
    inputs (``%name``, :func:`own_inputs`) copied first."""
    if not any(a.startswith("%") for a in run.args):
        return rep._prepare(run, tmp)
    own = own_inputs()
    args = []
    for a in run.args:
        if a.startswith("%"):
            shutil.copy(own / a[1:], tmp / a[1:])
            a = a[1:]
        args.append(a)
    return rep._prepare(dataclasses.replace(run, args=tuple(args)), tmp)


def _original_cases() -> list[tuple[str, str, rep.Run]]:
    """The original's own cases (the clean-room catch-up, 2026-11-05): the paths its answers
    changed that the replica suite's cases leave out (each once; FSL's runs cached under their
    own keys)."""
    r = rep.Run
    T, T2 = "@mni4_t1.nii.gz", "@mni4_t2.nii.gz"
    unset = (("FSLDIR", None),)
    return [
        (
            "original/v-n0",
            "-v -n 0: the loops without a class, then the abort in the partial volumes; standard "
            "output ends at the last flushed line (` BETA 0.1`, the unflushed heading lost: §7)",
            r(("-v", "-n", "0", "-o", "seg", T), fix_refuses="FSL-99"),
        ),
        (
            "original/v-n0-nopve",
            "-v -n 0 --nopve: the loops' lines without a class and `total tissue 0`, exit 0",
            r(("-v", "-n", "0", "--nopve", "-o", "seg", T), fix_refuses="FSL-99"),
        ),
        (
            "original/v-n0-H-nan",
            "-v -n 0 -H nan: FSL-99's TOTAL lines and pairs without a class, then the abort",
            r(("-v", "-n", "0", "-H", "nan", "-o", "seg", T), fix_refuses="FSL-99"),
        ),
        (
            "original/multi-v-n0",
            "-S 2 -v -n 0: several channels' loops without a class (empty CHANNEL lines), the "
            "partial-volume heading and `total tissue 0`, exit 0",
            r(("-S", "2", "-v", "-n", "0", "-o", "seg", T2, T), fix_refuses="FSL-99"),
        ),
        (
            "original/v-n-negative",
            "-v -n -1: FAST prints the image lines, then aborts",
            r(("-v", "-n", "-1", "-o", "seg", T), fix_refuses="FSL-99"),
        ),
        (
            "original/multi-v-n-negative",
            "-S 2 -v -n -2: the heading, then the abort",
            r(("-S", "2", "-v", "-n", "-2", "-o", "seg", T2, T), fix_refuses="FSL-99"),
        ),
        (
            "original/v-S0",
            "-S 0 -v: the multi-channel heading before the channels' error",
            r(("-S", "0", "-v", T)),
        ),
        (
            "original/v-missing-input",
            "-v with an image that cannot be read: the heading is printed before it is read",
            r(("-v", "-o", "seg", "nothere.nii.gz")),
        ),
        (
            "original/v-fsldir-unset",
            "-v with FSLDIR unset: the image lines, then FSL-100's abort",
            r(("-v", "-o", "seg", T), unset, fix_finishes="FSL-100"),
        ),
        (
            "original/multi-v-fsldir-unset",
            "-S 2 -v with FSLDIR unset: the heading, then FSL-100's abort",
            r(("-S", "2", "-v", "-o", "seg", T2, T), unset, fix_finishes="FSL-100"),
        ),
        (
            "original/v-fsldir-unset-n-negative",
            "-v -n -1 with FSLDIR unset: FSL-100's abort comes first",
            r(("-v", "-n", "-1", "-o", "seg", T), unset, fix_refuses="FSL-99"),
        ),
        (
            "original/nan-no-negative",
            "NaN voxels in an image without a negative value: §4.1's rule skipped, `_restore` NaN "
            "there (§4.1.1)",
            r(("-B", "-b", "-o", "seg", "%t1_nan_noneg.nii.gz")),
        ),
        (
            "original/nan-offset",
            "NaN voxels in an image whose 2nd percentile is negative: the minimum subtracted, "
            "`_restore` NaN there (§4.1.1)",
            r(("-B", "-o", "seg", "%t1_nan_offset.nii.gz")),
        ),
    ]


def _refusal(stderr: str) -> str:
    return next((ln for ln in stderr.splitlines() if ln.startswith("larmorx: error:")), "")


def _run_case(name: str, run: rep.Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    if run.refused:
        # FSL reads or writes outside its memory: the original refuses in both modes
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-fast_") as b_:
                b = Path(b_)
                r = run_original(_prepare(run, b), cwd=b, settings=settings, mode=mode)
                checks(
                    f"the original refuses ({mode})",
                    r.code == 1 and run.refused in _refusal(r.stderr),
                    detail=f"exit {r.code}: {_refusal(r.stderr)[:160]}",
                )
        if all(c.passed for c in checks.checks):
            raise Outcome(EXPECTED, f"undefined behaviour of FSL's ({run.refused}); refused")
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-fast_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-fast_") as b_,
        contextlib.ExitStack() as stack,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        before = set(_files(a, set()))
        oracle = rep.run_oracle(args, cwd=a, settings=settings, never=run.never)
        ours = run_original(args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.unwritten:
            rep._clear_unwritten(checks, a, run.unwritten)
        if run.no_bias_field:
            # FSL-97: FSL's files hold memory it never wrote; the defined result instead
            rep._undefined_bias_field(checks, a, args)
        band = _band_of(stack, lambda t: _prepare(run, t), args, settings, run.never, run.unwritten)
        compare(checks, oracle, ours, a, b, before, band)
        if run.fix and ours.code == 0:
            fix_pass(checks, lambda t: _prepare(run, t), args, settings, b, ours, band)
    if run.fix_finishes:
        with tempfile.TemporaryDirectory(prefix="lx-fast-fix_") as b_:
            b = Path(b_)
            r = run_original(_prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
            named = defects.notices_in(r.stderr)
            checks(
                f"the default run finishes and names {run.fix_finishes}",
                r.code == 0 and run.fix_finishes in named and bool(list(b.glob("*_seg.nii*"))),
                detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
            )
    if run.fix_refuses:
        with tempfile.TemporaryDirectory(prefix="lx-fast-fix_") as b_:
            b = Path(b_)
            r = run_original(_prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
            checks(
                f"the default run refuses, naming {run.fix_refuses}",
                r.code == 1 and run.fix_refuses in _refusal(r.stderr),
                detail=f"exit {r.code}: {_refusal(r.stderr)[:160]}",
            )
    why = EXPECTED_DEFAULT.get(name)
    bad = [c for c in checks.checks if not c.passed]
    if why is not None and bad and all(c.name.startswith(REPLICA_DEFAULT) for c in bad):
        raise Outcome(EXPECTED, f"{why} Measured: " + "; ".join(c.detail[:240] for c in bad))
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[:200]
        raise Outcome(BOTH_ERROR, f"fast exits {oracle.code}; the same from the original. {tail}")


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    src = rep.fmriprep_input(run, node)
    if src is None:
        raise Outcome(SKIPPED, f"{run}: the input of fMRIPrep's FAST node is not available")
    args = rep.fmriprep_args(Path(node))

    def prepare(t: Path) -> None:
        shutil.copy(src, t / args[-1])

    with (
        tempfile.TemporaryDirectory(prefix="lx-fast-fp_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-fast-fp_") as b_,
        contextlib.ExitStack() as stack,
    ):
        a, b = Path(a_), Path(b_)
        prepare(a)
        prepare(b)
        before = set(_files(a, set()))
        oracle = rep.run_oracle(args, cwd=a, settings={})
        ours = run_original(args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, oracle, ours, a, b, before, _band_of(stack, prepare, args, {}, ""))
        if oracle.code == 0 and ours.code == 0:
            fix_pass(checks, prepare, args, {}, b, ours)


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = list(rep.cases(tier))
    if in_tier("standard", tier):
        for name, desc, run in _original_cases():
            out.append(Case(name, name.split("/")[0], desc, ("direct", run)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        _run_case(case.id, payload, checks)
    else:
        _run_case(payload, rep._all_cases()[payload][2], checks)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="FAST (clean-room original)",
        tool=f"`larmorx fsl fast` (larmorx {larmorx.__version__}, crate larmorx-fsl: the "
        "clean-room original written from specs/fast.md, Apache-2.0)",
        reference="fast from FSL 6.0.7.23 (fsl-fast4 2111.3), the conda build at `~/fsl-6.0.7.23`",
        thresholds=[
            ("Exit status", "the same (where FSL aborts, the original exits 1)"),
            ("Files written", "the same names"),
            ("Standard output and error", "the same lines, numbers to 4 significant digits"),
            ("Segmentations", f"per-class Dice >= {DICE}, >= {SAME:.1%} of mask voxels identical"),
            (
                "Partial volumes",
                f"mean |d| <= {PVE_MEAN}, p99.9 <= {PVE_P999}, totals within 0.1 %",
            ),
            ("Probabilities", f"mean |d| <= {PROB_MEAN}"),
            ("Bias field, restored image", f"relative difference <= {BIAS_REL} at p99.9"),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The cases are the replica suite's (`fsl-fast-replica`); FSL's runs are served from "
            "the oracle cache under that suite's keys. The original is compared in `reproduce` "
            "mode (D13) and its default `fix` pass against the reproduced run."
        ),
    )
