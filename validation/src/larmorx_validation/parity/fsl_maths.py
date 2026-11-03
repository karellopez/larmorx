# SPDX-License-Identifier: Apache-2.0
"""Parity of fslmaths' clean-room original (``larmorx fsl fslmaths``, crate larmorx-fsl,
Apache-2.0, written from ``specs/fslmaths.md``) with FSL 6.0.7.23's ``fslmaths``.

The cases are the replica suite's (``fsl_maths_replica``: fMRIPrep's combine_masks calls on the
oracle runs, the goodvoxels chain on ds005040's BOLD series, every operation once per data type
and code path, the messages and failures, the defects in both modes), run here against the
original, and the original's own cases (``orig/``: paths the replica suite's inputs do not
reach, on inputs of :func:`own_inputs`). Every FSL run goes through the oracle-output cache with
the replica suite's keys and policy, so the runs that suite made are served from it. The
operations the original does not take yet (it refuses them, naming them) are skipped.

What is compared (``specs/fslmaths.md`` §21), each output against FSL's:

- the exit status (FSL's crash or abort against the original's status 1), standard output and
  standard error line by line (the run directory replaced; the original's ``larmorx`` lines
  left out), the files written;
- each image's header, byte for byte but ``descrip``; its values bit for bit where the
  operation is exactly defined by the computing type's arithmetic (§5-§8: every case of this
  suite but the double-precision functions), and for the double-precision functions every
  voxel within the case's units in the last place of FSL's (GLIBC-07: glibc's functions are not
  correctly rounded everywhere, the original's are), the fraction that differs reported; the
  sums (``_SUM_OPS``) under the floating types by §21's sums' metric, both of its parts as
  checks (the cases of ``EXPECTED_DIVERGENCES`` may miss its share of voxels within 4 units in
  the last place only, each with its reason);
  other files byte for byte;
- the D13 fix pass: a defect named exactly when the default's outputs differ from
  ``reproduce``.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_maths_replica as rep
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    CheckList,
    Outcome,
    Suite,
)

SUITE = "fsl-maths"
PROGRAM = "fslmaths"
#: The original's refusal of what it does not take yet (the next sub-tasks of its part).
NOT_YET = "is not implemented yet in the clean-room original"
#: The original's thread count in the cases.
LX_THREADS = 4
#: The voxelwise functions that run in double precision through glibc's maths in FSL.
_DOUBLE_FUNCTIONS = ("-exp", "-log", "-sin", "-cos", "-tan", "-asin", "-acos", "-atan", "-pow")
#: The operations whose order of sums the original may choose (§21): their floating outputs are
#: compared by the sums' metric, not bit for bit (bit-identity welcome, not required).
_SUM_OPS = (
    "-s",
    "-fmean",
    "-fmeanu",
    "-dilM",
    "-dilall",
    "-edge",
    "-subsamp2",
    "-subsamp2offc",
    "-inm",
    "-ing",
    "-Tmean",
    "-Xmean",
    "-Ymean",
    "-Zmean",
    "-Tstd",
    "-Xstd",
    "-Ystd",
    "-Zstd",
    "-Tar1",
    "-Xar1",
    "-Yar1",
    "-Zar1",
    "-ranknorm",
    "-bptf",
    "-ztop",
    "-ptoz",
)
#: The sums' metric: the largest difference at most this fraction of FSL's largest magnitude...
SUM_RELATIVE = 1e-5
#: ... and at least this fraction of the voxels within ``SUM_ULPS`` units in the last place.
SUM_SHARE = 0.99
SUM_ULPS = 4
#: The name of the share's check (the one an expected divergence may miss).
SUM_SHARE_CHECK = "the sums' share of voxels within 4 ulp"
_FFT = (
    "FSL's FFT convolution against the original's direct sum: the FFT's rounding is relative to "
    "the image's largest values, not to each voxel's own last place (findings 8; spec "
    "question 14)"
)
_SUBSAMPLING = (
    "FSL's single-precision subsampling arithmetic, which no simple order of the weighted sum "
    "gives (findings 13; spec question 13), against the spec's sums: the voxels beyond 4 ulp "
    "are those whose value cancels to near 0"
)
#: Cases expected to miss §21's share of voxels within 4 ulp (only that check), and why: their
#: largest difference stays inside the bound (1e-5 of FSL's largest magnitude), checked.
EXPECTED_DIVERGENCES: dict[str, str] = {
    **dict.fromkeys(
        ("conv/fft-sphere", "conv/fft-odd", "conv/fft-double", "conv/fft-4d", "orig/fft-asym"),
        _FFT,
    ),
    **dict.fromkeys(
        (
            "sub/subsamp2",
            "sub/subsamp2offc",
            "sub/subsamp2offc-double",
            "sub/subsamp2-noform",
            "sub/subsamp2-nan",
        ),
        _SUBSAMPLING,
    ),
}


# ------------------------------------------------------------------------------------------------
# The original


def original_command() -> list[str]:
    """The original's command line: ``LARMORX_MATHS_BIN`` (a ``larmorx`` binary), else this
    Python's ``larmorx`` package."""
    exe = os.environ.get("LARMORX_MATHS_BIN")
    return [exe] if exe else [sys.executable, "-m", "larmorx"]


