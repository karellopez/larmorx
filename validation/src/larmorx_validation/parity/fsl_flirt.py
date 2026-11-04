# SPDX-License-Identifier: Apache-2.0
"""Parity of FLIRT's clean-room original (``larmorx fsl flirt`` and ``convert_xfm``, crate
larmorx-fsl, Apache-2.0, written from ``specs/flirt.md``) with FSL 6.0.7.23's ``flirt`` and
``convert_xfm``.

The cases are the replica suite's (``fsl_flirt_replica``: fMRIPrep's three BBR calls, the real
cases on ds005040's images, the schedules, the edge cases, ``convert_xfm`` and FLIRT's other
programs and scripts: ``avscale``, the coordinate programs, ``rmsdiff``, ``makerot``,
``midtrans``, ``pointflirt``, ``applyxfm4D``, ``pairreg``, ``extracttxt``, ``aff2rigid``), run
here against the original. Every FSL run goes through the oracle-output cache with the replica
suite's keys, so the runs that suite made are served from it.

What is compared (``specs/flirt.md`` §13):

- the exit status (where FSL aborts, the original exits 1), the files written, standard output
  and error with the numbers in them read as numbers (identical text reported apart; the
  original's ``larmorx:`` lines left out);
- matrices: identical text, or the 95th-percentile displacement over the reference's brain (its
  voxels above 0) between FSL's matrix and the original's within 0.1 mm for the boundary-based
  and 6-dof registrations and 0.5 mm for the others, the RMS deviation (radius 80 mm) reported;
  ``-applyxfm`` and ``convert_xfm``: identical text;
- images: bit-identical when the matrices are identical (decompressed, ``descrip`` left out),
  otherwise the normalised correlation with FSL's image inside the reference's mask at least
  0.999;
- the D13 fix pass: a defect named exactly when the default's outputs differ from ``reproduce``;
- FLIRT's other programs (:data:`TEXT_PROGRAMS`): standard output and error identical (where FSL
  aborts, the original prints the same lines and exits 1), every file identical (``extracttxt``:
  GNU sed's usage text left out of FSL's standard error);
- the fits (:data:`FIT_PROGRAMS`): the same text, printed numbers within one unit of their last
  digit, matrices within 1e-9 of FSL's; ``aff2rigid``'s corrected matrix (FSL-81) against the
  replica's when its program is built;
- ``applyxfm4D``: the same text and files, nearest-neighbour and trilinear images bit-identical,
  sinc and spline images of the same header with a correlation of at least 0.9999;
- ``pairreg``: as FLIRT's matrices, its first brain the reference.
"""

from __future__ import annotations

import contextlib
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt_replica as rep
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    _files,
    _is_image,
    _same_streams,
)
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

SUITE = "fsl-flirt"
#: FLIRT's other programs whose results are their text and files (part 2): compared identical
#: (§13: every case of the replica's ``programs/`` category, identical in text and exit status).
TEXT_PROGRAMS = (
    "avscale",
    "img2imgcoord",
    "std2imgcoord",
    "img2stdcoord",
    "rmsdiff",
    "makerot",
    "extracttxt",
)
#: The fits (SVD-based, §13): the same text and statuses, the matrices within 1e-9 of FSL's.
FIT_PROGRAMS = ("midtrans", "pointflirt", "aff2rigid")
#: The programs with a clean-room original.
PROGRAMS = ("flirt", "convert_xfm", *TEXT_PROGRAMS, *FIT_PROGRAMS, "applyxfm4D", "pairreg")
#: GNU sed's usage text (GPL), which extracttxt's runs print and the Apache original does not
#: reproduce: left out of FSL's standard error (the original prints one line of its own there).
_SED_USAGE = re.compile(
    r"Usage: sed \[OPTION\]\.\.\..*?General help using GNU software: <https://www\.gnu\.org/"
    r"gethelp/>\.\n",
    re.S,
)
#: Options the original does not have yet: its cases are skipped.
PENDING: dict[str, str] = {}

_NUMBER = re.compile(r"[-+]?(?:nan|inf|(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)")


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_FLIRT_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_FLIRT_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    program: str,
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int = 4,
) -> Completed:
    env = rep._environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads)
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
        timeout=3600,
    )
    return Completed(p.returncode, p.stdout, p.stderr)


def _own(text: str) -> str:
    """The text without the original's own lines (notices, ``(larmorx)`` messages)."""
    keep = [
        ln
        for ln in text.split("\n")
        if not ln.startswith(("larmorx: ", "larmorx fsl ")) and not ln.endswith("(larmorx)")
    ]
    return "\n".join(keep)


def _masked(text: str) -> str:
    return _NUMBER.sub("#", text)


#: A cost evaluation's line of a verbose level of 5 or more (`<count> : <cost> :: <params>`).
_EVALUATION = re.compile(r"^\*?# : # :: (?:# )*$")
#: `-debug`'s volumes that do not depend on the registration's matrix (§9.4): the reference
#: pyramid and its weights, the input at 8 mm and its weight.
_DEBUG_VOLUMES = re.compile(r"^(refvol(_[248])?|global_refweight[1248]|testvol|testweight)\.")
#: `-debugsave`'s grids of the search (§2.3), which follow the search's optimisations.
_SEARCH_GRIDS = re.compile(r"^(tx|ty|tz|sc|costs)\.")


def _collapsed(text: str) -> tuple[str, int]:
    """The masked text with each run of evaluation lines as one line, and their count: the
    evaluations follow the optimisation's path, which the matrices' thresholds hold."""
    out: list[str] = []
    n = 0
    for ln in _masked(text).split("\n"):
        if _EVALUATION.match(ln):
            n += 1
            star = ln.startswith("*")
            if out and out[-1] in ("<evaluations>", "*<evaluations>"):
                if star:
                    out[-1] = "*<evaluations>"
                continue
            out.append("*<evaluations>" if star else "<evaluations>")
        else:
            out.append(ln)
    return "\n".join(out), n


def _verbose_level(args: list[str]) -> int:
    level = 0
    for i, w in enumerate(args):
        if w == "-v":
            level = max(level, 1)
        elif w == "-verbose" and i + 1 < len(args):
            with contextlib.suppress(ValueError):
                level = max(level, int(args[i + 1]))
    return level


def _numbers(text: str) -> list[float]:
    out = []
    for m in _NUMBER.findall(text):
        with contextlib.suppress(ValueError):
            out.append(float(m))
    return out


