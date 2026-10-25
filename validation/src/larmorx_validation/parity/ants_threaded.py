# SPDX-License-Identifier: Apache-2.0
"""Parity of ANTs programs whose result depends on ANTs' thread count (``N4BiasFieldCorrection``,
``DenoiseImage``): the comparison both suites share.

larmorx computes ANTs' **single-thread** result on any number of threads, so the references
run ANTs with ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1``; larmorx runs with
:data:`LX_THREADS` threads, and once more with 1, which must give the same bits.

Two references:

- **ANTs as built** (glibc's math): the unmodified program, the headline. Bit-identity is
  reported; where glibc's functions are not correctly rounded for a value the run meets, the
  outputs differ, and the difference must keep NCC > 0.995 (PLAN.md §11.3). It is reported
  next to ANTs' own thread band: ANTs run again at several thread counts, compared with ANTs
  at one thread.
- **ANTs with correctly rounded math**: the same binary with CORE-MATH's ``expf``, ``logf``,
  ``exp``, ``log``, ``sin``, ``cos`` and ``sincos`` preloaded (``scripts/build_crmath_preload.sh``),
  which is what larmorx computes with (CLAUDE.md rule 5). larmorx must be **bit-identical** to
  it: this explains any difference from the program as built.

larmorx runs with ANTs' defects reproduced (``LARMORX_UPSTREAM_DEFECTS=reproduce``, D13), then
once more in its default mode for the fix pass (:mod:`~larmorx_validation.parity.defects`).
"""

from __future__ import annotations

import tempfile
from collections.abc import Callable, Sequence
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.ants_programs import (
    AntsCase,
    affine_difference,
    ants_bin,
    case_never,
    crmath_preload,
    fix_pass,
    header_differences,
    known_summary,
    raw_header,
    resolve_arg,
    run_ants,
    run_larmorx,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    PASS,
    SKIPPED,
    CaseResult,
    Check,
    CheckList,
    Outcome,
)
from larmorx_validation.report import table

#: larmorx's thread count (its result does not depend on it).
LX_THREADS = 4
NCC_MIN = 0.995
AFFINE_ATOL = 1e-6


def load(path: Path) -> tuple[np.ndarray, np.ndarray]:
    """The values as stored and the affine, read with nibabel."""
    import nibabel as nib

    img = nib.load(path)
    return np.asanyarray(img.dataobj.get_unscaled()), img.affine


def metrics(a: np.ndarray, b: np.ndarray) -> dict[str, float]:
    """How far ``b`` is from ``a``: values differing, largest absolute and relative
    difference, and 1 - NCC (normalized cross-correlation)."""
    af, bf = a.astype(np.float64).ravel(), b.astype(np.float64).ravel()
    d = np.abs(af - bf)
    rel = d / np.maximum(np.abs(af), 1e-30)
    # 1 - NCC as half the squared distance of the normalised, centred images (stable for nearly
    # identical images), with numpy's own pairwise sums: `x @ x` goes to the BLAS, whose threads
    # change the last bits from run to run (2026-10-24).
    x, y = af - af.mean(), bf - bf.mean()
    nx, ny = float(np.sqrt(np.sum(x * x))), float(np.sqrt(np.sum(y * y)))
    if nx > 0 and ny > 0:
        u = x / nx - y / ny
        one_minus = 0.5 * float(np.sum(u * u))
    else:
        one_minus = 0.0 if np.array_equal(af, bf) else 1.0
    return {
        "differing": int(
            np.count_nonzero(
                np.ascontiguousarray(a).view(np.uint32) != np.ascontiguousarray(b).view(np.uint32)
            )
        ),
        "max_abs": float(d.max()) if d.size else 0.0,
        "max_rel": float(rel.max()) if rel.size else 0.0,
        "one_minus_ncc": one_minus,
    }


def run(tool: Callable, c: AntsCase, tmp: Path, name: str, threads: int, **kw):
    """Runs ``c`` with ``tool`` (``run_ants`` or ``run_larmorx``) into ``tmp/name``; ANTs'
    runs through the oracle cache, never kept for the cases :func:`case_never` names."""
    if tool is run_ants:
        kw.setdefault("never", case_never(c))
    outdir = tmp / name
    outdir.mkdir()
    args = [resolve_arg(a, tmp, outdir) for a in c.args]
    outputs = [Path(resolve_arg(o, tmp, outdir)) for o in c.outputs]
    code, out, err = tool(c.program, args, threads, **kw)
    produced = code == 0 and all(o.exists() for o in outputs)
    return code, out, err, outputs, produced