def run_original(
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    mode: str,
    threads: int | None = None,
) -> rep.Completed:
    env = rep._environment(settings)
    env[defects.ENV] = mode
    env["OMP_NUM_THREADS"] = str(threads or LX_THREADS)
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k, v in os.environ.items():
        if k in ("PYTHONPATH", "VIRTUAL_ENV"):
            env[k] = v
    p = subprocess.run(
        [*original_command(), "fsl", PROGRAM, *args],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        errors="replace",
        check=False,
        timeout=3600,
    )
    return rep.Completed(p.returncode, p.stdout, p.stderr)


def _refusal(stderr: str) -> str:
    return next((ln for ln in stderr.splitlines() if ln.startswith("larmorx: error:")), "")


def _not_yet(ours: rep.Completed) -> str:
    line = _refusal(ours.stderr)
    return line if ours.code == 1 and NOT_YET in line else ""


# ------------------------------------------------------------------------------------------------
# Comparison


def _own(text: str, d: Path) -> str:
    """The text without the original's own lines, its run directory replaced."""
    keep = [
        ln
        for ln in text.split("\n")
        if not ln.startswith(("larmorx: ", "larmorx fsl ")) and not ln.endswith("(larmorx)")
    ]
    return "\n".join(keep).replace(str(d), "<dir>")


def _files(d: Path, before: set[str]) -> dict[str, Path]:
    return {
        str(p.relative_to(d)): p
        for p in sorted(d.rglob("*"))
        if p.is_file() and str(p.relative_to(d)) not in before
    }


def _is_image(name: str) -> bool:
    return name.endswith((".nii", ".nii.gz", ".hdr", ".hdr.gz", ".nii.zst", ".nii.bz2"))


def _header_block(img) -> bytes:
    h = img.header.copy()
    h["descrip"] = b""
    return bytes(h.binaryblock)


def _bits(a: np.ndarray) -> np.ndarray:
    if a.dtype == np.float32:
        return a.view(np.int32).astype(np.int64)
    if a.dtype == np.float64:
        return a.view(np.int64)
    return a.astype(np.int64)


def _ulps(x: np.ndarray, y: np.ndarray) -> np.ndarray:
    """The distance in units in the last place of two float arrays of one type (NaN equal to
    NaN of the same bits; values of opposite signs far apart)."""
    xi, yi = _bits(x), _bits(y)
    same_sign = (xi < 0) == (yi < 0)
    with np.errstate(over="ignore"):
        d = np.where(same_sign, np.abs(xi - yi), np.iinfo(np.int64).max)
    return d


def _double_ulps(args: list[str]) -> int:
    """The units in the last place allowed for a case's double-precision values: one per
    double-precision function of the chain (each may differ by one, GLIBC-07)."""
    return sum(a in _DOUBLE_FUNCTIONS for a in args)