def _load(path: Path):
    """A nibabel image of any file FSL writes: gzip and plain by nibabel, zstd and bzip2 (which
    the validation environment cannot read) from their decompressed bytes (a pair's two files
    together)."""
    import io

    import nibabel as nib

    name = path.name
    for c in (".zst", ".bz2"):
        if not name.endswith(c):
            continue
        from larmorx_validation.parity.fsl_bet import content

        for stem in (".hdr", ".img"):
            if name.endswith(stem + c):
                base = name[: -len(stem + c)]
                head = content(path.with_name(base + ".hdr" + c))
                data = content(path.with_name(base + ".img" + c))
                two = int.from_bytes(head[:4], "little") == 540
                klass = nib.Nifti2Pair if two else nib.Nifti1Pair
                fm = klass.make_file_map({"header": io.BytesIO(head), "image": io.BytesIO(data)})
                return klass.from_file_map(fm)
        raw = content(path)
        two = int.from_bytes(raw[:4], "little") == 540
        return (nib.Nifti2Image if two else nib.Nifti1Image).from_bytes(raw)
    return nib.load(str(path))


def _reference_mask(path: Path) -> tuple[np.ndarray, np.ndarray] | None:
    """The reference's brain (voxels above 0, all voxels when none) as FSL millimetres (voxel
    index times voxel size, radiological order), and the image's voxel sizes."""
    try:
        img = _load(path)
    except Exception:
        return None
    data = np.asanyarray(img.dataobj)
    if data.ndim > 3:
        data = data[..., 0]
    while data.ndim < 3:
        data = data[..., None]
    zooms = np.abs(np.array(img.header.get_zooms()[:3], dtype=np.float64))
    zooms[zooms == 0] = 1.0
    h = img.header
    aff = img.get_sform() if int(h["sform_code"]) > 0 else img.get_qform()
    flipped = (int(h["sform_code"]) > 0 or int(h["qform_code"]) > 0) and np.linalg.det(
        aff[:3, :3]
    ) > 0
    idx = np.argwhere(np.isfinite(data) & (data > 0))
    if len(idx) == 0:
        idx = np.argwhere(np.ones(data.shape, bool))
    if len(idx) > 200_000:
        idx = idx[:: len(idx) // 200_000 + 1]
    idx = idx.astype(np.float64)
    if flipped:
        idx[:, 0] = data.shape[0] - 1 - idx[:, 0]
    return idx * zooms, zooms


def displacement(m1: np.ndarray, m2: np.ndarray, pts: np.ndarray) -> tuple[float, float]:
    """The 95th percentile and the maximum of `|M1⁻¹ y − M2⁻¹ y|` over the reference points
    `y` (mm): how far apart the two matrices take each reference point in the input."""
    h = np.c_[pts, np.ones(len(pts))]
    a = h @ np.linalg.inv(m1).T
    b = h @ np.linalg.inv(m2).T
    d = np.linalg.norm((a - b)[:, :3], axis=1)
    return float(np.percentile(d, 95)), float(d.max())


def _threshold(args: list[str]) -> float:
    """0.1 mm for the boundary-based and 6-dof registrations, 0.5 mm otherwise (§13)."""
    j = " ".join(args)
    if "bbr" in j or re.search(r"-dof 6\b", j):
        return 0.1
    return 0.5


def _ncc(a: np.ndarray, b: np.ndarray, mask: np.ndarray) -> float:
    x, y = a[mask].astype(np.float64), b[mask].astype(np.float64)
    x, y = x - x.mean(), y - y.mean()
    d = np.sqrt((x * x).sum() * (y * y).sum())
    return float((x * y).sum() / d) if d > 0 else (1.0 if np.array_equal(a, b) else 0.0)


def compare_text(
    checks: CheckList, program: str, oracle: Completed, ours: Completed, a: Path, b: Path
) -> None:
    """FLIRT's other programs (§13): exit status (where FSL aborts, the original exits 1),
    standard output and error identical (the original's ``larmorx`` lines left out), the same
    files with the same bytes."""
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the original exits 1)",
            ours.code == 1,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
    else:
        checks("exit status", oracle.code == ours.code, detail=f"{oracle.code} vs {ours.code}")
    for stream in ("stdout", "stderr"):
        want = rep._clean(getattr(oracle, stream), a, program)
        got = rep._clean(_own(getattr(ours, stream)), b, program)
        if program == "extracttxt":
            want = _SED_USAGE.sub("", want)
        detail = ""
        if want != got:
            wl, gl = want.splitlines(), got.splitlines()
            first = next(
                (i for i, (x, y) in enumerate(zip(wl, gl, strict=False)) if x != y),
                min(len(wl), len(gl)),
            )
            detail = (
                f"line {first + 1}: FSL {wl[first] if first < len(wl) else '<end>'!r}, "
                f"original {gl[first] if first < len(gl) else '<end>'!r}"
            )
        checks(f"{stream} identical", want == got, detail=detail)
    theirs, mine = _files(a, set()), _files(b, set())
    checks(
        "files written",
        set(theirs) == set(mine),
        detail=f"only FSL {sorted(set(theirs) - set(mine))[:6]}, "
        f"only the original {sorted(set(mine) - set(theirs))[:6]}",
    )
    differ = [n for n in sorted(set(theirs) & set(mine)) if not rep._same_bytes(theirs[n], mine[n])]
    checks("every file identical", not differ, detail="; ".join(differ[:6]))


def _first_difference(want: str, got: str, same) -> str:
    wl, gl = want.splitlines(), got.splitlines()
    first = next(
        (i for i, (x, y) in enumerate(zip(wl, gl, strict=False)) if not same(x, y)),
        min(len(wl), len(gl)),
    )
    return (
        f"line {first + 1}: FSL {wl[first] if first < len(wl) else '<end>'!r}, "
        f"original {gl[first] if first < len(gl) else '<end>'!r}"
    )


def _unit10(v: float) -> float:
    """One unit of the 10th significant digit of ``v`` (FSL writes matrix files in ``%.10g``)."""
    return 10.0 ** (np.floor(np.log10(abs(v))) - 9) if v and np.isfinite(v) else 0.0


