# SPDX-License-Identifier: Apache-2.0
"""Parity of topup's clean-room original (``larmorx fsl topup``, crate larmorx-fsl, Apache-2.0,
written from ``specs/topup.md``) with FSL 6.0.7.23's ``topup``.

The cases are the replica suite's (``fsl_topup_replica``: fMRIPrep's two field-map estimations,
the debug runs, every option, FSL's configurations, the messages and failures, the thread
counts, applytopup), run here against the original. Every FSL run goes through the
oracle-output cache with the replica suite's keys and policies, so the runs that suite made are
served from it. The original takes every option of both programs (an option it did not take
would be skipped, naming its refusal: none is).

What is compared (``specs/topup.md`` §12), each output against FSL's:

- the exit status, the files written, standard error line by line (the original's ``larmorx``
  lines left out) with the numbers in them to 4 significant digits; standard output the same,
  except ``-v``'s cost lines: the same levels, the first cost to 4 significant digits and the
  final cost within 0.1 %;
- each image's header: the grid, the voxel sizes, the forms and their codes, the intent, the
  data type; the log (``--logout``) identical;
- the field (``--fout``, and the field ``_fieldcoef`` defines, evaluated on the input's grid)
  inside the brain (the corrected mean, else the input's mean, above 10 % of its 99th
  percentile): the 99th percentile of the absolute difference at most 0.5 Hz, the mean at most
  0.1 Hz, the correlation at least 0.999 (``--featout``'s field in rad/s: the same times 2π);
- the movements (``_movpar.txt``, ``--rbmout``): translations within 0.05 mm, rotations within
  0.001 rad;
- the corrected images (``--iout``, its mean, ``--featout``'s magnitude): inside the brain, the
  relative difference at most 1e-3 at the 99th percentile and the mean absolute difference at
  most 1e-3 of the mean intensity;
- the Jacobians and displacement fields (``--jacout``, ``--dfout``): the field's bounds times the
  volume's read-out time (and the voxel size along its phase encoding, in mm);
- applytopup's output (``--out``): inside the brain (FSL's output above 10 % of its 99th
  percentile), the relative difference at most 1e-3 at the 99th percentile and the mean absolute
  difference at most 1e-3 of the mean intensity;
- the D13 fix pass: a defect named exactly when the default's outputs differ from
  ``reproduce``.

**The allowance of FSL's own last-bit band** (§12): where FSL's run at another thread count is in
the cache (fMRIPrep's calls at ``--nthr=4``, made for the replica's evidence; the replica suite's
thread cases), the same metrics between FSL at that thread count and at one measure FSL's own
band (FSL-116: its threaded sums change its last bits, which its optimisation carries through
the levels). A metric beyond the fixed threshold passes when the original is within that band,
and says so with both numbers; no threshold is loosened without a measured band.
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
from collections.abc import Callable
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup_replica as rep
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

SUITE = "fsl-topup"
PROGRAM = "topup"
_NUMBER = re.compile(r"[-+]?(?:nan|inf|(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)")
_COST = re.compile(r"Cost = (\S+)")
#: The vb methods' iteration lines (``specs/topup.md`` §7.5): their three numbers.
_VB = re.compile(
    r"([Rr]elative lambda = )(\S+?)(,\s+phi = )(\S+?)(,\s+lambda = )(\S+)$", re.MULTILINE
)
#: The printed values' threshold: 4 significant digits.
VB_REL = 5e-4
#: The thresholds of §12.
FIELD_P99, FIELD_MEAN, FIELD_CORR = 0.5, 0.1, 0.999
TRANS, ROT = 0.05, 0.001
REL_P99, MEAN_REL = 1e-3, 1e-3
COST_REL = 1e-3
#: The brain: the mean image above this fraction of its 99th percentile.
BRAIN = 0.1
#: The original's refusal of what it does not take yet (sub-tasks 2 and 3 of its part).
NOT_YET = "is not implemented yet in the clean-room original"
#: FSL's thread count for its own band on fMRIPrep's calls (the evidence's runs, cached).
BAND_THREADS = 4
#: The original's thread count in the cases (random testing sets its worker's).
LX_THREADS = 4
#: FSL-201's forward differences make FSL's own path depend on the last bits of its inputs
#: (``validation/investigations/cleanroom-topup/fsl201_spread.py``, the clean-room catch-up of
#: 2026-11-04): the corrected images' p99 relative difference from FSL's unperturbed run, of FSL's
#: runs with every read-out time changed by 1, 16 and 256 units of its last float bit, and of the
#: original's ``reproduce`` runs with the same changes.
_FSL201_SPREAD = {
    "opt/int16": ("0.00084 to 0.00089", "2.1e-5 to 0.00105"),
    "opt/interp-linear": ("0.00056 to 0.0021", "0.0010 to 0.0020"),
    "opt/levels": ("0.0010 to 0.0020", "1.7e-5 to 0.0018"),
    "opt/four-volumes": ("0.021 to 0.047 (also 64 and 1024 last bits)", "0.043 to 0.057"),
    "cfg/b02b0_4": ("0.0019 to 0.0033", "0.0013 to 0.0037"),
    "cfg/b02b0_7T": (
        "0.0017 to 0.0054",
        "0.0079 to 0.0105 (its -v costs follow FSL's to 4 or 5 digits level by level, the final "
        "within 3e-4)",
    ),
}
#: Cases whose metrics are expected outside the thresholds, and why (their exit status, files,
#: messages and headers are still compared): the movements estimated and not converged, where
#: FSL-201's ``reproduce`` gives FSL's derivatives but FSL's path is not determined beyond its
#: inputs' last bits (``_FSL201_SPREAD``; the other cases of findings 14 pass since the catch-up);
#: FSL-126's negative weight, which makes both minimisations diverge; FSL-143's indefinite prior,
#: whose diverging iterations amplify the moved images' float-level differences; vb2D's transients.
#: (The long lines pass since FSL-133's ``reproduce`` reads FSL's wrap-around.)
EXPECTED_DIVERGENCES: dict[str, str] = {
    **{
        name: "FSL-201 reproduced, the movements estimated and not converged: FSL's rotation "
        "derivatives are forward differences of single-precision matrices, so each gradient "
        "entry moves by about 2 % of its lever-weighted sum when its angle's rounding to single "
        "precision flips, and FSL's own path is not determined beyond its inputs' last bits: "
        f"FSL's own runs with the read-out times changed by 1 to 256 last bits lie {fsl} from "
        f"its unperturbed run (the corrected images' p99), the original's reproduce runs {lx}; "
        "the original's first gradients agree with FSL's to 1e-6 (findings 37), its default "
        "(the exact derivatives) moves by 1e-7 to 1.4e-5 under the same changes"
        for name, (fsl, lx) in _FSL201_SPREAD.items()
    },
    "opt/lambda-negative": "FSL-126 reproduced: a negative weight rewards a rough field, both "
    "minimisations diverge",
    **{
        name: "FSL-143 reproduced: FSL's smoothness prior is indefinite, so its iterations "
        "diverge (lambda below 0 in the first iterations, then phi and lambda fall towards "
        "1e-23 to 1e-206, the image following their ratio): the divergence amplifies the moved "
        "images' float-level differences in the printed values (and, for vb3d-three, in the "
        "image); the default, the consistent prior, agrees with the replica's default to 1e-5 "
        "(findings 26)"
        for name in ("at/vb3d-aniso", "at/vb4d-four", "at/vb3d-three")
    },
    "at/vb2d-aniso": "the printed phi and lambda of the top slices (few voxels in the brain) "
    "part from FSL's in the 4th digit during the iterations (8e-4) and meet again (their final "
    "values within 1e-5): the iterations amplify the moved images' float-level differences "
    "there; the image within 1e-5 (p99), and the same against the replica's default (findings "
    "26)",
}
#: Cases whose FSL behaviour the original does not reproduce and refuses in ``reproduce`` mode
#: too (its refusal must name the entry), and why: none since the clean-room catch-up
#: (``at/jac-other-shape``, FSL-134's field of another shape, is reproduced from
#: ``specs/topup.md`` §7.3).
ORIGINAL_REFUSES: dict[str, tuple[str, str]] = {}
#: Cases whose default run the original refuses where the replica suite's case does not say so
#: (defects found by the original's part, rule B), with the entry its refusal names: none since
#: the replica suite's ``at/jac-z`` names FSL-144 itself.
FIX_REFUSES: dict[str, str] = {}
#: The checks of an expected divergence that must still pass (everything but §12's metrics).
_EXACT_KINDS = (
    "Exit status",
    "Files written",
    "Standard error",
    "Standard output",
    "header",
    "identical",
    "-v: the same levels",
    "the default run",
    "D13 fix pass",
)


# ------------------------------------------------------------------------------------------------
# The original


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_TOPUP_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_TOPUP_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int | None = None,
    program: str = PROGRAM,
) -> rep.Completed:
    env = rep._environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads or LX_THREADS)
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k, v in os.environ.items():
        if k in ("PYTHONPATH", "VIRTUAL_ENV"):
            env[k] = v
    p = subprocess.run(
        [*original_command(), "fsl", program, *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=7200,
    )
    return rep.Completed(p.returncode, p.stdout, p.stderr)


def _refusal(stderr: str) -> str:
    return next((ln for ln in stderr.splitlines() if ln.startswith("larmorx: error:")), "")


# ------------------------------------------------------------------------------------------------
# Streams


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
        k = next((i for i, (a, b) in enumerate(zip(wl, gl, strict=False)) if a != b), None)
        k = min(len(wl), len(gl)) if k is None else k
        first = (wl[k] if k < len(wl) else "")[:80], (gl[k] if k < len(gl) else "")[:80]
        return (
            False,
            f"{len(wl)} lines, the original {len(gl)}; line {k + 1}: {first[0]!r} / {first[1]!r}",
        )
    for k, (a, b) in enumerate(zip(wl, gl, strict=True)):
        if a == b:
            continue
        if _NUMBER.sub("#", a) != _NUMBER.sub("#", b):
            return False, f"line {k + 1}: {a[:90]!r} / {b[:90]!r}"
        for x, y in zip(_NUMBER.findall(a), _NUMBER.findall(b), strict=True):
            fx, fy = float(x), float(y)
            if fx == fy or (np.isnan(fx) and np.isnan(fy)):
                continue
            if not (np.isfinite(fx) and np.isfinite(fy)):
                return False, f"line {k + 1}: {x} / {y}"
            if abs(fx - fy) > 5e-4 * max(abs(fx), abs(fy)):
                return False, f"line {k + 1}: {x} / {y}"
    return True, ""


def _split_costs(text: str) -> tuple[str, list[list[float]]]:
    """``-v``'s output: the text with each level's cost lines replaced by one marker, and the
    costs of each level."""
    lines, levels, run = [], [], None
    for ln in text.split("\n"):
        m = _COST.search(ln) if ln.startswith("SSD = ") else None
        if m is None:
            run = None
            lines.append(ln)
            continue
        if run is None:
            run = []
            levels.append(run)
            lines.append("<cost lines>")
        run.append(float(m.group(1)))
    return "\n".join(lines), levels


def _split_vb(text: str) -> tuple[str, list[float]]:
    """The vb methods' iteration lines with their numbers replaced by ``#``, and the numbers."""
    values: list[float] = []

    def take(m: re.Match) -> str:
        for g in (2, 4, 6):
            values.append(float(m.group(g)))
        return f"{m.group(1)}#{m.group(3)}#{m.group(5)}#"

    return _VB.sub(take, text), values


