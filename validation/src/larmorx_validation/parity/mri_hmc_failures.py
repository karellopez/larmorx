# SPDX-License-Identifier: Apache-2.0
"""The failure cases of ``specs/mcflirt.md`` §14.1 in the ``mri-hmc`` suite: outputs mcflirt
cannot write (§10.6: FSL-20, FSL-65) and bins no histogram can have (§12.1: FSL-21, FSL-66).

Each case runs mcflirt (the FSL version the suite validates against, live, through the oracle
cache) and ``larmorx mri hmc`` in both modes on a 24 × 24 × 16 × 5 phantom (written by this module
into a temporary directory; plain arithmetic), each in a run directory of its own prepared alike
(a directory named like an output, a file where a directory is expected, a read-only directory).
The checks, as §14.1 says:

- mcflirt still behaves as the spec observed (its exit status; a signal is negative);
- ``reproduce``: the exit status (mcflirt's, or 1 where mcflirt ends by a signal), the first line
  of standard error that is mcflirt's (larmorx's own lines, ``larmorx: ...`` notices and
  ``Error: ...``, set aside), the files written (names), the defects named, and, where both run
  to the end, the matrices (the record's 95th-percentile RMS deviation; reported only where the
  cost has no bins, whose matrices are arbitrary);
- the default: the exit status, the defects named, and nothing written where it refuses (the
  same files as mcflirt's where it runs).

The runs with a directory, a file or a permission prepared are never cached (the key holds
neither directories outside the arguments' paths nor permissions); the others are.
"""

from __future__ import annotations

import functools
import re
import shutil
import stat
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import BOTH_ERROR, Case, CheckList, Outcome

#: The phantom: 24 × 24 × 16 voxels of 3 mm, 5 volumes (§14.1).
SHAPE = (24, 24, 16)
VOLUMES = 5
CATEGORY = "failure cases (spec §14.1)"


@dataclass(frozen=True)
class FailureCase:
    """A §14.1 case: mcflirt's arguments (``{in}``, ``{one}``, ``{ref}``: the series, a
    one-volume series, a reference; ``{run}``: the run directory), what each run directory holds
    first (``("dir" | "file" | "ro", name)``), and what is expected: mcflirt's exit status (a
    signal negative), larmorx's default (``refuse`` or ``run``) and the defects each mode names."""

    name: str
    args: tuple[str, ...]
    mcflirt: int
    default: str
    default_defects: tuple[str, ...]
    reproduce_defects: tuple[str, ...]
    setup: tuple[tuple[str, str], ...] = ()
    #: A stage has no bin (FSL-21 reproduced) or one: the cost is rounding noise, its matrices
    #: arbitrary, reported and not required.
    arbitrary: bool = False
    tier: str = "standard"
    description: str = ""


def _c(name, args, mcflirt, default, d_defects, r_defects, **kw) -> FailureCase:
    return FailureCase(name, tuple(args.split()), mcflirt, default, d_defects, r_defects, **kw)


