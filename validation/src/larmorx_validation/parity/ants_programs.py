# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants <program>`` with ANTs 2.6.5's own binaries: the shared harness.

Every suite of an ANTs image program (ImageMath, ThresholdImage, MultiplyImages, SmoothImage,
ResampleImageBySpacing, ...) is a list of :class:`AntsCase` and a call to :func:`make_suite`.
For each case the harness:

1. resolves the arguments: ``{out}`` (and ``{out2}``, ...) are output files, a separate one
   for each program; ``{in:<catalog path>}`` is a ``larmorx-testdata`` file;
   ``{gen:<name>}`` a generated input (:mod:`larmorx_validation.parity.ants_inputs`);
   ``{tmp}`` the case's temporary directory;
2. runs the ANTs binary (``LARMORX_ANTS_BIN``, else the workspace's
   ``oracles/ants-2.6.5/bin``, built by ``scripts/build_ants_oracle.sh``) and
   ``larmorx ants <program>`` (in-process, through the Python console entry point) on the
   same inputs, with ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`` set for both;
3. compares what they wrote, read with nibabel (a reader independent of both):
   - the **header**, field by field from the raw 348 bytes (every field must match; the
     report says whether all bytes do);
   - the **data**: type, shape and every value (bit for bit; NaN equals NaN). A case passes
     only when the values are bit-identical, unless it declares ``tolerance_ulps`` (the
     largest distance in units in the last place) with the reason; the number of differing
     values, the largest difference and the largest distance in ulps are always reported;
   - the **affine** (as nibabel reads it), and **stdout** when ``stdout=True``.

A case ends as ``both-error`` when neither program produces its outputs (ANTs exits non-zero,
crashes, or exits 0 without writing, as ImageMath does after printing an error).