def _numbers_close(want: str, got: str, tol) -> tuple[bool, float]:
    """The same text with the numbers masked, and every number within ``tol(fsl value)``
    (NaN equal to NaN); the largest difference."""
    if _masked(want) != _masked(got):
        return False, float("inf")
    na, nb = _numbers(want), _numbers(got)
    if len(na) != len(nb):
        return False, float("inf")
    worst = 0.0
    for x, y in zip(na, nb, strict=True):
        if np.isnan(x) and np.isnan(y):
            continue
        d = abs(x - y) if np.isfinite(x) and np.isfinite(y) else (0.0 if x == y else np.inf)
        worst = max(worst, d)
        if d > tol(x):
            return False, worst
    return True, worst


def compare_fit(
    checks: CheckList, program: str, oracle: Completed, ours: Completed, a: Path, b: Path
) -> None:
    """``midtrans``, ``pointflirt`` and ``aff2rigid`` (§13: SVD-based): exit status and the same
    text, the numbers printed (6 decimals) within one unit of their last digit and the matrices
    written within 1e-9 of FSL's (plus one unit of the 10th significant digit they are written
    with); every other file identical."""
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the original exits 1)",
            ours.code == 1,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
    else:
        checks("exit status", oracle.code == ours.code, detail=f"{oracle.code} vs {ours.code}")
    printed = 1e-6 + 1e-12
    for stream in ("stdout", "stderr"):
        want = rep._clean(getattr(oracle, stream), a, program)
        got = rep._clean(_own(getattr(ours, stream)), b, program)
        ok, worst = (True, 0.0) if want == got else _numbers_close(want, got, lambda _x: printed)
        checks(
            f"{stream}: the same text, the numbers within one unit of their last printed digit",
            ok,
            metric="max difference",
            value=float(f"{worst:.3g}"),
            threshold=printed,
            detail="" if ok else _first_difference(want, got, lambda x, y: x == y),
        )
    theirs, mine = _files(a, set()), _files(b, set())
    checks(
        "files written",
        set(theirs) == set(mine),
        detail=f"only FSL {sorted(set(theirs) - set(mine))[:6]}, "
        f"only the original {sorted(set(mine) - set(theirs))[:6]}",
    )
    for name in sorted(set(theirs) & set(mine)):
        if rep._same_bytes(theirs[name], mine[name]):
            continue
        ta = theirs[name].read_text(errors="replace")
        tb = mine[name].read_text(errors="replace")
        ok, worst = _numbers_close(ta, tb, lambda x: 1e-9 + _unit10(x))
        checks(
            f"{name}: within 1e-9 of FSL's in every element",
            ok,
            metric="max difference",
            value=float(f"{worst:.3g}"),
            threshold="1e-9 + one unit of the 10th significant digit",
        )


def _interp_of(args: list[str]) -> str:
    """applyxfm4D's interpolation (§15.7: the words after the fourth; sinc by default)."""
    method = "sinc"
    words = args[4:]
    for i, w in enumerate(words[:-1]):
        if w.lstrip("-") == "interp":
            method = words[i + 1]
    return "nearestneighbour" if method == "nn" else method


def compare_4d(
    checks: CheckList, args: list[str], oracle: Completed, ours: Completed, a: Path, b: Path
) -> None:
    """``applyxfm4D`` (§13): exit status, the same text, the same files; the images
    bit-identical for nearest neighbour and trilinear, for sinc and spline the same header and
    grid and a normalised correlation with FSL's values of at least 0.9999 (identical where
    both are exact)."""
    compare_text_streams(checks, "applyxfm4D", oracle, ours, a, b)
    theirs, mine = _files(a, set()), _files(b, set())
    checks(
        "files written",
        set(theirs) == set(mine),
        detail=f"only FSL {sorted(set(theirs) - set(mine))[:6]}, "
        f"only the original {sorted(set(mine) - set(theirs))[:6]}",
    )
    method = _interp_of(args)
    for name in sorted(set(theirs) & set(mine)):
        fa, fb = theirs[name], mine[name]
        if rep._same_bytes(fa, fb):
            continue
        if not _is_image(name):
            checks(f"{name} identical", False, detail="the bytes differ")
            continue
        same = _same_streams(fa, fb, blank_descrip=True)
        if same or method in ("nearestneighbour", "trilinear"):
            checks(
                f"{name} bit-identical ({method})",
                same,
                detail="" if same else _image_difference(fa, fb),
            )
            continue
        ia, ib = _load(fa), _load(fb)
        fields = [
            k for k in ia.header if k not in ("descrip",) and _differs(ia.header[k], ib.header[k])
        ]
        checks(f"{name}: the same header", not fields, detail=", ".join(fields))
        da = np.asanyarray(ia.dataobj).astype(np.float64)
        db = np.asanyarray(ib.dataobj).astype(np.float64)
        if da.shape != db.shape:
            checks(f"{name}: the same grid", False, detail=f"{da.shape} vs {db.shape}")
            continue
        ok = np.isfinite(da) & np.isfinite(db)
        nonfinite_same = bool(np.array_equal(np.isnan(da), np.isnan(db)))
        c = _ncc(np.where(ok, da, 0), np.where(ok, db, 0), np.ones(da.shape, bool))
        checks(
            f"{name}: correlation with FSL's values ({method})",
            c >= 0.9999 and nonfinite_same,
            metric="NCC",
            value=round(c, 7) if np.isfinite(c) else str(c),
            threshold=0.9999,
            detail=_image_difference(fa, fb)
            + ("" if nonfinite_same else "; the NaN voxels differ"),
        )


def compare_text_streams(
    checks: CheckList, program: str, oracle: Completed, ours: Completed, a: Path, b: Path
) -> None:
    """The exit status (where FSL aborts, the original exits 1) and both streams identical."""
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the original exits 1)",
            ours.code == 1,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
    else:
        checks("exit status", oracle.code == ours.code, detail=f"{oracle.code} vs {ours.code}")
    for stream in ("stdout", "stderr"):
        want = rep._clean(getattr(oracle, stream), a, program)
        got = rep._clean(_own(getattr(ours, stream)), b, program)
        checks(
            f"{stream} identical",
            want == got,
            detail="" if want == got else _first_difference(want, got, lambda x, y: x == y),
        )