def compare_stdout(checks: CheckList, want: str, got: str, band: str | None = None) -> None:
    """Standard output: with ``-v``'s cost lines, the same levels, the first cost to 4
    significant digits, the final cost within 0.1 % (or within FSL's own band ``band``'s final
    cost, when given); the vb methods' printed values to 4 significant digits as a metric (FSL-143
    makes FSL's iterations diverge); the other lines to 4 significant digits."""
    tw, cw = _split_costs(want)
    tg, cg = _split_costs(got)
    tw, vw = _split_vb(tw)
    tg, vg = _split_vb(tg)
    ok, why = _numbers_close(tw, tg)
    checks("Standard output (numbers to 4 significant digits)", ok, detail=why)
    if vw or vg:
        worst, where = 0.0, ""
        for k, (a, b) in enumerate(zip(vw, vg, strict=False)):
            r = abs(a - b) / max(abs(a), abs(b), 1e-300) if a != b else 0.0
            if not np.isfinite(r):
                r = float("inf")
            if r > worst:
                worst, where = r, f"value {k + 1} of {len(vw)}: FSL {a:.6g}, the original {b:.6g}"
        checks.add(
            Check(
                "vb: the printed phi, lambda and their ratio",
                len(vw) == len(vg) and worst <= VB_REL,
                metric="max relative difference",
                value=float(f"{worst:.3g}"),
                threshold="4 significant digits",
                detail=where
                if len(vw) == len(vg)
                else f"FSL {len(vw)} values, the original {len(vg)}",
            )
        )
    if not cw:
        return
    checks(
        "-v: the same levels",
        len(cw) == len(cg),
        detail=f"FSL {len(cw)} levels ({sum(map(len, cw))} cost lines), the original "
        f"{len(cg)} ({sum(map(len, cg))})",
    )
    if not cg or not cg[0]:
        return
    first = abs(cg[0][0] - cw[0][0]) / max(abs(cw[0][0]), 1e-300)
    checks.add(
        Check(
            "-v: the first cost (the prepared images)",
            first <= 5e-4,
            metric="relative difference",
            value=float(f"{first:.3g}"),
            threshold="4 significant digits",
            detail=f"FSL {cw[0][0]:.6g}, the original {cg[0][0]:.6g}",
        )
    )
    last = abs(cg[-1][-1] - cw[-1][-1]) / max(abs(cw[-1][-1]), 1e-300)
    detail = f"FSL {cw[-1][-1]:.6g}, the original {cg[-1][-1]:.6g}"
    passed = last <= COST_REL
    if band is not None:
        _, cb = _split_costs(band)
        if cb and cb[-1]:
            b = abs(cb[-1][-1] - cw[-1][-1]) / max(abs(cw[-1][-1]), 1e-300)
            detail += f"; FSL's own band {b:.3g}"
            if not passed and last <= b:
                passed, detail = True, f"within FSL's own band: {detail}"
    checks.add(
        Check(
            "-v: the final cost",
            passed,
            metric="relative difference",
            value=float(f"{last:.3g}"),
            threshold=COST_REL,
            detail=detail,
        )
    )


