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
- the D13 fix pass: a defect named exactly when the default's outputs differ from ``reproduce``.
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
    import nibabel as nib

    img = nib.load(str(path))
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
    base = re.sub(r"\.(nii(\.gz)?|hdr|img)$", "", name)
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
        if not _is_image(fa.name) or fa.suffix == ".img":
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


def fix_pass(
    checks: CheckList,
    prepare,
    args: list[str],
    settings,
    reproduced_dir: Path,
    reproduced: Completed,
) -> None:
    """D13: the original's default run against its ``reproduce`` run."""
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
            checks, entries, change, other_changes="; ".join(others), silent_ok=rep.SILENT
        )


def _band_of(stack: contextlib.ExitStack, prepare, args, settings, never: str, unwritten=()):
    """The band's run (:func:`compare`), made only when a check needs it: FSL with the CORE-MATH
    preload through the oracle cache (the preload's content is part of the key)."""

    def band() -> tuple[Completed, Path] | None:
        lib = crmath_preload()
        if lib is None:
            return None
        c = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="lx-fast-cr_")))
        prepare(c)
        p = rep.run_oracle(args, cwd=c, settings={**settings, "LD_PRELOAD": str(lib)}, never=never)
        if unwritten:
            rep._clear_unwritten(CheckList(), c, unwritten)
        return p, c

    return band


def _refusal(stderr: str) -> str:
    return next((ln for ln in stderr.splitlines() if ln.startswith("larmorx: error:")), "")


def _run_case(name: str, run: rep.Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    if run.refused:
        # FSL reads or writes outside its memory: the original refuses in both modes
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-fast_") as b_:
                b = Path(b_)
                r = run_original(rep._prepare(run, b), cwd=b, settings=settings, mode=mode)
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
        args = rep._prepare(run, a)
        rep._prepare(run, b)
        before = set(_files(a, set()))
        oracle = rep.run_oracle(args, cwd=a, settings=settings, never=run.never)
        ours = run_original(args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.unwritten:
            rep._clear_unwritten(checks, a, run.unwritten)
        if run.no_bias_field:
            # FSL-97: FSL's files hold memory it never wrote; the defined result instead
            rep._undefined_bias_field(checks, a, args)
        band = _band_of(
            stack, lambda t: rep._prepare(run, t), args, settings, run.never, run.unwritten
        )
        compare(checks, oracle, ours, a, b, before, band)
        if run.fix and ours.code == 0:
            fix_pass(checks, lambda t: rep._prepare(run, t), args, settings, b, ours)
    if run.fix_finishes:
        with tempfile.TemporaryDirectory(prefix="lx-fast-fix_") as b_:
            b = Path(b_)
            r = run_original(rep._prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
            named = defects.notices_in(r.stderr)
            checks(
                f"the default run finishes and names {run.fix_finishes}",
                r.code == 0 and run.fix_finishes in named and bool(list(b.glob("*_seg.nii*"))),
                detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
            )
    if run.fix_refuses:
        with tempfile.TemporaryDirectory(prefix="lx-fast-fix_") as b_:
            b = Path(b_)
            r = run_original(rep._prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
            checks(
                f"the default run refuses, naming {run.fix_refuses}",
                r.code == 1 and run.fix_refuses in _refusal(r.stderr),
                detail=f"exit {r.code}: {_refusal(r.stderr)[:160]}",
            )
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
    return list(rep.cases(tier))


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
