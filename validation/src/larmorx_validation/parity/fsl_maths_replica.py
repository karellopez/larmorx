# SPDX-License-Identifier: Apache-2.0
"""Parity of the fslmaths replica (``larmorx-nc fsl fslmaths``, FSL Licence) with FSL 6.0.7.23's
``fslmaths``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own calls (niworkflows' ``combine_masks``: ``fslmaths <mask a>
  -mul <mask b> <out>`` in every BOLD reference's brain mask) on the oracle runs, the masks read
  from the runs' working directories;
- **cli**, **dt**, **op**, **thr**, **fn**, **d13**: the command line, the data types, the binary
  operations, the thresholds, the voxelwise operations and the defects (FSL-151 to FSL-159) on
  small synthetic images made once (``oracles/inputs/fsl-maths-replica/``, seeded, every
  NIfTI type and header the cases need).

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every FSL run goes through the oracle-output cache (fslmaths is single-threaded and
deterministic: its random numbers start from a fixed state); cases where FSL reads memory it
never wrote (``undefined``) do not run FSL: the replica must refuse in both modes.
"""

from __future__ import annotations

import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.fsl_flirt_replica import (
    _agree_on_errors,
    _environment,
    compare,
    fix_pass,
    fsldir,
    run_replica,
)
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import Case, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-maths-replica"
PROGRAM = "fslmaths"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: fMRIPrep's combine_masks calls of the standard tier: ds000005 (single-band 3 T), ds005040
#: (both of its kinds: the coregistration reference's two skull-stripping passes, and the
#: head-motion reference's first pass with the template mask) and ds003592 (multi-echo).
STANDARD_RUNS = (
    "ds000005-sub-01-noreconall",
    "ds005040-sub-002-noreconall",
    "ds003592-sub-280-noreconall",
)
#: The defects whose notice may name a condition the comparison cannot see.
_SILENT = ("FSL-158",)


# ------------------------------------------------------------------------------------------------
# The programs