def describe(m: dict[str, float], size: int) -> str:
    if m["differing"] == 0:
        return "identical"
    return (
        f"{m['differing']} of {size} differ; max |diff| {m['max_abs']:.3g}, max rel "
        f"{m['max_rel']:.2g}, 1 - NCC {m['one_minus_ncc']:.2g}"
    )


def compare(
    c: AntsCase,
    checks: CheckList,
    labels: Sequence[str],
    band_threads: Sequence[int],
    prefix: str,
) -> None:
    """Runs ``c`` with ANTs (one thread, as built and with correctly rounded math) and larmorx,
    and records the checks; ``labels`` names the outputs."""
    if ants_bin() is None:
        raise Outcome(SKIPPED, "the ANTs 2.6.5 oracle was not found (LARMORX_ANTS_BIN)")
    with tempfile.TemporaryDirectory(prefix=prefix) as tmp_name:
        tmp = Path(tmp_name)
        a_code, a_out, a_err, a_outputs, a_ok = run(run_ants, c, tmp, "ants", 1)
        l_code, l_out, l_err, l_outputs, l_ok = run(run_larmorx, c, tmp, "larmorx", LX_THREADS)
        summary = (
            f"ANTs exit {a_code}: {(a_err or a_out).strip()[-200:]!r}; "
            f"larmorx exit {l_code}: {(l_err or l_out).strip()[-200:]!r}"
        ).replace(str(tmp), "<tmp>")
        reproduced = (l_code, l_out, l_err, l_outputs, l_ok)
        if not a_ok and not l_ok:
            if fix_pass(c, tmp, checks, reproduced, LX_THREADS):
                raise Outcome(BOTH_ERROR, summary)
            return
        checks("both succeed", a_ok and l_ok, detail=summary)
        if c.expect == "error":
            checks("both reject the arguments", False, detail="expected an error from both")
        if not (a_ok and l_ok) or not a_outputs:
            return
        fix_pass(c, tmp, checks, reproduced, LX_THREADS)
        # larmorx's result does not depend on its thread count.
        *_, l1_outputs, l1_ok = run(run_larmorx, c, tmp, "larmorx-1", 1)
        same = l1_ok and all(
            load(p)[0].tobytes() == load(q)[0].tobytes()
            for p, q in zip(l_outputs, l1_outputs, strict=True)
        )
        checks(
            f"larmorx: {LX_THREADS} threads and 1 thread give the same bits",
            same,
            metric="identical",
            value=str(same),
            threshold="True",
        )
        # ANTs with correctly rounded math (CORE-MATH preloaded): the exact reference.
        cr_outputs: list[Path] | None = None
        if crmath_preload() is not None:
            *_, outs, ok = run(run_ants, c, tmp, "ants-cr", 1, correctly_rounded=True)
            checks("ANTs with correctly rounded math succeeds", ok)
            cr_outputs = outs if ok else None
        band_outputs: dict[int, list[Path]] = {}

        def band(index: int) -> dict[str, float]:
            if not band_outputs:
                for t in band_threads:
                    *_, outs, ok = run(run_ants, c, tmp, f"ants-{t}", t)
                    if ok:
                        band_outputs[t] = outs
            ref = load(a_outputs[index])[0]
            worst = {"max_abs": 0.0, "max_rel": 0.0, "one_minus_ncc": 0.0}
            for outs in band_outputs.values():
                m = metrics(ref, load(outs[index])[0])
                for k in worst:
                    worst[k] = max(worst[k], m[k])
            return worst

        for i, (pa, pl) in enumerate(zip(a_outputs, l_outputs, strict=True)):
            label = labels[i]
            ha, hb = raw_header(pa), raw_header(pl)
            differing = header_differences(ha, hb)
            checks(
                f"{label}: header (every field)",
                not differing,
                metric="fields differing",
                value=len(differing),
                threshold=0,
                detail="; ".join(differing),
            )
            a, a_aff = load(pa)
            b, b_aff = load(pl)
            checks(
                f"{label}: data type and shape",
                a.dtype == b.dtype and a.shape == b.shape,
                detail=f"ANTs {a.dtype} {a.shape}, larmorx {b.dtype} {b.shape}",
            )
            aff = affine_difference(a_aff, b_aff)
            checks(
                f"{label}: affine",
                aff <= AFFINE_ATOL,
                metric="max |diff| (mm)",
                value=f"{aff:.1e}",
                threshold=f"{AFFINE_ATOL:g}",
            )
            if a.shape != b.shape or a.dtype != b.dtype:
                continue
            if cr_outputs is not None:
                cr = load(cr_outputs[i])[0]
                mc = metrics(cr, b)
                checks(
                    f"{label}: bit-identical to ANTs with correctly rounded math",
                    mc["differing"] == 0,
                    metric="values differing",
                    value=mc["differing"],
                    threshold=0,
                    detail=describe(mc, cr.size),
                )
            m = metrics(a, b)
            checks.add(
                Check(
                    f"{label}: bit-identical to ANTs as built",
                    True,
                    metric="values differing",
                    value=m["differing"],
                    threshold="reported",
                    detail=describe(m, a.size),
                )
            )
            if m["differing"] == 0:
                continue
            checks(
                f"{label}: NCC with ANTs as built",
                1.0 - m["one_minus_ncc"] > NCC_MIN,
                metric="NCC",
                value=f"{1.0 - m['one_minus_ncc']:.10f}",
                threshold=f"> {NCC_MIN}",
            )
            w = band(i)
            for key, name in (("max_abs", "max |diff|"), ("one_minus_ncc", "1 - NCC")):
                ratio = m[key] / w[key] if w[key] > 0 else (0.0 if m[key] == 0 else float("inf"))
                checks.add(
                    Check(
                        f"{label}: {name} vs ANTs' thread band",
                        True,
                        metric=name,
                        value=f"{m[key]:.3g}",
                        threshold="reported",
                        detail=f"ANTs at {', '.join(map(str, band_threads))} threads vs 1: up "
                        f"to {w[key]:.3g} (ratio {ratio:.2g})",
                    )
                )