def compare(
    checks: CheckList,
    oracle: rep.Completed,
    ours: rep.Completed,
    a: Path,
    b: Path,
    before: set[str],
    args: list[str],
) -> None:
    """Compares the original's run with FSL's (§21's metrics as checks)."""
    import nibabel as nib

    ok_exit = oracle.code == ours.code or (
        oracle.code != 0 and ours.code != 0 and (oracle.code < 0 or oracle.code > 128)
    )
    checks(
        "exit status",
        ok_exit,
        metric="exact",
        value=f"{ours.code} (FSL {oracle.code})",
        threshold="the same (FSL's crash: 1)",
    )
    so, su = _own(oracle.stdout, a), _own(ours.stdout, b)
    checks(
        "standard output",
        so == su,
        detail="" if so == su else _first_diff(so, su),
    )
    eo, eu = _own(oracle.stderr, a), _own(ours.stderr, b)
    checks(
        "standard error",
        eo == eu,
        detail="" if eo == eu else _first_diff(eo, eu),
    )
    fa, fb = _files(a, before), _files(b, before)
    checks(
        "files written",
        set(fa) == set(fb),
        detail="" if set(fa) == set(fb) else f"FSL {sorted(fa)}, original {sorted(fb)}",
    )
    ulps = _double_ulps(args)
    for name in sorted(set(fa) & set(fb)):
        if name.endswith(".img") or name.endswith(".img.gz"):
            continue
        if not _is_image(name):
            same = fa[name].read_bytes() == fb[name].read_bytes()
            checks(f"{name}: bytes", same)
            continue
        ia, ib = nib.load(str(fa[name])), nib.load(str(fb[name]))
        ha, hb = _header_block(ia), _header_block(ib)
        checks(
            f"{name}: header (descrip left out)",
            ha == hb,
            detail="" if ha == hb else _header_diff(ia, ib),
        )
        x = np.asarray(ia.dataobj)
        y = np.asarray(ib.dataobj)
        if x.dtype != y.dtype or x.shape != y.shape:
            checks(f"{name}: values", False, detail=f"{x.dtype}{x.shape} vs {y.dtype}{y.shape}")
            continue
        if x.dtype.kind == "f":
            d = _ulps(x.ravel(), y.ravel())
            differ = int(np.count_nonzero(d))
            worst = int(d.max()) if d.size else 0
            if differ and any(a in _SUM_OPS for a in args):
                _sum_check(checks, name, x.ravel(), y.ravel(), d)
            elif ulps and x.dtype == np.float64:
                checks(
                    f"{name}: values (units in the last place)",
                    worst <= ulps,
                    metric="max ulp",
                    value=worst,
                    threshold=ulps,
                    detail=f"{differ} of {d.size} voxels differ (GLIBC-07)",
                )
            else:
                checks(
                    f"{name}: values bit for bit",
                    differ == 0,
                    metric="differing voxels",
                    value=differ,
                    threshold=0,
                    detail="" if differ == 0 else _value_diff(x, y),
                )
        else:
            differ = int(np.count_nonzero(x != y))
            checks(
                f"{name}: values bit for bit",
                differ == 0,
                metric="differing voxels",
                value=differ,
                threshold=0,
                detail="" if differ == 0 else _value_diff(x, y),
            )


def _sum_check(checks: CheckList, name: str, x: np.ndarray, y: np.ndarray, ulp: np.ndarray) -> None:
    """§21's metric for the sums, as two checks: the non-finite voxels identical and the largest
    difference at most ``SUM_RELATIVE`` of FSL's largest magnitude; and at least ``SUM_SHARE`` of
    the voxels within ``SUM_ULPS`` units in the last place. A case of
    :data:`EXPECTED_DIVERGENCES` may miss the second only (its reason in the report)."""
    fx, fy = np.isfinite(x), np.isfinite(y)
    same_nonfinite = bool(np.array_equal(fx, fy) and np.array_equal(_bits(x[~fx]), _bits(y[~fy])))
    both = fx & fy
    scale = float(np.abs(x[both]).max()) if both.any() else 0.0
    worst = (
        float(np.abs(x[both].astype(np.float64) - y[both].astype(np.float64)).max())
        if both.any()
        else 0.0
    )
    share = float(np.count_nonzero(ulp <= SUM_ULPS)) / max(1, ulp.size)
    checks(
        f"{name}: values within the sums' bound",
        same_nonfinite and worst <= SUM_RELATIVE * scale,
        metric="max |diff| / max |FSL|",
        value=f"{worst / scale if scale else worst:.3g}",
        threshold=f"<= {SUM_RELATIVE:g}, the NaN and infinite voxels identical",
        detail=(
            f"{np.count_nonzero(ulp)} of {ulp.size} voxels differ"
            + ("" if same_nonfinite else "; the NaN or infinite voxels differ")
        ),
    )
    checks(
        f"{name}: {SUM_SHARE_CHECK}",
        share >= SUM_SHARE,
        metric=f"voxels within {SUM_ULPS} ulp",
        value=f"{share:.1%}",
        threshold=f">= {SUM_SHARE:.0%}",
        detail=f"max {int(ulp.max())} ulp",
    )