def oracle_run(args: list[str], *, cwd: Path, settings: dict[str, str | None]):
    """FSL's fslmaths with ``args`` in ``cwd``, through the oracle-output cache: single-threaded
    and deterministic, so a run is served once it ran; every file it writes is in ``cwd`` and
    every file there counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / PROGRAM), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=600,
    )


def run_oracle(args: list[str], *, cwd: Path, settings: dict[str, str | None]) -> Completed:
    p = oracle_run(args, cwd=cwd, settings=settings)
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Inputs


def inputs() -> Path:
    """The synthetic inputs, made once (seeded; the same bytes every time)."""
    ws = workspace()
    base = (ws / "oracles" / "inputs" if ws else Path(tempfile.gettempdir())) / SUITE
    marker = base / "v5.done"
    if marker.exists():
        return base
    base.mkdir(parents=True, exist_ok=True)
    _make_inputs(base)
    marker.write_text("1\n", encoding="utf-8")
    return base


def _affine(shape: tuple[int, ...], vox=(2.0, 2.5, 3.0), neuro: bool = False) -> np.ndarray:
    a = np.diag([-vox[0] if not neuro else vox[0], vox[1], vox[2], 1.0])
    a[:3, 3] = [vox[0] * shape[0] / 2 * (1 if not neuro else -1), -vox[1] * shape[1] / 2, -10.0]
    return a


def _save(path: Path, data: np.ndarray, *, dtype, affine=None, slope=None, noform=False) -> None:
    """Writes the image unless it is there (the inputs keep their bytes, and so their oracle
    runs' cache entries, when new ones are added)."""
    import nibabel as nib

    if path.exists():
        return

    shape = data.shape
    aff = _affine(shape) if affine is None else affine
    img = nib.Nifti1Image(data.astype(dtype), aff)
    h = img.header
    h.set_data_dtype(dtype)
    h.set_xyzt_units("mm", "sec")
    if noform:
        h.set_sform(None, code=0)
        h.set_qform(None, code=0)
    else:
        img.set_sform(aff, 1)
        img.set_qform(aff, 1)
    if slope is not None:
        h["scl_slope"], h["scl_inter"] = slope
    else:
        h["scl_slope"], h["scl_inter"] = 1.0, 0.0
    h["cal_max"], h["cal_min"] = 7.0, -3.0
    if path.name.endswith(".hdr"):
        img = nib.Nifti1Pair(img.dataobj, aff, h)
    nib.save(img, str(path))


def _make_inputs(d: Path) -> None:
    rng = np.random.default_rng(20261103)
    sh = (7, 6, 5)
    f3 = (rng.standard_normal(sh) * 40 + 10).astype(np.float32)
    f3b = (rng.standard_normal(sh) * 5 + 2).astype(np.float32)
    f3b[0, 0, 0] = 0.0
    f3b[3, 2, 1] = 0.0
    f4 = (rng.standard_normal((*sh, 3)) * 20).astype(np.float32)
    m3 = (rng.random(sh) > 0.4).astype(np.uint8)
    m3[1, 1, 1] = 2
    m4 = (rng.random((*sh, 3)) > 0.5).astype(np.uint8)
    s3 = rng.integers(-300, 3000, sh).astype(np.int16)
    u8 = rng.integers(0, 256, sh).astype(np.uint8)
    i8 = rng.integers(-128, 128, sh).astype(np.int8)
    u16 = rng.integers(0, 65535, sh).astype(np.uint16)
    rem3 = rng.integers(1, 9, sh).astype(np.float32)
    rem3[2, 2, 2] = 0.0
    frac3 = rem3.copy()
    frac3[4, 4, 4] = 0.5
    nan3 = f3.copy()
    nan3[0, 0, 0] = np.nan
    nan3[2, 3, 1] = np.nan
    nan3[5, 1, 4] = np.inf
    big3 = (rng.random(sh) * 3e7).astype(np.float32)
    big3[1, 2, 3] = np.float32(2**25)
    inf3 = f3.copy()
    inf3[3, 3, 3] = np.inf
    inf3[4, 4, 4] = -np.inf
    neg3 = -np.abs(f3) - 1
    unit3 = np.clip(rng.random(sh) * 2.2 - 1.1, -1.1, 1.1).astype(np.float32)
    p3 = rng.random(sh).astype(np.float32)
    _save(d / "f3.nii.gz", f3, dtype=np.float32)
    _save(d / "f3b.nii.gz", f3b, dtype=np.float32)
    _save(d / "f4.nii.gz", f4, dtype=np.float32)
    _save(d / "m3.nii.gz", m3, dtype=np.uint8)
    _save(d / "m4.nii.gz", m4, dtype=np.uint8)
    _save(d / "s3.nii.gz", s3, dtype=np.int16)
    _save(d / "u8.nii.gz", u8, dtype=np.uint8)
    _save(d / "i8.nii.gz", i8, dtype=np.int8)
    _save(d / "u16.nii.gz", u16, dtype=np.uint16)
    _save(d / "d3.nii.gz", f3.astype(np.float64) / 3, dtype=np.float64)
    _save(d / "rem3.nii.gz", rem3, dtype=np.float32)
    _save(d / "frac3.nii.gz", frac3, dtype=np.float32)
    _save(d / "nan3.nii.gz", nan3, dtype=np.float32)
    _save(d / "big3.nii.gz", big3, dtype=np.float32)
    _save(d / "inf3.nii.gz", inf3, dtype=np.float32)
    _save(d / "neg3.nii.gz", neg3, dtype=np.float32)
    _save(d / "unit3.nii.gz", unit3, dtype=np.float32)
    _save(d / "p3.nii.gz", p3, dtype=np.float32)
    _save(d / "scaled.nii.gz", s3, dtype=np.int16, slope=(0.5, 3.0))
    _save(d / "noform.nii.gz", f3, dtype=np.float32, noform=True)
    _save(d / "neuro.nii.gz", f3, dtype=np.float32, affine=_affine(sh, neuro=True))
    _save(d / "f3.nii", f3, dtype=np.float32)
    _save(d / "pair.hdr", f3, dtype=np.float32)
    rot = _affine(sh).copy()
    c, s = np.cos(0.3), np.sin(0.3)
    rot[:3, :3] = rot[:3, :3] @ np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])
    _save(d / "rot.nii.gz", f3b, dtype=np.float32, affine=rot)
    other = rng.standard_normal((8, 6, 5)).astype(np.float32) + 3
    _save(d / "other.nii.gz", other, dtype=np.float32, affine=_affine((8, 6, 5)))
    small = rng.standard_normal((5, 6, 5)).astype(np.float32) + 3
    _save(d / "small.nii.gz", small, dtype=np.float32, affine=_affine((5, 6, 5)))
    row = (rng.standard_normal((7, 1, 1)) + 2).astype(np.float32)
    _save(d / "row.nii.gz", row, dtype=np.float32, affine=_affine((7, 1, 1)))
    f45 = (rng.standard_normal((*sh, 5)) * 3).astype(np.float32)
    _save(d / "f45.nii.gz", f45, dtype=np.float32)
    # v2 (2026-11-03): a smaller image of integer divisors (FSL-159's -rem without FSL-156)
    smallint = rng.integers(1, 6, (5, 6, 5)).astype(np.float32)
    _save(d / "smallint.nii.gz", smallint, dtype=np.float32, affine=_affine((5, 6, 5)))
    # v3 (2026-11-03, sub-task 2): sparse images for the dilations, labels, a binary blob, an
    # odd-sized anisotropic image, a 2D image and kernel files
    sparse3 = (rng.integers(1, 4, sh) * (rng.random(sh) > 0.6)).astype(np.float32)
    sparse3[rng.random(sh) > 0.8] *= -1
    _save(d / "sparse3.nii.gz", sparse3, dtype=np.float32)
    sparsenan3 = sparse3.copy()
    sparsenan3[1, 1, 1] = np.nan
    sparsenan3[4, 3, 2] = np.nan
    sparsenan3[6, 5, 4] = np.nan
    sparsenan3[0, 0, 0] = 0.0
    _save(d / "sparsenan3.nii.gz", sparsenan3, dtype=np.float32)
    lab3 = (rng.integers(1, 6, sh) * (rng.random(sh) > 0.55)).astype(np.int16)
    _save(d / "lab3.nii.gz", lab3, dtype=np.int16)
    blob3 = np.zeros(sh, dtype=np.uint8)
    blob3[1:6, 1:5, 0:4] = 1
    blob3[3, 2, 2] = 0
    blob3[6, 0, 4] = 1
    _save(d / "blob3.nii.gz", blob3, dtype=np.uint8)
    osh = (9, 7, 5)
    odd3 = (rng.standard_normal(osh) * 30 + 50).astype(np.float32)
    odd3[rng.random(osh) > 0.7] = 0.0
    _save(d / "odd3.nii.gz", odd3, dtype=np.float32, affine=_affine(osh, vox=(1.5, 2.0, 2.5)))
    two2d = (rng.standard_normal((8, 7, 1)) * 10).astype(np.float32)
    two2d[rng.random((8, 7, 1)) > 0.6] = 0.0
    _save(d / "two2d.nii.gz", two2d, dtype=np.float32, affine=_affine((8, 7, 1)))
    levels = np.array([0.0, 0.3, 0.5, 0.7, 1.0, 2.0], dtype=np.float32)
    kern3 = levels[rng.integers(0, 6, (3, 3, 3))]
    kern3[1, 1, 1] = 1.0
    _save(d / "kern3.nii.gz", kern3, dtype=np.float32, affine=_affine((3, 3, 3)))
    kern_even = (rng.random((2, 3, 3)) > 0.3).astype(np.float32)
    _save(d / "kern_even.nii.gz", kern_even, dtype=np.float32, affine=_affine((2, 3, 3)))
    kern_inf = kern3.copy()
    kern_inf[0, 0, 0] = np.inf
    _save(d / "kern_inf.nii.gz", kern_inf, dtype=np.float32, affine=_affine((3, 3, 3)))
    kern4d = levels[rng.integers(0, 6, (3, 3, 3, 2))]
    _save(d / "kern4d.nii.gz", kern4d, dtype=np.float32, affine=_affine((3, 3, 3)))
    # v4: an infinite kernel value with no value at 0.5, an even 4D image, masks with holes
    kern_inf2 = np.ones((3, 3, 3), dtype=np.float32)
    kern_inf2[0, 0, 0] = np.inf
    _save(d / "kern_inf2.nii.gz", kern_inf2, dtype=np.float32, affine=_affine((3, 3, 3)))
    even4 = (rng.standard_normal((6, 4, 4, 2)) * 10).astype(np.float32)
    _save(d / "even4.nii.gz", even4, dtype=np.float32, affine=_affine((6, 4, 4)))
    holes3 = np.ones((9, 8, 7), dtype=np.uint8)
    holes3[0, 0, 0] = 0  # outside, at the corner
    holes3[1, 1, 1] = 0  # 6-enclosed, 26-connected to the corner
    holes3[4, 4, 3] = 0  # a cavity
    holes3[5:7, 5, 2:4] = 0  # a larger cavity
    holes3[8, 4, 3] = 0  # on a face: touches the edge
    _save(d / "holes3.nii.gz", holes3, dtype=np.uint8, affine=_affine((9, 8, 7)))
    blob2d = np.zeros((7, 6, 1), dtype=np.uint8)
    blob2d[1:6, 1:5, 0] = 1
    blob2d[3, 2, 0] = 0
    _save(d / "blob2d.nii.gz", blob2d, dtype=np.uint8, affine=_affine((7, 6, 1)))
    # v5: diffusion tensors (xx, xy, xz, yy, yz, zz), with non-positive and NaN voxels
    tsh = (5, 4, 3)
    a = rng.standard_normal((*tsh, 3, 3))
    dten = np.einsum("...ij,...kj->...ik", a, a) * 1e-3
    dten[0, 0, 0] = 0.0
    dten[1, 0, 0] = -dten[1, 0, 0]
    dten[2, 1, 1] = np.diag([2e-3, 1e-3, 1e-3])
    tensor = np.stack(
        [
            dten[..., 0, 0],
            dten[..., 0, 1],
            dten[..., 0, 2],
            dten[..., 1, 1],
            dten[..., 1, 2],
            dten[..., 2, 2],
        ],
        axis=-1,
    ).astype(np.float32)
    _save(d / "tensor.nii.gz", tensor, dtype=np.float32, affine=_affine(tsh))
    tensornan = tensor.copy()
    tensornan[3, 2, 1, 4] = np.nan
    _save(d / "tensornan.nii.gz", tensornan, dtype=np.float32, affine=_affine(tsh))


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: fslmaths' arguments (``@name`` names an input copied into the run directory) and
    the environment; ``undefined``: FSL reads memory it never wrote (the register entry): FSL is
    not run, the replica must refuse in both modes; ``fixed``: FSL stops (the register entry),
    the default must give an output and name the defect."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True
    undefined: str = ""
    fixed: str = ""