# ------------------------------------------------------------------------------------------------
# Files


def _files(d: Path, before: set[str]) -> dict[str, Path]:
    return {p.name: p for p in sorted(d.iterdir()) if p.is_file() and p.name not in before}


def _is_image(name: str) -> bool:
    return name.endswith((".nii", ".nii.gz", ".hdr", ".img", ".hdr.gz", ".img.gz"))


def _stem(name: str) -> str:
    for ext in (".nii.gz", ".nii", ".hdr.gz", ".img.gz", ".hdr", ".img"):
        if name.endswith(ext):
            return name[: -len(ext)]
    return name


def _load(p: Path):
    import nibabel as nib

    return nib.load(str(p))


def _data(img) -> np.ndarray:
    return np.asarray(img.dataobj, dtype=np.float64)


def _header_difference(a, b) -> str:
    """The first difference between two images' headers that the outputs define: grid, voxel
    sizes, forms and codes, intent, data type."""
    ha, hb = a.header, b.header
    if a.shape != b.shape:
        return f"shape {a.shape} / {b.shape}"
    za, zb = np.asarray(ha.get_zooms(), float), np.asarray(hb.get_zooms(), float)
    if za.shape != zb.shape or not np.allclose(za, zb, rtol=1e-6, atol=1e-7):
        return f"voxel sizes {za} / {zb}"
    if ha.get_data_dtype() != hb.get_data_dtype():
        return f"data type {ha.get_data_dtype()} / {hb.get_data_dtype()}"
    for form in ("qform", "sform"):
        ma, ca = getattr(a, f"get_{form}")(coded=True)
        mb, cb = getattr(b, f"get_{form}")(coded=True)
        if int(ca or 0) != int(cb or 0):
            return f"{form} code {ca} / {cb}"
        if ma is not None and mb is not None and not np.allclose(ma, mb, rtol=1e-6, atol=1e-5):
            return f"{form} {ma.tolist()} / {mb.tolist()}"
    for key in ("intent_code", "intent_p1", "intent_p2", "intent_p3"):
        if key in ha and key in hb:
            va, vb = float(ha[key]), float(hb[key])
            if not np.isclose(va, vb, rtol=1e-6, atol=1e-7):
                return f"{key} {va} / {vb}"
    return ""


def _b3(t: np.ndarray) -> np.ndarray:
    a = np.abs(t)
    return np.where(a < 1, 2 / 3 - a**2 + a**3 / 2, np.where(a < 2, (2 - a) ** 3 / 6, 0.0))


def _b2(t: np.ndarray) -> np.ndarray:
    a = np.abs(t)
    return np.where(a < 0.5, 0.75 - a**2, np.where(a < 1.5, 0.5 * (1.5 - a) ** 2, 0.0))