def _expected_divergence(checks: CheckList, name: str) -> None:
    """A case of :data:`EXPECTED_DIVERGENCES` whose only failures are the sums' share checks
    ends as an expected divergence, its failing checks' numbers in the reason."""
    reason = EXPECTED_DIVERGENCES.get(name)
    failed = [c for c in checks.checks if not c.passed]
    if reason is None or not failed or any(SUM_SHARE_CHECK not in c.name for c in failed):
        return
    worst = "; ".join(f"{c.name} {c.value} (threshold {c.threshold}, {c.detail})" for c in failed)
    bound = "; ".join(
        f"max |diff| {c.value} of max |FSL| (bound {SUM_RELATIVE:g})"
        for c in checks.checks
        if "values within the sums' bound" in c.name
    )
    raise Outcome(EXPECTED, f"{reason}. Inside the bound: {bound}. Outside the threshold: {worst}")


def _first_diff(x: str, y: str) -> str:
    xs, ys = x.split("\n"), y.split("\n")
    for k, (p, q) in enumerate(zip(xs, ys, strict=False)):
        if p != q:
            return f"line {k + 1}: FSL {p[:160]!r} / original {q[:160]!r}"
    return f"{len(xs)} vs {len(ys)} lines; FSL {x[-200:]!r} / original {y[-200:]!r}"


def _header_diff(ia, ib) -> str:
    out = []
    for key in ia.header:
        if key == "descrip":
            continue
        va, vb = ia.header[key], ib.header[key]
        if np.asarray(va).tobytes() != np.asarray(vb).tobytes():
            out.append(f"{key}: {va} / {vb}")
    return "; ".join(out)[:400]


def _value_diff(x: np.ndarray, y: np.ndarray) -> str:
    xr, yr = x.ravel(), y.ravel()
    if x.dtype.kind == "f":
        bad = np.flatnonzero(_bits(xr) != _bits(yr))
    else:
        bad = np.flatnonzero(xr != yr)
    k = bad[:3]
    return f"{bad.size} differ; first at {k.tolist()}: FSL {xr[k].tolist()} / original {yr[k].tolist()}"


def fix_pass(checks: CheckList, prepare, args, settings, reproduced_dir, reproduced, before):
    """D13: the original's default run against its ``reproduce`` run."""
    import nibabel as nib

    with tempfile.TemporaryDirectory(prefix="lx-maths-fix_") as name:
        tmp = Path(name)
        prepare(tmp)
        r = run_original(args, cwd=tmp, settings=settings, mode=defects.FIX)
        entries = defects.notices_in(r.stderr)
        others = []
        if r.code != reproduced.code:
            others.append(f"exit {reproduced.code} -> {r.code}")
        x, y = _files(reproduced_dir, before), _files(tmp, before)
        if set(x) != set(y):
            others.append(f"files {sorted(set(x) ^ set(y))[:4]}")
        change = defects.Change(0, 0, 0.0)
        for key in sorted(set(x) & set(y)):
            if _is_image(key):
                ix, iy = nib.load(str(x[key])), nib.load(str(y[key]))
                change = change + defects.Change.of(np.asarray(iy.dataobj), np.asarray(ix.dataobj))
                if _header_block(ix) != _header_block(iy):
                    others.append(f"{key} header")
            elif x[key].read_bytes() != y[key].read_bytes():
                others.append(key)
        defects.add_fix_check(
            checks, entries, change, other_changes="; ".join(others), silent_ok=rep._SILENT
        )


def _agree_on_errors(checks: CheckList, oracle: rep.Completed) -> None:
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        text = " ".join(oracle.stderr.split())[:160]
        raise Outcome(
            BOTH_ERROR,
            f"fslmaths exits {oracle.code}; the same from the original."
            + (f" {text}" if text else ""),
        )


# ------------------------------------------------------------------------------------------------
# The original's own cases (paths the replica suite's inputs do not reach)