def _prepare(run: Run, tmp: Path) -> list[str]:
    src = inputs()
    out = []
    for a in run.args:
        if a.startswith("@"):
            name = a[1:]
            p = src / name if (src / name).exists() else src / f"{name}.nii.gz"
            shutil.copy(p, tmp / p.name)
            if p.suffix == ".hdr":
                shutil.copy(p.with_suffix(".img"), tmp / p.with_suffix(".img").name)
            out.append(p.name)
        else:
            out.append(a)
    return out


def _run_case(run: Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    if run.undefined:
        for mode in (defects.REPRODUCE, defects.FIX):
            with tempfile.TemporaryDirectory(prefix="lx-nc-maths-") as b_:
                b = Path(b_)
                args = _prepare(run, b)
                r = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=mode)
                checks(
                    f"{run.undefined}: the replica refuses ({mode})",
                    r.code == 1 and run.undefined in r.stderr,
                    detail=f"exit {r.code}: {r.stderr[-200:]}",
                )
        return
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-maths-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-maths-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = run_oracle(args, cwd=a, settings=settings)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if run.fix and replica.code == 0:
            fix_pass(
                checks,
                PROGRAM,
                lambda t: _prepare(run, t),
                args,
                settings,
                b,
                replica,
                silent_ok=_SILENT,
            )
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)
    if run.fixed:
        with tempfile.TemporaryDirectory(prefix="lx-nc-maths-") as c_:
            c = Path(c_)
            args = _prepare(run, c)
            r = run_replica(PROGRAM, args, cwd=c, settings=settings, mode=defects.FIX)
            checks(
                f"{run.fixed}: the default gives an output and names the defect",
                r.code == 0 and run.fixed in r.stderr and any(c.glob("out*")),
                detail=f"exit {r.code}: {r.stderr[-200:]}",
            )


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, combine_masks node directory)`` of every combine_masks call of the oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for cmd in sorted((ws / FMRIPREP_RUNS).glob("*/work/**/combine_masks/command.txt")):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def fmriprep_args(run: str, node: Path) -> list[str]:
    """fMRIPrep's call: the container's ``/work`` mapped onto the run's working directory, the
    output written into the run directory under its own name."""
    ws = workspace()
    assert ws is not None
    work = ws / FMRIPREP_RUNS / run / "work"
    argv = (node / "command.txt").read_text(encoding="utf-8").split()[1:]
    out = []
    for k, a in enumerate(argv):
        if k == len(argv) - 1:
            out.append(Path(a).name)
        elif a.startswith("/work/"):
            out.append(str(work / a.removeprefix("/work/")))
        else:
            out.append(a)
    return out


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    args = fmriprep_args(run, Path(node))
    missing = [x for x in args if x.startswith("/") and not Path(x).exists()]
    if missing:
        raise Outcome("skipped", f"input not available: {missing[0]}")
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-maths-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-maths-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        theirs = next(Path(node).glob("*_maths.nii.gz"), None)
        ours = next(b.glob("*_maths.nii.gz"), None)
        if theirs is not None and ours is not None:
            import nibabel as nib

            same = np.array_equal(
                np.asarray(nib.load(str(theirs)).dataobj),
                np.asarray(nib.load(str(ours)).dataobj),
            )
            checks(
                "fMRIPrep's own mask (its container's FSL 6.0.7.x) has the same values",
                True,
                metric="identical",
                value="yes" if same else "no",
                threshold="reported",
            )
        if oracle.code == 0 and replica.code == 0:
            fix_pass(checks, PROGRAM, lambda t: None, args, {}, b, replica, silent_ok=_SILENT)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _r(
    *args: str,
    env: dict[str, str | None] | None = None,
    fix: bool = True,
    undefined: str = "",
    fixed: str = "",
) -> Run:
    return Run(tuple(args), tuple((env or {}).items()), fix, undefined, fixed)