def field_of_coefficients(img) -> np.ndarray | None:
    """The field (Hz) a cubic (intent 2016) or quadratic (2017) ``_fieldcoef`` image defines on
    the input's grid (its knot spacings are its voxel sizes, the field's size its qform's
    translation; coefficient ``i`` centred at voxel ``(i - 1) k``, or ``i`` with ``k = 1``, for
    both orders, with each order's count: observed); ``None`` for another layout."""
    h = img.header
    code = int(h["intent_code"])
    if code not in (2016, 2017):
        return None
    basis = _b3 if code == 2016 else _b2
    k = [round(float(v)) for v in h.get_zooms()[:3]]
    q = img.get_qform()
    n = [round(float(v)) for v in q[:3, 3]]
    c = _data(img)
    mats = []
    for a in range(3):
        nc = c.shape[a]
        if k[a] == 1:
            if nc != n[a]:
                return None
            centres = np.arange(nc, dtype=float)
        else:
            if code == 2016:
                counts = {-(-(n[a] + 1) // k[a]) + 2}
            else:
                # quadratic: ceil((2n + k - 4) / 2k) + 2, and FSL's two on an axis shorter than
                # two spacings (FSL-145; findings of the trial of 2026-11-03)
                counts = {-(-max(2 * n[a] + k[a] - 4, 0) // (2 * k[a])) + 2}
                if n[a] < 2 * k[a]:
                    counts.add(2)
            if nc not in counts:
                return None
            centres = (np.arange(nc) - 1.0) * k[a]
        x = np.arange(n[a], dtype=float)
        mats.append(basis((x[:, None] - centres[None, :]) / k[a]))
    out = c
    for a in range(3):
        out = np.moveaxis(np.tensordot(mats[a], np.moveaxis(out, a, 0), axes=(1, 0)), 0, a)
    return out


@dataclasses.dataclass(frozen=True)
class Measure:
    """One metric of one output: its check's name, value and threshold."""

    name: str
    value: float
    threshold: float
    metric: str
    higher_is_better: bool = False

    def passes(self, allowance: float | None = None) -> bool:
        limit = self.threshold if allowance is None else allowance
        if not np.isfinite(self.value):
            return False
        return self.value >= limit if self.higher_is_better else self.value <= limit


def _option(args: list[str], flag: str) -> str | None:
    for a in args:
        if a.startswith(flag + "="):
            return a.split("=", 1)[1]
    return None


def _acquisition(d: Path, args: list[str]) -> np.ndarray | None:
    name = _option(args, "--datain") or _option(args, "--acqp")
    if not name or not (d / name).is_file():
        return None
    try:
        m = np.loadtxt(d / name, ndmin=2)
    except ValueError:
        return None
    # the transposed form: four rows (the vectors' components, the read-out times)
    return m.T if m.shape[0] == 4 and m.shape[1] != 4 else m


def effective_args(d: Path, args: list[str]) -> list[str]:
    """``args`` with the options of their ``--config`` file (the case's own in ``d``, or one of
    FSL's shipped configurations) that the command line does not give: FSL's ``test_acqp``
    names outputs (``--fout=field_test``, ``--iout=iout_test``)."""
    name = _option(args, "--config")
    if not name:
        return args
    cands = [d / name, d / f"{name}.cnf"]
    cands += [rep.fsldir() / "etc" / "flirtsch" / n for n in (name, f"{name}.cnf")]
    path = next((c for c in cands if c.is_file()), None)
    if path is None:
        return args
    given = {a.split("=", 1)[0] for a in args}
    extra = []
    for ln in path.read_text(encoding="utf-8", errors="replace").splitlines():
        ln = ln.strip()
        if ln.startswith("--") and "=" in ln and ln.split("=", 1)[0] not in given:
            extra.append(ln)
    return [*args, *extra]


def _image_in(d: Path, name: str | None) -> Path | None:
    """The image ``name`` names in ``d`` (as given, or with an extension), if it is one."""
    if not name:
        return None
    for cand in (name, name + ".nii.gz", name + ".nii"):
        if (d / cand).is_file() and _is_image(cand):
            return d / cand
    return None


def _mean_stems(iout: str) -> tuple[str, str]:
    """``--iout``'s mean: ``_mean`` after the name as given (an extension included: FSL's
    ``<name>.nii.gz_mean``), or after its stem."""
    return f"{_stem(iout)}_mean", f"{iout}_mean"


def _out_option(args: list[str]) -> str | None:
    """applytopup's ``--out`` (or ``-o``)."""
    v = _option(args, "--out")
    if v is None and "-o" in args[:-1]:
        v = args[args.index("-o") + 1]
    return v


def brain_mask(
    a: Path, args: list[str], files: dict[str, Path], program: str = PROGRAM
) -> np.ndarray | None:
    """The brain of §12: FSL's corrected mean above 10 % of its 99th percentile, else the mean
    of the input's volumes the same way; applytopup's: FSL's output (its mean over volumes)."""
    iout = _option(args, "--iout")
    img = None
    if program != PROGRAM:
        out = _out_option(args)
        for name, p in files.items():
            if out and _is_image(name) and _stem(name) == _stem(out):
                x = _data(_load(p))
                img = x.mean(axis=3) if x.ndim == 4 else x
        if img is None or img.ndim != 3 or not np.any(img > 0):
            return None
        return img > BRAIN * np.percentile(img, 99)
    if iout:
        for name, p in files.items():
            if _stem(name) in _mean_stems(iout):
                img = _data(_load(p))
    if img is None:
        src = _image_in(a, _option(args, "--imain"))
        if src is None:
            return None
        x = _data(_load(src))
        img = x.mean(axis=3) if x.ndim == 4 else x
    if img.ndim != 3 or not np.any(img > 0):
        return None
    return img > BRAIN * np.percentile(img, 99)


def _field_measures(tag: str, fa: np.ndarray, fb: np.ndarray, mask, scale: float = 1.0):
    if mask is None or fa.shape[:3] != mask.shape or fa.shape != fb.shape:
        return [Measure(f"{tag}: the same grid", float("inf"), 0.0, "shape")]
    x, y = fa[mask], fb[mask]
    if x.size == 0:
        return []
    d = np.abs(x - y)
    out = [
        Measure(
            f"{tag}: |d| inside the brain, 99th percentile",
            float(np.percentile(d, 99)),
            FIELD_P99 * scale,
            "p99 |d|",
        ),
        Measure(
            f"{tag}: |d| inside the brain, mean", float(d.mean()), FIELD_MEAN * scale, "mean |d|"
        ),
    ]
    if scale == 1.0 or tag.endswith("rad/s"):
        if np.std(x) > 0 and np.std(y) > 0:
            corr = float(np.corrcoef(x, y)[0, 1])
        else:
            corr = 1.0 if np.array_equal(x, y) else 0.0
        out.append(
            Measure(
                f"{tag}: correlation inside the brain",
                corr,
                FIELD_CORR,
                "correlation",
                higher_is_better=True,
            )
        )
    return out


def _intensity_measures(tag: str, ia: np.ndarray, ib: np.ndarray, mask):
    if mask is None or ia.shape[:3] != mask.shape or ia.shape != ib.shape:
        return [Measure(f"{tag}: the same grid", float("inf"), 0.0, "shape")]
    vols = [(ia, ib)] if ia.ndim == 3 else [(ia[..., i], ib[..., i]) for i in range(ia.shape[3])]
    rel_all, absd, total = [], 0.0, 0.0
    for x, y in vols:
        x, y = x[mask], y[mask]
        nz = np.abs(x) > 0
        rel_all.append(np.abs(x - y)[nz] / np.abs(x)[nz])
        absd += float(np.abs(x - y).sum())
        total += float(np.abs(x).sum())
    rel = np.concatenate(rel_all) if rel_all else np.zeros(0)
    out = []
    if rel.size:
        out.append(
            Measure(
                f"{tag}: relative difference inside the brain, 99th percentile",
                float(np.percentile(rel, 99)),
                REL_P99,
                "p99 relative |d|",
            )
        )
    if total > 0:
        out.append(
            Measure(
                f"{tag}: mean |d| / mean intensity inside the brain",
                absd / total,
                MEAN_REL,
                "mean |d| / mean",
            )
        )
    return out


def _numbers(p: Path) -> np.ndarray | None:
    try:
        return np.loadtxt(p, ndmin=2)
    except ValueError:
        return None


def measures(
    a: Path, b: Path, args: list[str], files_a: dict[str, Path], mask, program: str = PROGRAM
) -> tuple[list[Measure], list[Check]]:
    """§12's metrics of every output of ``b`` against ``a``'s (the same file names), and the
    exact checks (headers, logs, other files)."""
    out: list[Measure] = []
    exact: list[Check] = []
    acq = _acquisition(a, args)
    stems = {k: _stem(v) for k, v in (("fout", _option(args, "--fout")),) if v}
    iout, jac, dfo = (_option(args, f) for f in ("--iout", "--jacout", "--dfout"))
    feat, rbm = _option(args, "--featout"), _option(args, "--rbmout")
    at_out = _out_option(args) if program != PROGRAM else None
    src = _image_in(a, (_option(args, "--imain") or "").split(",")[0])
    vox = np.asarray(_load(src).header.get_zooms()[:3], float) if src else np.ones(3)
    centre = float(np.linalg.norm(np.asarray(_load(src).shape[:3]) * vox / 2)) if src else 100.0

    def read_time(i: int) -> tuple[float, float]:
        """Volume ``i``'s read-out time and the voxel size along its phase encoding."""
        if acq is None or i >= acq.shape[0]:
            return 1.0, float(vox.max())
        row = acq[i]
        axis = int(np.argmax(np.abs(row[:3])))
        return abs(float(row[3])), float(vox[axis])

    for name, pa in files_a.items():
        pb = b / name
        if not pb.is_file():
            continue
        stem = _stem(name)
        if name.startswith("TopupDebug") or name.endswith(rep.PROFILE_SUFFIX):
            # the debug files hold each implementation's own intermediate values (the names
            # are compared); the profile's durations are the clock's
            continue
        if _is_image(name):
            ia, ib = _load(pa), _load(pb)
            why = _header_difference(ia, ib)
            exact.append(Check(f"{name}: header", not why, detail=why))
            if why and ia.shape != ib.shape:
                continue
            da, db = _data(ia), _data(ib)
            if at_out and stem == _stem(at_out) and mask is None:
                # no brain (FSL's output 0 everywhere: every line skipped): the values exactly
                exact.append(Check(f"{name}: values identical (no brain)", np.array_equal(da, db)))
            elif at_out and stem == _stem(at_out):
                out += _intensity_measures(f"{stem} (applytopup's output)", da, db, mask)
            elif stem.endswith("_fieldcoef"):
                fa, fb = field_of_coefficients(ia), field_of_coefficients(ib)
                if fa is not None and fb is not None:
                    out += _field_measures("_fieldcoef (its field, Hz)", fa, fb, mask)
                else:
                    rel = float(np.abs(da - db).max() / max(np.abs(da).max(), 1e-300))
                    out.append(Measure("_fieldcoef: max relative |d|", rel, 1e-3, "max rel |d|"))
            elif stems.get("fout") == stem:
                out += _field_measures("--fout (Hz)", da, db, mask)
            elif iout and (stem == _stem(iout) or stem in _mean_stems(iout)):
                out += _intensity_measures(f"{stem}", da, db, mask)
            elif feat and stem == f"{_stem(feat)}_fieldmap_rads":
                out += _field_measures(f"{stem} (rad/s)", da, db, mask, scale=2 * np.pi)
            elif feat and stem == f"{_stem(feat)}_fieldmap_mag":
                out += _intensity_measures(f"{stem}", da, db, mask)
            elif jac and re.fullmatch(re.escape(_stem(jac)) + r"_\d\d", stem):
                t, _ = read_time(int(stem[-2:]) - 1)
                out += _field_measures(stem, da, db, mask, scale=t)
            elif dfo and re.fullmatch(re.escape(_stem(dfo)) + r"_\d\d", stem):
                t, v = read_time(int(stem[-2:]) - 1)
                d3 = np.abs(da - db).max(axis=3) if da.ndim == 4 else np.abs(da - db)
                out += _field_measures(stem + " (mm)", d3, np.zeros_like(d3), mask, scale=t * v)[:2]
            else:
                same = np.array_equal(da, db)
                exact.append(Check(f"{name}: values identical", same))
        elif name.endswith("_movpar.txt") or (rbm and name.startswith(f"{rbm}_")):
            na, nb = _numbers(pa), _numbers(pb)
            if na is None or nb is None or na.shape != nb.shape:
                exact.append(Check(f"{name}: the same numbers' shape", False))
                continue
            if name.endswith("_movpar.txt"):
                t_d = float(np.abs(na[:, :3] - nb[:, :3]).max())
                r_d = float(np.abs(na[:, 3:] - nb[:, 3:]).max())
                t_lim = TRANS
            else:
                r_d = float(np.abs(na[:3, :3] - nb[:3, :3]).max())
                t_d = float(np.abs(na[:3, 3] - nb[:3, 3]).max())
                t_lim = TRANS + ROT * centre
            out.append(Measure(f"{name}: translations", t_d, t_lim, "max |d| mm"))
            out.append(Measure(f"{name}: rotations", r_d, ROT, "max |d|"))
        elif name.endswith(".topup_log") or name == _option(args, "--logout"):
            ta = pa.read_text(encoding="utf-8", errors="replace").replace(str(a), "{cwd}")
            tb = pb.read_text(encoding="utf-8", errors="replace").replace(str(b), "{cwd}")
            exact.append(Check(f"{name}: identical", ta == tb, detail=_first_line_diff(ta, tb)))
        else:
            same = pa.read_bytes() == pb.read_bytes()
            exact.append(Check(f"{name}: identical", same))
    return out, exact


def _first_line_diff(a: str, b: str) -> str:
    for k, (x, y) in enumerate(zip(a.split("\n"), b.split("\n"), strict=False)):
        if x != y:
            return f"line {k + 1}: {x[:80]!r} / {y[:80]!r}"
    return "" if a == b else "lengths differ"


# ------------------------------------------------------------------------------------------------
# A case


def compare(
    checks: CheckList,
    oracle: rep.Completed,
    ours: rep.Completed,
    a: Path,
    b: Path,
    args: list[str],
    before: set[str],
    band: Callable[[], tuple[rep.Completed, Path] | None] | None = None,
    program: str = PROGRAM,
    band_name: str = f"--nthr={BAND_THREADS} against 1",
) -> None:
    """The original's run (``b``) against FSL's (``a``). ``band()``, when given, returns FSL's
    run at another thread count (its own last-bit band, FSL-116) and its directory: a metric
    beyond its threshold passes when the original is within that band, and says so."""
    checks(
        "Exit status",
        ours.code == oracle.code or (oracle.code < 0 and ours.code == 1),
        detail=f"FSL {oracle.code}, the original {ours.code}",
    )
    fa, fb = _files(a, before), _files(b, before)
    args = effective_args(a, args)
    checks(
        "Files written",
        set(fa) == set(fb),
        detail=f"FSL only {sorted(set(fa) - set(fb))[:4]}, original only "
        f"{sorted(set(fb) - set(fa))[:4]}",
    )
    prog = str(rep.fsldir() / "bin" / program)
    se, why_e = _numbers_close(
        oracle.stderr.replace(str(a), "{cwd}").replace(prog, program),
        _own(ours.stderr.replace(str(b), "{cwd}")),
    )
    checks("Standard error (the original's own lines left out)", se, detail=why_e)
    mask = brain_mask(a, args, fa, program)
    got, exact = measures(a, b, args, fa, mask, program)
    banded: tuple[rep.Completed, Path] | None = None
    costs_off = bool(_split_costs(oracle.stdout)[1]) and not _final_cost_ok(
        oracle.stdout, ours.stdout
    )
    if band is not None and (not all(m.passes() for m in got) or costs_off):
        banded = band()
    band_values: dict[str, float] = {}
    if banded is not None:
        bm, _ = measures(a, banded[1], args, fa, mask, program)
        band_values = {m.name: m.value for m in bm}
    want_out, got_out = oracle.stdout, ours.stdout
    if "--trace" in args:
        # --trace names each implementation's own stages (FSL's internal routines, the
        # original's): the other lines are compared, the original's stages checked as pairs
        want_out, _ = _split_trace(want_out)
        got_out, stages = _split_trace(got_out)
        checks(
            "--trace: the original's stages, each entered and left",
            bool(stages) and _balanced(stages),
            detail=f"{len(stages)} lines",
        )
    compare_stdout(
        checks,
        want_out.replace(str(a), "{cwd}"),
        got_out.replace(str(b), "{cwd}"),
        None if banded is None else banded[0].stdout,
    )
    for c in exact:
        checks.add(c)
    for m in got:
        passed, detail = m.passes(), ""
        bv = band_values.get(m.name)
        if bv is not None:
            detail = f"FSL's own band ({band_name}): {bv:.3g}"
            if not passed and np.isfinite(bv) and m.passes(bv):
                passed, detail = True, f"within FSL's own band: {detail}"
        checks.add(
            Check(
                m.name,
                passed,
                metric=m.metric,
                value=float(f"{m.value:.4g}"),
                threshold=m.threshold,
                detail=detail,
            )
        )


_STAGE = re.compile(r"(?:Entering|Leaving) [^\n]*(?:\n|$)")


def _split_trace(text: str) -> tuple[str, list[str]]:
    """``--trace``'s lines (``Entering``/``Leaving`` a stage, FSL's update notes) taken out,
    within a line too: with ``-v`` FSL's stages interrupt a cost line where they run (``SSD =
    6519.96\tn = 2240Entering TopupCF::FieldEnergy``), and the line goes on after them."""
    stages = [m.group(0).rstrip("\n") for m in _STAGE.finditer(text)]
    rest = _STAGE.sub("", text)
    keep = []
    for ln in rest.split("\n"):
        if "Things not up to date" in ln:
            stages.append(ln)
        else:
            keep.append(ln)
    return "\n".join(keep), stages


def _balanced(stages: list[str]) -> bool:
    open_: list[str] = []
    for ln in stages:
        kind, _, name = ln.partition(" ")
        if kind == "Entering":
            open_.append(name)
        elif kind == "Leaving" and (not open_ or open_.pop() != name):
            return False
    return not open_


def _final_cost_ok(want: str, got: str) -> bool:
    _, cw = _split_costs(want)
    _, cg = _split_costs(got)
    if not (cw and cg and cw[-1] and cg[-1]):
        return True
    return abs(cg[-1][-1] - cw[-1][-1]) <= COST_REL * abs(cw[-1][-1])


def fix_pass(
    checks: CheckList,
    prepare,
    args: list[str],
    settings,
    reproduced_dir: Path,
    reproduced,
    program: str = PROGRAM,
) -> None:
    """D13: the original's default run against its ``reproduce`` run."""
    with tempfile.TemporaryDirectory(prefix="lx-topup-fix_") as name:
        tmp = Path(name)
        prepare(tmp)
        before = set(_files(tmp, set()))
        r = run_original(args, cwd=tmp, settings=settings, mode=defects.FIX, program=program)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        x, y = _files(reproduced_dir, set()), _files(tmp, set())
        if set(x) != set(y):
            others.append(f"files {sorted(set(x) ^ set(y))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted((set(x) & set(y)) - before):
            if key.endswith(".topup_log"):
                continue
            if _is_image(key):
                ix, iy = _load(x[key]), _load(y[key])
                change = change + defects.Change.of(_data(iy), _data(ix))
                why = _header_difference(ix, iy)
                if why:
                    others.append(f"{key} header ({why})")
            elif x[key].read_bytes() != y[key].read_bytes():
                others.append(key)
        defects.add_fix_check(
            checks,
            entries,
            change,
            other_changes="; ".join(others),
            silent_ok=rep.SILENT,
            # topup names no GLIBC-07 (an iterative fit; defects.UNNAMED_GLIBC); applytopup
            # calls none of glibc's functions
            unnamed_ok=defects.UNNAMED_GLIBC if program == PROGRAM else (),
        )


def _not_yet(ours: rep.Completed) -> str:
    """The original's refusal of an option or input it does not take yet, if it refused so."""
    line = _refusal(ours.stderr)
    return line if ours.code == 1 and NOT_YET in line else ""


def _expected_divergence(checks: CheckList, name: str) -> None:
    """A case of :data:`EXPECTED_DIVERGENCES` whose only failures are §12's metrics ends as an
    expected divergence, its failing metrics kept in the report."""
    reason = EXPECTED_DIVERGENCES.get(name)
    failed = [c for c in checks.checks if not c.passed]
    if reason is None or not failed:
        return
    if any(any(k in c.name for k in _EXACT_KINDS) for c in failed):
        return
    worst = "; ".join(f"{c.name} {c.value} (threshold {c.threshold})" for c in failed[:3])
    raise Outcome(EXPECTED, f"{reason}. Outside the thresholds: {worst}")


#: The relative change of the read-out times in random cases' band: one bit of a float's
#: mantissa (2^-23).
LAST_BIT = 2.0**-23


def _perturbed_acquisition(d: Path, args: list[str]) -> bool:
    """The acquisition file ``args`` name in ``d`` rewritten with every read-out time changed
    in its last float bit (times ``1 + LAST_BIT``), in its own layout; whether it was."""
    name = _option(args, "--datain") or _option(args, "--acqp")
    if not name or not (d / name).is_file():
        return False
    try:
        m = np.loadtxt(d / name, ndmin=2)
    except ValueError:
        return False
    transposed = m.shape[0] == 4 and m.shape[1] != 4
    if transposed:
        m[3, :] *= 1.0 + LAST_BIT
    elif m.shape[1] == 4:
        m[:, 3] *= 1.0 + LAST_BIT
    else:
        return False
    text = "".join(" ".join(f"{v:.17g}" for v in row) + "\n" for row in m)
    (d / name).write_text(text, encoding="utf-8")
    return True


def _random_band(run: rep.Run, node, settings, stack: contextlib.ExitStack):
    """A random case's band: FSL's run with its read-out times changed in their last float bit
    (through the oracle cache), a measure of how far FSL's own result moves for a last-bit
    change of its inputs on this case (its minimisation's conditioning: short iterations on
    small phantoms, unregularised fits); ``None`` when it cannot be made."""

    def band() -> tuple[rep.Completed, Path] | None:
        c = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="lx-topup-bit_")))
        args = rep._prepare(run, c, node)
        if not _perturbed_acquisition(c, args):
            return None
        r = rep._oracle(run, args, c, settings)
        return (r, c) if r.code == 0 else None

    return band


def _run_case(run: rep.Run, checks: CheckList, name: str = "", random_case: bool = False) -> None:
    node = rep._source_node()
    settings = dict(run.settings)
    prog = run.program
    if name in ORIGINAL_REFUSES:
        entry, reason = ORIGINAL_REFUSES[name]
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-topup_") as b_:
                b = Path(b_)
                args = rep._prepare(run, b, node)
                r = run_original(args, cwd=b, settings=settings, mode=mode, program=prog)
                checks(
                    f"the original refuses ({mode}), naming {entry}",
                    r.code == 1 and entry in _refusal(r.stderr),
                    detail=f"exit {r.code}: {_refusal(r.stderr)[:160]}",
                )
        if all(c.passed for c in checks.checks):
            raise Outcome(EXPECTED, f"{entry}: refused by the original in both modes: {reason}")
        return
    if run.refused:
        # FSL needs more memory than the machine holds: the original refuses in both modes
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-topup_") as b_:
                b = Path(b_)
                r = run_original(
                    rep._prepare(run, b, node), cwd=b, settings=settings, mode=mode, program=prog
                )
                if _not_yet(r):
                    raise Outcome(SKIPPED, f"the original does not take this yet: {_not_yet(r)}")
                checks(
                    f"the original refuses ({mode})",
                    r.code == 1 and run.refused in _refusal(r.stderr),
                    detail=f"exit {r.code}: {_refusal(r.stderr)[:160]}",
                )
        if all(c.passed for c in checks.checks):
            raise Outcome(EXPECTED, f"{run.refused}: refused; FSL is not run: {run.refused_reason}")
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-topup_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-topup_") as b_,
        tempfile.TemporaryDirectory(prefix="lx-topup-band_") as n_,
        contextlib.ExitStack() as stack,
    ):
        a, b, an = Path(a_), Path(b_), Path(n_)
        band = _random_band(run, node, settings, stack) if random_case else None
        if run.single is not None:
            # a thread case: the original at this thread count against FSL at one; FSL's own
            # run at this thread count is its band
            s_args, s_settings = run.single
            one = rep.Run(s_args, s_settings, timeout=run.timeout)
            args = rep._prepare(one, a, node)
            oracle = rep._oracle(one, args, a, dict(s_settings))
            n_args = rep._prepare(run, an, node)
            oracle_n = rep._oracle(run, n_args, an, settings)
            for log in an.glob("*.topup_log"):
                shutil.copy(log, a / log.name)

            def band():
                return oracle_n, an

            ours_args = n_args
        else:
            args = rep._prepare(run, a, node)
            oracle = rep._oracle(run, args, a, settings)
            ours_args = args
        rep._prepare(run, b, node)
        before = {p.name for p in b.iterdir()}
        ours = run_original(
            ours_args, cwd=b, settings=settings, mode=defects.REPRODUCE, program=prog
        )
        if _not_yet(ours):
            raise Outcome(SKIPPED, f"the original does not take this yet: {_not_yet(ours)}")
        # FSL-201: with the movements estimated, FSL's rotation derivatives are forward
        # differences of single-precision matrices, so its path depends on the last bits of its
        # movement parameters (each angle's rounding to single precision: about 2 % in the
        # rotations' gradient entries, specs/topup.md §4.6); FSL's own last-bit band is the
        # allowance of such a case, as of a random one
        last_bit = random_case or (band is None and run.single is None and "FSL-201" in ours.stderr)
        if last_bit and band is None:
            band = _random_band(run, node, settings, stack)
        compare(
            checks,
            oracle,
            ours,
            a,
            b,
            args,
            before,
            band,
            program=prog,
            band_name="its read-out times changed in their last float bit"
            if last_bit
            else f"--nthr={BAND_THREADS} against 1",
        )
        if run.fix and ours.code == 0 and not run.profile:
            fix_pass(
                checks,
                lambda t: rep._prepare(run, t, node),
                ours_args,
                settings,
                b,
                ours,
                program=prog,
            )
    for attr, how in (("fix_refuses", "refuses"), ("fix_finishes", "finishes")):
        entry = getattr(run, attr) or (FIX_REFUSES.get(name, "") if how == "refuses" else "")
        if not entry:
            continue
        with tempfile.TemporaryDirectory(prefix="lx-topup-fix_") as b_:
            b = Path(b_)
            r = run_original(
                rep._prepare(run, b, node), cwd=b, settings=settings, mode=defects.FIX, program=prog
            )
            named = defects.notices_in(r.stderr) + ([_refusal(r.stderr)] if r.code else [])
            ok = (
                (r.code == 1 and entry in _refusal(r.stderr))
                if how == "refuses"
                else (r.code == 0 and entry in named)
            )
            checks(f"the default run {how}, naming {entry}", ok, detail=f"exit {r.code}: {named}")
    if run.fix_says:
        with tempfile.TemporaryDirectory(prefix="lx-topup-fix_") as b_:
            b = Path(b_)
            r = run_original(
                rep._prepare(run, b, node), cwd=b, settings=settings, mode=defects.FIX, program=prog
            )
            checks(
                "the default run's message",
                run.fix_says in r.stderr,
                detail=f"exit {r.code}: {' '.join(r.stderr.split())[:160]}",
            )
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[:200]
        raise Outcome(BOTH_ERROR, f"{prog} exits {oracle.code}; the same from the original. {tail}")


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node_s = payload
    node = Path(node_s)
    args = rep.fmriprep_args(node)
    with (
        tempfile.TemporaryDirectory(prefix="lx-topup-fp_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-topup-fp_") as b_,
        contextlib.ExitStack() as stack,
    ):
        a, b = Path(a_), Path(b_)
        if not (rep._prepare_fmriprep(node, a) and rep._prepare_fmriprep(node, b)):
            raise Outcome(SKIPPED, f"{run}: the inputs of fMRIPrep's topup node are missing")
        before = {p.name for p in a.iterdir()}
        oracle = rep.run_oracle(args, cwd=a, settings={})
        ours = run_original(args, cwd=b, settings={}, mode=defects.REPRODUCE)

        def band() -> tuple[rep.Completed, Path] | None:
            """FSL at --nthr=4 (the replica's evidence made this run: served from the cache)."""
            c = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="lx-topup-band_")))
            rep._prepare_fmriprep(node, c)
            p = rep.oracle_run(
                [*args, f"--nthr={BAND_THREADS}"], cwd=c, settings={}, threads=BAND_THREADS
            )
            if p.returncode != 0:
                return None
            for log in a.glob("*.topup_log"):
                shutil.copy(log, c / log.name)  # the log records the options as given
            return rep.Completed(p.returncode, p.stdout, p.stderr), c

        compare(checks, oracle, ours, a, b, args, before, band)
        if oracle.code == 0 and ours.code == 0:
            fix_pass(checks, lambda t: rep._prepare_fmriprep(node, t), args, {}, b, ours)


