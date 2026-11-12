# SPDX-License-Identifier: Apache-2.0
"""The parity suites of the confounds' part: the bit-comparison scripts of
``validation/investigations/confounds`` (each one validated in the kept library environments
when it was written, NOTES of the part, ``docs/findings/confounds-*.md``) run as cases, so that
their results become records and the parity tiers run them.

Three suites:

- ``confounds-linalg``: the numerics under the confounds: numpy's OpenBLAS (``dgemm``,
  ``dsyrk``, ``dgemv``, ``dtrmm``, the level-2 and level-1 routines, numpy's ``@``), LAPACK's
  QR, LQ, bidiagonal reduction, divide-and-conquer SVD, ``DGESDD`` (numpy's ``svd`` and
  ``pinv``), ``DGELSD`` (numpy's ``lstsq``), SciPy's ``DGESVD`` fallback with nipype's
  ``fallback_svd`` and ``_full_rank``, numpy's reductions and order statistics at the
  confounds' call sites, SciPy's float32 FFTs (``larmorx_transform::numpy_linalg``,
  ``openblas``, ``larmorx_core::numpy``, ``larmorx_confounds::scipy_fft``);
- ``confounds-nipype``: nipype's ``algorithms/confounds.py`` (``compute_dvars`` with nitime's
  AR(1), ``ComputeDVARS``, CompCor and its interfaces, ``TSNR``, ``NonSteadyStateDetector``,
  ``FramewiseDisplacement`` with its ``NIPY`` source) and niworkflows' ``Robust*CompCor``
  (``lx.confounds``), against nipype 1.12.0 in-process and fMRIPrep's 180 real nodes;
- ``confounds-fmriprep``: fMRIPrep's and niworkflows' confound steps (pandas' text round trips,
  the table steps, ``ExpandModel``, ``SpikeRegressors``, the motion parameters and RMSD, the
  aCompCor masks and the tissue signals) and the workflow ``init_bold_confs_wf``
  (``larmorx.pipelines.larmorprepx.confounds``), against fMRIPrep 25.2.7 (25.2.6's workflow is
  the same file), niworkflows and pandas in-process, the 36 confounds workflows of the oracle
  runs node by node and fMRIPrep's 36 confounds files column by column.

**The environments** (each case names its own; the harness's interpreter does not matter):
``e1`` the newest releases (``<workspace>/oracles/python-sdcflows-2.17``: numpy 2.5.4 with
OpenBLAS 0.3.34 and LAPACK 3.12.1, SciPy 1.18.1, pandas 3.0.6, nitime 0.12.1, niworkflows
1.15.4) with ``OPENBLAS_L2_SIZE=512`` (OpenBLAS's Zen 4/5 GEMM_Q of 512 switched off, so numpy
blocks its products by 256 as on Haswell: ``docs/findings/confounds-blas.md``); ``e1-zen`` the
same at this machine's GEMM_Q 512 (the linear algebra's cases whose products are then blocked
otherwise: every defined case identical, the crashes in the undefined class only, NP-09);
``e2`` the shared environment (``<workspace>/.venv``: numpy 2.3.5 with OpenBLAS 0.3.30 and
LAPACK 3.12.0, SciPy 1.15.3, pandas 3.0.6, niworkflows 1.15.3); ``e3`` fMRIPrep 25.2.6's own
image (``<workspace>/oracles/images/fmriprep-25.2.6.sif`` through Apptainer: numpy 2.2.6,
SciPy 1.15.2, pandas 2.2.3, nitime 0.11, niworkflows 1.15.2). Every run: numpy's OpenBLAS on
its Haswell kernels at one thread, this larmorx and this validation package on
``PYTHONPATH`` (fMRIPrep 25.2.7 from ``<workspace>/tags/fmriprep-25.2.7`` where the script
imports it), ``LARMORX_WORKSPACE``; some runs with numpy's AVX-512 kernels off
(``NPY_DISABLE_CPU_FEATURES``: nipy's and niworkflows' powers, whose AVX-512 ``pow`` differs
from glibc's), as the scripts were validated.

**Each case** runs one script, or some of its sections, as a separate process (a memory box of
its own when the parity run is not in one). Every summary line is one check: the linear
algebra's ``<name>: <K> / <N> identical; <C> cases crashed ...`` (``K == N``) and ``crashes:
<X>, of which <Y> in the undefined class`` (``X == Y``), the tables' ``<name> identical <K>
[nan-bits <B>] differ <D>`` (``D == 0``; a NaN whose payload differs is counted apart: x86-64's
default NaN), the reductions' ``<name> <K> of <N> identical``; ``INFO``, ``apart``, ``note``
and timing lines are reported values; the script's final line (``ALL IDENTICAL``, ``CHECK
PASS``, ``TOTAL ... differ 0``) and its exit status are two more checks, and a run that
compared nothing fails. **Negative controls** (``-control`` cases: larmorx told the other
library release) pass when the script tells the releases apart: some comparisons differ.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from larmorx_validation.parity import python_versions as pv
from larmorx_validation.parity.harness import (
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
    in_tier,
)
from larmorx_validation.parity.sdcflows_scripts import BOXED, _box, _paths, _reported

#: Where the scripts are (the repository's ``validation/investigations/confounds``).
SCRIPTS = Path(__file__).resolve().parents[3] / "investigations" / "confounds"
#: The environments: name -> (interpreter under the workspace, or ``None`` for the image;
#: what it holds).
ENVIRONMENTS: dict[str, tuple[str | None, str]] = {
    "e1": (
        "oracles/python-sdcflows-2.17/bin/python",
        "numpy 2.5.4 / OpenBLAS 0.3.34 (OPENBLAS_L2_SIZE=512), SciPy 1.18.1, pandas 3.0.6",
    ),
    "e1-zen": (
        "oracles/python-sdcflows-2.17/bin/python",
        "numpy 2.5.4 / OpenBLAS 0.3.34 at this machine's GEMM_Q 512",
    ),
    "e2": (".venv/bin/python", "numpy 2.3.5 / OpenBLAS 0.3.30, SciPy 1.15.3, pandas 3.0.6"),
    "e3": (None, "fMRIPrep 25.2.6's image: numpy 2.2.6, SciPy 1.15.2, pandas 2.2.3"),
}
#: fMRIPrep 25.2.6's image (``e3``).
IMAGE = "oracles/images/fmriprep-25.2.6.sif"
#: fMRIPrep 25.2.7's sources (``PYTHONPATH`` for the scripts that import it outside the image).
TAG = "tags/fmriprep-25.2.7"
#: numpy's AVX-512 kernels off (``NPY_DISABLE_CPU_FEATURES``).
AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"


@dataclass(frozen=True)
class Run:
    """One case: a script, its arguments and environment."""

    script: str
    args: tuple[str, ...]
    env: str
    tier: str
    description: str
    #: The memory box in GiB (the size the script's full runs were validated in).
    gib: int = 12
    #: Whether fMRIPrep 25.2.7's tag goes on ``PYTHONPATH``.
    tag: bool = False
    #: Whether numpy's AVX-512 kernels are off.
    avx512_off: bool = False
    #: Whether the case is a negative control (larmorx told the other release).
    control: bool = False
    timeout: int = 3600


#: The linear algebra's environments (its scripts call numpy's own exported routines; the
#: image's numpy is reached through the real calls of the other scripts).
LINALG_ENVS = ("e2", "e1")
#: The other scripts' environments.
LIBRARY_ENVS = ("e1", "e2", "e3")


@dataclass(frozen=True)
class Script:
    """One check script and the runs it was validated with."""

    name: str
    script: str
    what: str
    linalg: bool = False
    envs: tuple[str, ...] = LIBRARY_ENVS
    gib: int = 12
    tag_envs: tuple[str, ...] = ()
    avx512_off: bool = False
    control: tuple[str, ...] | None = None
    control_envs: tuple[str, ...] = ()
    full_timeout: int = 3600
    #: Extra runs: (id suffix, arguments, environments, AVX-512 off, tier).
    extra: tuple[tuple[str, tuple[str, ...], tuple[str, ...], bool, str], ...] = ()
    #: Whether the quick tier in ``e2`` is in the smoke tier.
    smoke: bool = False


LINALG = (
    Script(
        "blas",
        "check_blas.py",
        "OpenBLAS's dgemm, dsyrk, dgemv, dtrmm, dger, dtrmv, the level-1 routines and numpy's @ "
        "and dot against numpy's library",
        linalg=True,
        smoke=True,
    ),
    Script(
        "lapack",
        "check_lapack.py",
        "LAPACK's blocked QR and LQ (DGEQRF, DGELQF, DLARFT, DLARFB), DORGQR/DORGLQ/DORMQR/DORMLQ, "
        "DORMBR/DORGBR and the bidiagonal reduction (DGEBRD) against numpy's library",
        linalg=True,
    ),
    Script(
        "svd",
        "check_svd.py",
        "the divide-and-conquer bidiagonal SVD (DBDSDC, DLASD0 to DLASD5, DLASDQ, DLAED6) "
        "against numpy's library",
        linalg=True,
        control=("--control",),
        control_envs=LINALG_ENVS,
    ),
    Script(
        "gesdd",
        "check_gesdd.py",
        "DGESDD's four thin paths, numpy's svd(full_matrices=False) and pinv up to CompCor's "
        "largest real shapes",
        linalg=True,
        control=("--control",),
        control_envs=LINALG_ENVS,
    ),
    Script(
        "gelsd",
        "check_gelsd.py",
        "DGELSD's three paths, DLALSD's two (DLASDA, DLALS0, DLALSA), DLASRT and numpy's lstsq, "
        "the cosine filter's real calls included",
        linalg=True,
        control=("--control",),
        control_envs=LINALG_ENVS,
    ),
    Script(
        "gesvd",
        "check_gesvd.py",
        "SciPy's DGESVD fallback (DBDSQR, OpenBLAS 0.3.28's DLARFT) and nipype's fallback_svd "
        "and _full_rank against SciPy's own library",
        linalg=True,
        control=("--control",),
        control_envs=LINALG_ENVS,
    ),
    Script(
        "reductions",
        "check_numpy_reductions.py",
        "numpy's ordered sums, means, variances, cumsum, extremes, percentiles, searchsorted, "
        "nan_to_num, diff and the Legendre basis at the confounds' call sites",
        control=("--control",),
        control_envs=LIBRARY_ENVS,
    ),
    Script(
        "fft",
        "check_fft.py",
        "SciPy's float32 FFTs (1.15's pocketfft, 1.18.1's ducc0) and nitime's fftconvolve under "
        "nipype's DVARS",
        control=("--control",),
        control_envs=LIBRARY_ENVS,
    ),
)
NIPYPE = (
    Script(
        "dvars",
        "check_dvars.py",
        "numpy's complex64 product, nitime's crosscov and AR(1), SciPy's 1 x 1 solve, nipype's "
        "compute_dvars and ComputeDVARS, fMRIPrep's 36 real dvars nodes",
        gib=20,
        control=("--control", "--sections", "cmul,crosscov,solve,ar,dvars,node"),
        control_envs=LIBRARY_ENVS,
        smoke=True,
    ),
    Script(
        "compcor",
        "check_compcor.py",
        "nipype's cosine and Legendre filters, compute_noise_components, CompCor, ACompCor and "
        "TCompCor over their options, fMRIPrep's 108 real CompCor nodes",
        gib=24,
        control=("--control",),
        control_envs=("e1", "e3"),
    ),
    Script(
        "misc",
        "check_nipype_misc.py",
        "nipype's TSNR, NonSteadyStateDetector, FramewiseDisplacement (every source, NIPY "
        "included) and niworkflows' RobustACompCor/RobustTCompCor (PIPE-48)",
        gib=24,
        control=("--control", "--sections", "tsnr,nssd,fd,nipy"),
        control_envs=("e1", "e3"),
        extra=(("nipy-avx512-off", ("--sections", "nipy"), ("e1", "e3"), True, "full"),),
    ),
)
FMRIPREP = (
    Script(
        "tables",
        "check_tables.py",
        "pandas' text round trips (precise_xstrtod of pandas 3.0.6 and 2.2.3, ujson), "
        "fMRIPrep's FramewiseDisplacement, FilterDropped, RenameACompCor, GatherConfounds, "
        "niworkflows' AddTSVHeader, DictMerge, TSV2JSON, fMRIPrep's real nodes",
        gib=16,
        tag_envs=("e1", "e2"),
        smoke=True,
    ),
    Script(
        "model",
        "check_model.py",
        "niworkflows' ExpandModel, parse_formula, SpikeRegressors, NormalizeMotionParams, "
        "fMRIPrep's real model_expansion and spike_regressors nodes",
        avx512_off=True,
        extra=(("power-avx512-on", ("--sections", "power"), ("e1", "e3"), False, "full"),),
    ),
    Script(
        "motion",
        "check_motion.py",
        "SciPy's Rotation, nitransforms' ITK and FSL linear transforms, fMRIPrep's "
        "FSLMotionParams (25.2.7 and 25.2.6) and FSLRMSDeviation, fMRIPrep's real nodes",
        tag_envs=("e1",),
    ),
    Script(
        "masks",
        "check_masks.py",
        "fMRIPrep's _binary_union, aCompCorMasks, niworkflows' BinaryDilation, "
        "BinarySubtraction and SignalExtraction, fMRIPrep's real mask and signals nodes",
        gib=20,
        tag_envs=("e1",),
    ),
    Script(
        "workflow",
        "check_workflow.py",
        "init_bold_confs_wf built by nipype for eight option sets against larmorx's steps, and "
        "the 36 confounds workflows end to end: every node file, fMRIPrep's 36 confounds TSV "
        "files column by column and their JSON byte for byte",
        envs=("e2", "e1"),
        gib=24,
        tag_envs=("e1", "e2"),
        full_timeout=7200,
    ),
)
FAMILIES = {"confounds-linalg": LINALG, "confounds-nipype": NIPYPE, "confounds-fmriprep": FMRIPREP}


def _tier_args(s: Script, full: bool) -> tuple[str, ...]:
    if s.linalg:
        return ("--threads", "4") if full else ("--quick", "--threads", "4")
    return ("--tier", "full" if full else "quick")


def _runs(s: Script) -> dict[str, Run]:
    out: dict[str, Run] = {}
    envs = list(LINALG_ENVS if s.linalg else s.envs)
    if s.linalg:
        envs = [*envs, "e1-zen"]
    for env in envs:
        tag, off = env in s.tag_envs, s.avx512_off
        for full in (False, True):
            if env == "e1-zen" and not full:
                continue  # the machine's GEMM_Q 512 at the full tier only
            tier = "full" if full else ("smoke" if s.smoke and env == "e2" else "standard")
            out[f"{s.name}-{'full' if full else 'quick'}-{env}"] = Run(
                s.script,
                _tier_args(s, full),
                env,
                tier,
                f"{s.what} ({'full' if full else 'quick'} tier, {ENVIRONMENTS[env][1]})",
                gib=s.gib,
                tag=tag,
                avx512_off=off,
                timeout=s.full_timeout if full else 3600,
            )
    if s.control is not None:
        for env in s.control_envs:
            for full in (False, True):
                args = (*s.control, *_tier_args(s, full))
                out[f"{s.name}-control-{'full' if full else 'quick'}-{env}"] = Run(
                    s.script,
                    args,
                    env,
                    "full" if full else "standard",
                    f"the negative control of {s.script}: larmorx told the other library "
                    f"release must differ ({'full' if full else 'quick'} tier, "
                    f"{ENVIRONMENTS[env][1]})",
                    gib=s.gib,
                    tag=env in s.tag_envs,
                    avx512_off=s.avx512_off,
                    control=True,
                    timeout=s.full_timeout if full else 3600,
                )
    for suffix, args, envs_x, off, tier in s.extra:
        for env in envs_x:
            out[f"{s.name}-{suffix}-{env}"] = Run(
                s.script,
                (*args, *_tier_args(s, True)),
                env,
                tier,
                f"{s.what}: sections {' '.join(args[1:])} with numpy's AVX-512 kernels "
                f"{'off' if off else 'on'} ({ENVIRONMENTS[env][1]})",
                gib=s.gib,
                tag=env in s.tag_envs,
                avx512_off=off,
            )
    return out


#: suite -> case id -> what the case runs.
SUITES: dict[str, dict[str, Run]] = {
    suite: {cid: run for s in scripts for cid, run in _runs(s).items()}
    for suite, scripts in FAMILIES.items()
}

ABOUT = {
    "confounds-linalg": (
        "numpy's, OpenBLAS's, LAPACK's and SciPy's arithmetic under fMRIPrep's confounds",
        "`larmorx_transform::numpy_linalg`, `larmorx_transform::openblas`, "
        "`larmorx_core::numpy`, `larmorx_confounds::scipy_fft` (through `larmorx._core`)",
        "numpy's and SciPy's own OpenBLAS and LAPACK through their exported routines, numpy, "
        "SciPy and nitime in-process, in numpy 2.5.4 / OpenBLAS 0.3.34 / LAPACK 3.12.1 (and at "
        "this machine's GEMM_Q 512), numpy 2.3.5 / OpenBLAS 0.3.30 / LAPACK 3.12.0 and fMRIPrep "
        "25.2.6's image",
    ),
    "confounds-nipype": (
        "nipype's confound algorithms (DVARS, CompCor, TSNR, the non-steady-state detector, "
        "framewise displacement)",
        "`lx.confounds` (`crates/larmorx-confounds`), niworkflows' `robust_acompcor` and "
        "`robust_tcompcor`",
        "nipype 1.12.0 and nitime in-process in the newest and the shared library releases and "
        "in fMRIPrep 25.2.6's image, and fMRIPrep 25.2.6's own node files of 21 oracle runs",
    ),
    "confounds-fmriprep": (
        "fMRIPrep's and niworkflows' confound steps and fMRIPrep's confounds workflow",
        "`lx.confounds`, `lx.niworkflows`, `larmorx.pipelines.larmorprepx.confounds`",
        "fMRIPrep 25.2.7 (its confounds workflow is 25.2.6's file), niworkflows and pandas "
        "in-process in the newest and the shared releases and in fMRIPrep 25.2.6's image, and "
        "fMRIPrep 25.2.6's own node files and confounds files of 21 oracle runs",
    ),
}

_LA = re.compile(
    r"^(?P<name>.+?): (?P<k>\d+) / (?P<n>\d+) identical"
    r"(?:; (?P<c>\d+) cases crashed in numpy's OpenBLAS)?(?P<rest>.*)$"
)
_CRASHES = re.compile(
    r"^crashes: (?P<x>\d+), of which (?P<y>\d+) in the undefined class(?P<rest>.*)$"
)
_CONTROL_LA = re.compile(r"^control (?P<name>.+?): (?P<d>\d+) / (?P<n>\d+) differ(?P<rest>.*)$")
_TABLE = re.compile(
    r"^(?P<name>.+?)\s+identical\s+(?P<k>\d+)(?:\s+nan-bits\s+(?P<b>\d+))?\s+differ\s+(?P<d>\d+)\s*$"
)
_OF = re.compile(r"^(?P<name>\S+)\s+(?P<k>\d+) of\s+(?P<n>\d+) (?P<word>identical|differ)\b.*$")
_CONTROL_KIND = re.compile(r"^control: (?P<name>.+) (?P<d>\d+) cases differ$")
_CHECK_FINAL = re.compile(
    r"^(?P<who>CHECK|CONTROL) (?P<verdict>PASS|DIFFER): (?P<d>\d+) cases differ$"
)
_TOTAL_TABLE = re.compile(
    r"^TOTAL identical (?P<k>\d+) nan-bits (?P<b>\d+) differ (?P<d>\d+)\b.*?(?P<control> CONTROL)?$"
)
_TOTAL_OF = re.compile(r"^TOTAL (?P<k>\d+) of (?P<n>\d+) identical\b.*?(?P<control>\[control:.*)?$")
_INFO = ("INFO ", "apart:", "note:", "numpy ")
#: The scripts' timing lines: not part of a record (they change from run to run; the speed is
#: the benchmarks').
_TIMING = ("seconds:", "larmorx's seconds", "time ", "TIME ")


class _Parsed:
    def __init__(self) -> None:
        self.items: list[dict] = []
        self.final: dict | None = None
        self.control_differ = 0
        self.control_lines = 0
        self._names: dict[str, int] = {}

    def add(self, name: str, **kw) -> None:
        name = name.strip()[:200]
        k = self._names.get(name, 0)
        self._names[name] = k + 1
        if k:
            name = f"{name} #{k + 1}"
        self.items.append({"name": name, **kw})

    @property
    def comparisons(self) -> int:
        return sum(1 for i in self.items if i.get("compared"))


def parse(stdout: str, control: bool = False) -> _Parsed:
    """The checks of a script's output (see the module's description)."""
    out = _Parsed()
    for raw in stdout.splitlines():
        s = raw.strip()
        if not s or s.startswith(("BEGIN ", "RES ", "On entry to")):
            continue
        if m := _CHECK_FINAL.match(s):
            out.final = {"line": s, "ok": int(m["d"]) > 0 if control else int(m["d"]) == 0}
            if control:
                out.control_differ += int(m["d"])
            continue
        if m := _TOTAL_TABLE.match(s):
            d = int(m["d"])
            out.final = {"line": s, "ok": d > 0 if control else d == 0}
            if control:
                out.control_differ += d
            continue
        if m := _TOTAL_OF.match(s):
            k, n = int(m["k"]), int(m["n"])
            out.final = {"line": s, "ok": k < n if control else k == n}
            if control:
                out.control_differ += n - k
            continue
        if s in ("ALL IDENTICAL", "DIFFERENCES FOUND"):
            out.final = {"line": s, "ok": s == "ALL IDENTICAL"}
            continue
        if s.startswith("FAIL "):
            out.add(f"FAIL {s[5:80]}", passed=False, detail=_reported(s), compared=True)
            continue
        if m := _CONTROL_LA.match(s):
            d, n = int(m["d"]), int(m["n"])
            out.control_differ += d
            out.control_lines += 1
            out.add(
                f"control {m['name']}",
                passed=True,
                threshold="reported (a negative control: larmorx told the other release)",
                value=f"{d}/{n} differ",
            )
            continue
        if m := _CONTROL_KIND.match(s):
            out.control_differ += int(m["d"])
            out.control_lines += 1
            out.add(
                f"control: {m['name']}",
                passed=True,
                threshold="reported (a negative control)",
                value=f"{m['d']} differ",
            )
            continue
        if m := _CRASHES.match(s):
            x, y = int(m["x"]), int(m["y"])
            out.add(
                "crashes of numpy's OpenBLAS in the undefined class (NP-09)",
                passed=x == y,
                value=f"{y}/{x}",
                detail=m["rest"].strip()[:300],
                compared=True,
            )
            continue
        if m := _LA.match(s):
            k, n = int(m["k"]), int(m["n"])
            crashed = int(m["c"] or 0)
            out.add(
                m["name"],
                passed=k == n,
                value=f"{k}/{n}",
                metric="identical cases",
                detail=(
                    f"{crashed} cases crashed in numpy's OpenBLAS (the undefined class)"
                    if crashed
                    else ""
                )
                + m["rest"].strip()[:200],
                compared=True,
            )
            continue
        if m := _TABLE.match(s):
            k, d = int(m["k"]), int(m["d"])
            b = int(m["b"] or 0)
            if control:
                out.control_differ += d
                out.control_lines += 1
                out.add(
                    f"control {m['name']}",
                    passed=True,
                    threshold="reported (a negative control: larmorx told the other release)",
                    value=f"{d}/{k + b + d} differ",
                )
            else:
                out.add(
                    m["name"],
                    passed=d == 0,
                    value=f"{k + b}/{k + b + d}",
                    metric="identical cases",
                    detail=f"{b} with another NaN payload (x86-64's default NaN)" if b else "",
                    compared=True,
                )
            continue
        if m := _OF.match(s):
            k, n = int(m["k"]), int(m["n"])
            if control:
                out.control_differ += k if m["word"] == "differ" else n - k
                out.control_lines += 1
                out.add(
                    f"control {m['name']}",
                    passed=True,
                    threshold="reported (a negative control)",
                    value=f"{k}/{n} {m['word']}",
                )
            else:
                ok = k == n if m["word"] == "identical" else k == 0
                out.add(m["name"], passed=ok, value=f"{k}/{n}", metric=m["word"], compared=True)
            continue
        if s.startswith(_TIMING):
            continue
        if s.startswith(_INFO):
            body = s.removeprefix("INFO ")
            name, _, value = body.rpartition(": ")
            if not value:
                name, value = body[:80], body
            out.add(
                f"INFO {name[:120]}", passed=True, threshold="reported", value=_reported(value or s)
            )
            continue
    return out


def _workspace() -> Path | None:
    return pv.workspace()


def _environment(run: Run, ws: Path, folder: Path) -> tuple[dict[str, str], list[str]]:
    """The process's environment and the command's prefix (the interpreter, or Apptainer)."""
    paths = _paths()
    if run.tag:
        paths.append(str(ws / TAG))
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LARMORX_ORACLE_SHIM",))}
    for name in ("PYTHONPATH", "OPENBLAS_L2_SIZE", "NPY_DISABLE_CPU_FEATURES", "PYTHONHOME"):
        env.pop(name, None)
    settings = {
        "OPENBLAS_CORETYPE": "Haswell",
        "OPENBLAS_NUM_THREADS": "1",
        "OMP_NUM_THREADS": "1",
        "LARMORX_WORKSPACE": str(ws),
        "PYTHONUNBUFFERED": "1",
        "PYTHONPATH": os.pathsep.join(paths),
        "TMPDIR": str(folder / "tmp"),
    }
    if run.env != "e1-zen":
        settings["OPENBLAS_L2_SIZE"] = "512"
    if run.avx512_off:
        settings["NPY_DISABLE_CPU_FEATURES"] = AVX512
    interpreter, _ = ENVIRONMENTS[run.env]
    if interpreter is None:
        image = ws / IMAGE
        apptainer = shutil.which("apptainer")
        if apptainer is None or not image.is_file():
            raise Outcome(SKIPPED, f"fMRIPrep 25.2.6's image ({IMAGE}) or Apptainer is missing")
        for k, v in settings.items():
            env[f"APPTAINERENV_{k}"] = v
        env["TMPDIR"] = settings["TMPDIR"]
        return env, [apptainer, "exec", "--cleanenv", str(image), "python"]
    python = ws / interpreter
    if not python.exists():
        raise Outcome(SKIPPED, f"the environment {run.env} ({interpreter}) is missing")
    env.update(settings)
    return env, [str(python)]


def cases(suite: str, tier: str) -> list[Case]:
    return [
        Case(cid, run.script.removesuffix(".py"), run.description, payload=(suite, cid))
        for cid, run in SUITES[suite].items()
        if in_tier(run.tier, tier)
    ]


def run_case(case: Case, checks: CheckList) -> None:
    suite, cid = case.payload
    run = SUITES[suite][cid]
    ws = _workspace()
    if ws is None:
        raise Outcome(SKIPPED, "the workspace (its oracles/ folder) was not found")
    folder = Path(tempfile.mkdtemp(prefix=f"lxcf_{cid.replace('-', '_')}_"))
    (folder / "tmp").mkdir()
    keep = False
    try:
        env, prefix = _environment(run, ws, folder)
        cmd = [*prefix, "-P", str(SCRIPTS / run.script), *run.args]
        if run.gib and _box() is None and BOXED.is_file() and shutil.which("systemd-run"):
            cmd = ["bash", str(BOXED), str(run.gib), *cmd]
        try:
            done = subprocess.run(
                cmd,
                env=env,
                cwd=str(folder),
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=run.timeout,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            keep = True
            tail = (exc.stdout or "")[-600:] if isinstance(exc.stdout, str) else ""
            checks(
                "the script ends within its time limit", False, detail=f"{run.timeout} s; {tail}"
            )
            return
        parsed = parse(done.stdout, control=run.control)
        checks.add(
            Check(
                "the library environment",
                True,
                threshold="reported",
                value=run.env,
                detail=ENVIRONMENTS[run.env][1]
                + ("; numpy's AVX-512 kernels off" if run.avx512_off else ""),
            )
        )
        for item in parsed.items:
            checks(
                item["name"],
                item["passed"],
                metric=item.get("metric", "exact"),
                value=item.get("value"),
                threshold=item.get("threshold"),
                detail=item.get("detail", ""),
            )
        err_tail = " | ".join(done.stderr.strip().splitlines()[-6:])[:600]
        if run.control:
            checks(
                "the control tells the releases apart (some comparisons differ)",
                parsed.control_differ > 0,
                value=parsed.control_differ,
                detail=(parsed.final or {}).get("line", "")[:300],
            )
        else:
            final = parsed.final
            checks(
                "the script's summary",
                final is not None and final["ok"],
                detail=_reported(final["line"])
                if final
                else f"no summary; standard error: {err_tail}",
            )
            checks(
                "the script compared something",
                parsed.comparisons > 0,
                detail=f"{parsed.comparisons} comparison lines (a run that compares nothing is a "
                "misconfigured case)",
            )
        checks(
            "the script's exit status",
            done.returncode == 0,
            value=done.returncode,
            detail=("" if done.returncode == 0 else f"standard error: {err_tail}")
            + ("; 137: the memory box stopped it" if done.returncode == 137 else ""),
        )
        keep = done.returncode != 0
    finally:
        if keep:
            checks.add(
                Check(
                    "the case's folder (kept for a failure)",
                    True,
                    threshold="reported",
                    detail=str(folder),
                )
            )
        else:
            shutil.rmtree(folder, ignore_errors=True)


def _highlights(results: list[CaseResult]) -> list[str]:
    compared = sum(
        1
        for r in results
        for c in r.checks
        if c.threshold is None and not c.name.startswith(("the script", "the control"))
    )
    by_env: dict[str, int] = {}
    for r in results:
        env = r.case.rsplit("-", 1)[-1] if not r.case.endswith("-zen") else "e1-zen"
        by_env[env] = by_env.get(env, 0) + 1
    envs = ", ".join(f"{k} {v}" for k, v in sorted(by_env.items()))
    return [f"{compared} comparison lines in {len(results)} script runs ({envs})."]


def make_suite(name: str) -> Suite:
    title, tool, reference = ABOUT[name]
    return Suite(
        name=name,
        title=title,
        tool=tool,
        reference=reference,
        thresholds=[
            (
                "Every array, file (decompressed), header, table cell, warning, message and "
                "exception text",
                "bit-identical (a NaN's payload apart: x86-64's default NaN)",
            ),
            (
                "upstream_defects='reproduce'",
                "the original's result bit for bit, its errors and warnings",
            ),
            (
                "A corrected defect (the default)",
                "the correction the register names, checked against an independent "
                "construction or against reproduce where the defect does not apply",
            ),
            ("A negative control", "the other library release differs somewhere"),
        ],
        cases=lambda tier: cases(name, tier),
        run_case=run_case,
        packages=("numpy", "scipy", "pandas", "nibabel", "nipype", "niworkflows", "nitime"),
        highlights=_highlights,
        notes=(
            "Each case is one of the investigation's check scripts (some of its sections, a "
            "tier) run as a separate process in the environment its id names (e1: numpy 2.5.4, "
            "e1-zen: the same at this machine's GEMM_Q 512, e2: numpy 2.3.5, e3: fMRIPrep "
            "25.2.6's image); one check per summary line it prints, its final line and its exit "
            "status."
        ),
    )