# ------------------------------------------------------------------------------------------------
# Report


def _values(r: CaseResult, suffix: str) -> list:
    return [ch.value for ch in r.checks if ch.name.endswith(suffix)]


def _ratio(r: CaseResult, output: str, metric: str) -> str:
    for ch in r.checks:
        if ch.name == f"{output}: {metric} vs ANTs' thread band":
            return ch.detail.split("ratio ")[-1].rstrip(")")
    return "–"


def highlights(results: list[CaseResult], functions: str, band_threads: Sequence[int]) -> list[str]:
    """The report's summary: bit-identity with the unmodified program first, then with
    correctly rounded math as the explanation; ``functions`` names the glibc functions."""
    compared = [r for r in results if r.status == PASS and _values(r, "to ANTs as built")]
    exact = [r for r in compared if all(v == 0 for v in _values(r, "correctly rounded math"))]
    built = [r for r in compared if all(v == 0 for v in _values(r, "to ANTs as built"))]
    threads = ", ".join(map(str, band_threads[:-1])) + f" and {band_threads[-1]}"
    lines = [
        f"**Bit-identical to the unmodified ANTs program (glibc's math, one thread): "
        f"{len(built)} of {len(compared)} compared cases.** The others differ where glibc's "
        f"{functions} is not correctly rounded for a value the run meets; the table compares "
        "each difference with ANTs' own difference between one thread and "
        f"{threads} threads (a ratio below 1 is inside ANTs' thread band).",
        f"**The cause, confirmed: with those glibc functions replaced by correctly rounded ones "
        f"(CORE-MATH preloaded into the same ANTs program, one thread), {len(exact)} of "
        f"{len(compared)} are bit-identical.**",
    ]
    rows = []
    for r in compared:
        for ch in r.checks:
            if ch.name.endswith("bit-identical to ANTs as built") and ch.value:
                output = ch.name.split(":")[0]
                rows.append(
                    (
                        f"`{r.case}`",
                        output,
                        ch.detail,
                        _ratio(r, output, "max |diff|"),
                        _ratio(r, output, "1 - NCC"),
                    )
                )
    lines += defects.summary(results)
    lines += known_summary(results)
    if rows:
        lines += [
            "",
            table(
                (
                    "Case",
                    "Output",
                    "Difference from ANTs as built",
                    "max |diff| / band",
                    "(1 - NCC) / band",
                ),
                rows,
            ),
        ]
    return lines


__all__ = [
    "AFFINE_ATOL",
    "LX_THREADS",
    "NCC_MIN",
    "compare",
    "describe",
    "highlights",
    "load",
    "metrics",
    "run",
]