def difftest_case(payload: tuple[rep.Run, rep.Run | None], checks: CheckList) -> None:
    """A random case (``difftest/tools/fsl_topup_original.py``, the replica's draw) or a
    permanent case it found: ``(topup, None)`` compares a topup run as every case does;
    ``(topup, applytopup)`` takes FSL's topup run of the case's own pair (through the oracle
    cache; its ``--out`` is ``rep.DIFFTEST_SOURCE``) and compares applytopup on its coefficients
    and movements, as the replica suite's :func:`~.fsl_topup_replica.difftest_case` does."""
    topup, at = payload
    if at is None:
        _run_case(topup, checks, random_case=True)
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-topup-dts_") as s_,
        tempfile.TemporaryDirectory(prefix="lx-topup-dtsrc_") as src_,
    ):
        s, src = Path(s_), Path(src_)
        r = rep._oracle(topup, rep._prepare(topup, s, None), s, dict(topup.settings))
        if r.code != 0:
            tail = " ".join(r.stderr.split())[-200:]
            raise Outcome(SKIPPED, f"FSL's topup (applytopup's source) exits {r.code}: {tail}")
        found = False
        for f in sorted(s.iterdir()):
            for stem in ("_fieldcoef", "_movpar.txt"):
                if f.name.startswith(rep.DIFFTEST_SOURCE + stem):
                    shutil.copy(f, src / f.name)
                    found = found or stem == "_fieldcoef"
        if not found:
            raise Outcome(SKIPPED, "FSL's topup (applytopup's source) wrote no coefficients")
        _run_case(dataclasses.replace(at, source_dir=str(src)), checks, random_case=True)


