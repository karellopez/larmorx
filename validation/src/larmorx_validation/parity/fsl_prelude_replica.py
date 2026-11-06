# SPDX-License-Identifier: Apache-2.0
"""Parity of the PRELUDE replica (``larmorx-nc fsl prelude``, FSL Licence) with FSL 6.0.7.23's
``prelude``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own unwrapping calls (sdcflows' phase-difference workflow,
  ``fmap_preproc_wf/<estimator>/phdiff_wf/prelude``: ``prelude --abs=<magnitude> --mask=<mask>
  --phase=<phase difference in rad, 0 to 2pi> --unwrap=<out>``) of the oracle runs: the inputs
  from the runs' working directories, copied into the run directory under their names;
  fMRIPrep's own outputs (its container's FSL) compared too, as a report, and the Python API
  (``lx.fsl.prelude``) on the same inputs, whose unwrapped phase must be FSL's;
- **opt**, **out**, **err**: every option of prelude (complex or phase and magnitude input, the
  mask and its threshold, 3D and 2D labels and unwrapping, the volume range, the saved mask,
  labels and raw phase, the phase ramps, the messages), the output types, the messages and
  failures, on synthetic phase images with known wraps (``oracles/inputs/fsl-prelude-replica``);
- **panel**: the data panel's phase-difference field maps that no fMRIPrep run unwrapped (no
  BOLD names them: ds000256's child, ds004958's GRE map, from ``larmorx-testdata``), made into
  prelude's input as sdcflows makes it (``au2rads``: the phase difference rescaled from its
  minimum and maximum to 0 to 2 pi, float32), with the first magnitude: prelude's own mask, and
  ``-s`` or ``--removeramps`` with the saved outputs; the raw scanner values given as they are
  (FSL's range error);
- **difftest**: random cases (``larmorx_validation.difftest.tools.fsl_prelude``), a :class:`Run`
  of their own (``direct``), inputs named ``@dt:<key>``; where the replica refuses in
  ``reproduce`` because FSL's sort would read memory it does not own (FSL-268, FSL-269), the
  defect is named to the random-testing runner (the verdict ``undefined``).

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every original run goes through the oracle-output cache: prelude is single-threaded and
draws no random numbers.
"""

from __future__ import annotations

import dataclasses
import os
import re
import shlex
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

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
    _differing_values,
    _same_streams,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import BOTH_ERROR, Case, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-prelude-replica"
PROGRAM = "prelude"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: fMRIPrep's prelude calls of the standard tier (the others: the full tier).
STANDARD_RUNS = (
    "ds005454-sub-16-noreconall",
    "ds002603-sub-01-noreconall",
    "ds002422-sub-02-noreconall",
    "ds001600-sub-1-noreconall",
)


# ------------------------------------------------------------------------------------------------
# The programs