def own_inputs() -> Path:
    """The original's own synthetic inputs, made once (fixed bytes): ``ramp16`` (16 x 13 x 11,
    ``x + 20 y + 400 z``), ``rampnan`` (its first voxel NaN), ``asym`` (a 13 x 11 x 9 kernel,
    ``1 + x + 3 y + 7 z``); voxel sizes 2, 2.5, 3."""
    ws = rep.workspace()
    base = (ws / "oracles" / "inputs" if ws else Path(tempfile.gettempdir())) / SUITE
    marker = base / "v1.done"
    if marker.exists():
        return base
    base.mkdir(parents=True, exist_ok=True)
    x, y, z = np.meshgrid(np.arange(16), np.arange(13), np.arange(11), indexing="ij")
    ramp = (x + 20 * y + 400 * z).astype(np.float32)
    rep._save(base / "ramp16.nii.gz", ramp, dtype=np.float32, affine=rep._affine(ramp.shape))
    nan = ramp.copy()
    nan[0, 0, 0] = np.nan
    rep._save(base / "rampnan.nii.gz", nan, dtype=np.float32, affine=rep._affine(nan.shape))
    x, y, z = np.meshgrid(np.arange(13), np.arange(11), np.arange(9), indexing="ij")
    asym = (1 + x + 3 * y + 7 * z).astype(np.float32)
    rep._save(base / "asym.nii.gz", asym, dtype=np.float32, affine=rep._affine(asym.shape))
    marker.write_text("1\n", encoding="utf-8")
    return base


def _prepare(run: rep.Run, tmp: Path) -> list[str]:
    """The run's arguments in ``tmp``: ``@orig:<name>`` one of :func:`own_inputs`, the rest as
    the replica suite prepares them."""
    if not any(a.startswith("@orig:") for a in run.args):
        return rep._prepare(run, tmp)
    out: list[str] = []
    for a in run.args:
        if a.startswith("@orig:"):
            src = own_inputs() / f"{a[len('@orig:') :]}.nii.gz"
            shutil.copy(src, tmp / src.name)
            out.append(src.name)
        else:
            out.extend(rep._prepare(rep._r(a), tmp))
    return out


def _own_cases() -> dict[str, tuple[str, str, rep.Run]]:
    T = "standard"
    return {
        "orig/fft-asym": (
            T,
            "-kernel file (asymmetric, 13 x 11 x 9) -fmeanu of a 16 x 13 x 11 image (FSL's FFT "
            "path: a true convolution, the kernel flipped)",
            rep._r("@orig:ramp16", "-kernel", "file", "@orig:asym", "-fmeanu", "out"),
        ),
        "orig/fft-nan-local": (
            T,
            "FSL-191: -kernel boxv 11 -fmeanu (FSL's FFT path) of an image with one NaN voxel "
            "beyond the kernel's reach of most voxels",
            rep._r("@orig:rampnan", "-kernel", "boxv", "11", "-fmeanu", "out", fixed="FSL-191"),
        ),
    }


# ------------------------------------------------------------------------------------------------
# Cases