**Adding cases** (another filter group): write a module with a list of :class:`AntsCase`
(for ImageMath, ``parity/ants_image_math/<group>.py`` with ``CASES``; register it in that
package's ``GROUPS``), or a new suite module calling :func:`make_suite` and registered in
``larmorx_validation/__main__.py`` and ``tests/parity/test_ants_programs.py``. Inputs that
need building go in :mod:`~larmorx_validation.parity.ants_inputs` with ``@builder``.
"""

from __future__ import annotations

import contextlib
import functools
import io
import os
import re
import subprocess
import tempfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.report import table

ORACLE = "oracles/ants-2.6.5/bin"
#: Threads for both programs (ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS).
THREADS = 4
AFFINE_ATOL = 1e-6
_TIERS = ("smoke", "standard", "full")


@functools.cache
def ants_bin() -> Path | None:
    """The directory of the ANTs oracle binaries: ``LARMORX_ANTS_BIN``, else the workspace's
    ``oracles/ants-2.6.5/bin``. ``None`` if neither has ``ImageMath``."""
    if env := os.environ.get("LARMORX_ANTS_BIN"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        candidate = parent / ORACLE
        if (candidate / "ImageMath").is_file():
            return candidate
    return None


@functools.cache
def ants_version() -> str:
    """What the oracle build records (``BUILD_INFO.txt``): ANTs and ITK commits, compiler."""
    b = ants_bin()
    if b is None:
        return "not found"
    info = b.parent / "BUILD_INFO.txt"
    if info.is_file():
        text = info.read_text(encoding="utf-8")
        ants = re.search(r"ANTs: .*?\(([0-9a-f]{7,})", text)
        itk = re.search(r"ITK: .*?\(([0-9a-f]{7,})", text)
        compiler = re.search(r"compiler: (.*)", text)
        return (
            f"ANTs v2.6.5 ({ants.group(1)[:10] if ants else '?'}) on ITK v5.4.5 "
            f"({itk.group(1)[:10] if itk else '?'}), {compiler.group(1) if compiler else ''}"
        ).strip(", ")
    return str(b)


@dataclass(frozen=True)
class AntsCase:
    """One invocation of an ANTs program, compared with larmorx's.

    ``args`` are the program's arguments with placeholders (see the module docstring).
    ``expect`` is ``"pass"``, ``"error"`` (both must fail) or ``"divergence"`` (ANTs succeeds,
    larmorx refuses on purpose, ``reason`` says why). ``tolerance_ulps`` allows values that are
    not bit-identical, up to that many units in the last place, with ``reason`` explaining
    the cause.
    """

    id: str
    category: str
    description: str
    program: str
    args: tuple[str, ...]
    tier: str = "smoke"
    expect: str = "pass"
    tolerance_ulps: int | None = None
    reason: str = ""
    stdout: bool = False
    outputs: tuple[str, ...] = ("{out}",)
    files: tuple[tuple[str, str], ...] = field(default=())  # (name, text) written to {tmp}


def cases_for(tier: str, all_cases: Sequence[AntsCase]) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(c.id, c.category, c.description, c) for c in all_cases if _TIERS.index(c.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Running


def _resolve(arg: str, tmp: Path, outdir: Path) -> str:
    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key.startswith("out"):
            return str(outdir / f"{key}.nii.gz")
        if key == "tmp":
            return str(tmp)
        if key.startswith("in:"):
            import larmorx_testdata as td

            return str(td.get(key[3:]))
        if key.startswith("gen:"):
            return str(ants_inputs.path(key[4:]))
        if key.startswith("missing:"):
            return str(tmp / key[8:])
        raise KeyError(f"unknown placeholder {{{key}}}")

    return re.sub(r"\{([^{}]+)\}", sub, arg)


def _env() -> dict[str, str]:
    return {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(THREADS)}


def _run_ants(program: str, args: list[str]) -> tuple[int, str, str]:
    p = subprocess.run(
        [str(ants_bin() / program), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=_env(),
        timeout=600,
    )
    return p.returncode, p.stdout, p.stderr


def _run_larmorx(program: str, args: list[str]) -> tuple[int, str, str]:
    from larmorx.cli import run

    out, err = io.StringIO(), io.StringIO()
    previous = os.environ.get("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
    os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(THREADS)
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = run(["larmorx", "ants", program, *args])
    finally:
        if previous is None:
            del os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"]
        else:
            os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = previous
    return code, out.getvalue(), err.getvalue()


def _last_line(text: str, tmp: Path) -> str:
    lines = [ln.strip() for ln in text.strip().splitlines() if ln.strip()]
    line = lines[-1] if lines else ""
    return line.replace(str(tmp), "<tmp>")[:300]


def _exit_text(code: int) -> str:
    if code < 0:
        return f"killed by signal {-code}"
    return f"exit {code}"


# ------------------------------------------------------------------------------------------------
# Comparing


def raw_header(path: Path) -> np.ndarray:
    """The 348-byte NIfTI-1 header as stored, as a structured array (nibabel's layout)."""
    import gzip

    import nibabel as nib

    with path.open("rb") as f:
        magic = f.read(2)
    opener = gzip.open if magic == b"\x1f\x8b" else open
    with opener(path, "rb") as f:
        block = f.read(348)
    return np.frombuffer(block, dtype=nib.nifti1.header_dtype)[0]


def header_differences(a: np.ndarray, b: np.ndarray) -> list[str]:
    """The header fields whose bytes differ, with both values."""
    out = []
    for name in a.dtype.names:
        if a[name].tobytes() != b[name].tobytes():
            out.append(
                f"{name}: ANTs {np.asarray(a[name]).tolist()}, larmorx {np.asarray(b[name]).tolist()}"
            )
    return out


def _canonical_bits(x: np.ndarray) -> np.ndarray:
    x = np.ascontiguousarray(x)
    if x.dtype.kind == "f":
        x = np.where(np.isnan(x), np.array(np.nan, dtype=x.dtype), x)
        return x.view(f"u{x.dtype.itemsize}")
    return x


def ulps(a: np.ndarray, b: np.ndarray) -> int:
    """Largest distance in units in the last place between two float arrays (NaN pairs and
    equal values count 0)."""
    if a.dtype.kind != "f" or a.size == 0:
        return 0
    it = np.int64 if a.dtype.itemsize == 8 else np.int32
    ia = np.ascontiguousarray(a).view(it).astype(np.int64)
    ib = np.ascontiguousarray(b).view(it).astype(np.int64)
    ia = np.where(ia < 0, np.iinfo(it).min - ia, ia)
    ib = np.where(ib < 0, np.iinfo(it).min - ib, ib)
    d = np.abs(ia - ib)
    d = np.where(np.isnan(a) & np.isnan(b), 0, d)
    return int(d.max())


def compare_outputs(
    checks: CheckList, ants_path: Path, lx_path: Path, label: str, tolerance_ulps: int | None
) -> None:
    """Compare two output files (header, type, shape, values, affine)."""
    import nibabel as nib

    prefix = f"{label}: " if label else ""
    ha, hb = raw_header(ants_path), raw_header(lx_path)
    differing = header_differences(ha, hb)
    checks(
        f"{prefix}header (every field)",
        not differing,
        metric="fields differing",
        value=len(differing),
        threshold=0,
        detail="; ".join(differing),
    )
    a_img, b_img = nib.load(ants_path), nib.load(lx_path)
    a = np.asanyarray(a_img.dataobj.get_unscaled())
    b = np.asanyarray(b_img.dataobj.get_unscaled())
    checks(
        f"{prefix}data type",
        a.dtype == b.dtype,
        metric="dtype",
        value=str(b.dtype),
        threshold=str(a.dtype),
    )
    checks(f"{prefix}shape", a.shape == b.shape, detail=f"ANTs {a.shape}, larmorx {b.shape}")
    affine_diff = float(np.abs(a_img.affine - b_img.affine).max())
    checks(
        f"{prefix}affine",
        affine_diff <= AFFINE_ATOL,
        metric="max |diff| (mm)",
        value=f"{affine_diff:.1e}",
        threshold=f"{AFFINE_ATOL:g}",
    )
    if a.shape != b.shape or a.dtype != b.dtype:
        return
    diff_mask = _canonical_bits(a) != _canonical_bits(b)
    n = int(np.count_nonzero(diff_mask))
    if n == 0:
        rel, max_ulp, max_abs = 0.0, 0, 0.0
    else:
        af, bf = a.astype(np.float64), b.astype(np.float64)
        with np.errstate(invalid="ignore"):
            d = np.abs(af - bf)
        d = np.where(np.isnan(af) & np.isnan(bf), 0.0, d)
        d = np.where(np.isnan(d), np.inf, d)
        max_abs = float(d.max())
        finite = np.abs(af[np.isfinite(af)])
        scale = float(finite.max()) if finite.size else 1.0
        rel = max_abs / (scale or 1.0)
        max_ulp = ulps(a, b)
    detail = f"{n} of {a.size} values differ" + (
        f"; max |diff| {max_abs:.3g} ({rel:.1e} of max |ANTs|), at most {max_ulp} ulp" if n else ""
    )
    if tolerance_ulps is None:
        checks(
            f"{prefix}values (bit-identical)",
            n == 0,
            metric="values differing",
            value=n,
            threshold=0,
            detail=detail,
        )
    else:
        checks(
            f"{prefix}values (within tolerance)",
            max_ulp <= tolerance_ulps,
            metric="max distance (ulp)",
            value=max_ulp,
            threshold=tolerance_ulps,
            detail=detail,
        )
    checks.add(
        Check(
            f"{prefix}bit-identical",
            True,
            metric="values differing",
            value=n,
            threshold="reported",
            detail="identical" if n == 0 else detail,
        )
    )
    checks.add(
        Check(
            f"{prefix}header bytes identical",
            True,
            metric="identical",
            value=str(not differing),
            threshold="reported",
        )
    )


def run_case(case: Case, checks: CheckList) -> None:
    c: AntsCase = case.payload
    if ants_bin() is None:
        raise Outcome(SKIPPED, "the ANTs 2.6.5 oracle was not found (LARMORX_ANTS_BIN)")
    with tempfile.TemporaryDirectory(prefix="lx-ants-") as tmp_name:
        tmp = Path(tmp_name)
        for name, text in c.files:
            (tmp / name).write_text(text, encoding="utf-8")
        results = {}
        for tool, runner in [("ants", _run_ants), ("larmorx", _run_larmorx)]:
            outdir = tmp / tool
            outdir.mkdir()
            args = [_resolve(a, tmp, outdir) for a in c.args]
            outputs = [Path(_resolve(o, tmp, outdir)) for o in c.outputs]
            code, out, err = runner(c.program, args)
            produced = code == 0 and all(o.exists() for o in outputs)
            results[tool] = (code, out, err, outputs, produced)
        a_code, a_out, a_err, a_outputs, a_ok = results["ants"]
        l_code, l_out, l_err, l_outputs, l_ok = results["larmorx"]
        summary = (
            f"ANTs {_exit_text(a_code)}: {_last_line(a_err or a_out, tmp)}; "
            f"larmorx {_exit_text(l_code)}: {_last_line(l_err or l_out, tmp)}"
        )
        if c.expect == "divergence":
            ok = a_ok and not l_ok
            checks("ANTs succeeds and larmorx refuses", ok, detail=summary)
            if ok:
                raise Outcome(EXPECTED, c.reason)
            return
        if not a_ok and not l_ok:
            raise Outcome(BOTH_ERROR, summary)
        checks("both succeed", a_ok and l_ok, detail=summary)
        if c.expect == "error":
            checks("both reject the arguments", False, detail="expected an error from both")
        if not (a_ok and l_ok):
            return
        if c.stdout:
            same = a_out.strip() == l_out.strip()
            checks(
                "stdout",
                same,
                detail="" if same else f"ANTs {a_out.strip()!r}, larmorx {l_out.strip()!r}",
            )
        for i, (pa, pl) in enumerate(zip(a_outputs, l_outputs, strict=True)):
            label = "" if len(a_outputs) == 1 else f"output {i + 1}"
            compare_outputs(checks, pa, pl, label, c.tolerance_ulps)


# ------------------------------------------------------------------------------------------------
# Reporting


def _identical(r: CaseResult) -> bool:
    return any(ch.name.endswith("bit-identical") for ch in r.checks) and all(
        ch.value == 0 for ch in r.checks if ch.name.endswith("bit-identical")
    )


def _headers_identical(r: CaseResult) -> bool:
    return all(ch.value == "True" for ch in r.checks if ch.name.endswith("header bytes identical"))


def highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == PASS]
    with_data = [r for r in compared if any(ch.name.endswith("bit-identical") for ch in r.checks)]
    identical = [r for r in with_data if _identical(r)]
    lines = [
        f"**Bit-identical: {len(identical)} of {len(with_data)} passing cases** produce exactly "
        "the values ANTs writes; "
        f"{sum(map(_headers_identical, with_data))} of {len(with_data)} also write exactly its "
        "header bytes.",
    ]
    rest = [r for r in with_data if not _identical(r)]
    if rest:
        lines.append(
            "Not bit-identical (each within its declared tolerance; the cause is in the case's "
            "reason): " + ", ".join(f"`{r.case}`" for r in rest) + "."
        )

    def worst(rs: list[CaseResult]) -> str:
        values = [
            int(ch.value)
            for r in rs
            for ch in r.checks
            if ch.name.endswith("bit-identical") and isinstance(ch.value, int)
        ]
        return str(max(values)) if values else "–"

    rows = []
    for category in sorted({r.category for r in results}):
        rs = [r for r in results if r.category == category]
        passing = [r for r in rs if r.status == PASS]
        rows.append(
            (
                category,
                len(rs),
                sum(r.status == PASS for r in rs),
                sum(r.status == BOTH_ERROR for r in rs),
                sum(r.status == EXPECTED for r in rs),
                sum(map(_identical, passing)),
                worst(passing),
            )
        )
    lines += [
        "",
        table(
            (
                "Category",
                "Cases",
                "Pass",
                "Both error",
                "Expected divergence",
                "Bit-identical",
                "Most values differing",
            ),
            rows,
        ),
    ]
    return lines


def make_suite(
    name: str,
    title: str,
    tool: str,
    all_cases: Callable[[], Sequence[AntsCase]],
    notes: str = "",
) -> Suite:
    """A parity suite over ``all_cases()``."""
    return Suite(
        name=name,
        title=title,
        tool=tool,
        reference=f"{title} from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`",
        thresholds=[
            ("Exit status", "both produce their outputs, or neither does"),
            ("Header", "every field of the 348-byte header identical"),
            ("Data type and shape", "identical"),
            ("Affine (as nibabel reads it)", f"max |diff| ≤ {AFFINE_ATOL:g} mm"),
            (
                "Values",
                "bit-identical, unless the case declares a tolerance in ulps with its cause",
            ),
            ("stdout (where compared)", "identical text"),
        ],
        cases=lambda tier: cases_for(tier, all_cases()),
        run_case=run_case,
        packages=("nibabel",),
        highlights=highlights,
        notes=(
            "Both programs get the same arguments and files; ANTs runs as its own binary and "
            "larmorx in-process through its Python console entry point, both with "
            f"`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS={THREADS}`. Outputs are read with nibabel "
            "(values as stored, before `scl_slope`) and the headers from their raw bytes. "
            "Inputs named `gen:<name>` are built by `larmorx_validation.parity.ants_inputs`. "
            + notes
        ).strip(),
    )


__all__ = [
    "THREADS",
    "AntsCase",
    "ants_bin",
    "ants_version",
    "cases_for",
    "compare_outputs",
    "header_differences",
    "highlights",
    "make_suite",
    "raw_header",
    "run_case",
    "ulps",
]