def _aff2rigid_fix(
    checks: CheckList, run: rep.Run, args: list[str], settings: dict[str, str | None]
) -> None:
    """FSL-81's correction: the original's default matrix against the replica's (which computes
    the script's matrix with NumPy's arithmetic), within 1e-9; reported when the replica's
    program is not built."""
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary

    if replica_binary() is None or len(args) < 2:
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-flirt-a2r-") as c_,
        tempfile.TemporaryDirectory(prefix="lx-flirt-a2r-") as d_,
    ):
        c, d = Path(c_), Path(d_)
        rep._prepare(run, c)
        rep._prepare(run, d)
        r = rep.run_replica("aff2rigid", args, cwd=c, settings=settings, mode=defects.FIX)
        o = run_original("aff2rigid", args, cwd=d, settings=settings, mode=defects.FIX)
        fa, fb = c / args[1], d / args[1]
        if r.code != 0 or not fa.is_file():
            return
        ok, worst = (
            _numbers_close(fa.read_text(), fb.read_text(), lambda _x: 1e-9)
            if o.code == 0 and fb.is_file()
            else (False, float("inf"))
        )
        checks(
            "FSL-81's correction: the matrix within 1e-9 of the replica's (NumPy's arithmetic)",
            ok,
            metric="max difference",
            value=float(f"{worst:.3g}"),
            threshold=1e-9,
            detail=f"original exit {o.code}",
        )