def _run_case(run: rep.Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    if run.undefined:
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-maths-") as b_:
                b = Path(b_)
                args = _prepare(run, b)
                r = run_original(args, cwd=b, settings=settings, mode=mode)
                if _not_yet(r):
                    raise Outcome(SKIPPED, _not_yet(r))
                checks(
                    f"{run.undefined}: the original refuses ({mode})",
                    r.code == 1 and run.undefined in r.stderr,
                    detail=f"exit {r.code}: {r.stderr[-200:]}",
                )
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-maths-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-maths-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        before = set(_files(b, set()))
        ours = run_original(args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if _not_yet(ours):
            raise Outcome(SKIPPED, _not_yet(ours))
        oracle = rep.run_oracle(args, cwd=a, settings=settings)
        compare(checks, oracle, ours, a, b, before, args)
        if run.fix and ours.code == 0:
            fix_pass(checks, lambda t: _prepare(run, t), args, settings, b, ours, before)
    _agree_on_errors(checks, oracle)
    if run.fixed:
        with tempfile.TemporaryDirectory(prefix="lx-maths-") as c_:
            c = Path(c_)
            args = _prepare(run, c)
            r = run_original(args, cwd=c, settings=settings, mode=defects.FIX)
            checks(
                f"{run.fixed}: the default gives an output and names the defect",
                r.code == 0 and run.fixed in r.stderr and any(c.glob("out*")),
                detail=f"exit {r.code}: {r.stderr[-200:]}",
            )


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    args = rep.fmriprep_args(run, Path(node))
    missing = [x for x in args if x.startswith("/") and not Path(x).exists()]
    if missing:
        raise Outcome(SKIPPED, f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-maths-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-maths-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = rep.run_oracle(args, cwd=a, settings={})
        ours = run_original(args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, oracle, ours, a, b, set(), args)
        if oracle.code == 0 and ours.code == 0:
            fix_pass(checks, lambda t: None, args, {}, b, ours, set())
    _agree_on_errors(checks, oracle)


def _goodvoxels_case(payload: tuple[str, str], checks: CheckList) -> None:
    import json

    run, node = payload
    base = rep._goodvoxels_chain(run)
    if base is None:
        raise Outcome(SKIPPED, f"the oracle run {run} is not available")
    values = json.loads((base / "values.json").read_text(encoding="utf-8"))
    files = {"bold": rep._goodvoxels_sources(run)[0], "ribbon": base / "ribbon.nii.gz"}
    for n, prog, _ in rep.GOODVOXELS_STEPS:
        if prog == "fslmaths":
            files[n] = base / n / f"{n}.nii.gz"
    template = next(t for n, _, t in rep.GOODVOXELS_STEPS if n == node)
    with (
        tempfile.TemporaryDirectory(prefix="lx-maths-gv-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-maths-gv-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = [*rep._goodvoxels_args(template, files, values, a), f"{node}.nii.gz"]
        rep._goodvoxels_args(template, files, values, b)
        before = set(_files(b, set()))
        ours = run_original(args, cwd=b, settings={}, mode=defects.REPRODUCE)
        if _not_yet(ours):
            raise Outcome(SKIPPED, _not_yet(ours))
        oracle = rep.run_oracle(args, cwd=a, settings={})
        compare(checks, oracle, ours, a, b, before, args)
        if oracle.code == 0 and ours.code == 0:

            def prepare(t: Path) -> None:
                rep._goodvoxels_args(template, files, values, t)

            fix_pass(checks, prepare, args, {}, b, ours, before)
    _agree_on_errors(checks, oracle)


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = list(rep.cases(tier))
    for name, (t, desc, _run) in _own_cases().items():
        if in_tier(t, tier):
            out.append(Case(name, "orig", desc, ("own", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "goodvoxels":
        _goodvoxels_case(payload, checks)
    elif kind == "direct":
        _run_case(payload, checks)
    elif kind == "own":
        _run_case(_own_cases()[payload][2], checks)
    else:
        _run_case(rep._cases()[payload][2], checks)
    _expected_divergence(checks, case.id)


def suite() -> Suite:
    import larmorx

    return Suite(
        name=SUITE,
        title="fslmaths (clean-room original)",
        tool=f"`larmorx fsl fslmaths` (larmorx {larmorx.__version__}, crate larmorx-fsl: the "
        "clean-room original written from specs/fslmaths.md, Apache-2.0)",
        reference="fslmaths from FSL 6.0.7.23 (fsl-avwutils 2209.6), the conda build at "
        "`~/fsl-6.0.7.23`, as built: glibc 2.35's maths",
        thresholds=[
            ("Exit status", "the same (FSL's crash or abort: the original exits 1)"),
            (
                "Standard output and error",
                "identical (the run directory replaced; the original's `larmorx` lines left out)",
            ),
            ("Files written", "the same names"),
            ("Headers", "byte for byte (`descrip` left out)"),
            (
                "Values",
                "bit for bit; double-precision functions within one unit in the last place per "
                "function of the chain (GLIBC-07); the sums (-s, -fmean, -fmeanu, -dilM, "
                "-dilall, -edge, -subsamp2, -subsamp2offc, -inm, -ing, means, standard "
                "deviations, ar1, -ranknorm, -bptf, -ztop, -ptoz) in floating types: the largest "
                "difference at most 1e-5 of FSL's largest magnitude and 99 % of the voxels within "
                "4 units in the last place, the NaN and infinite voxels identical (§21)",
            ),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The cases are the replica suite's (`fsl-maths-replica`); FSL's runs are served from "
            "the oracle cache under that suite's keys. The original is compared in `reproduce` "
            "mode (D13) and its default `fix` pass against the reproduced run. The operations "
            "the original does not take yet are skipped until its next sub-tasks. The goodvoxels "
            "chain's ribbon is a stand-in: the segmentation's grey matter (the no-reconall runs "
            "have no FreeSurfer ribbon)."
        ),
    )


__all__ = ["suite"]