def oracle_run(
    args: list[str], *, cwd: Path, settings: dict[str, str | None], never: str = ""
) -> Any:
    """FSL's prelude with ``args`` in ``cwd``, through the oracle-output cache: every file it
    writes is in ``cwd`` and every file there counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / PROGRAM), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl").but_never(never),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=3600,
    )


def _oracle(args: list[str], d: Path, settings: dict[str, str | None], never: str) -> Completed:
    p = oracle_run(args, cwd=d, settings=settings, never=never)
    code = -9 if p.returncode is None else p.returncode
    return Completed(code, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# fMRIPrep's calls


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, node directory)`` of every prelude node of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for run_dir in sorted((ws / FMRIPREP_RUNS).glob("*")):
        for node in sorted(
            run_dir.glob("work/fmriprep_25_2_wf/*/fmap_preproc_wf/*/phdiff_wf/prelude")
        ):
            if (node / "command.txt").is_file():
                out.append((run_dir.name, node))
    return out


def _host_path(run: str, p: str) -> Path:
    ws = workspace()
    assert ws is not None
    return ws / FMRIPREP_RUNS / run / p.lstrip("/")


def fmriprep_args(run: str, node: Path) -> tuple[list[str], dict[str, Path]]:
    """The node's arguments with every input named by its file name in the run directory, and
    the inputs to copy there (name -> host path)."""
    words = shlex.split((node / "command.txt").read_text(encoding="utf-8"))[1:]
    args, files = [], {}
    for w in words:
        key, sep, value = w.partition("=")
        if sep and key in ("--abs", "--mask", "--phase"):
            src = _host_path(run, value)
            files[src.name] = src
            args.append(f"{key}={src.name}")
        elif sep and key == "--unwrap":
            args.append(f"{key}={Path(value).name}")
        else:
            args.append(w)
    return args, files


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node_s = payload
    node = Path(node_s)
    args, files = fmriprep_args(run, node)
    if not all(p.is_file() for p in files.values()):
        raise Outcome("skipped", f"{run}: the inputs of fMRIPrep's prelude node are missing")
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-prelude-fp_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-prelude-fp_") as b_,
    ):
        a, b = Path(a_), Path(b_)

        def prepare(d: Path) -> None:
            for name, src in files.items():
                shutil.copy(src, d / name)

        prepare(a)
        prepare(b)
        oracle = _oracle(args, a, {}, "")
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        out = next(x.partition("=")[2] for x in args if x.startswith("--unwrap="))
        theirs = node / out
        ours = b / out
        if theirs.is_file() and ours.is_file():
            same = _same_streams(theirs, ours, blank_descrip=True)
            checks(
                "fMRIPrep's own output (its container's FSL) against the replica's (reported)",
                True,
                metric="identical",
                value="yes" if same else "no",
                threshold="reported",
                detail="" if same else _differing_values(theirs, ours),
            )
        if oracle.code == 0 and replica.code == 0:
            fix_pass(checks, PROGRAM, prepare, args, {}, b, replica)
            _python_check(checks, args, files, a)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


#: The options of fMRIPrep's call that ``lx.fsl.prelude`` takes (:func:`_python_check`).
_PYTHON_OPTIONS = {"--abs", "--mask", "--phase", "--unwrap"}


def _python_check(checks: CheckList, args: list[str], files: dict[str, Path], fsl: Path) -> None:
    """``lx.fsl.prelude`` (the Python API, ``implementation="replica"``, ``reproduce``) on the
    call's inputs: its unwrapped phase must be FSL's, bit for bit."""
    import warnings

    import nibabel as nib

    import larmorx as lx
    from larmorx_validation.parity.fsl_bet import process_environment

    opts = dict(a.partition("=")[::2] for a in args)
    if not set(opts) <= _PYTHON_OPTIONS or not {"--abs", "--phase", "--unwrap"} <= set(opts):
        return
    env = dict(os.environ, LARMORX_NC_BIN=str(replica_binary()))
    with process_environment(env), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        res = lx.fsl.prelude(
            str(files[opts["--phase"]]),
            str(files[opts["--abs"]]),
            mask=str(files[opts["--mask"]]) if "--mask" in opts else None,
            upstream_defects="reproduce",
            implementation="replica",
        )
    ours = np.asarray(res.unwrapped.data)
    theirs = np.asarray(nib.load(fsl / opts["--unwrap"]).dataobj)
    same = (
        ours.dtype == theirs.dtype == np.float32
        and ours.shape == theirs.shape
        and np.array_equal(ours.view(np.uint32), theirs.view(np.uint32))
    )
    checks(
        "Python API (lx.fsl.prelude): the unwrapped phase",
        same,
        detail="FSL's, bit for bit" if same else "differs from FSL's",
    )


# ------------------------------------------------------------------------------------------------
# Derived inputs

#: The derived inputs' recipe version: a change makes them again (a new directory).
INPUTS_VERSION = "3"


def _atomic_dir(target: Path, make: Any) -> Path:
    """``target`` made once by ``make(tmp)`` in a private directory renamed into place."""
    if (target / "DONE").is_file():
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix=f".{target.name}_", dir=target.parent))
    make(tmp)
    (tmp / "DONE").write_text(f"made by the {SUITE} suite\n", encoding="utf-8")
    try:
        tmp.rename(target)
    except OSError:
        shutil.rmtree(tmp, ignore_errors=True)
    return target


def _grid(shape: tuple[int, ...]) -> list[np.ndarray]:
    return np.meshgrid(*(np.arange(n) - (n - 1) / 2.0 for n in shape[:3]), indexing="ij")


def _true_phase(shape: tuple[int, int, int], seed: int, scale: float = 1.0) -> np.ndarray:
    """A smooth phase spanning several cycles (a linear ramp and a quadratic bowl), with noise."""
    rng = np.random.default_rng(seed)
    i, j, k = _grid(shape)
    smooth = 0.32 * i + 0.21 * j + 0.45 * k + 0.018 * (i * i + j * j) - 0.011 * i * j
    return scale * smooth + rng.normal(0.0, 0.12, shape)


def _magnitude(shape: tuple[int, int, int], seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed + 1000)
    i, j, k = _grid(shape)
    nx, ny, nz = shape
    r2 = (i / (0.42 * nx)) ** 2 + (j / (0.42 * ny)) ** 2 + (k / (0.48 * nz)) ** 2
    return 1000.0 * np.exp(-1.5 * r2) + rng.uniform(0.0, 40.0, shape)


def _wrapped(x: np.ndarray) -> np.ndarray:
    return ((x + np.pi) % (2.0 * np.pi) - np.pi).astype(np.float32)


#: An affine whose determinant is negative (FSL's radiological order: no swap in memory).
RADIOLOGICAL = np.diag([-2.0, 2.0, 2.5, 1.0])
#: A RAS affine (positive determinant: FSL swaps the image in memory and back on saving).
NEUROLOGICAL = np.diag([2.0, 2.0, 2.5, 1.0])


def _save(path: Path, data: np.ndarray, affine: np.ndarray = RADIOLOGICAL, **kw: Any) -> None:
    import nibabel as nib

    hdr = nib.Nifti1Header(endianness=kw.pop("endianness", "<"))
    img = nib.Nifti1Image(data, affine, hdr)
    img.header.set_data_dtype(kw.pop("dtype", data.dtype))
    if "slope" in kw:
        img.header.set_slope_inter(kw.pop("slope"), 0.0)
    img.header.set_xyzt_units("mm", "sec")
    if data.ndim == 4:
        img.header["pixdim"][4] = 2.0
    nib.save(img, path)


def _make_inputs(d: Path) -> None:
    small, big, one = (40, 36, 12), (72, 70, 8), (40, 36, 1)
    for name, shape, seed, aff in (
        ("s", small, 1, RADIOLOGICAL),
        ("b", big, 2, RADIOLOGICAL),
        ("o", one, 3, RADIOLOGICAL),
        ("n", small, 4, NEUROLOGICAL),
    ):
        true = _true_phase(shape, seed, 1.0 if name != "o" else 2.0)
        mag = _magnitude(shape, seed).astype(np.float32)
        _save(d / f"{name}_ph.nii.gz", _wrapped(true), aff)
        _save(d / f"{name}_ph2pi.nii.gz", (_wrapped(true) + np.float32(np.pi)), aff)
        _save(d / f"{name}_mag.nii.gz", mag, aff)
        _save(d / f"{name}_mask.nii.gz", (mag > 250).astype(np.float32), aff)
        if name == "s":
            c = (mag * np.exp(1j * true)).astype(np.complex64)
            _save(d / "s_cplx.nii.gz", c)
            _save(d / "s_cplx_be.nii", c.astype(">c8"), endianness=">")
            _save(d / "s_cplx128.nii.gz", c.astype(np.complex128))
            _save(d / "s_real.nii.gz", mag)
            _save(d / "s_real16.nii.gz", mag.astype(np.int16))
            ph16 = np.round(_wrapped(true) / (np.pi / 30000)).astype(np.int16)
            _save(d / "s_ph16.nii.gz", ph16, slope=np.pi / 30000)
            nanmag = mag.copy()
            nanmag[5:8, 6:9, 4] = np.nan
            _save(d / "s_nanmag.nii.gz", nanmag)
            nanfirst = mag.copy()
            nanfirst[0, 0, 0] = np.nan
            _save(d / "s_nanfirst.nii.gz", nanfirst)
            nanph = _wrapped(true)
            nanph[18:20, 17:19, 6] = np.nan
            _save(d / "s_nanph.nii.gz", nanph)
            far = _wrapped(true)
            far[20, 18, 6] = 3.5 * np.pi
            _save(d / "s_range.nii.gz", far)
            _save(d / "s_mag_other.nii.gz", mag[:, :, :10])
            _save(
                d / "s_mag_larger.nii.gz",
                np.pad(mag, ((0, 4), (0, 3), (0, 2)), constant_values=5.0),
            )
            # NaN phases at the first voxels of the mask in FSL's scan order (z, y, x): their
            # label is 1, merged with the others through NaN constraints
            m = mag > 250
            first = np.flatnonzero(m.ravel(order="F"))[:3]
            corner = _wrapped(true).ravel(order="F")
            corner[first] = np.nan
            _save(d / "s_nanph_first.nii.gz", corner.reshape(shape, order="F"))
            # a NaN phase inside a region of the first phase bin (below -3pi/4), near the centre:
            # the region shares the NaN's label
            w = _wrapped(true)
            i, j, k = _grid(shape)
            score = np.where(m & (w < -2.6), -(i * i + j * j + k * k), -np.inf)
            at = np.unravel_index(int(np.argmax(score)), shape)
            inbin = w.copy()
            inbin[at] = np.nan
            _save(d / "s_nanph_bin1.nii.gz", inbin)
        if name == "b":
            gap = (mag > 250).astype(np.float32)
            gap[:, :, 4] = 0.0
            _save(d / "b_mask_gap.nii.gz", gap)
            _save(d / "b_mask_short.nii.gz", (mag[:, :, :6] > 250).astype(np.float32))
    # a series of three volumes: each its own phase; a mask series of two volumes
    shape = small
    ph = np.stack([_wrapped(_true_phase(shape, 10 + t, 0.6 + 0.4 * t)) for t in range(3)], -1)
    mag = np.stack([_magnitude(shape, 10 + t) for t in range(3)], -1).astype(np.float32)
    _save(d / "t_ph.nii.gz", ph)
    _save(d / "t_mag.nii.gz", mag)
    _save(d / "t_mask2.nii.gz", (mag[..., :2] > 250).astype(np.float32))


def inputs() -> Path:
    ws = workspace()
    base = (ws / "oracles" / "inputs") if ws else Path(tempfile.gettempdir())
    return _atomic_dir(base / SUITE / f"v{INPUTS_VERSION}", _make_inputs)


#: The data panel's phase-difference field maps that no fMRIPrep oracle run unwrapped (no BOLD
#: names them): name -> (phase difference, first magnitude) in ``larmorx-testdata``.
PANEL_MAPS = {
    "ds000256": (
        "openneuro/ds000256/sub-CTS203/fmap/sub-CTS203_phasediff.nii.gz",
        "openneuro/ds000256/sub-CTS203/fmap/sub-CTS203_magnitude1.nii.gz",
    ),
    "ds004958": (
        "openneuro/ds004958/sub-fsm64ps/fmap/sub-fsm64ps_acq-gre_phasediff.nii.gz",
        "openneuro/ds004958/sub-fsm64ps/fmap/sub-fsm64ps_acq-gre_magnitude1.nii.gz",
    ),
}
#: The panel inputs' recipe version.
PANEL_VERSION = "1"


def au2rads(src: Path, dst: Path) -> None:
    """A phase map in arbitrary units as sdcflows makes prelude's input of it (``au2rads``):
    rescaled from its minimum and maximum to [0, 2 pi] in double precision, rounded to float32
    and clipped; the header kept (float32, millimetres)."""
    import nibabel as nib

    im = nib.load(src)
    data = np.asarray(im.dataobj, dtype=np.float64)
    data = (data - data.min()) * (2 * np.pi / (data.max() - data.min()))
    data = np.clip(np.float32(data), 0.0, 2 * np.pi)
    hdr = im.header.copy()
    hdr.set_data_dtype(np.float32)
    hdr.set_xyzt_units("mm")
    nib.Nifti1Image(data, None, hdr).to_filename(dst)


def _make_panel_inputs(d: Path) -> None:
    import larmorx_testdata as td

    for name, (phase, magnitude) in PANEL_MAPS.items():
        shutil.copy(td.get(phase), d / f"{name}_phasediff.nii.gz")
        shutil.copy(td.get(magnitude), d / f"{name}_magnitude1.nii.gz")
        au2rads(d / f"{name}_phasediff.nii.gz", d / f"{name}_phasediff_rads.nii.gz")


def panel_inputs() -> Path | None:
    """The panel's derived inputs (``oracles/inputs/fsl-prelude-replica/panel-v1``), or ``None``
    without ``larmorx-testdata`` or its files."""
    try:
        import larmorx_testdata  # noqa: F401
    except ImportError:
        return None
    ws = workspace()
    base = (ws / "oracles" / "inputs") if ws else Path(tempfile.gettempdir())
    try:
        return _atomic_dir(base / SUITE / f"panel-v{PANEL_VERSION}", _make_panel_inputs)
    except Exception:  # a file the catalog cannot fetch here
        return None


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: the arguments (``@name`` a derived input, copied into the run directory under
    its name; the image's sibling files of a pair too), the environment."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True
    #: Why FSL's run is never cached.
    never: str = ""
    #: A case whose values FSL takes from memory it never wrote or does not own: the files'
    #: values are reported, not compared (D13 class (a)); the exit status, streams and file
    #: names are compared.
    undefined: str = ""
    #: The default refuses (rule B): the replica's ``fix`` run exits 1 naming this entry.
    fix_refuses: str = ""
    #: The replica refuses in both modes, naming this entry (FSL reads memory it does not own).
    refused: str = ""
    #: A random case (differential random testing): a class (a) refusal of the replica's
    #: ``reproduce`` run is named to the random-testing runner (:func:`_note_memory_refusal`).
    random: bool = False


#: The replica's refusal where FSL would read memory it does not own (``reproduce`` included).
_MEMORY_REFUSAL = re.compile(r"\((FSL-\d+): FSL reads memory it does not own here\)")


def _note_memory_refusal(replica: Completed) -> None:
    """A random case the replica refused in ``reproduce`` because FSL's result there depends on
    memory it does not own (a NaN sort, FSL-268 and FSL-269): the defect is named to the
    random-testing runner, whose verdict is then ``undefined`` (not comparable), as for the
    defects larmorx names while reproducing in-process."""
    ids = _MEMORY_REFUSAL.findall(replica.stderr) if replica.code == 1 else []
    if ids:
        from larmorx_validation.difftest import runner

        runner.note_defects(*ids)


def _resolve(word: str, d: Path) -> str:
    key, sep, value = word.partition("=")
    if not sep:
        key, value = "", word
    if not value.startswith("@"):
        return word
    if value.startswith("@panel/"):
        base = panel_inputs()
        if base is None:
            raise Outcome("skipped", "the data panel's field maps (larmorx-testdata) are missing")
        src = base / value.removeprefix("@panel/")
        shutil.copy(src, d / src.name)
        return f"{key}{sep}{src.name}" if sep else src.name
    if value.startswith("@dt:"):
        # a random case's input (differential random testing), built from its recipe; a
        # pair's .img beside its .hdr
        from larmorx_validation.difftest import inputs as dt_inputs

        src = dt_inputs.path(value[1:])
        for p in (src, src.with_suffix(".img") if src.suffix == ".hdr" else None):
            if p is not None and p.is_file() and not (d / p.name).exists():
                shutil.copy(p, d / p.name)
        return f"{key}{sep}{src.name}" if sep else src.name
    src = inputs() / value[1:]
    for p in src.parent.glob(src.name.split(".")[0] + ".*"):
        if not (d / p.name).exists():
            shutil.copy(p, d / p.name)
    return f"{key}{sep}{src.name}" if sep else src.name


def _prepare(run: Run, d: Path) -> list[str]:
    return [_resolve(a, d) for a in run.args]


def _fix_refuses(run: Run, checks: CheckList) -> None:
    with tempfile.TemporaryDirectory(prefix="lx-nc-prelude-fix_") as b_:
        b = Path(b_)
        args = _prepare(run, b)
        r = run_replica(PROGRAM, args, cwd=b, settings=dict(run.settings), mode=defects.FIX)
        refusal = next(
            (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
        )
        checks(
            f"the default refuses ({run.fix_refuses})",
            r.code == 1 and run.fix_refuses in refusal,
            detail=f"exit {r.code}: {refusal[:200]}",
        )


def _refused_case(run: Run, checks: CheckList) -> None:
    from larmorx_validation.parity.harness import EXPECTED

    with tempfile.TemporaryDirectory(prefix="lx-nc-prelude_") as a_:
        a = Path(a_)
        args = _prepare(run, a)
        oracle = _oracle(args, a, dict(run.settings), run.never)
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-nc-prelude_") as b_:
            b = Path(b_)
            args = _prepare(run, b)
            r = run_replica(PROGRAM, args, cwd=b, settings=dict(run.settings), mode=mode)
            refusal = next(
                (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
            )
            checks(
                f"the replica refuses ({mode})",
                r.code == 1 and run.refused in refusal,
                detail=f"exit {r.code}: {refusal[:200]}",
            )
    if all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[-160:]
        raise Outcome(
            EXPECTED,
            f"{run.refused}: refused in both modes; FSL exits {oracle.code}"
            + (f" ({tail})" if tail else ""),
        )


def _run_case(run: Run, checks: CheckList) -> None:
    if run.refused:
        _refused_case(run, checks)
        return
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-prelude_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-prelude_") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = _oracle(args, a, settings, run.never)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.random:
            _note_memory_refusal(replica)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if run.undefined:
            for i, c in enumerate(checks.checks):
                if c.name == "every file bit-identical" and not c.passed:
                    checks.checks[i] = dataclasses.replace(
                        c,
                        passed=True,
                        threshold="reported",
                        detail=f"values from memory ({run.undefined}): {c.detail}"[:400],
                    )
        if run.fix and replica.code == 0 and not run.fix_refuses:
            fix_pass(checks, PROGRAM, lambda t: _prepare(run, t), args, settings, b, replica)
    if run.fix_refuses:
        _fix_refuses(run, checks)
    if oracle.code != 0 and all(c.passed for c in checks.checks):
        tail = " ".join(oracle.stderr.split())[-200:]
        raise Outcome(BOTH_ERROR, f"prelude exits {oracle.code}; the same from the replica. {tail}")


def _option_cases() -> dict[str, tuple[str, str, Run]]:
    pa = ("-p", "@s_ph.nii.gz", "-a", "@s_mag.nii.gz")
    big = ("-p", "@b_ph.nii.gz", "-a", "@b_mag.nii.gz")
    ts = ("-p", "@t_ph.nii.gz", "-a", "@t_mag.nii.gz")
    o = ("-o", "out")
    c: dict[str, tuple[str, str, Run]] = {
        "opt/default": (
            "smoke",
            "phase and magnitude, the defaults (3D labels and unwrapping)",
            Run((*pa, *o)),
        ),
        "opt/twopi-mask": (
            "smoke",
            "a phase from 0 to 2pi (rewrapped) with a mask (fMRIPrep's form)",
            Run(
                (
                    "--abs=@s_mag.nii.gz",
                    "--mask=@s_mask.nii.gz",
                    "--phase=@s_ph2pi.nii.gz",
                    "--unwrap=out",
                )
            ),
        ),
        "opt/verbose": ("standard", "-v: every message", Run((*pa, *o, "-v"))),
        "opt/big-auto-2d-labels": (
            "smoke",
            "an image wider than 64 voxels: labels slice by slice, 3D unwrapping",
            Run((*big, *o, "-v")),
        ),
        "opt/big-force3d": ("standard", "-f on a wide image: 3D labels", Run((*big, *o, "-f"))),
        "opt/labelslices": ("standard", "--labelslices", Run((*pa, *o, "--labelslices"))),
        "opt/slices": (
            "standard",
            "-s: masks, labels and unwrapping slice by slice",
            Run((*pa, *o, "-s", "-v")),
        ),
        "opt/slices-force3d": ("standard", "-s -f: -f wins", Run((*pa, *o, "-s", "-f"))),
        "opt/n2": ("standard", "-n 2", Run((*pa, *o, "-n", "2"))),
        "opt/n3": ("standard", "-n 3", Run((*pa, *o, "-n", "3"))),
        "opt/n24": ("standard", "-n 24", Run((*pa, *o, "-n", "24"))),
        "opt/thresh": ("standard", "-t 300", Run((*pa, *o, "-t", "300"))),
        "opt/thresh-zero": ("standard", "-t 0: every voxel", Run((*pa, *o, "-t", "0"))),
        "opt/mask": ("standard", "-m", Run((*pa, *o, "-m", "@s_mask.nii.gz"))),
        "opt/mask-thresh": (
            "standard",
            "-m with -t 0.2",
            Run((*pa, *o, "-m", "@s_mask.nii.gz", "-t", "0.2")),
        ),
        "opt/series": ("standard", "three volumes", Run((*ts, *o))),
        "opt/series-mask2": (
            "standard",
            "three volumes, a mask of two (the last mask for the third)",
            Run((*ts, *o, "-m", "@t_mask2.nii.gz")),
        ),
        "opt/series-start-end": (
            "standard",
            "--start 1 --end 2",
            Run((*ts, *o, "--start=1", "--end=2")),
        ),
        "opt/series-start": ("standard", "--start 2", Run((*ts, *o, "--start=2"))),
        "opt/series-end": ("standard", "--end 0", Run((*ts, *o, "--end=0"))),
        "opt/series-saved": (
            "standard",
            "--savemask, -l and -r with three volumes (FSL-262)",
            Run((*ts, *o, "--savemask=m", "-l", "l", "-r", "r")),
        ),
        "opt/saved": (
            "smoke",
            "--savemask, -l and -r",
            Run((*pa, *o, "--savemask=m", "-l", "l", "-r", "r")),
        ),
        "opt/saved-slices": (
            "standard",
            "--savemask, -l, -r with -s",
            Run((*pa, *o, "-s", "--savemask=m", "-l", "l", "-r", "r")),
        ),
        "opt/removeramps": (
            "smoke",
            "--removeramps (FSL-261)",
            Run((*pa, *o, "--removeramps", "-r", "r")),
        ),
        "opt/removeramps-verbose": (
            "standard",
            "--removeramps -v (FSL-266: grot)",
            Run((*pa, *o, "--removeramps", "-v")),
        ),
        "opt/removeramps-slices": (
            "standard",
            "--removeramps -s",
            Run((*pa, *o, "--removeramps", "-s")),
        ),
        "opt/removeramps-one-slice": (
            "standard",
            "--removeramps on one slice (FSL-261)",
            Run(("-p", "@o_ph.nii.gz", "-a", "@o_mag.nii.gz", *o, "--removeramps")),
        ),
        "opt/one-slice": (
            "standard",
            "an image of one slice",
            Run(("-p", "@o_ph.nii.gz", "-a", "@o_mag.nii.gz", *o, "-l", "l")),
        ),
        "opt/neurological": (
            "standard",
            "a neurological image (swapped in memory)",
            Run(
                (
                    "-p",
                    "@n_ph.nii.gz",
                    "-a",
                    "@n_mag.nii.gz",
                    *o,
                    "--savemask=m",
                    "-l",
                    "l",
                    "-r",
                    "r",
                )
            ),
        ),
        "opt/int16-phase": (
            "standard",
            "an int16 phase with scl_slope",
            Run(("-p", "@s_ph16.nii.gz", "-a", "@s_mag.nii.gz", *o)),
        ),
        "opt/gap-slice": (
            "standard",
            "a mask with an empty slice between two parts, labels slice by slice (the largest "
            "component kept)",
            Run((*big, *o, "-m", "@b_mask_gap.nii.gz", "-l", "l")),
        ),
        "opt/complex": (
            "smoke",
            "-c: a complex64 image",
            Run(("-c", "@s_cplx.nii.gz", *o, "-r", "r", "-v")),
        ),
        "opt/complex-big-endian": (
            "standard",
            "-c: a big-endian complex64 image (FSL-264)",
            Run(("-c", "@s_cplx_be.nii", *o, "-r", "r")),
        ),
        "opt/complex128": (
            "standard",
            "-c: a complex128 image (FSL-265)",
            Run(("-c", "@s_cplx128.nii.gz", *o), undefined="FSL-265: past an empty buffer"),
        ),
        "opt/complex-real": (
            "standard",
            "-c: a float32 image (FSL-263)",
            Run(
                ("-c", "@s_real.nii.gz", *o),
                undefined="FSL-263: the imaginary part never written",
                fix_refuses="FSL-263",
            ),
        ),
        "opt/complex-int16": (
            "standard",
            "-c: an int16 image (FSL-263)",
            Run(
                ("-c", "@s_real16.nii.gz", *o),
                undefined="FSL-263: past the buffer",
                fix_refuses="FSL-263",
            ),
        ),
        "opt/nan-magnitude": (
            "standard",
            "NaN magnitudes (FSL-268)",
            Run(("-p", "@s_ph.nii.gz", "-a", "@s_nanmag.nii.gz", *o)),
        ),
        "opt/nan-first-magnitude": (
            "standard",
            "a NaN first magnitude voxel (FSL-268)",
            Run(("-p", "@s_ph.nii.gz", "-a", "@s_nanfirst.nii.gz", *o)),
        ),
        "opt/nan-phase": (
            "standard",
            "NaN phases inside the mask (FSL-269)",
            Run(("-p", "@s_nanph.nii.gz", "-a", "@s_mag.nii.gz", *o)),
        ),
        "opt/nan-phase-first": (
            "standard",
            "NaN phases at the mask's first voxels (FSL-269)",
            Run(
                (
                    "-p",
                    "@s_nanph_first.nii.gz",
                    "-a",
                    "@s_mag.nii.gz",
                    "-m",
                    "@s_mask.nii.gz",
                    *o,
                    "-l",
                    "l",
                )
            ),
        ),
        "opt/nan-phase-region": (
            "standard",
            "a NaN phase inside a region of the first phase bin (FSL-269)",
            Run(("-p", "@s_nanph_bin1.nii.gz", "-a", "@s_mag.nii.gz", "-m", "@s_mask.nii.gz", *o)),
        ),
        "opt/nan-phase-slices": (
            "standard",
            "NaN phases with -s (FSL-269)",
            Run(("-p", "@s_nanph.nii.gz", "-a", "@s_mag.nii.gz", *o, "-s")),
        ),
    }
    for t in ("NIFTI", "NIFTI_PAIR", "NIFTI2_GZ", "NIFTI_PAIR_GZ", "NIFTI2"):
        c[f"out/{t.lower()}"] = (
            "standard",
            f"FSLOUTPUTTYPE={t}",
            Run((*pa, *o, "-l", "l"), (("FSLOUTPUTTYPE", t),)),
        )
    c["out/invalid"] = (
        "standard",
        "FSLOUTPUTTYPE=bogus",
        Run((*pa, *o), (("FSLOUTPUTTYPE", "bogus"),)),
    )
    c["opt/long-names"] = (
        "standard",
        "the long option names (--phase, --abs, --out, --numphasesplit, --thresh, --labels, "
        "--rawphase, --verbose, --slices, --force3D)",
        Run(
            (
                "--phase=@s_ph.nii.gz",
                "--abs=@s_mag.nii.gz",
                "--out=out",
                "--numphasesplit=4",
                "--thresh=200",
                "--labels=l",
                "--rawphase=r",
                "--verbose",
                "--slices",
            )
        ),
    )
    c["opt/u-alias"] = (
        "standard",
        "-u (the output's other name) and grouped flags -fv",
        Run((*pa, "-u", "out", "-fv")),
    )
    c.update(_panel_cases())
    return c


def _panel_cases() -> dict[str, tuple[str, str, Run]]:
    """The data panel's two phase-difference maps no fMRIPrep run unwrapped (module doc)."""
    c: dict[str, tuple[str, str, Run]] = {}
    for name, extra, what in (
        ("ds000256", ("-s", "-v", "--savemask=m", "-l", "l"), "-s -v with the mask and labels"),
        ("ds004958", ("--removeramps", "-r", "r", "-v"), "--removeramps -v with the raw phase"),
    ):
        mag, rads = f"@panel/{name}_magnitude1.nii.gz", f"@panel/{name}_phasediff_rads.nii.gz"
        c[f"panel/{name}/rads"] = (
            "standard",
            f"the data panel's {name} phase difference in radians (sdcflows' au2rads) and its "
            "first magnitude: prelude's own mask",
            Run((f"--abs={mag}", f"--phase={rads}", "--unwrap=out")),
        )
        c[f"panel/{name}/options"] = (
            "standard",
            f"the data panel's {name} phase difference in radians: {what}",
            Run(("-a", mag, "-p", rads, "-o", "out", *extra)),
        )
    c["panel/ds000256/raw"] = (
        "standard",
        "the data panel's ds000256 phase difference in the scanner's units (FSL's range error)",
        Run(
            (
                "--abs=@panel/ds000256_magnitude1.nii.gz",
                "--phase=@panel/ds000256_phasediff.nii.gz",
                "--unwrap=out",
            )
        ),
    )
    return c