#: name -> (tier, description, run).
def _cases() -> dict[str, tuple[str, str, Run]]:
    S, T = "smoke", "standard"
    F3, F4, M3 = "@f3", "@f4", "@m3"
    c: dict[str, tuple[str, str, Run]] = {
        # the command line
        "cli/usage": (S, "no arguments: the usage (exit 1)", _r()),
        "cli/help": (T, "-h: the usage (exit 0)", _r("-h")),
        "cli/help-long": (T, "--help", _r("--help")),
        "cli/missing-input": (T, "an input that does not exist", _r("nothere", "out")),
        "cli/no-output": (T, "no output name", _r(F3)),
        "cli/copy": (S, "no operation: a copy (the display range kept)", _r(F3, "out")),
        "cli/unknown-op": (T, "an unknown operation: the usage", _r(F3, "-foo", "out")),
        "cli/odt-unknown": (T, "-odt with an unknown type", _r(F3, "out", "-odt", "bogus")),
        "cli/dt-unknown": (T, "-dt with an unknown type", _r("-dt", "bogus", F3, "out")),
        "cli/dt-no-input": (T, "-dt float without an input (FSL-158)", _r("-dt", "float")),
        "cli/op-as-output": (T, "an operation in the output's place", _r(F3, "-add")),
        "cli/add-output-image": (
            T,
            "-add reading the output's name as an image",
            _r(F3, "-add", "out"),
        ),
        "cli/chain": (
            S,
            "a chain of operations",
            _r(F3, "-mul", "2", "-add", "@f3b", "-abs", "-sqrt", "out"),
        ),
        "cli/datatype-long": (
            T,
            "-datatype and -output_datatype",
            _r("-datatype", "double", F3, "-div", "3", "out", "-output_datatype", "int"),
        ),
        "cli/save-restart": (
            T,
            "-save mid-chain and -restart",
            _r(F3, "-mul", "3", "-save", "mid", "-restart", "@f3b", "-add", "1", "out"),
        ),
        "cli/range": (
            T,
            "-range: the display range of the result",
            _r(F3, "-mul", "2", "-range", "out"),
        ),
        "cli/nifti": (
            T,
            "FSLOUTPUTTYPE=NIFTI",
            _r(F3, "-add", "1", "out", env={"FSLOUTPUTTYPE": "NIFTI"}),
        ),
        "cli/pair": (
            T,
            "FSLOUTPUTTYPE=NIFTI_PAIR, a pair as input",
            _r("@pair.hdr", "-add", "1", "out", env={"FSLOUTPUTTYPE": "NIFTI_PAIR"}),
        ),
        "cli/nifti2": (
            T,
            "FSLOUTPUTTYPE=NIFTI2_GZ",
            _r(F3, "-add", "1", "out", env={"FSLOUTPUTTYPE": "NIFTI2_GZ"}),
        ),
        "cli/analyze": (
            T,
            "FSLOUTPUTTYPE=ANALYZE",
            _r(F3, "-add", "1", "out", env={"FSLOUTPUTTYPE": "ANALYZE"}),
        ),
        "cli/outtype-bad": (
            T,
            "FSLOUTPUTTYPE unknown",
            _r(F3, "-add", "1", "out", env={"FSLOUTPUTTYPE": "BOGUS"}),
        ),
        "cli/nii-input": (T, "an uncompressed input", _r("@f3.nii", "-add", "1", "out")),
        "cli/neuro": (
            T,
            "a neurological input (swapped in memory, back on output)",
            _r("@neuro", "-mul", "@f3b", "out"),
        ),
        "cli/noform": (T, "an input without sform and qform", _r("@noform", "-add", "@f3b", "out")),
        # data types
        "dt/char": (T, "-dt char on a uint8 image", _r("-dt", "char", "@u8", "-add", "100", "out")),
        "dt/short": (T, "-dt short (wrap-around)", _r("-dt", "short", "@s3", "-mul", "20", "out")),
        "dt/int": (T, "-dt int", _r("-dt", "int", "@s3", "-div", "7", "out")),
        "dt/double": (T, "-dt double", _r("-dt", "double", F3, "-div", "3", "out")),
        "dt/float": (T, "-dt float", _r("-dt", "float", "@d3", "-mul", "3", "out")),
        "dt/input-short": (T, "-dt input on int16", _r("-dt", "input", "@s3", "-div", "7", "out")),
        "dt/input-char": (T, "-dt input on uint8", _r("-dt", "input", "@u8", "-bin", "out")),
        "dt/input-uint16": (
            T,
            "-dt input on uint16 (FSL-155)",
            _r("-dt", "input", "@u16", "-div", "3", "out"),
        ),
        "dt/input-int8": (
            T,
            "-dt input on int8 (FSL-155)",
            _r("-dt", "input", "@i8", "-add", "1", "out"),
        ),
        "dt/double-default": (
            T,
            "a double input: computed and written in double",
            _r("@d3", "-mul", "3", "out"),
        ),
        "dt/scaled": (T, "a scaled int16 input (read as float)", _r("@scaled", "-add", "1", "out")),
        "dt/odt-char": (
            T,
            "-odt char (rounded; values out of range: FSL-151)",
            _r(F3, "out", "-odt", "char"),
        ),
        "dt/odt-short": (T, "-odt short", _r(F3, "-mul", "100", "out", "-odt", "short")),
        "dt/odt-int": (T, "-odt int", _r(F3, "-mul", "1e8", "out", "-odt", "int")),
        "dt/odt-double": (T, "-odt double", _r(F3, "out", "-odt", "double")),
        "dt/odt-input": (T, "-odt input on int16", _r("@s3", "-div", "3", "out", "-odt", "input")),
        "dt/odt-input-uint16": (
            T,
            "-odt input on uint16",
            _r("@u16", "-div", "3", "out", "-odt", "input"),
        ),
        "dt/odt-nan": (
            T,
            "-odt short with NaN and infinite voxels (FSL-151)",
            _r("@nan3", "out", "-odt", "short"),
        ),
        "dt/int-overflow": (
            T,
            "-dt int with a number beyond int (FSL-151)",
            _r("-dt", "int", "@s3", "-add", "1e10", "out"),
        ),
        "dt/int-exp": (
            T,
            "-dt short -exp (results beyond short: FSL-151)",
            _r("-dt", "short", "@s3", "-exp", "out"),
        ),
        # binary operations
        "op/add-number": (S, "-add a number", _r(F3, "-add", "2.5", "out")),
        "op/sub-number": (T, "-sub a number", _r(F3, "-sub", "-1e3", "out")),
        "op/mul-number": (T, "-mul a number", _r(F3, "-mul", "0.1", "out")),
        "op/div-number": (T, "-div a number", _r(F3, "-div", "7", "out")),
        "op/div-zero": (T, "-div 0: nothing done", _r(F3, "-div", "0", "out")),
        "op/add-image": (S, "-add an image", _r(F3, "-add", "@f3b", "out")),
        "op/sub-image": (T, "-sub an image", _r(F3, "-sub", "@f3b", "out")),
        "op/mul-image": (T, "-mul an image", _r(F3, "-mul", "@f3b", "out")),
        "op/div-image": (T, "-div an image with zeros (0 there)", _r(F3, "-div", "@f3b", "out")),
        "op/3d-by-4d": (
            T,
            "a 3D image times a 4D one: cloned over time",
            _r(F3, "-mul", F4, "out"),
        ),
        "op/4d-by-3d": (T, "a 4D image times a 3D one", _r(F4, "-sub", F3, "out")),
        "op/4d-by-4d-other-t": (
            T,
            "4D images of 3 and 5 volumes: refused",
            _r(F4, "-add", "@f45", "out"),
        ),
        "op/size-mismatch": (
            T,
            "-add an image of another size: refused",
            _r(F3, "-add", "@other", "out"),
        ),
        "op/row": (
            T,
            "-add a 7x1x1 image (newimage compares the first dimension only)",
            _r(F3, "-add", "@row", "out"),
        ),
        "op/orientation": (
            T,
            "-add an image of another orientation: the warning",
            _r(F3, "-add", "@rot", "out"),
        ),
        "op/mas": (S, "-mas a uint8 mask", _r(F3, "-mas", M3, "out")),
        "op/mas-4d": (T, "-mas a 4D mask on a 3D image", _r(F3, "-mas", "@m4", "out")),
        "op/mas-inf": (T, "-mas: infinite voxels outside the mask", _r("@inf3", "-mas", M3, "out")),
        "op/rem-number": (T, "-rem a number", _r(F3, "-rem", "7", "out")),
        "op/rem-image": (T, "-rem an image (0 voxels skipped)", _r("@s3", "-rem", "@rem3", "out")),
        "op/max-number": (T, "-max a number", _r(F3, "-max", "0", "out")),
        "op/min-number": (T, "-min a number", _r(F3, "-min", "5", "out")),
        "op/max-image": (T, "-max an image", _r(F3, "-max", "@f3b", "out")),
        "op/min-image-4d": (T, "-min a 4D image", _r(F3, "-min", F4, "out")),
        "op/nan-number": (T, "-add nan", _r("@nan3", "-add", "nan", "out")),
        # thresholds
        "thr/thr": (S, "-thr", _r(F3, "-thr", "5", "out")),
        "thr/uthr": (T, "-uthr", _r(F3, "-uthr", "5", "out")),
        "thr/thrp": (T, "-thrp: the robust range", _r(F3, "-thrp", "40", "out")),
        "thr/thrP": (T, "-thrP: the positive voxels' robust range", _r(F3, "-thrP", "40", "out")),
        "thr/uthrp": (T, "-uthrp", _r(F3, "-uthrp", "60", "out")),
        "thr/uthrP": (T, "-uthrP", _r(F3, "-uthrP", "60", "out")),
        "thr/thrP-empty": (
            T,
            "-thrP of an image without positive voxels: the warnings",
            _r("@neg3", "-thrP", "20", "out"),
        ),
        "thr/thr-word": (T, "-thr without a number", _r(F3, "-thr", "five", "out")),
        "thr/uthrP-word": (T, "-uthrP without a number", _r(F3, "-uthrP", "x", "out")),
        "thr/thr-short": (
            T,
            "-thr under -dt short",
            _r("-dt", "short", "@s3", "-thr", "1000", "out"),
        ),
        "thr/thr-4d": (T, "-thrp of a 4D image", _r(F4, "-thrp", "50", "out")),
        "thr/thr-inf": (T, "-thr with infinite voxels", _r("@inf3", "-thr", "0", "out")),
        # voxelwise operations
        "fn/exp": (T, "-exp", _r(F3, "-div", "10", "-exp", "out")),
        "fn/log": (T, "-log", _r(F3, "-log", "out")),
        "fn/sin": (T, "-sin", _r(F3, "-sin", "out")),
        "fn/cos": (T, "-cos", _r(F3, "-cos", "out")),
        "fn/tan": (T, "-tan", _r(F3, "-tan", "out")),
        "fn/asin": (T, "-asin (NaN outside [-1, 1])", _r("@unit3", "-asin", "out")),
        "fn/acos": (T, "-acos", _r("@unit3", "-acos", "out")),
        "fn/atan": (T, "-atan", _r(F3, "-atan", "out")),
        "fn/sqr": (T, "-sqr", _r(F3, "-sqr", "out")),
        "fn/sqrt": (T, "-sqrt (0 for non-positive values)", _r(F3, "-sqrt", "out")),
        "fn/recip": (T, "-recip", _r(F3, "-recip", "out")),
        "fn/abs": (T, "-abs", _r("@nan3", "-abs", "out")),
        "fn/sin-inf": (T, "-sin of infinite and NaN voxels", _r("@nan3", "-sin", "out")),
        "fn/tan-inf": (T, "-tan of infinite and NaN voxels", _r("@nan3", "-tan", "out")),
        "fn/exp-inf": (T, "-exp of infinite and NaN voxels", _r("@inf3", "-exp", "out")),
        "fn/log-inf": (T, "-log of infinite voxels", _r("@inf3", "-log", "out")),
        "fn/double-sin": (T, "-sin in double", _r("@d3", "-sin", "out")),
        "fn/double-cos": (T, "-cos in double", _r("@d3", "-cos", "out")),
        "fn/double-tan": (T, "-tan in double", _r("@d3", "-tan", "out")),
        "fn/double-atan": (T, "-atan in double", _r("@d3", "-atan", "out")),
        "fn/double-asin": (
            T,
            "-asin and -acos in double",
            _r("@d3", "-div", "50", "-asin", "-mul", "0.5", "-acos", "out"),
        ),
        "fn/double-exp-log": (
            T,
            "-exp, -log and -pow in double",
            _r("@d3", "-div", "10", "-exp", "-log", "-pow", "1.7", "out"),
        ),
        "fn/bin": (S, "-bin", _r(F3, "-bin", "out")),
        "fn/binv": (T, "-binv", _r(F3, "-binv", "out")),
        "fn/nan": (T, "-nan", _r("@nan3", "-nan", "out")),
        "fn/nanm": (T, "-nanm", _r("@nan3", "-nanm", "out")),
        "fn/index": (T, "-index", _r(F3, "-index", "out")),
        "fn/grid": (T, "-grid", _r(F3, "-grid", "100", "3", "out")),
        "fn/rand": (T, "-rand (fixed initial state)", _r(F3, "-rand", "out")),
        "fn/randn": (T, "-randn", _r(F3, "-randn", "out")),
        "fn/seed-rand": (T, "-seed then -rand", _r(F3, "-seed", "42", "-rand", "out")),
        "fn/seed-randn": (T, "-seed then -randn", _r(F4, "-seed", "-7", "-randn", "out")),
        "fn/pow": (T, "-pow (undocumented)", _r("@p3", "-pow", "2.5", "out")),
        "fn/invbin": (T, "-invbin (undocumented)", _r(M3, "-invbin", "out")),
        "fn/int-recip": (T, "-recip under -dt int", _r("-dt", "int", "@s3", "-recip", "out")),
        "fn/int-sqrt": (T, "-sqrt under -dt short", _r("-dt", "short", "@s3", "-sqrt", "out")),
        "fn/char-index": (
            T,
            "-index under -dt char (wraps)",
            _r("-dt", "char", "@u8", "-index", "out"),
        ),
        # the defects
        "d13/bin-large": (T, "FSL-152: -bin of values from 2^24", _r("@big3", "-bin", "out")),
        "d13/bin-char": (
            T,
            "FSL-152: -bin under -dt char (max 127 + 1 wraps)",
            _r("-dt", "char", "@i8", "-bin", "out"),
        ),
        "d13/thr-inf": (
            T,
            "FSL-152: -thr when the maximum is infinite",
            _r("@inf3", "-bin", "out"),
        ),
        "d13/uthr-char": (
            T,
            "FSL-152: -uthr under -dt char (min - 1 wraps)",
            _r("-dt", "char", "@i8", "-uthr", "50", "out"),
        ),
        "d13/nan-thr": (T, "FSL-153: -thr with a NaN first voxel", _r("@nan3", "-thr", "0", "out")),
        "d13/nan-bin": (T, "FSL-153: -bin with a NaN first voxel", _r("@nan3", "-bin", "out")),
        "d13/nan-mas": (
            T,
            "FSL-153: -mas by a mask with a NaN first voxel",
            _r(F3, "-mas", "@nan3", "out"),
        ),
        "d13/nan-range": (
            T,
            "FSL-153: -range with a NaN first voxel",
            _r("@nan3", "-range", "out"),
        ),
        "d13/nan-thrp": (T, "FSL-153: -thrp with NaN voxels", _r("@nan3", "-thrp", "30", "out")),
        "d13/rem-zero": (T, "FSL-156: -rem 0 (FSL crashes)", _r(F3, "-rem", "0", "out")),
        "d13/rem-half": (T, "FSL-156: -rem 0.5 (FSL crashes)", _r(F3, "-rem", "0.5", "out")),
        "d13/rem-image-half": (
            T,
            "FSL-156: -rem by an image with a voxel 0.5",
            _r(F3, "-rem", "@frac3", "out"),
        ),
        "d13/div-int-half": (
            T,
            "FSL-156: -dt int -div 0.5 (FSL crashes)",
            _r("-dt", "int", "@s3", "-div", "0.5", "out"),
        ),
        "d13/grid-zero": (T, "FSL-156: -grid with spacing 0", _r(F3, "-grid", "1", "0", "out")),
        "d13/grid-null": (
            T,
            "FSL-158: -grid without its spacing (reads the null pointer)",
            _r(F3, "-grid", "out"),
        ),
        "d13/div-other": (
            T,
            "FSL-159: -div by an image of another size",
            _r(F3, "-div", "@other", "out"),
        ),
        "d13/rem-other": (
            T,
            "FSL-159: -rem by a smaller image",
            _r(F3, "-rem", "@smallint", "out"),
        ),
        "d13/max-other": (T, "FSL-159: -max with a larger image", _r(F3, "-max", "@other", "out")),
        "d13/min-small": (
            T,
            "FSL-159: -min with a smaller image (FSL reads past its end)",
            _r(F3, "-min", "@small", "out", undefined="FSL-159"),
        ),
    }
    c.update(_filter_cases())
    return c