def compare(
    checks: CheckList,
    program: str,
    args: list[str],
    oracle: Completed,
    ours: Completed,
    a: Path,
    b: Path,
    band=None,
) -> None:
    """Exit status, streams, files, matrices and images (the module's rules). ``band()``, when
    given, is FSL's own variability for this registration (its p95 displacement on a perturbed
    input), asked for only where the original is beyond the fixed threshold."""
    refused_75 = ours.code == 1 and any(
        ln.startswith("larmorx:") and "FSL-75" in ln and "refused" in ln
        for ln in ours.stderr.splitlines()
    )
    if refused_75 and oracle.code <= 0:
        # FSL-75's single voxel: FSL reads an empty buffer (its run is undefined: it finishes or
        # is killed); the original refuses before writing anything (specs/flirt.md §10a)
        checks(
            "a single voxel's background value (FSL-75): the original refuses (exit 1)",
            True,
            detail=f"{program} {oracle.code}, original {ours.code}",
        )
        return
    if oracle.code < 0 and "FSL-75" in defects.notices_in(ours.stderr):
        # FSL-75: FSL wrote past its buffer and went on with a corrupted heap until the C library
        # stopped it; the original takes every border value and names the entry (§10a)
        checks(
            f"{program} is killed after FSL-75's heap corruption; the original goes on and names "
            "FSL-75",
            ours.code >= 0,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
        return
    if oracle.code < 0:
        checks(
            f"exit status ({program} is killed; the original exits 1)",
            ours.code == 1,
            detail=f"{program} signal {-oracle.code}, original {ours.code}",
        )
    else:
        checks("exit status", oracle.code == ours.code, detail=f"{oracle.code} vs {ours.code}")
    for stream in ("stdout", "stderr"):
        want = rep._clean(getattr(oracle, stream), a, program)
        got = rep._clean(_own(getattr(ours, stream)), b, program)
        if oracle.code < 0:
            want = re.sub(r"terminate called after[^\n]*\n(  what\(\):[^\n]*\n)?", "", want)
            got = re.sub(r"terminate called after[^\n]*\n(  what\(\):[^\n]*\n)?", "", got)
        same = want == got
        close = _masked(want) == _masked(got)
        detail = ""
        value = "yes" if same else ("numbers differ" if close else "no")
        if not close and _verbose_level(args) >= 5:
            # every cost evaluation printed: their number follows the optimisation's path
            (cw, nw), (cg, ng) = _collapsed(want), _collapsed(got)
            if cw == cg:
                close = True
                value = "evaluations differ"
                detail = f"{nw} evaluation lines from FSL, {ng} from the original"
        if not close:
            wl, gl = want.splitlines(), got.splitlines()
            first = next(
                (
                    i
                    for i, (x, y) in enumerate(zip(wl, gl, strict=False))
                    if _masked(x) != _masked(y)
                ),
                min(len(wl), len(gl)),
            )
            detail = (
                f"line {first + 1}: FSL {wl[first] if first < len(wl) else '<end>'!r}, "
                f"original {gl[first] if first < len(gl) else '<end>'!r}"
            )
        checks(
            f"{stream}: the same lines (numbers aside)",
            close,
            metric="identical",
            value=value,
            threshold="the same text, numbers read as numbers"
            + (" (runs of evaluation lines as one)" if _verbose_level(args) >= 5 else ""),
            detail=detail,
        )
    theirs, mine = _files(a, set()), _files(b, set())
    checks(
        "files written",
        set(theirs) == set(mine),
        detail=f"only FSL {sorted(set(theirs) - set(mine))[:6]}, "
        f"only the original {sorted(set(mine) - set(theirs))[:6]}",
    )
    applying = "-applyxfm" in args or "-applyisoxfm" in args or program == "convert_xfm"
    # the images compare bit for bit where both programs used the same matrix: -applyxfm's, or
    # a registration's whose written matrices are identical
    mats_identical = applying or any(n.endswith(".mat") for n in theirs)
    ref_path = _reference_path(args, a)
    pts = None
    #: a matrix within FSL's own variability only (an ill-posed registration): the image follows
    #: the matrix, so its correlation with FSL's is reported, not held to 0.999
    ill_posed = False
    for name in sorted(set(theirs) & set(mine)):
        fa, fb = theirs[name], mine[name]
        if _is_image(name):
            continue
        ta, tb = fa.read_text(errors="replace"), fb.read_text(errors="replace")
        if ta == tb:
            continue
        mats_identical = False
        na, nb = _numbers(ta), _numbers(tb)
        is_mat = name.endswith(".mat") and len(na) == 16 and len(nb) == 16
        usesqform = "-usesqform" in args and "-init" not in args
        if is_mat and "-fixscaleskew" in args:
            # the decomposition and composition's last digits (spec question)
            rel = float(np.max(np.abs(np.array(na) - np.array(nb)) / np.maximum(np.abs(na), 1)))
            checks(
                f"{name}: within 1e-7 of FSL's (relative)",
                rel <= 1e-7,
                metric="max relative difference",
                value=float(f"{rel:.3g}"),
                threshold=1e-7,
            )
            continue
        if not applying and not is_mat and len(na) == len(nb) and na:
            # a schedule's text output (rows, parameters) of a registration: its numbers
            rel = float(np.max(np.abs(np.array(na) - np.array(nb)) / np.maximum(np.abs(na), 1.0)))
            checks(
                f"{name}: the same numbers within 5 % (a registration's rows)",
                rel <= 0.05,
                metric="max relative difference",
                value=float(f"{rel:.3g}"),
                threshold=0.05,
            )
            continue
        if applying or not is_mat:
            if is_mat and usesqform and len(na) == len(nb):
                # the sform/qform initialisation: its last digits (spec question)
                dev = float(np.max(np.abs(np.array(na) - np.array(nb))))
                checks(
                    f"{name}: the sform/qform matrix within 1e-5 mm",
                    dev <= 1e-5,
                    metric="max abs difference",
                    value=dev,
                    threshold=1e-5,
                )
                continue
            if is_mat or len(na) != len(nb):
                checks(f"{name} identical", False, detail="the text differs")
                continue
            dev = float(np.max(np.abs(np.array(na) - np.array(nb)))) if na else 0.0
            checks(
                f"{name}: the same numbers within 1e-4",
                bool(np.allclose(na, nb, rtol=1e-4, atol=1e-4, equal_nan=True)),
                metric="max abs difference",
                value=round(dev, 6),
                threshold=1e-4,
            )
            continue
        m1, m2 = np.array(na).reshape(4, 4), np.array(nb).reshape(4, 4)
        if pts is None and ref_path is not None:
            pts = _reference_mask(ref_path)
        if pts is None or not (np.all(np.isfinite(m1)) and np.all(np.isfinite(m2))):
            checks(f"{name}: the matrices", np.allclose(m1, m2, equal_nan=True))
            continue
        try:
            p95, mx = displacement(m1, m2, pts[0])
        except np.linalg.LinAlgError:
            checks(f"{name}: invertible matrices", False)
            continue
        thr = _threshold(args)
        detail = f"maximum {mx:.4f} mm, RMS deviation {rep._rms80(m1, m2):.4f} mm"
        limit = thr
        if p95 > thr and band is not None:
            own = band(name, pts[0])
            if own is not None:
                limit = max(thr, own)
                detail += f"; FSL's own p95 on a perturbed input {own:.4f} mm"
                ill_posed = ill_posed or p95 <= limit
        checks(
            f"{name}: 95th-percentile displacement over the reference's brain",
            p95 <= limit,
            metric="p95 displacement (mm)",
            value=round(p95, 4),
            threshold=thr if limit == thr else f"{thr} or FSL's own variability",
            detail=detail,
        )
    for name in sorted(set(theirs) & set(mine)):
        if not _is_image(name):
            continue
        fa, fb = theirs[name], mine[name]
        if fa.read_bytes() == fb.read_bytes():
            continue
        if _DEBUG_VOLUMES.match(name):
            # -debug's volumes before any registration: the header and the values (the
            # pyramid's last bits, docs/findings/fsl-flirt-original.md)
            same = _same_streams(fa, fb, blank_descrip=True)
            rel = 0.0 if same else _relative_difference(fa, fb)
            checks(
                f"{name}: the same header, values within 1e-6 of the image's range",
                same or (_same_header(fa, fb) and rel <= 1e-6),
                metric="max difference / range",
                value=float(f"{rel:.3g}"),
                threshold=1e-6,
                detail="" if same else _image_difference(fa, fb),
            )
            continue
        if _SEARCH_GRIDS.match(name):
            # the search's grids: its optimised translations, scales and costs, which follow
            # the optimisations' paths (the matrices' thresholds hold them)
            da = np.asanyarray(_load(fa).dataobj).astype(np.float64)
            db = np.asanyarray(_load(fb).dataobj).astype(np.float64)
            ok = da.shape == db.shape and _same_header(fa, fb)
            rel = (
                float(np.max(np.abs(da - db)) / max(float(np.max(np.abs(da))), 1e-12))
                if ok
                else 1.0
            )
            c = float(np.corrcoef(da.ravel(), db.ravel())[0, 1]) if ok and da.std() > 0 else 1.0
            checks(
                f"{name}: the same grid and header, correlation 0.999 (or within 1e-4 when "
                "constant)",
                ok and (c >= 0.999 if da.std() > 0 else rel <= 1e-4),
                metric="correlation" if ok and da.std() > 0 else "max relative difference",
                value=round(c, 6) if ok and da.std() > 0 else float(f"{rel:.3g}"),
                threshold=0.999 if ok and da.std() > 0 else 1e-4,
            )
            continue
        kernel = any(f"-interp {k}" in " ".join(args) for k in ("sinc", "spline"))
        if mats_identical and not (kernel and not _same_streams(fa, fb, blank_descrip=True)):
            same = _same_streams(fa, fb, blank_descrip=True)
            detail = "" if same else _image_difference(fa, fb)
            checks(f"{name} bit-identical", same, detail=detail)
        elif mats_identical:
            # sinc and spline from the same matrix: the kernels' last bits (spec question)
            rel = _relative_difference(fa, fb)
            checks(
                f"{name}: the same values within 1e-5 of the image's range",
                rel <= 1e-5,
                metric="max difference / range",
                value=float(f"{rel:.3g}"),
                threshold=1e-5,
                detail=_image_difference(fa, fb),
            )
        else:
            ia, ib = _load(fa), _load(fb)
            da = np.asanyarray(ia.dataobj).astype(np.float64)
            db = np.asanyarray(ib.dataobj).astype(np.float64)
            if da.shape != db.shape:
                checks(f"{name}: the same grid", False, detail=f"{da.shape} vs {db.shape}")
                continue
            rm = None
            if ref_path is not None:
                try:
                    r = np.asanyarray(_load(ref_path).dataobj)
                    if r.ndim > 3:
                        r = r[..., 0]
                    r = r.reshape(da.shape[:3])
                    rm = r > 0
                except Exception:
                    rm = None
            if rm is None or not rm.any():
                rm = np.ones(da.shape[:3], bool)
            if da.ndim > 3:
                rm = np.broadcast_to(rm[..., None], da.shape)
            ok = np.isfinite(da) & np.isfinite(db)
            c = _ncc(np.where(ok, da, 0), np.where(ok, db, 0), rm)
            checks(
                f"{name}: correlation with FSL's image inside the reference's mask"
                + (" (reported: the matrix within FSL's own variability)" if ill_posed else ""),
                ill_posed or c >= 0.999,
                metric="NCC",
                value=round(c, 6) if np.isfinite(c) else str(c),
                threshold="reported" if ill_posed else 0.999,
            )


def _same_header(fa: Path, fb: Path) -> bool:
    """The two images' headers equal but for their descriptions."""
    ha, hb = _load(fa).header.copy(), _load(fb).header.copy()
    ha["descrip"] = hb["descrip"] = b""
    return bytes(ha.binaryblock) == bytes(hb.binaryblock)


def _relative_difference(fa: Path, fb: Path) -> float:
    da = np.asanyarray(_load(fa).dataobj).astype(np.float64)
    db = np.asanyarray(_load(fb).dataobj).astype(np.float64)
    if da.shape != db.shape:
        return float("inf")
    ok = np.isfinite(da) & np.isfinite(db)
    if not ok.any():
        return 0.0
    rng = float(np.ptp(da[ok])) or 1.0
    return float(np.max(np.abs(da[ok] - db[ok]))) / rng


def _band(run: rep.Run, args: list[str], settings: dict[str, str | None], a: Path):
    """FSL's own variability for a registration (§13's band): FSL on the input with uniform noise
    of ±0.5 intensity units (seeded), its matrix against its own unperturbed one; the
    95th-percentile displacement over the reference's brain (``None`` where it cannot run)."""

    def band(name: str, pts: np.ndarray) -> float | None:
        import nibabel as nib

        try:
            i = args.index("-in")
        except ValueError:
            return None
        with tempfile.TemporaryDirectory(prefix="lx-flirt-band-") as c_:
            c = Path(c_)
            rep._prepare(run, c)
            src = c / args[i + 1]
            if not src.is_file():
                return None
            img = nib.load(str(src))
            data = np.asanyarray(img.dataobj).astype(np.float32)
            rng = np.random.default_rng(20261102)
            noisy = data + rng.uniform(-0.5, 0.5, data.shape).astype(np.float32)
            out = nib.Nifti1Image(noisy, img.affine, img.header)
            out.set_data_dtype(np.float32)
            src.unlink()
            nib.save(out, str(src))
            p = rep.run_oracle(run.program, args, cwd=c, settings=settings)
            f1, f2 = a / name, c / name
            if p.code != 0 or not f1.is_file() or not f2.is_file():
                return None
            try:
                return displacement(np.loadtxt(f1), np.loadtxt(f2), pts)[0]
            except (ValueError, np.linalg.LinAlgError):
                return None

    return band


def _differs(a, b) -> bool:
    """Whether two header fields differ (NaN equal to NaN)."""
    x, y = np.asarray(a), np.asarray(b)
    if x.dtype.kind in "fc" and y.dtype.kind in "fc":
        return not np.array_equal(x, y, equal_nan=True)
    return not np.array_equal(x, y)


def _image_difference(fa: Path, fb: Path) -> str:
    try:
        ia, ib = _load(fa), _load(fb)
        da = np.asanyarray(ia.dataobj).astype(np.float64)
        db = np.asanyarray(ib.dataobj).astype(np.float64)
    except Exception as e:
        return f"cannot read: {e}"
    if da.shape != db.shape:
        return f"shapes {da.shape} vs {db.shape}"
    diff = ~((da == db) | (np.isnan(da) & np.isnan(db)))
    if not diff.any():
        fields = [k for k in ia.header if k != "descrip" and _differs(ia.header[k], ib.header[k])]
        return "the headers differ" + (f": {', '.join(fields)}" if fields else "")
    return f"{int(diff.sum())} of {diff.size} values differ (max {np.nanmax(np.abs(da - db)):.4g})"


def _reference_path(args: list[str], cwd: Path) -> Path | None:
    for i, x in enumerate(args[:-1]):
        if x == "-ref":
            p = Path(args[i + 1])
            if not p.is_absolute():
                p = cwd / p
            for cand in (p, Path(str(p) + ".nii.gz"), Path(str(p) + ".nii")):
                if cand.is_file():
                    return cand
    return None


def fix_pass(
    checks: CheckList,
    program: str,
    prepare,
    args: list[str],
    settings: dict[str, str | None],
    reproduced_dir: Path,
    reproduced: Completed,
) -> None:
    """D13: the original's default run against its ``reproduce`` run (a defect named exactly
    when something changed)."""
    with tempfile.TemporaryDirectory(prefix="lx-flirt-fix-") as name:
        tmp = Path(name)
        if prepare is not None:
            prepare(tmp)
        before = set(_files(tmp, set()))
        r = run_original(program, args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        if program in (*TEXT_PROGRAMS, *FIT_PROGRAMS) and r.stdout != reproduced.stdout:
            # the programs whose results are printed
            others.append("standard output")
        a = dict(_files(reproduced_dir, set()))
        bb = dict(_files(tmp, set()))
        if set(a) != set(bb):
            others.append(f"files {sorted(set(a) ^ set(bb))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted((set(a) & set(bb)) - before):
            if _is_image(key):
                from larmorx_validation.parity.fsl_mcflirt_replica import _change

                change = change + _change(bb[key], a[key])
            elif a[key].read_bytes() != bb[key].read_bytes():
                try:
                    va = np.array([float(x) for x in a[key].read_text().split()])
                    vb = np.array([float(x) for x in bb[key].read_text().split()])
                    change = change + defects.Change.of(vb, va)
                except ValueError:
                    others.append(key)
        defects.add_fix_check(
            checks,
            entries,
            change,
            other_changes="; ".join(others),
            silent_ok=(*rep._SILENT, *defects.CONDITION_NOTICES),
        )


def _pending(args: list[str], run: rep.Run) -> str | None:
    words = list(args) + [t for _, t in run.files]
    for k, why in PENDING.items():
        if k in words:
            return why
    return None


def _run_case(run: rep.Run, checks: CheckList) -> None:
    pending = _pending(list(run.args), run)
    if pending:
        raise Outcome(SKIPPED, f"not in the clean-room original yet: {pending}")
    if run.hang or run.crash:
        _undefined_case(run, checks)
        return
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-flirt-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-flirt-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = rep._prepare(run, a)
        rep._prepare(run, b)
        oracle = rep.run_oracle(run.program, args, cwd=a, settings=settings)
        ours = run_original(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.program in TEXT_PROGRAMS:
            compare_text(checks, run.program, oracle, ours, a, b)
        elif run.program in FIT_PROGRAMS:
            compare_fit(checks, run.program, oracle, ours, a, b)
        elif run.program == "applyxfm4D":
            compare_4d(checks, args, oracle, ours, a, b)
        elif run.program == "pairreg":
            # FLIRT's matrices (§13), the first brain the reference
            compare(checks, "pairreg", [*args, "-ref", args[0]], oracle, ours, a, b)
        else:
            compare(
                checks, run.program, args, oracle, ours, a, b, band=_band(run, args, settings, a)
            )
        if run.program == "aff2rigid":
            _aff2rigid_fix(checks, run, args, settings)
        if run.fix and ours.code == 0:
            fix_pass(checks, run.program, lambda t: rep._prepare(run, t), args, settings, b, ours)
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[:200]
        raise Outcome(
            BOTH_ERROR, f"{run.program} exits {oracle.code}; the same from the original. {tail}"
        )


def _undefined_case(run: rep.Run, checks: CheckList) -> None:
    """FSL never finishes or crashes (the register entry): FSL is not run again here (the
    replica suite measures it, never cached); the original refuses in both modes (or, where
    FSL-75's result is defined, finishes naming it; with ``hang_fixed``, FSL-93, its default
    corrects the entry and finishes naming it, and only ``reproduce`` refuses)."""
    settings = dict(run.settings)
    entry = run.hang or run.crash
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-flirt-") as b_:
            b = Path(b_)
            r = run_original(run.program, rep._prepare(run, b), cwd=b, settings=settings, mode=mode)
            if run.hang_fixed and mode == defects.FIX:
                # the default corrects the entry: it finishes, naming it (FSL-93)
                named = defects.notices_in(r.stderr)
                checks(
                    f"the original's default finishes and names {entry}",
                    r.code == 0 and entry in named and bool(list(b.glob("out*"))),
                    detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
                )
                continue
            if run.crash_finishes:
                named = defects.notices_in(r.stderr)
                checks(
                    f"the original finishes and names {entry} ({mode})",
                    r.code == 0 and entry in named,
                    detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
                )
            else:
                checks(
                    f"the original refuses naming {entry} ({mode})",
                    r.code == 1 and entry in r.stderr,
                    detail=f"exit {r.code}: {' '.join(r.stderr.split())[-160:]}",
                )
    if all(c.passed for c in checks.checks):
        raise Outcome(
            EXPECTED,
            f"FSL {'never finishes' if run.hang else 'crashes'} ({entry}: undefined behaviour); "
            "the original "
            + (
                "finishes naming it"
                if run.crash_finishes
                else "corrects it by default and refuses with reproduce"
                if run.hang_fixed
                else "refuses in both modes"
            ),
        )


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    args = rep.fmriprep_args(run, node)
    missing = [x for x in args if x.startswith("/") and not Path(x).exists()]
    if missing:
        raise Outcome(SKIPPED, f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-flirt-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-flirt-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = rep.run_oracle("flirt", args, cwd=a, settings={})
        ours = run_original("flirt", args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, "flirt", args, oracle, ours, a, b)
        theirs = next(Path(node).glob("*.mat"), None)
        mine = next(b.glob("*.mat"), None)
        ref = _reference_path(args, a)
        if theirs is not None and mine is not None and ref is not None:
            pts = _reference_mask(ref)
            if pts is not None:
                p95, _ = displacement(np.loadtxt(theirs), np.loadtxt(mine), pts[0])
                checks.add(
                    Check(
                        "fMRIPrep's own matrix (its container's FSL): displacement",
                        True,
                        metric="p95 displacement (mm)",
                        value=round(p95, 4),
                        threshold="reported",
                    )
                )
        if oracle.code == 0 and ours.code == 0:
            fix_pass(checks, "flirt", None, args, {}, b, ours)


_ROW = "setrow UQ 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"
_COST = "measurecost 6 UQ 0 0 0 0 0 0 rel"


def _sched(*lines: str) -> tuple[tuple[str, str], ...]:
    return (("s.sch", "\n".join(lines) + "\n"),)


def _original_cases() -> list[tuple[str, str, rep.Run]]:
    """The original's own cases (part 2): the verbose levels' and -debug's code paths the replica
    suite's cases leave out (each once; FSL's runs cached under their own keys)."""
    E, T = "@epi.nii", "@t1.nii"
    base = (E, "-ref", T)
    sch = ("-omat", "out.mat", "-schedule", "s.sch")
    r = rep.Run
    return [
        (
            "verbose/6-search-debugsave",
            "-verbose 6 -debugsave on the search alone: its inner level 4 and the grids it saves",
            r(
                "flirt",
                ("-in", *base, "-verbose", "6", "-debugsave", *sch),
                files=_sched("setscale 8", "clear S", "clear P", "search", "print S"),
            ),
        ),
        (
            "verbose/7",
            "-verbose 7 on the default schedule: the search's evaluations, starts and tolerances",
            r("flirt", ("-in", *base, "-verbose", "7", "-dof", "6", "-omat", "out.mat")),
        ),
        (
            "verbose/5-optimisers",
            "-verbose 5: gridmeasurecost, Brent, Powell's correction, a parameter subset",
            r(
                "flirt",
                ("-in", *base, "-verbose", "5", *sch),
                files=_sched(
                    "setscale 4",
                    _ROW,
                    _COST,
                    "gridmeasurecost 6 UQ -0.1 0.1 0.1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 abs",
                    "setoption optimisationtype brent",
                    "optimise 6 UQ 0 0 0 0 0 0 0 rel 2",
                    "setoption optimisationtype powell",
                    "setoption paramsubset 1  0 0 0 1 0 0 0 0 0 0 0 0",
                    "optimise 6 UQ 0 0 0 0 0 0 0 rel 2",
                    "print U",
                ),
            ),
        ),
        (
            "verbose/7-subset",
            "-verbose 7: Brent's and a subset's starts, tolerances and doubled evaluations",
            r(
                "flirt",
                ("-in", *base, "-verbose", "7", *sch),
                files=_sched(
                    "setscale 4",
                    _ROW,
                    "setoption optimisationtype brent",
                    "optimise 6 UQ 0 0 0 0 0 0 0 rel 2",
                    "setoption optimisationtype powell",
                    "setoption paramsubset 1  0 0 0 1 0 0 0 0 0 0 0 0",
                    "optimise 6 UQ 0 0 0 0 0 0 0 rel 2",
                ),
            ),
        ),
        (
            "verbose/3-scales-options",
            "-verbose 3: forced and input-only scales (the evaluation counts), every setoption "
            "echoed, a short tolerance list's index error",
            r(
                "flirt",
                ("-in", *base, "-verbose", "3", *sch),
                files=_sched(
                    "setscale 8",
                    _ROW,
                    _COST,
                    "setscale 2 force",
                    _COST,
                    "setscale 1",
                    _COST,
                    "setscale 4",
                    _COST,
                    "setscale 2",
                    "setoption tolerance 0.1 0.2 0.3",
                    "setoption costfunction normcorr",
                    "setoption optimisationtype brent",
                    "setoption fuzzyfraction 0.3",
                    "setoption bbrtype global_abs",
                    "setoption smoothing 3",
                    "setoption nosubset 1 2",
                    "setoption boundguess 0",
                    "setoption bbrstep 3",
                    "setoption bbrslope 0.4",
                    "setoption minsampling 2",
                    "setoption frobnicate 2",
                    _COST,
                    "setscale 8",
                    _COST,
                ),
            ),
        ),
        (
            "verbose/2-noclamp",
            "-verbose 2 -noclamp: the images' ranges unclamped",
            r(
                "flirt",
                ("-in", *base, "-verbose", "2", "-noclamp", *sch),
                files=_sched("setscale 4", _ROW, _COST),
            ),
        ),
        (
            "verbose/2-noresample",
            "-verbose 2 -noresample: no subsampling lines",
            r(
                "flirt",
                ("-in", *base, "-verbose", "2", "-noresample", *sch),
                files=_sched("setscale 4", _ROW, _COST),
            ),
        ),
        (
            "verbose/2-applyxfm",
            "-verbose 2 -applyxfm: the images read and the resampled volume",
            r(
                "flirt",
                ("-in", *base, "-verbose", "2", "-applyxfm", "-init", "@init.mat", "-out", "out"),
            ),
        ),
        (
            "verbose/2-out",
            "-verbose 2 -out without -omat: the transformed input, then the final result",
            r(
                "flirt",
                ("-in", *base, "-verbose", "2", "-out", "out", "-schedule", "s.sch"),
                files=_sched("setscale 8", _ROW, _COST),
            ),
        ),
        (
            "verbose/3-2d",
            "-2D -verbose 3: the tripled slice, no sform messages",
            r(
                "flirt",
                ("-in", "@slice_in.nii", "-ref", "@slice_ref.nii", "-2D", "-verbose", "3", *sch),
                files=_sched("setscale 4", _ROW, _COST),
            ),
        ),
        (
            "verbose/3-debug",
            "-verbose 3 -debug: each setscale's debug lines before the input's",
            r(
                "flirt",
                ("-in", *base, "-verbose", "3", "-debug", *sch),
                files=_sched("setscale 4", "setscale 2", _ROW, _COST),
            ),
        ),
        (
            "debug/scales",
            "-debug over forced and input-only scales: the pair's input at each setscale",
            r(
                "flirt",
                ("-in", *base, "-debug", *sch),
                files=_sched(
                    "setscale 8",
                    _ROW,
                    _COST,
                    "setscale 2 force",
                    _COST,
                    "setscale 1",
                    _COST,
                    "setscale 4",
                    _COST,
                ),
            ),
        ),
        (
            "debug/bbr-setoption",
            "-debug -wmseg: the boundary-based cost set up by setoption and by a new pair",
            r(
                "flirt",
                ("-in", *base, "-wmseg", "@wm.nii", "-debug", *sch),
                files=_sched(
                    "setscale 4",
                    "setoption costfunction bbr",
                    "setscale 2 force",
                    "setoption costfunction normcorr",
                    "setscale 4",
                ),
            ),
        ),
    ]


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for c in rep.cases(tier):
        kind, payload = c.payload
        if kind == "run":
            run = rep._all_cases()[payload][2]
            if run.program not in PROGRAMS:
                continue
        out.append(c)
    if in_tier("standard", tier):
        for name, desc, run in _original_cases():
            out.append(Case(name, name.split("/")[0], desc, ("direct", run)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        _run_case(payload, checks)
    else:
        _run_case(rep._all_cases()[payload][2], checks)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="FLIRT (clean-room original)",
        tool=f"`larmorx fsl flirt`, `convert_xfm` and FLIRT's other programs (larmorx {larmorx.__version__}, crate "
        "larmorx-fsl: the clean-room original written from specs/flirt.md on the shared "
        "registration engine of larmorx-mri, Apache-2.0)",
        reference="flirt, convert_xfm and FLIRT's other programs from FSL 6.0.7.23 (fsl-flirt 2111.4), the conda build at "
        "`~/fsl-6.0.7.23`",
        thresholds=[
            ("Exit status", "the same (where FSL aborts, the original exits 1)"),
            ("Standard output and error", "the same lines, the numbers in them read as numbers"),
            ("Files written", "the same names"),
            (
                "Matrices",
                "identical text, or the 95th-percentile displacement over the reference's brain "
                "within 0.1 mm (boundary-based, 6 dof) or 0.5 mm (other registrations); "
                "`-applyxfm` and `convert_xfm` identical",
            ),
            (
                "Images",
                "bit-identical from the same matrix; from the original's own matrix, NCC with "
                "FSL's image at least 0.999 inside the reference's mask",
            ),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel", "scipy"),
        notes=(
            "The cases are the replica suite's (`fsl-flirt-replica`) for `flirt`, "
            "`convert_xfm` and FLIRT's other programs; FSL's runs are served from the oracle cache under that suite's keys. "
            "The original is compared in `reproduce` mode (D13) and its default `fix` pass "
            "against the reproduced run. The original's own cases (`verbose/` and `debug/` "
            "beyond the replica's: part 2) cover the verbose levels' and -debug's code paths; at "
            "a verbose level of 5 or more the streams are compared with each run of cost "
            "evaluation lines as one (the evaluations follow the optimisation's path). "
            "-debug's volumes before the registration (the pyramid, its weights, the input at "
            "8 mm) are held to FSL's header and values within 1e-6 of their range, "
            "-debugsave's search grids to FSL's header and a correlation of 0.999."
        ),
    )