S1 = "-stages 1"
F20, F21, F65, F66 = ("FSL-20",), ("FSL-21",), ("FSL-65",), ("FSL-66",)
#: §14.1's table, case by case.
CASES = (
    _c("fsl20-mats", f"-in {{in}} -out {{run}}/missing/o -mats {S1}", 0, "refuse", F20, F20,
       tier="smoke", description="-mats in a directory that does not exist (FSL-20)"),
    _c("fsl20-mats-plots", f"-in {{in}} -out {{run}}/missing/o -mats -plots {S1}", 0, "refuse",
       F20, F20, description="-mats -plots in a directory that does not exist (FSL-20)"),
    _c("fsl20-reffile", f"-in {{in}} -reffile {{ref}} -out {{run}}/missing/o -mats {S1}", 0,
       "refuse", F20, F20, description="-mats with -reffile in a missing directory (FSL-20)"),
    _c("fsl20-file-parent", f"-in {{in}} -out {{run}}/afile/o -mats {S1}", 0, "refuse", F20, F20,
       setup=(("file", "afile"),), description="-mats where the directory is a file (FSL-20)"),
    _c("fsl20-rmsrel", f"-in {{in}} -out {{run}}/missing/o -rmsrel {S1}", 0, "refuse",
       ("FSL-13", "FSL-20"), ("FSL-13", "FSL-20"),
       description="-rmsrel without -mats in a missing directory (FSL-13, FSL-20)"),
    _c("fsl20-rmsabs-report", f"-in {{in}} -out {{run}}/missing/o -rmsabs -report {S1}", 0,
       "refuse", ("FSL-13", "FSL-20"), ("FSL-13", "FSL-20"),
       description="-rmsabs -report without -mats in a missing directory (FSL-13, FSL-20)"),
    _c("fsl20-perm", f"-in {{in}} -out {{run}}/ro/o -mats {S1}", -6, "refuse", (), (),
       setup=(("ro", "ro"),),
       description="-mats in a read-only directory: mcflirt's permission message, then it aborts"),
    _c("fsl20-plots-only", f"-in {{in}} -out {{run}}/missing/o -plots {S1}", 0, "refuse", F65,
       F65, description="-plots alone in a missing directory (FSL-65)"),
    _c("fsl20-none", f"-in {{in}} -out {{run}}/missing/o {S1}", -6, "refuse", (), (),
       description="no output option, in a missing directory: the series cannot be written"),
    _c("par-dir-plots-mats", f"-in {{in}} -out {{run}}/o -mats -plots -rmsrel {S1}", 0, "refuse",
       F65, F65, setup=(("dir", "o.par"),), tier="smoke",
       description="a directory named o.par, with -mats -plots -rmsrel (FSL-65)"),
    _c("par-dir-plots", f"-in {{in}} -out {{run}}/o -plots {S1}", 0, "refuse", F65, F65,
       setup=(("dir", "o.par"),), description="a directory named o.par, -plots (FSL-65)"),
    _c("par-dir-plots-mats-ref0", f"-in {{in}} -out {{run}}/o -mats -plots -refvol 0 {S1}", 0,
       "refuse", F65, F65, setup=(("dir", "o.par"),),
       description="a directory named o.par, -mats -plots -refvol 0 (FSL-65)"),
    _c("rms-dir", f"-in {{in}} -out {{run}}/o -mats -rmsrel -rmsabs {S1}", 0, "refuse", F65, F65,
       setup=(("dir", "o_rel.rms"), ("dir", "o_abs_mean.rms")),
       description="directories named o_rel.rms and o_abs_mean.rms (FSL-65)"),
    _c("bins7-mutualinfo", "-in {in} -out {run}/o -mats -cost mutualinfo -bins 7", 0, "refuse",
       F21, F21, arbitrary=True, description="mutualinfo with -bins 7 (FSL-21)"),
    _c("bins4-mutualinfo-s2", "-in {in} -out {run}/o -mats -cost mutualinfo -bins 4 -stages 2", 0,
       "refuse", F21, F21, arbitrary=True, description="mutualinfo, -bins 4, two stages (FSL-21)"),
    _c("bins4-mutualinfo-s3-plots", "-in {in} -out {run}/o -mats -plots -cost mutualinfo -bins 4",
       0, "refuse", F21, F21, arbitrary=True,
       description="mutualinfo, -bins 4, three stages, -plots (FSL-21)"),
    _c("bins0-normmi", "-in {in} -out {run}/o -mats -cost normmi -bins 0", 0, "refuse", F21, F21,
       arbitrary=True, description="normmi with -bins 0 (FSL-21)"),
    _c("bins1-woods", "-in {in} -out {run}/o -mats -cost woods -bins 1", 0, "refuse", F21, F21,
       arbitrary=True, description="woods with -bins 1 (FSL-21)"),
    _c("binsabc-mutualinfo", "-in {in} -out {run}/o -mats -cost mutualinfo -bins abc", 0, "error",
       ("FSL-07",), ("FSL-07", "FSL-21"), arbitrary=True,
       description="-bins abc, read as 0 (FSL-07; FSL-21 reproduced)"),
    _c("bins2-corratio", "-in {in} -out {run}/o -mats -cost corratio -bins 2", -6, "refuse", F21,
       F21, tier="smoke", description="corratio with -bins 2: mcflirt aborts (FSL-21)"),
    _c("bins2-corratio-smooth0", "-in {in} -out {run}/o -mats -cost corratio -bins 2 -smooth 0",
       -6, "refuse", F21, F21, description="corratio, -bins 2, -smooth 0 (FSL-21)"),
    _c("bins2-corratio-report", "-in {in} -out {run}/o -mats -cost corratio -bins 2 -report", -6,
       "refuse", F21, F21, description="corratio, -bins 2, -report (FSL-21)"),
    _c("bins8-corratio", "-in {in} -out {run}/o -mats -cost corratio -bins 8", 0, "run", (), (),
       description="corratio with -bins 8: one bin in stage 1"),
    # One bin in stage 1: the mutual information is 0 but for rounding (measured: 3 of the 4
    # registered volumes identical as text, the others 0.4 and 2 mm away; from 16 bins on within
    # 0.07 mm), so the matrices are reported.
    _c("bins8-mutualinfo-s3", "-in {in} -out {run}/o -mats -cost mutualinfo -bins 8", 0, "run",
       (), (), arbitrary=True,
       description="mutualinfo with -bins 8: one bin in stage 1 (a cost of rounding noise)"),
    _c("bins2-normcorr", "-in {in} -out {run}/o -mats -bins 2", 0, "run", (), (),
       description="normcorr with -bins 2 (it uses no bins)"),
    _c("bins2-leastsq", "-in {in} -out {run}/o -mats -cost leastsquares -bins 2", 0, "run", (),
       (), description="leastsquares with -bins 2 (it uses no bins)"),
    _c("binsm1-normcorr", "-in {in} -out {run}/o -mats -bins -1", 0, "refuse", F66, F66,
       description="normcorr with -bins -1: no bins anywhere (FSL-66)"),
    _c("bins4-mutualinfo-s0", "-in {in} -out {run}/o -mats -cost mutualinfo -bins 4 -stages 0", 0,
       "run", (), (), description="mutualinfo, -bins 4, -stages 0: no stage runs"),
    _c("bins4-mutualinfo-1vol", "-in {one} -out {run}/o -mats -cost mutualinfo -bins 4", 0,
       "refuse", F21, (), description="mutualinfo, -bins 4, one volume: nothing is registered"),
    _c("binsm4-normcorr-s2", "-in {in} -out {run}/o -mats -bins -4 -stages 2", -8, "refuse", F66,
       F66, description="-bins -4, two stages: mcflirt is killed by SIGFPE (FSL-66)"),
    _c("binsm8-normcorr", "-in {in} -out {run}/o -mats -bins -8", -8, "refuse", F66, F66,
       description="-bins -8: SIGFPE in stage 1 (FSL-66)"),
    _c("binsm16-normcorr", "-in {in} -out {run}/o -mats -bins -16", -6, "refuse", F66, F66,
       tier="smoke", description="-bins -16: std::bad_array_new_length (FSL-66)"),
    _c("binsm16-mutualinfo", "-in {in} -out {run}/o -mats -cost mutualinfo -bins -16", -6,
       "refuse", F66, F66, description="mutualinfo, -bins -16 (FSL-66)"),
)  # fmt: skip

TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    if tier not in TIERS:
        return []
    rank = TIERS.index(tier)
    return [
        Case(f"failures/{c.name}", CATEGORY, c.description, c)
        for c in CASES
        if TIERS.index(c.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Inputs


def _phantom(volumes: int) -> np.ndarray:
    """A smooth ellipsoid with a ramp and a pattern, moved by a fraction of a voxel per volume."""
    x, y, z = np.meshgrid(*(np.arange(n, dtype=np.float64) for n in SHAPE), indexing="ij")
    vols = []
    for t in range(volumes):
        cx, cy, cz = 11.5 + 0.3 * t, 11.5 - 0.2 * t, 7.5 + 0.1 * t
        r2 = ((x - cx) / 8) ** 2 + ((y - cy) / 9) ** 2 + ((z - cz) / 5) ** 2
        vols.append(1000.0 * np.clip(1.0 - r2, 0, None) + 50.0 * x / 24.0 + 20.0 * ((y + 2 * z) % 5))
    return np.stack(vols, axis=-1).astype(np.float32)


@functools.cache
def inputs() -> dict[str, Path]:
    """The series, a one-volume series and a reference (its first volume), in a directory of
    this process."""
    import nibabel as nib

    d = Path(tempfile.mkdtemp(prefix="lx-hmc-failures-inputs_"))
    affine = np.diag([3.0, 3.0, 3.0, 1.0])
    data = _phantom(VOLUMES)
    out = {"in": d / "series.nii.gz", "one": d / "one.nii.gz", "ref": d / "ref.nii.gz"}
    nib.save(nib.Nifti1Image(data, affine), out["in"])
    nib.save(nib.Nifti1Image(data[..., :1], affine), out["one"])
    nib.save(nib.Nifti1Image(data[..., 0], affine), out["ref"])
    return out


def _prepare(run: Path, case: FailureCase) -> set[str]:
    """The run directory as the case wants it; the names it holds before the run."""
    run.mkdir(parents=True)
    for kind, name in case.setup:
        p = run / name
        if kind == "file":
            p.write_text("not a directory\n", encoding="utf-8")
        else:
            p.mkdir()
            if kind == "ro":
                p.chmod(stat.S_IRUSR | stat.S_IXUSR)
    return {name for _, name in case.setup}


def _release(run: Path) -> None:
    """Write permission back, so the temporary directory can be removed."""
    for p in [run, *run.rglob("*")]:
        if p.is_dir():
            p.chmod(stat.S_IRWXU)


def _args(case: FailureCase, run: Path) -> list[str]:
    files = inputs()
    return [
        w.replace("{in}", str(files["in"]))
        .replace("{one}", str(files["one"]))
        .replace("{ref}", str(files["ref"]))
        .replace("{run}", str(run))
        for w in case.args
    ]


def _written(run: Path, setup: set[str]) -> list[str]:
    """Every file under ``run`` (relative names), the prepared ones left out."""
    return sorted(
        p.relative_to(run).as_posix()
        for p in run.rglob("*")
        if p.is_file() and p.relative_to(run).as_posix() not in setup
    )


def _normalise(text: str, run: Path) -> str:
    return text.replace(str(run), "<run>").replace(str(inputs()["in"].parent), "<inputs>")


#: larmorx's own lines: its notices and its errors.
_OWN = re.compile(r"^(larmorx: |Error: )")
_NAMED = re.compile(r"\b(FSL-\d\d)\b")


def first_mcflirt_line(stderr: str) -> str:
    """The first line of standard error that is mcflirt's (larmorx's own lines set aside)."""
    for line in stderr.splitlines():
        if not _OWN.match(line):
            return line
    return ""


def named(stderr: str) -> set[str]:
    """The defects larmorx names: in its notices and in its ``Error:`` lines."""
    out = set(defects.notices_in(stderr))
    for line in stderr.splitlines():
        if line.startswith("Error: "):
            out.update(_NAMED.findall(line))
    return out


# ------------------------------------------------------------------------------------------------
# Running


def _environment() -> tuple[str, dict[str, str]]:
    """mcflirt and the environment its recorded runs used (FSLOUTPUTTYPE NIFTI_GZ, a plain
    PATH; OpenBLAS's Haswell kernels, FSL-64)."""
    from larmorx_validation.difftest.tools.mri_hmc import _fsl

    fsl = _fsl("original")
    env = {
        "HOME": str(Path.home()),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsl),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "OPENBLAS_CORETYPE": "Haswell",
    }
    return str(fsl / "bin" / "mcflirt"), env


def _run_mcflirt(case: FailureCase, run: Path):
    from larmorx_validation import oracle_cache

    program, env = _environment()
    never = (
        "the case prepares a directory, a file or a permission in the run directory, which the "
        "key does not hold"
        if case.setup
        else ""
    )
    policy = oracle_cache.Policy(threads=1, family="fsl", never=never)
    return oracle_cache.run(
        [program, *_args(case, run)],
        env=env,
        cwd=run,
        outputs=[run],
        policy=policy,
        runner="mri-hmc-failures",
    )


def _run_larmorx(case: FailureCase, run: Path, mode: str) -> tuple[int, str, str]:
    """``larmorx mri hmc`` in process (the record's runner), every path absolute."""
    from larmorx_validation.parity import mri_hmc

    _, env = _environment()
    return mri_hmc._run_larmorx(_args(case, run), env, mode)


def _matrices(a: Path, b: Path) -> tuple[float, int] | None:
    """The 95th-percentile RMS deviation (mm) of two ``.mat`` directories and the number of
    matrices identical as text, when both hold the same count."""
    from larmorx_validation.parity import mri_hmc

    if not (a.is_dir() and b.is_dir()):
        return None
    ma, ta = mri_hmc._read_mats(a)
    mb, tb = mri_hmc._read_mats(b)
    if len(ma) != len(mb) or not ma:
        return None
    centre = mri_hmc._fov_centre(inputs()["in"])
    devs = [mri_hmc.rms_deviation(x, y, centre) for x, y in zip(ma, mb, strict=True)]
    return float(np.percentile(devs, 95)), sum(x == y for x, y in zip(ta, tb, strict=True))


def run_case(case: Case, checks: CheckList) -> dict:
    from larmorx_validation.parity import mri_hmc

    c: FailureCase = case.payload
    if not Path(_environment()[0]).is_file():
        raise Outcome("skipped", "FSL's mcflirt was not found")
    root = Path(tempfile.mkdtemp(prefix="lx-hmc-failures_"))
    try:
        runs = {k: root / k for k in ("mcflirt", "reproduce", "fix")}
        setups = {k: _prepare(r, c) for k, r in runs.items()}
        done = _run_mcflirt(c, runs["mcflirt"])
        m_code = done.returncode if done.returncode is not None else -9
        m_err = _normalise(done.stderr, runs["mcflirt"])
        r_code, _, r_err = _run_larmorx(c, runs["reproduce"], defects.REPRODUCE)
        f_code, _, f_err = _run_larmorx(c, runs["fix"], defects.FIX)
        r_err, f_err = _normalise(r_err, runs["reproduce"]), _normalise(f_err, runs["fix"])
        m_files = _written(runs["mcflirt"], setups["mcflirt"])
        r_files = _written(runs["reproduce"], setups["reproduce"])
        f_files = _written(runs["fix"], setups["fix"])
        checks(
            "mcflirt: the exit status the spec observed",
            m_code == c.mcflirt,
            detail=f"expected {c.mcflirt}, mcflirt {m_code}: {' '.join(m_err.split())[:200]}",
        )
        expected_r = m_code if m_code >= 0 else 1
        checks(
            "reproduce: exit status (1 where mcflirt ends by a signal)",
            r_code == expected_r,
            detail=f"mcflirt {m_code}, larmorx {r_code}: {' '.join(r_err.split())[-240:]}",
        )
        checks(
            "reproduce: mcflirt's first line of standard error",
            first_mcflirt_line(m_err) == first_mcflirt_line(r_err),
            detail=f"mcflirt {first_mcflirt_line(m_err)!r}; larmorx {first_mcflirt_line(r_err)!r}",
        )
        checks(
            "reproduce: files written",
            m_files == r_files,
            detail=f"mcflirt {m_files}; larmorx {r_files}",
        )
        r_named = named(r_err)
        checks(
            "reproduce: defects named",
            set(c.reproduce_defects) <= r_named,
            detail=f"expected {sorted(c.reproduce_defects)}, named {sorted(r_named)}",
        )
        metrics: dict = {}
        if m_code == 0 and r_code == 0:
            mats = _matrices(runs["mcflirt"] / "o.mat", runs["reproduce"] / "o.mat")
            if mats is not None:
                p95, identical = mats
                metrics.update(rms_p95=p95, identical=identical)
                checks(
                    "reproduce: matrix RMS deviation, 95th percentile",
                    c.arbitrary or p95 <= mri_hmc.P95_RMS_MM,
                    metric="mm",
                    value=f"{p95:.4f}",
                    threshold="reported (a stage with no bin or one: arbitrary matrices)"
                    if c.arbitrary
                    else f"≤ {mri_hmc.P95_RMS_MM}",
                    detail=f"{identical} matrices identical as text",
                )
        f_named = named(f_err)
        if c.default == "run":
            checks(
                "default: runs as mcflirt",
                f_code == 0 and f_files == m_files,
                detail=f"exit {f_code}; files {f_files}",
            )
        else:
            code = 255 if c.default == "error" else 1
            checks(
                "default: refused (exit status, nothing written)",
                f_code == code and not f_files,
                detail=f"exit {f_code} (expected {code}); files {f_files}: "
                f"{' '.join(f_err.split())[-240:]}",
            )
            checks(
                "default: defects named",
                set(c.default_defects) <= f_named,
                detail=f"expected {sorted(c.default_defects)}, named {sorted(f_named)}",
            )
            if c.default_defects:
                # A correction that refuses what mcflirt runs: named, as every change.
                defects.add_fix_check(
                    checks, sorted(f_named), defects.Change(0, 0, 0.0), other_changes=f"exit {f_code}"
                )
    finally:
        _release(root)
        shutil.rmtree(root, ignore_errors=True)
    if m_code < 0 and all(ch.passed for ch in checks.checks):
        raise Outcome(
            BOTH_ERROR,
            f"mcflirt ends by signal {-m_code} ({first_mcflirt_line(m_err) or 'no message'}); "
            f"larmorx exits {r_code} (reproduce) and {f_code} (default) with its error",
        )
    return metrics