def cases(tier: str) -> list[Case]:
    return list(rep.cases(tier))


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload, *expected = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "difftest":
        difftest_case(payload, checks)
        if expected:
            # a permanent case expected to diverge: only §12's metrics may fail
            failed = [c for c in checks.checks if not c.passed]
            if failed and not any(any(k in c.name for k in _EXACT_KINDS) for c in failed):
                worst = "; ".join(
                    f"{c.name} {c.value} (threshold {c.threshold})" for c in failed[:3]
                )
                raise Outcome(EXPECTED, f"{expected[0]}. Outside the thresholds: {worst}")
    else:
        _run_case(rep._all_cases()[payload][2], checks, payload)
        _expected_divergence(checks, payload)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="topup (clean-room original)",
        tool=f"`larmorx fsl topup` (larmorx {larmorx.__version__}, crate larmorx-fsl: the "
        "clean-room original written from specs/topup.md, Apache-2.0)",
        reference="topup from FSL 6.0.7.23 (fsl-topup 2203.5), the conda build at "
        "`~/fsl-6.0.7.23`, one thread",
        thresholds=[
            ("Exit status", "the same"),
            ("Files written", "the same names"),
            (
                "Standard output and error",
                "the same lines, numbers to 4 significant digits; -v's costs: the same levels, "
                "the first to 4 digits, the final within 0.1 %",
            ),
            ("Headers, the log", "identical"),
            (
                "Field (Hz) inside the brain",
                f"p99 |d| <= {FIELD_P99}, mean |d| <= {FIELD_MEAN}, correlation >= {FIELD_CORR}",
            ),
            ("Movements", f"translations within {TRANS} mm, rotations within {ROT} rad"),
            (
                "Corrected images inside the brain",
                f"relative |d| <= {REL_P99} at p99, mean |d| <= {MEAN_REL} of the mean",
            ),
            ("Jacobians, displacement fields", "the field's bounds times t (and the voxel size)"),
            (
                "FSL's own band",
                f"where measured (FSL at --nthr={BAND_THREADS} against 1, from the cache): a "
                "metric beyond its threshold passes within that band, and says so",
            ),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The cases are the replica suite's (`fsl-topup-replica`); FSL's runs are served from "
            "the oracle cache under that suite's keys. The original is compared in `reproduce` "
            "mode (D13) and its default `fix` pass against the reproduced run. The original "
            "takes every option of both programs (none is skipped)."
        ),
    )