def _error_cases() -> dict[str, tuple[str, str, Run]]:
    pa = ("-p", "@s_ph.nii.gz", "-a", "@s_mag.nii.gz")
    ts = ("-p", "@t_ph.nii.gz", "-a", "@t_mag.nii.gz")
    o = ("-o", "out")
    return {
        "err/help": ("smoke", "-h", Run(("-h",))),
        "err/no-arguments": ("standard", "no arguments", Run(())),
        "err/no-output": ("standard", "no -o", Run(pa)),
        "err/complex-and-phase": ("standard", "-c with -p", Run(("-c", "@s_cplx.nii.gz", *pa, *o))),
        "err/phase-without-abs": ("standard", "-p without -a", Run(("-p", "@s_ph.nii.gz", *o))),
        "err/abs-without-phase": ("standard", "-a without -p", Run(("-a", "@s_mag.nii.gz", *o))),
        "err/n1": ("standard", "-n 1", Run((*pa, *o, "-n", "1"))),
        "err/unknown-option": ("standard", "an unknown option", Run((*pa, *o, "--bogus"))),
        "err/bad-number": ("standard", "-n abc", Run((*pa, *o, "-n", "abc"))),
        "err/stray-word": ("standard", "a word that is no option", Run((*pa, *o, "extra"))),
        "err/missing-file": (
            "standard",
            "a phase image that does not exist",
            Run(("-p", "nothere", "-a", "@s_mag.nii.gz", *o)),
        ),
        "err/no-input": ("standard", "neither -c nor -p and -a", Run(o)),
        "err/complex-empty-name": (
            "standard",
            "--complex= (an empty name)",
            Run(("--complex=", *o)),
        ),
        "err/range": (
            "standard",
            "a phase range above 3pi (exit 255)",
            Run(("-p", "@s_range.nii.gz", "-a", "@s_mag.nii.gz", *o)),
        ),
        "err/long-option-space": (
            "standard",
            "--start 1 (a long option's value needs =)",
            Run((*ts, *o, "--start", "1")),
        ),
        "err/start-outside": ("standard", "--start beyond the series", Run((*ts, *o, "--start=5"))),
        "err/start-negative": ("standard", "--start=-1", Run((*ts, *o, "--start=-1"))),
        "err/end-outside": (
            "standard",
            "--end beyond the series",
            Run((*ts, *o, "--end=5", "-r", "r")),
        ),
        "err/empty-range": (
            "standard",
            "--end before --start (FSL-270)",
            Run((*ts, *o, "--start=2", "--end=1", "-r", "r"), fix_refuses="FSL-270"),
        ),
        "err/size-mismatch": (
            "standard",
            "a magnitude of another size (FSL-271)",
            Run(
                ("-p", "@s_ph.nii.gz", "-a", "@s_mag_other.nii.gz", *o, "-l", "l"),
                fix_refuses="FSL-271",
            ),
        ),
        "err/size-mismatch-larger": (
            "standard",
            "a magnitude larger than the phase (FSL-271)",
            Run(
                ("-p", "@s_ph.nii.gz", "-a", "@s_mag_larger.nii.gz", *o, "--savemask=m"),
                fix_refuses="FSL-271",
            ),
        ),
        "err/size-mismatch-slices": (
            "standard",
            "a mask of fewer slices, labels slice by slice (FSL-271)",
            Run(
                (
                    "-p",
                    "@b_ph.nii.gz",
                    "-a",
                    "@b_mag.nii.gz",
                    "-m",
                    "@b_mask_short.nii.gz",
                    *o,
                    "-l",
                    "l",
                ),
                fix_refuses="FSL-271",
            ),
        ),
    }


def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {**_option_cases(), **_error_cases()}


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for run, node in fmriprep_calls():
        t = "smoke" if run == STANDARD_RUNS[0] else "standard" if run in STANDARD_RUNS else "full"
        if in_tier(t, tier):
            est = node.parent.parent.name
            out.append(
                Case(
                    f"fmriprep/{run}/{est}",
                    "fmriprep",
                    "fMRIPrep's phase unwrapping call (sdcflows' phase-difference workflow)",
                    ("fmriprep", (run, str(node))),
                )
            )
    for name, (t, desc, _run) in _all_cases().items():
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
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="PRELUDE replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl prelude` (crate larmorx-nc-fugue, translated from FSL "
        f"{fsl_versions.NEWEST}'s source)",
        reference=f"prelude from FSL {fsl_versions.NEWEST} (fsl-fugue 2603.0, fsl-newimage 2601.0, "
        "fsl-miscmaths 2412.6, fsl-newnifti 5.0.0, fsl-utils 2412.6; the conda build at "
        "`~/fsl-6.0.7.23`, as built)",
        thresholds=[
            ("Exit status", "the same"),
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
            "against the reproduced run. fMRIPrep's own outputs come from its container's FSL "
            "and are reported only."
        ),
    )