def _filter_cases() -> dict[str, tuple[str, str, Run]]:
    """Sub-task 2: the kernels, morphological filters and convolutions (one case per path)."""
    S, T = "smoke", "standard"
    F3 = "@f3"
    return {
        # morphfilter, the default 3x3x3 box
        "morph/dilF": (S, "-dilF (maximum filter, default kernel)", _r(F3, "-dilF", "out")),
        "morph/dilF-nan": (T, "-dilF with NaN voxels (left out)", _r("@nan3", "-dilF", "out")),
        "morph/dilM": (T, "-dilM (mean dilation)", _r("@sparse3", "-dilM", "out")),
        "morph/dilM-nan": (T, "-dilM with NaN neighbours", _r("@sparsenan3", "-dilM", "out")),
        "morph/dilD": (T, "-dilD (modal dilation)", _r("@sparse3", "-dilD", "out")),
        "morph/dilD-short": (
            T,
            "-dilD of labels under -dt short",
            _r("-dt", "short", "@lab3", "-dilD", "out", "-odt", "short"),
        ),
        "morph/ero": (T, "-ero (zeroing next to zeros)", _r("@blob3", "-ero", "out")),
        "morph/eroF": (T, "-eroF (minimum filter)", _r(F3, "-eroF", "out")),
        "morph/eroF-nan": (T, "-eroF with NaN voxels", _r("@nan3", "-eroF", "out")),
        "morph/fmedian": (T, "-fmedian", _r(F3, "-fmedian", "out")),
        "morph/fmedian-2D": (T, "-kernel 2D -fmedian", _r(F3, "-kernel", "2D", "-fmedian", "out")),
        "morph/fmedian-char": (
            T,
            "-fmedian under -dt char",
            _r("-dt", "char", "@u8", "-fmedian", "out", "-odt", "char"),
        ),
        "morph/eroF-double": (T, "-eroF of a double image", _r("@d3", "-eroF", "out")),
        "morph/sphere-4d": (
            T,
            "-kernel sphere 4 -dilF on each volume of a 4D image",
            _r("@f4", "-kernel", "sphere", "4", "-dilF", "out"),
        ),
        "morph/boxv-even": (
            T,
            "-kernel boxv 2: the off-centre warning",
            _r(F3, "-kernel", "boxv", "2", "-dilF", "out"),
        ),
        "morph/boxv3": (
            T,
            "-kernel boxv3 3 1 5 -fmedian",
            _r(F3, "-kernel", "boxv3", "3", "1", "5", "-fmedian", "out"),
        ),
        "morph/box-mm": (
            T,
            "-kernel box 4 (mm, anisotropic voxels) -eroF",
            _r("@odd3", "-kernel", "box", "4", "-eroF", "out"),
        ),
        "morph/file": (
            T,
            "-kernel file (values around 0.5) -dilF",
            _r(F3, "-kernel", "file", "@kern3", "-dilF", "out"),
        ),
        "morph/file-even": (
            T,
            "-kernel file of even size -dilM",
            _r("@sparse3", "-kernel", "file", "@kern_even", "-dilM", "out"),
        ),
        "morph/file-4d": (
            T,
            "-kernel file of a 4D image (the first volume, a warning)",
            _r(F3, "-kernel", "file", "@kern4d", "-eroF", "out"),
        ),
        "morph/file-inf": (
            T,
            "-kernel file with an infinite value (Armadillo's bounds check)",
            _r(F3, "-kernel", "file", "@kern_inf", "-dilF", "out"),
        ),
        "morph/2d-ero": (
            T,
            "-kernel 2D -ero of a 2D image",
            _r("@two2d", "-kernel", "2D", "-ero", "out"),
        ),
        "morph/2d-dilD": (T, "-dilD of a 2D image", _r("@two2d", "-dilD", "out")),
        "dilall/3d": (T, "-dilall", _r("@sparse3", "-dilall", "out")),
        "dilall/4d": (
            T,
            "-thr 10 -dilall of a 4D image",
            _r("@f4", "-thr", "10", "-dilall", "out"),
        ),
        "dilall/int": (
            T,
            "-dilall under -dt int",
            _r("-dt", "int", "@lab3", "-dilall", "out", "-odt", "int"),
        ),
        "dilall/nan": (T, "-dilall with NaN voxels", _r("@sparsenan3", "-dilall", "out")),
        # convolutions
        "conv/s": (S, "-s 2 (separable Gaussian)", _r(F3, "-s", "2", "out")),
        "conv/s-odd": (T, "-s 1.5 of an odd, anisotropic image", _r("@odd3", "-s", "1.5", "out")),
        "conv/s-4d": (T, "-s 1 of a 4D image", _r("@f4", "-s", "1", "out")),
        "conv/s-2d": (T, "-s 2 of a 2D image", _r("@two2d", "-s", "2", "out")),
        "conv/s-nan": (T, "-s 1 with NaN voxels", _r("@nan3", "-s", "1", "out")),
        "conv/fmean-default": (
            T,
            "-fmean with the default kernel (not separable)",
            _r(F3, "-fmean", "out"),
        ),
        "conv/fmean-box": (
            T,
            "-kernel box 5 -fmean (separable)",
            _r("@odd3", "-kernel", "box", "5", "-fmean", "out"),
        ),
        "conv/fmean-boxv-even": (
            T,
            "-kernel boxv 4 -fmean (separable, even: warnings)",
            _r(F3, "-kernel", "boxv", "4", "-fmean", "out"),
        ),
        "conv/fmean-sphere": (
            T,
            "-kernel sphere 3 -fmean (direct sum)",
            _r("@odd3", "-kernel", "sphere", "3", "-fmean", "out"),
        ),
        "conv/fmean-sphere-nan": (
            T,
            "-kernel sphere 4 -fmean with NaN voxels",
            _r("@nan3", "-kernel", "sphere", "4", "-fmean", "out"),
        ),
        "conv/fmean-double": (
            T,
            "-kernel sphere 4 -fmean of a double image",
            _r("@d3", "-kernel", "sphere", "4", "-fmean", "out"),
        ),
        "conv/fmean-file": (
            T,
            "-kernel file -fmean (not separable)",
            _r(F3, "-kernel", "file", "@kern3", "-fmean", "out"),
        ),
        "conv/fmean-file-even": (
            T,
            "-kernel file of even size -fmean (warnings)",
            _r(F3, "-kernel", "file", "@kern_even", "-fmean", "out"),
        ),
        "conv/fmean-2D": (T, "-kernel 2D -fmean", _r(F3, "-kernel", "2D", "-fmean", "out")),
        "conv/separable-kept": (
            T,
            "-kernel gauss 1 -kernel 3D -fmean (separable kept from gauss)",
            _r(F3, "-kernel", "gauss", "1", "-kernel", "3D", "-fmean", "out"),
        ),
        "conv/fmeanu": (
            T,
            "-kernel sphere 5 -fmeanu",
            _r(F3, "-kernel", "sphere", "5", "-fmeanu", "out"),
        ),
        "conv/fmeanu-gauss": (
            T,
            "-kernel gauss 2 -fmeanu (not separable)",
            _r(F3, "-kernel", "gauss", "2", "-fmeanu", "out"),
        ),
        "conv/fmeanu-char": (
            T,
            "-kernel sphere 3 -fmeanu under -dt char",
            _r("-dt", "char", "@u8", "-kernel", "sphere", "3", "-fmeanu", "out", "-odt", "char"),
        ),
        "conv/fmean-int": (
            T,
            "-kernel boxv 3 -fmean under -dt int (separable)",
            _r("-dt", "int", "@s3", "-kernel", "boxv", "3", "-fmean", "out"),
        ),
        "conv/kernel-no-output": (
            T,
            "-kernel box 3 without an output name",
            _r(F3, "-kernel", "box", "3"),
        ),
        "conv/kernel-missing-size": (
            T,
            "-kernel box with the output name as its size: no output name left",
            _r(F3, "-kernel", "box", "out"),
        ),
        "morph/file-inf2": (
            T,
            "-kernel file with an infinite value and none at 0.5: more values than the vector holds",
            _r(F3, "-kernel", "file", "@kern_inf2", "-dilF", "out"),
        ),
        # subsampling, edges, holes, normalisation
        "sub/subsamp2": (T, "-subsamp2", _r(F3, "-subsamp2", "out")),
        "sub/subsamp2offc": (T, "-subsamp2offc", _r(F3, "-subsamp2offc", "out")),
        "sub/subsamp2-short": (
            T,
            "-subsamp2 under -dt short",
            _r("-dt", "short", "@s3", "-subsamp2", "out", "-odt", "short"),
        ),
        "sub/subsamp2offc-int": (
            T,
            "-subsamp2offc under -dt int",
            _r("-dt", "int", "@s3", "-subsamp2offc", "out"),
        ),
        "sub/subsamp2offc-double": (
            T,
            "-subsamp2offc of a double image",
            _r("@d3", "-subsamp2offc", "out"),
        ),
        "sub/subsamp2-noform": (
            T,
            "-subsamp2 without sform or qform",
            _r("@noform", "-subsamp2", "out"),
        ),
        "sub/subsamp2-nan": (T, "-subsamp2 with NaN voxels", _r("@nan3", "-subsamp2", "out")),
        "edge/3d": (T, "-edge", _r(F3, "-edge", "out")),
        "edge/4d": (T, "-edge of a 4D image", _r("@f4", "-edge", "out")),
        "edge/2d": (T, "-edge of a 2D image", _r("@two2d", "-edge", "out")),
        "edge/short": (T, "-edge under -dt short", _r("-dt", "short", "@s3", "-edge", "out")),
        "edge/nan": (T, "-edge with NaN voxels", _r("@nan3", "-edge", "out")),
        "edge/aniso": (T, "-edge with anisotropic voxels", _r("@odd3", "-edge", "out")),
        "dog/3d": (T, "-dog_edge 1 1.6", _r(F3, "-dog_edge", "1", "1.6", "out")),
        "dog/aniso": (
            T,
            "-dog_edge 0.5 2 of an odd image",
            _r("@odd3", "-dog_edge", "0.5", "2", "out"),
        ),
        "dog/double": (T, "-dog_edge of a double image", _r("@d3", "-dog_edge", "1", "1.6", "out")),
        "dog/int": (
            T,
            "-dog_edge under -dt int",
            _r("-dt", "int", "@s3", "-dog_edge", "1", "1.6", "out"),
        ),
        "dog/radius0": (
            T,
            "-dog_edge 0.1 0.2 (radius 0)",
            _r(F3, "-dog_edge", "0.1", "0.2", "out"),
        ),
        "fillh/6": (T, "-fillh", _r("@holes3", "-fillh", "out")),
        "fillh/26": (T, "-fillh26", _r("@holes3", "-fillh26", "out")),
        "fillh/blob": (T, "-fillh of a blob at the edge", _r("@blob3", "-fillh", "out")),
        "fillh/short": (
            T,
            "-fillh under -dt short",
            _r("-dt", "short", "@holes3", "-fillh", "out"),
        ),
        "fillh/4d": (T, "-fillh26 of a 4D mask", _r("@m4", "-fillh26", "out")),
        "norm/inm": (T, "-inm 100 of a 4D image", _r("@f4", "-inm", "100", "out")),
        "norm/ing": (T, "-ing 100 of a 4D image", _r("@f4", "-ing", "100", "out")),
        "norm/inm-sparse": (T, "-inm 100 of a sparse image", _r("@sparse3", "-inm", "100", "out")),
        "norm/inm-empty": (
            T,
            "-inm of an image of zeros (an empty mask: NaN)",
            _r("@sparse3", "-mul", "0", "-inm", "100", "out"),
        ),
        "norm/ing-int": (T, "-ing under -dt int", _r("-dt", "int", "@s3", "-ing", "1000", "out")),
        "norm/inm-nan": (T, "-inm with NaN voxels", _r("@nan3", "-inm", "100", "out")),
        "tensor/decomp": (T, "-tensor_decomp", _r("@tensor", "-tensor_decomp", "out")),
        "tensor/chain": (
            T,
            "-tensor_decomp, then the FA image multiplied by 2",
            _r("@tensor", "-tensor_decomp", "-mul", "2", "out"),
        ),
        "tensor/double": (
            T,
            "-tensor_decomp under -dt double, an output name with its extension",
            _r("-dt", "double", "@tensor", "-tensor_decomp", "out.nii.gz"),
        ),
        "d13/tensor-nan": (
            T,
            "FSL-166: -tensor_decomp with a NaN tensor",
            _r("@tensornan", "-tensor_decomp", "out", fixed="FSL-166"),
        ),
        "tensor/not-a-tensor": (
            T,
            "-tensor_decomp of a 3-volume image",
            _r("@f4", "-tensor_decomp", "out"),
        ),
        # D13
        "d13/kernel-boxv3-null": (
            T,
            "FSL-158: -kernel boxv3 without its sizes (reads the null pointer)",
            _r(F3, "-kernel", "boxv3", "3", "out"),
        ),
        "d13/subsamp2-4d": (
            T,
            "FSL-163: -subsamp2 of a 4D image",
            _r("@f4", "-subsamp2", "out", fixed="FSL-163"),
        ),
        "d13/subsamp2offc-4d-odd": (
            T,
            "FSL-163: -subsamp2offc of a 4D image of odd sizes",
            _r("@f4", "-subsamp2offc", "out", fixed="FSL-163"),
        ),
        "d13/subsamp2offc-4d-even": (
            T,
            "FSL-163: -subsamp2offc of a 4D image of even sizes (the first volume)",
            _r("@even4", "-subsamp2offc", "out"),
        ),
        "d13/dog-4d": (
            T,
            "FSL-164: -dog_edge of a 4D image",
            _r("@f4", "-dog_edge", "1", "1.6", "out"),
        ),
        "d13/dog-negative": (
            T,
            "FSL-161: -dog_edge -1 -2 (a kernel without voxels)",
            _r(F3, "-dog_edge", "-1", "-2", "out"),
        ),
        "d13/fillh-2d": (
            T,
            "FSL-165: -fillh of a 2D image",
            _r("@blob2d", "-fillh", "out", fixed="FSL-165"),
        ),
        "d13/s-zero": (T, "FSL-160: -s 0", _r(F3, "-s", "0", "out")),
        "d13/gauss-zero": (
            T,
            "FSL-160: -kernel gauss 0 -fmean",
            _r(F3, "-kernel", "gauss", "0", "-fmean", "out"),
        ),
        "d13/gauss-zero-dilF": (
            T,
            "FSL-160: -kernel gauss 0 -dilF (no change: no notice)",
            _r(F3, "-kernel", "gauss", "0", "-dilF", "out"),
        ),
        "d13/kernel-boxv0": (
            T,
            "FSL-161: -kernel boxv 0 -dilF",
            _r(F3, "-kernel", "boxv", "0", "-dilF", "out"),
        ),
        "d13/kernel-negative": (
            T,
            "FSL-161: -kernel boxv -3 -dilF",
            _r(F3, "-kernel", "boxv", "-3", "-dilF", "out"),
        ),
        "d13/kernel-unknown": (
            T,
            "FSL-161: -kernel foo 3 -fmean",
            _r(F3, "-kernel", "foo", "3", "-fmean", "out"),
        ),
        "d13/kernel-unknown-dilM": (
            T,
            "FSL-161: -kernel foo 3 -dilM",
            _r(F3, "-kernel", "foo", "3", "-dilM", "out"),
        ),
        "d13/gauss-negative": (
            T,
            "FSL-161: -kernel gauss -1 -fmean",
            _r(F3, "-kernel", "gauss", "-1", "-fmean", "out"),
        ),
        "d13/s-negative": (T, "FSL-161: -s -1", _r(F3, "-s", "-1", "out")),
        "d13/fmedian-nan": (
            T,
            "FSL-162: -fmedian with NaN voxels",
            _r("@nan3", "-fmedian", "out", fixed="FSL-162"),
        ),
        "d13/dilD-nan": (
            T,
            "FSL-162: -dilD with a NaN next to a zero voxel",
            _r("@sparsenan3", "-dilD", "out", fixed="FSL-162"),
        ),
        "d13/fmean-int-trap": (
            T,
            "FSL-156: -kernel sphere 5 -fmean under -dt int (an integer division by zero)",
            _r("-dt", "int", "@s3", "-kernel", "sphere", "5", "-fmean", "out"),
        ),
    }


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    seen: set[tuple[str, str]] = set()
    for run, node in fmriprep_calls():
        kind = node.parent.name
        t = "standard" if run in STANDARD_RUNS and (run, kind) not in seen else "full"
        if run == STANDARD_RUNS[0] and not seen:
            t = "smoke"
        seen.add((run, kind))
        if in_tier(t, tier):
            rel = node.relative_to(node.parents[4])
            out.append(
                Case(
                    f"fmriprep/{run}/{rel.parts[0]}/{kind}",
                    "fmriprep",
                    "fMRIPrep's combine_masks call (fslmaths <mask> -mul <mask> <out>)",
                    ("fmriprep", (run, str(node))),
                )
            )
    for name, (t, desc, _run) in _cases().items():
        if in_tier(t, tier):
            out.append(Case(name, name.split("/")[0], desc, ("run", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        _run_case(payload, checks)
    else:
        _run_case(_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="fslmaths replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl fslmaths` (crate larmorx-nc-avwutils, translated from "
        f"FSL {fsl_versions.NEWEST}'s source)",
        reference=f"fslmaths from FSL {fsl_versions.NEWEST} (fsl-avwutils 2209.6, fsl-newimage "
        "2601.0, fsl-miscmaths 2412.6, fsl-cprob 2111.0; the conda build at `~/fsl-6.0.7.23`, as "
        "built: glibc 2.35's maths)",
        thresholds=[
            ("Exit status", "the same (a crash: the replica exits 1)"),
            (
                "Standard output and error",
                "identical (the run directory and FSL's program path replaced; the replica's "
                "`larmorx-nc:` lines left out)",
            ),
            ("Files written", "the same names"),
            ("Every file", "bit-identical (images decompressed; `descrip` left out)"),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), "
            "compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked "
            "against the reproduced run. The synthetic inputs are made once "
            "(`oracles/inputs/fsl-maths-replica/`); fMRIPrep's masks come from the oracle runs' "
            "working directories."
        ),
    )
