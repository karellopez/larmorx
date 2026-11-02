# SPDX-License-Identifier: Apache-2.0
"""Parity of the FAST replica (``larmorx-nc fsl fast``, FSL Licence) with FSL 6.0.7.23's
``fast``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own segmentation calls (``anat_fit_wf/fast``: ``fast -I 0 -N -p
  -g -S 1 <T1w brain>``) of the oracle runs, their input taken from the runs' working
  directories and copied into the run directory, as fMRIPrep's node holds it; the standard runs
  in the standard tier, every call in the full tier; fMRIPrep's own outputs (its container's
  FSL) compared too, as a report;
- **real**: fMRIPrep's call on derived images (the smoke tier's half-resolution T1w);
- **multi**: the multi-channel segmentation (``-S n``) on MNI152NLin2009cAsym's 2 mm T1w, T2w,
  BOLD reference and T1w/T2w ratio inside its brain mask (the 4 mm T1w and T2w in the smoke
  tier): one case per path. Where FSL saves memory it never wrote (``-p``'s ``_prob_*``
  outside the mask; ``_bias_<n>`` and ``_restore_<n>`` with ``-N``), those values are set to
  the defined ones before the comparison, reported, and the run is never cached.

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every original run goes through the oracle-output cache (:func:`oracle_run`): FAST draws
its random numbers from the C library's ``rand`` with a fixed seed, so a run is served once it
ran.
"""

from __future__ import annotations

import functools
import re
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
    _save,
    _strided,
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
from larmorx_validation.parity.harness import Case, Check, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-fast-replica"
PROGRAM = "fast"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The fMRIPrep runs of the standard tier.
STANDARD_RUNS = ("ds000005-sub-01-noreconall",)
#: The run the derived inputs come from.
SOURCE_RUN = "ds000005-sub-01-noreconall"


# ------------------------------------------------------------------------------------------------
# The programs


def oracle_run(args: list[str], *, cwd: Path, settings: dict[str, str | None], never: str = ""):
    """FSL's ``fast`` with ``args`` in ``cwd``, through the oracle-output cache: FAST is
    single-threaded and seeds the C library's ``rand`` with a constant, so a run is served once
    it ran (unless ``never`` says why not); every file it writes is in ``cwd`` and every file
    there counts by its content."""
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
        timeout=7200,
    )


def run_oracle(
    args: list[str], *, cwd: Path, settings: dict[str, str | None], never: str = ""
) -> Completed:
    p = oracle_run(args, cwd=cwd, settings=settings, never=never)
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Inputs


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, fast node directory)`` of every FAST call of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for cmd in sorted((ws / FMRIPREP_RUNS).glob("*/work/*/*/anat_fit_wf/fast/command.txt")):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def fmriprep_args(node: Path) -> list[str]:
    """fMRIPrep's call as the suite runs it: the input named by its file name (the suite copies
    it into the run directory, where fMRIPrep's node held it)."""
    argv = (node / "command.txt").read_text(encoding="utf-8").split()[1:]
    return [*argv[:-1], Path(argv[-1]).name]


@functools.cache
def fmriprep_input(run: str, node: str) -> Path | None:
    """The T1w brain fMRIPrep segmented: the file of that name in the run's working directory
    (the node itself keeps only its outputs)."""
    name = fmriprep_args(Path(node))[-1]
    work = Path(node).parents[3]
    for p in sorted(work.rglob(name)):
        if Path(node) not in p.parents and p.is_file():
            return p
    return None


#: The template the multi-channel inputs come from (fMRIPrep's TemplateFlow cache of the
#: workspace, ``oracles/templateflow``): its T1w, T2w and BOLD reference share one 2 mm grid.
MNI = "oracles/templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_"


def _multichannel_inputs(ws: Path, out: Path) -> None:
    """The multi-channel inputs: MNI152NLin2009cAsym's 2 mm T1w, T2w and BOLD reference inside
    its brain mask, their T1w/T2w ratio (a myelin-sensitive contrast), and the T1w and T2w at
    4 mm (every second voxel of the 2 mm grid)."""
    import nibabel as nib

    def load(name: str) -> Any:
        return nib.load(str(ws / f"{MNI}{name}.nii.gz"))

    t1i = load("T1w")
    mask = np.asanyarray(load("desc-brain_mask").dataobj) > 0.5
    chans = {
        "t1": np.asarray(t1i.dataobj, np.float32),
        "t2": np.asarray(load("T2w").dataobj, np.float32),
        "bold": np.asarray(load("desc-fMRIPrep_boldref").dataobj, np.float32),
    }
    for k in chans:
        chans[k] = np.where(mask, chans[k], np.float32(0))
    with np.errstate(divide="ignore", invalid="ignore"):
        ratio = np.where(chans["t2"] > 0, chans["t1"] / chans["t2"], 0).astype(np.float32)
    chans["ratio"] = np.where(mask, ratio, np.float32(0))
    # more channels (-S 5 to 8): contrasts derived from the T1w, T2w and BOLD reference
    t1, t2, bold = chans["t1"], chans["t2"], chans["bold"]
    chans["sum"] = t1 + t2
    chans["sqrt"] = np.float32(50) * np.sqrt(np.maximum(t1, np.float32(0)))
    chans["prod"] = t1 * t2 / np.float32(1000)
    chans["mix"] = bold + t1 / np.float32(2)
    for k, v in chans.items():
        _save(out / f"mni_{k}.nii.gz", v, t1i.affine)
    for k in ("t1", "t2"):
        img = nib.Nifti1Image(chans[k], t1i.affine)
        d, a = _strided(img, 2)
        _save(out / f"mni4_{k}.nii.gz", np.asarray(d, np.float32), a)
    # `-s`: three class intensities per channel, channel 1 (the T1w, given last) first: the
    # 20th, 50th and 80th percentiles of each channel in the brain
    rows = []
    for k in ("t1", "t2"):
        v = chans[k][mask]
        rows.append(" ".join(f"{float(np.percentile(v, q)):.1f}" for q in (20, 50, 80)))
    (out / "mni_means.txt").write_text("\n".join(rows) + "\n", encoding="utf-8")


def _means_file(path: Path, rows: list[list[float]]) -> None:
    path.write_text("\n".join(" ".join(f"{v:.1f}" for v in r) for r in rows) + "\n", "utf-8")


def _single_inputs(out: Path) -> None:
    """The single-channel edge inputs, from the half-resolution T1w brain: one slice (a 2D
    image), two volumes (4D), a negative offset (its 2nd percentile below 0), a few negative
    voxels, NaN voxels, 16-bit integers, values above 2^24, a constant cube (k-means cannot
    start), an image of zeros; the class intensities for ``-s`` (in order, in a cyclic order,
    and too few)."""
    import nibabel as nib

    img = nib.load(str(out / "t1_half.nii.gz"))
    t = np.asarray(img.dataobj, np.float32)
    a = img.affine
    brain = np.argwhere(t > 0)
    nz = t.shape[2]
    _save(out / "t1_2d.nii.gz", t[:, :, nz // 2 : nz // 2 + 1], a)
    _save(out / "t1_4d.nii.gz", np.stack([t, np.float32(0.5) * t], axis=3), a)
    _save(out / "t1_offset.nii.gz", t - np.float32(40), a)
    pick = brain[:: max(1, len(brain) // 10)][:10]
    neg = t.copy()
    neg[tuple(pick.T)] = -3
    _save(out / "t1_negvox.nii.gz", neg, a)
    nan = t.copy()
    nan[tuple(brain[:: max(1, len(brain) // 5)][:5].T)] = np.nan
    _save(out / "t1_nan.nii.gz", nan, a)
    _save(out / "t1_int16.nii.gz", np.round(t), a, np.int16)
    big = (t * np.float32(2**25 / float(t.max()))).astype(np.float32)
    _save(out / "t1_big.nii.gz", big, a)
    const = np.zeros(t.shape, np.float32)
    const[20:60, 30:70, 30:70] = 100
    _save(out / "const.nii.gz", const, a)
    _save(out / "zeros.nii.gz", np.zeros((16, 16, 16), np.float32), a)
    v = t[t > 0]
    p = [float(np.percentile(v, q)) for q in (20, 50, 80)]
    _means_file(out / "t1_means.txt", [p])
    _means_file(out / "t1_means_cycle.txt", [[p[2], p[0], p[1]]])
    _means_file(out / "t1_means_two.txt", [p[:2]])


def _prior_inputs(ws: Path, out: Path) -> None:
    """The priors' inputs: FSL's matrix from FSL's standard tissue priors (avg152T1, 91 x 109
    x 91 at 2 mm, stored radiologically) to MNI152NLin2009cAsym's 2 mm grid (97 x 115 x 97,
    stored neurologically, which FSL's coordinates flip): the translation that aligns their
    world coordinates; the identity; ds000005's CSF, GM and WM probability maps on that grid
    (fMRIPrep's ``space-MNI152NLin2009cAsym_res-2`` outputs, for ``-A``); a T2w ``-s`` file whose classes are in a cyclic order (channel 1, then 2)."""
    (out / "std2mni.mat").write_text("1 0 0 5.5\n0 1 0 6.5\n0 0 1 6.5\n0 0 0 1\n", encoding="utf-8")
    (out / "ident.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
    anat = ws / FMRIPREP_RUNS / SOURCE_RUN / "out" / "sub-01" / "anat"
    for label in ("CSF", "GM", "WM"):
        src = anat / f"sub-01_space-MNI152NLin2009cAsym_res-2_label-{label}_probseg.nii.gz"
        shutil.copy(src, out / f"tpl_{label.lower()}.nii.gz")
    # the classes white matter, CSF, grey matter for the T1w and the T1w/T2w ratio (whose
    # tissues are in the T1w's order): their 80th, 20th and 50th percentiles in the brain, a
    # cycle once sorted by the T1w's means
    import nibabel as nib

    pct = []
    for k in ("t1", "ratio"):
        v = np.asarray(nib.load(str(out / f"mni_{k}.nii.gz")).dataobj, np.float32)
        v = v[v > 0]
        pct.append([float(np.percentile(v, q)) for q in (80, 20, 50)])
    (out / "mni_means_cycle.txt").write_text(
        "\n".join(" ".join(f"{x:.4f}" for x in r) for r in pct) + "\n", encoding="utf-8"
    )


def _phantom_inputs(out: Path) -> None:
    """A two-channel phantom of opposite contrasts (16 x 16 x 12 voxels of 1 x 1 x 1.5 mm, a
    border of zeros): three slabs along x, dark, middle and bright in the T1-like channel and
    the reverse in the T2-like one, with seeded noise. FAST starts each channel's class means
    from that channel's own percentiles in ascending order, so the classes it pairs across the
    channels hold no voxel."""
    rng = np.random.default_rng(5)
    a = np.diag([1.0, 1.0, 1.5, 1.0])
    t1 = np.zeros((16, 16, 12), np.float32)
    t2 = np.zeros((16, 16, 12), np.float32)
    for k, (v1, v2) in enumerate(((80, 300), (150, 150), (300, 80))):
        sl = (slice(2 + 4 * k, 6 + 4 * k), slice(2, 14), slice(2, 10))
        shape = t1[sl].shape
        t1[sl] = v1 + rng.normal(0, 3, shape).astype(np.float32)
        t2[sl] = v2 + rng.normal(0, 3, shape).astype(np.float32)
    _save(out / "phantom_t1.nii.gz", t1, a)
    _save(out / "phantom_t2.nii.gz", t2, a)


@functools.cache
def inputs() -> Path:
    """The derived inputs (``oracles/inputs/fsl-fast-replica/``), made once, deterministically:
    ds000005's fMRIPrep T1w brain at half resolution and its edge cases
    (:func:`_single_inputs`); the multi-channel images (:func:`_multichannel_inputs`); the
    priors' matrices and maps (:func:`_prior_inputs`)."""
    import nibabel as nib

    ws = workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / SUITE
    stamp = out / "DONE-v9"
    if stamp.is_file():
        return out
    out.mkdir(parents=True, exist_ok=True)
    call = next((n for r, n in fmriprep_calls() if r == SOURCE_RUN), None)
    assert call is not None, f"{SOURCE_RUN}'s FAST node is missing"
    src = fmriprep_input(SOURCE_RUN, str(call))
    assert src is not None
    t1 = nib.load(str(src))
    t, ta = _strided(t1, 2)
    _save(out / "t1_half.nii.gz", np.asarray(t, np.float32), ta)
    _single_inputs(out)
    _multichannel_inputs(ws, out)
    _prior_inputs(ws, out)
    _phantom_inputs(out)
    stamp.write_text("derived inputs of the fsl-fast-replica suite, version 9\n", encoding="utf-8")
    return out


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: the arguments (``@name`` names a derived input copied into the run directory),
    the environment."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True
    #: Why FSL's run is never cached (memory it never writes reaches its files).
    never: str = ""
    #: FSL's outputs (name suffixes) whose voxels outside the mask FSL never writes.
    unwritten: tuple[str, ...] = ()
    #: With ``-N``, the multi-channel bias field is never written: FSL's ``_bias_<n>`` and
    #: ``_restore_<n>`` hold what the heap held; they are compared with the defined result (no
    #: bias field: 1, and the channel's image) and their headers with FSL's.
    no_bias_field: bool = False
    #: A case FSL aborts or crashes on and the default corrects: the replica's ``fix`` run
    #: finishes, names this entry and writes the segmentation.
    fix_finishes: str = ""
    #: A case the replica refuses in both modes (FSL reads or writes outside its memory): FSL's
    #: run is never cached and only reported; the replica exits 1 naming this entry, writing
    #: nothing.
    refused: str = ""
    #: A case FSL runs or aborts on that the default refuses (rule B): the replica's ``fix`` run
    #: exits 1 with a refusal naming this entry.
    fix_refuses: str = ""


#: The entries the ``fix`` pass may name without a visible change: FSL-96 and FSL-97 fill the
#: memory FAST never writes in both modes (the suite compares FSL's files with those values).
SILENT = ("FSL-96", "FSL-97")


def _prepare(run: Run, tmp: Path) -> list[str]:
    out = []
    for a in run.args:
        if a.startswith("@"):
            src = inputs() / a[1:]
            shutil.copy(src, tmp / src.name)
            out.append(src.name)
        else:
            out.append(a)
    return out


def _refused_case(run: Run, checks: CheckList) -> None:
    """:attr:`Run.refused`: FSL's run reported (never cached), the replica's refusal in both
    modes."""
    from larmorx_validation.parity.harness import EXPECTED

    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as a_:
        a = Path(a_)
        args = _prepare(run, a)
        p = oracle_run(args, cwd=a, settings=settings, never=f"undefined behaviour ({run.refused})")
        fsl = f"FSL exits {p.returncode}: {' '.join(p.stderr.split())[-120:]}"
    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as b_:
            b = Path(b_)
            r = run_replica(PROGRAM, _prepare(run, b), cwd=b, settings=settings, mode=mode)
            refusal = next(
                (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
            )
            written = sorted(q.name for q in b.glob("seg*"))
            checks(
                f"the replica refuses ({mode})",
                r.code == 1 and run.refused in refusal and not written,
                detail=f"exit {r.code}: {refusal[:160]}; {fsl}",
            )
    if all(c.passed for c in checks.checks):
        raise Outcome(EXPECTED, f"undefined behaviour of FSL's ({run.refused}); refused. {fsl}")


def _fix_finishes(run: Run, checks: CheckList) -> None:
    """:attr:`Run.fix_finishes`: the default run finishes where FSL stops."""
    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-fast-fix-") as b_:
        b = Path(b_)
        r = run_replica(PROGRAM, _prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
        named = defects.notices_in(r.stderr)
        checks(
            f"the default run finishes and names {run.fix_finishes}",
            r.code == 0 and run.fix_finishes in named and bool(list(b.glob("*_seg.nii*"))),
            detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
        )


def _fix_refuses(run: Run, checks: CheckList) -> None:
    """:attr:`Run.fix_refuses`: the default run refuses."""
    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-fast-fix-") as b_:
        b = Path(b_)
        r = run_replica(PROGRAM, _prepare(run, b), cwd=b, settings=settings, mode=defects.FIX)
        refusal = next(
            (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
        )
        checks(
            f"the default run refuses, naming {run.fix_refuses}",
            r.code == 1 and run.fix_refuses in refusal,
            detail=f"exit {r.code}: {refusal[:160]}",
        )


def _run_case(run: Run, checks: CheckList) -> None:
    if run.refused:
        _refused_case(run, checks)
        return
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = run_oracle(args, cwd=a, settings=settings, never=run.never)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.unwritten:
            _clear_unwritten(checks, a, run.unwritten)
        if run.no_bias_field:
            _undefined_bias_field(checks, a, args)
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
                silent_ok=SILENT,
            )
    if run.fix_finishes:
        _fix_finishes(run, checks)
    if run.fix_refuses:
        _fix_refuses(run, checks)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


#: What a multi-channel case with ``-p`` cannot cache: ``members`` (the ``_prob_*`` files) is
#: never written outside the mask, where FSL's files hold whatever the heap held (pointers
#: that move with the address-space layout).
UNWRITTEN_PROB = "FSL's _prob_* files hold memory it never writes outside the mask"


def _nifti_gz(path: Path) -> tuple[bytes, np.ndarray, int]:
    """A gzipped NIfTI-1 file's bytes, its values (float32 or int32) and their offset."""
    import gzip
    import struct

    raw = gzip.decompress(path.read_bytes())
    dtype = {16: np.float32, 8: np.int32}[struct.unpack_from("<h", raw, 70)[0]]
    off = int(struct.unpack_from("<f", raw, 108)[0])
    return raw, np.frombuffer(raw, dtype=dtype, offset=off), off


def _clear_unwritten(checks: CheckList, oracle_dir: Path, suffixes: tuple[str, ...]) -> None:
    """Sets to 0, in FSL's files ending in ``suffixes``, the voxels outside its segmentation
    mask (``_seg`` 0), which FSL never writes (the replica writes 0 there), and reports them;
    the files are then compared bit for bit."""
    import gzip

    cleared = []
    for seg in oracle_dir.glob("*_seg.nii.gz"):
        _, mask, _ = _nifti_gz(seg)
        base = seg.name[: -len("_seg.nii.gz")]
        for f in sorted(oracle_dir.glob(f"{base}_*.nii.gz")):
            if not any(f.name.endswith(sfx) for sfx in suffixes):
                continue
            raw, data, off = _nifti_gz(f)
            outside = (mask == 0) & (data != 0)
            if outside.any():
                fixed = data.copy()
                fixed[outside] = 0
                f.write_bytes(gzip.compress(raw[:off] + fixed.tobytes()))
                cleared.append(f"{f.name}: {int(outside.sum())}")
    checks.add(
        Check(
            "FSL's voxels never written (outside the mask) set to 0",
            True,
            metric="files",
            value=str(len(cleared)),
            threshold="reported",
            detail="; ".join(cleared[:4]),
        )
    )


def _undefined_bias_field(checks: CheckList, oracle_dir: Path, args: list[str]) -> None:
    """Replaces the values of FSL's ``_bias_<n>`` (by 1) and ``_restore_<n>`` (by channel
    ``n``'s image: the ``n``-th word from the end) of a multi-channel run without a bias field,
    where FSL's are memory it never wrote, keeping FSL's headers; reports how many of FSL's
    values differed from the defined ones."""
    import gzip

    replaced = []
    for f in sorted(oracle_dir.glob("*.nii.gz")):
        m = re.search(r"_(bias|restore)_(\d+)\.nii\.gz$", f.name)
        if not m:
            continue
        raw, data, off = _nifti_gz(f)
        if m.group(1) == "bias":
            want = np.ones_like(data)
        else:
            _, want, _ = _nifti_gz(oracle_dir / args[-int(m.group(2))])
            want = want.astype(data.dtype)
        differ = int(np.sum(~((data == want) | (np.isnan(data) & np.isnan(want)))))
        f.write_bytes(gzip.compress(raw[:off] + want.tobytes()))
        replaced.append(f"{f.name}: {differ}")
    checks.add(
        Check(
            "FSL's bias field never written (-N): its values set to the defined ones",
            True,
            metric="files",
            value=str(len(replaced)),
            threshold="reported",
            detail="; ".join(replaced[:4]),
        )
    )


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    src = fmriprep_input(run, node)
    if src is None:
        raise Outcome("skipped", f"{run}: the input of fMRIPrep's FAST node is not available")
    args = fmriprep_args(Path(node))
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        shutil.copy(src, a / args[-1])
        shutil.copy(src, b / args[-1])
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        _fmriprep_outputs(checks, Path(node), b)
        if oracle.code == 0 and replica.code == 0:

            def prepare(t: Path) -> None:
                shutil.copy(src, t / args[-1])

            fix_pass(checks, PROGRAM, prepare, args, {}, b, replica, silent_ok=SILENT)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _fmriprep_outputs(checks: CheckList, node: Path, replica_dir: Path) -> None:
    """fMRIPrep's own outputs (its container's FSL), as a report."""
    theirs = sorted(node.glob("*_pve*.nii.gz"))
    same, total, notes = 0, 0, []
    for t in theirs:
        ours = replica_dir / t.name
        if not ours.is_file():
            continue
        total += 1
        if _same_streams(t, ours, blank_descrip=True):
            same += 1
        else:
            notes.append(f"{t.name}: {_differing_values(t, ours)}")
    checks.add(
        Check(
            "fMRIPrep's outputs (its container's FSL) identical",
            True,
            metric="files",
            value=f"{same}/{total}",
            threshold="reported",
            detail="; ".join(notes[:4]),
        )
    )


#: The real cases: name -> (tier, description, run).
def _real_cases() -> dict[str, tuple[str, str, Run]]:
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, tier: str, desc: str, *args: str, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    add(
        "real/fmriprep-call-half",
        "smoke",
        "fMRIPrep's call (-I 0 -N -p -g -S 1) on ds000005's T1w brain at half resolution",
        "-I",
        "0",
        "-N",
        "-p",
        "-g",
        "-S",
        "1",
        "@t1_half.nii.gz",
    )
    return c


def _multi_cases() -> dict[str, tuple[str, str, Run]]:
    """The multi-channel segmentation (``-S n``): FAST reads the last ``n`` words, channel 1
    the last (it names the outputs and orders the classes); one case per path: two, three and
    four channels (Armadillo's closed forms and LAPACK's 4 x 4 paths), two, three and four
    classes, ``-t 2``, the bias field on and off with its outputs, ``--nopve``, ``-s``,
    ``-v``, every numeric option, and the error below two channels."""
    c: dict[str, tuple[str, str, Run]] = {}
    unw = {
        "never": UNWRITTEN_PROB,
        "unwritten": ("_prob_0.nii.gz", "_prob_1.nii.gz", "_prob_2.nii.gz", "_prob_3.nii.gz"),
    }

    def add(name: str, tier: str, desc: str, *args: str, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    add(
        "multi/s2-outputs",
        "standard",
        "-S 2 (T1w, then T2w: the T2w is channel 1) with every output (-B -b -g -p)",
        *("-S", "2", "-B", "-b", "-g", "-p", "-o", "seg", "@mni_t1.nii.gz", "@mni_t2.nii.gz"),
        **unw,
    )
    add(
        "multi/s2-type2-verbose",
        "standard",
        "-S 2 -t 2 -v: the T2w as channel 1 with the classes in reversed order, every message",
        *("-S", "2", "-t", "2", "-v", "-o", "seg", "@mni_t1.nii.gz", "@mni_t2.nii.gz"),
    )
    add(
        "multi/s3",
        "standard",
        "-S 3 (BOLD reference, T2w, T1w) with the bias field's outputs (3 x 3 matrices)",
        *("-S", "3", "-B", "-b", "-o", "seg", "@mni_bold.nii.gz", "@mni_t2.nii.gz"),
        "@mni_t1.nii.gz",
    )
    add(
        "multi/s4-n2",
        "standard",
        "-S 4 -n 2 (T1w/T2w ratio, BOLD reference, T2w, T1w: LAPACK's 4 x 4 paths), two classes",
        *("-S", "4", "-n", "2", "-p", "-o", "seg", "@mni_ratio.nii.gz", "@mni_bold.nii.gz"),
        *("@mni_t2.nii.gz", "@mni_t1.nii.gz"),
        **unw,
    )
    add(
        "multi/s2-n4",
        "standard",
        "-S 2 -n 4: more than three classes (their own mixel types)",
        *("-S", "2", "-n", "4", "-g", "-o", "seg", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s2-nopve-default-name",
        "standard",
        "-S 2 --nopve without -o (the outputs named after channel 1)",
        *("-S", "2", "--nopve", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s2-nobias-outputs",
        "standard",
        "-S 2 -N -B -b: no bias field, its outputs written",
        *("-S", "2", "-N", "-B", "-b", "-o", "seg", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
        never="with -N FSL never writes the multi-channel bias field it saves",
        no_bias_field=True,
    )
    add(
        "multi/s2-manualseg",
        "standard",
        "-S 2 -s: the initial class intensities from a file (three per channel)",
        *("-S", "2", "-s", "@mni_means.txt", "-o", "seg", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s2-options",
        "standard",
        "-S 2 with every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5)",
        *("-S", "2", "-I", "2", "-O", "1", "-W", "5", "-l", "30", "-f", "0.05", "-H", "0.2"),
        *("-R", "0.5", "-o", "seg", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s2-quarter",
        "smoke",
        "-S 2 -B -b on the T1w and T2w at 4 mm",
        *("-S", "2", "-B", "-b", "-o", "seg", "@mni4_t2.nii.gz", "@mni4_t1.nii.gz"),
    )
    add(
        "multi/s5",
        "standard",
        "-S 5 -B -b (T1w+T2w, T1w/T2w ratio, BOLD reference, T2w, T1w): 5 x 5 covariances "
        "(LAPACK's LU, inverse and solve beyond 4 x 4)",
        *("-S", "5", "-B", "-b", "-o", "seg", "@mni_sum.nii.gz", "@mni_ratio.nii.gz"),
        *("@mni_bold.nii.gz", "@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s8",
        "standard",
        "-S 8 -n 2 -g: eight channels (the five, a square root, a product and a mix of the "
        "T1w and BOLD reference), 8 x 8 covariances",
        *("-S", "8", "-n", "2", "-g", "-o", "seg", "@mni_mix.nii.gz", "@mni_prod.nii.gz"),
        *("@mni_sqrt.nii.gz", "@mni_sum.nii.gz", "@mni_ratio.nii.gz", "@mni_bold.nii.gz"),
        *("@mni_t2.nii.gz", "@mni_t1.nii.gz"),
    )
    add(
        "multi/s0-error",
        "standard",
        "-S 0: fewer than two channels (FAST's error)",
        *("-S", "0", "@mni4_t1.nii.gz"),
    )
    return c


#: ``-p``'s probability maps: FSL never writes them outside the mask.
UNWRITTEN_SINGLE = {
    "never": UNWRITTEN_PROB,
    "unwritten": tuple(f"_prob_{i}.nii.gz" for i in range(5)),
}


def _option_cases() -> dict[str, tuple[str, str, Run]]:
    """The single-channel options on the half-resolution T1w brain, one case per path: the
    bias field (on by default) and its outputs, the image types, the class counts (1, 2, 4:
    no mixel types, two, their own), ``--nopve``, ``-s`` (in order, cyclic, too few values),
    every numeric option, ``-v``, ``-o``; the odd inputs (one slice, 4D, negative values both
    ways, NaN voxels, 16-bit integers, values above 2^24)."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    t1 = "@t1_half.nii.gz"
    add(
        "opt/bias-outputs",
        "the bias field (default) with -b -B -g",
        "-b",
        "-B",
        "-g",
        "-o",
        "seg",
        t1,
    )
    add("opt/type2-verbose", "-t 2 -v: reversed class order, every message", "-t", "2", "-v", t1)
    add("opt/type3-n2", "-t 3 -n 2 -g: PD-weighted, two classes", "-t", "3", "-n", "2", "-g", t1)
    add(
        "opt/n4-probs",
        "-n 4 -p: four classes, their mixel types",
        "-n",
        "4",
        "-p",
        "-o",
        "seg",
        t1,
        **UNWRITTEN_SINGLE,
    )
    add("opt/n1", "-n 1 -v: one class (no mixel types)", "-n", "1", "-v", "-o", "seg", t1)
    add("opt/nopve", "--nopve -g -B", "--nopve", "-g", "-B", "-o", "seg", t1)
    add(
        "opt/numeric",
        "every numeric option (-I 2 -O 1 -W 5 -l 30 -f 0.05 -H 0.2 -R 0.5) with -b",
        *("-I", "2", "-O", "1", "-W", "5", "-l", "30", "-f", "0.05", "-H", "0.2", "-R", "0.5"),
        *("-b", "-o", "seg", t1),
    )
    add("opt/H0-N", "-H 0 -N: no MRF smoothness, no bias field", "-H", "0", "-N", "-o", "seg", t1)
    add(
        "opt/long-names",
        "the long option names",
        "--class=3",
        "--iter=1",
        "--lowpass=15",
        "--type=1",
        "--fHard=0.03",
        "--segments",
        "--nobias",
        "--channels=1",
        "--out=seg",
        "--init=3",
        "--mixel=0.2",
        "--fixed=2",
        "--Hyper=0.15",
        "--verbose",
        t1,
    )
    add(
        "opt/manualseg", "-s: the initial class intensities", "-s", "@t1_means.txt", "-o", "seg", t1
    )
    add(
        "opt/manualseg-cycle",
        "-s with the classes in a cyclic order (sorted at the end)",
        "-s",
        "@t1_means_cycle.txt",
        "-v",
        "-o",
        "seg",
        t1,
    )
    add(
        "opt/manualseg-two",
        "-s with too few values (the percentiles instead)",
        "-s",
        "@t1_means_two.txt",
        "-o",
        "seg",
        t1,
    )
    add(
        "opt/manualseg-missing",
        "-s naming no file (the percentiles instead)",
        "-s",
        "nofile.txt",
        "-o",
        "seg",
        t1,
    )
    add("opt/2d", "a 2D image (one slice) with -B -b", "-B", "-b", "-o", "seg", "@t1_2d.nii.gz")
    add(
        "opt/4d",
        "a 4D image (its first volume, with newimage's warning)",
        "-o",
        "seg",
        "@t1_4d.nii.gz",
    )
    add(
        "opt/offset",
        "negative values, 2nd percentile below 0 (the minimum subtracted)",
        "-B",
        "-o",
        "seg",
        "@t1_offset.nii.gz",
    )
    add("opt/negvox", "a few negative voxels (set to 0)", "-B", "-o", "seg", "@t1_negvox.nii.gz")
    add(
        "opt/nan",
        "NaN voxels with -B -b -p",
        "-B",
        "-b",
        "-p",
        "-o",
        "seg",
        "@t1_nan.nii.gz",
        **UNWRITTEN_SINGLE,
    )
    add("opt/int16", "a 16-bit integer image", "-g", "-o", "seg", "@t1_int16.nii.gz")
    add(
        "opt/big",
        "values above 2^24 (the brightest voxels and the mask)",
        "-g",
        "-o",
        "seg",
        "@t1_big.nii.gz",
    )
    add(
        "opt/output-type",
        "FSLOUTPUTTYPE=NIFTI_PAIR",
        "-g",
        "-o",
        "seg",
        t1,
        settings=(("FSLOUTPUTTYPE", "NIFTI_PAIR"),),
    )
    add("opt/default-name", "no -o: the outputs named after the input", "-N", "--nopve", t1)
    return c


def _error_cases() -> dict[str, tuple[str, str, Run]]:
    """The messages, exit statuses and failure cases."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    q = "@mni4_t1.nii.gz"
    add("err/no-arguments", "no arguments: the usage")
    add("err/help", "-h: the usage", "-h")
    add("err/unknown-option", "an unknown option", "--bogus", q)
    add("err/missing-value", "an option without its value", "-n")
    add("err/bad-number", "a value that is not a number", "-n", "abc", q)
    add("err/H-negative", "-H below 0", "-H", "-0.5", q)
    add("err/H-nan", "-H nan (passes the check for a negative value)", "-H", "nan", "-o", "seg", q)
    add("err/H-inf", "-H inf", "-H", "inf", "-o", "seg", q)
    add("err/l-zero", "-l 0 (no smoothing extent)", "-l", "0", "-o", "seg", q)
    add("err/f-nan", "-f nan", "-f", "nan", "-o", "seg", q)
    add("err/R-nan", "-R nan", "-R", "nan", "-o", "seg", q)
    add("err/n0", "-n 0 (FAST aborts; FSL-99)", "-n", "0", "-o", "seg", q, fix_refuses="FSL-99")
    add(
        "err/n-negative",
        "-n -1 (FAST aborts; FSL-99)",
        "-n",
        "-1",
        "-o",
        "seg",
        q,
        fix_refuses="FSL-99",
    )
    add(
        "err/n-negative-multi",
        "-S 2 -n -2 (FAST aborts; FSL-99)",
        *("-S", "2", "-n", "-2", "-o", "seg", "@mni4_t2.nii.gz", q),
        fix_refuses="FSL-99",
    )
    add(
        "err/n0-multi",
        "-S 2 -n 0 (FSL-99)",
        *("-S", "2", "-n", "0", "-o", "seg", "@mni4_t2.nii.gz", q),
        fix_refuses="FSL-99",
    )
    add("err/n-minus2", "-n -2 (FAST aborts; FSL-99)", "-n", "-2", q, fix_refuses="FSL-99")
    add("err/l-negative", "-l -5 (FSL-99)", "-l", "-5", "-o", "seg", q)
    add("err/missing-input", "an input that does not exist", "-o", "seg", "nothere.nii.gz")
    add(
        "err/option-after-input",
        "an option after the input (its value read as the image)",
        "-o",
        "seg",
        q,
        "-n",
        "2",
    )
    add("err/constant", "a constant image (k-means cannot start)", "-o", "seg", "@const.nii.gz")
    add("err/zeros", "an image of zeros (an empty mask)", "-o", "seg", "@zeros.nii.gz")
    add(
        "err/fsldir-unset",
        "FSLDIR unset (no priors asked for: FAST aborts; FSL-100)",
        *("-o", "seg", q),
        settings=(("FSLDIR", None),),
        fix_finishes="FSL-100",
    )
    add(
        "err/fsldir-unset-a",
        "FSLDIR unset with -a (FAST aborts; refused by default)",
        *("-a", "@std2mni.mat", "-o", "seg", q),
        settings=(("FSLDIR", None),),
    )
    add(
        "err/fsldir-unset-A-a",
        "FSLDIR unset with -A and -a (FLIRT's command starts with (null): the shell fails and "
        "FAST crashes; FSL-102, FSL-101)",
        *("-A", "@tpl_csf.nii.gz", "@tpl_gm.nii.gz", "@tpl_wm.nii.gz", "-a", "@ident.mat"),
        *("-o", "seg", "@mni_t1.nii.gz"),
        settings=(("FSLDIR", None),),
        never="FSL crashes (FSL-101)",
        fix_finishes="FSL-102",
    )
    add(
        "err/S2-sizes",
        "-S 2 with images of different sizes, channel 1 the smaller (FAST aborts; FSL-104)",
        *("-S", "2", "-v", "-o", "seg", "@mni_t1.nii.gz", "@mni4_t1.nii.gz"),
        fix_refuses="FSL-104",
    )
    add(
        "err/S2-sizes-larger-first",
        "-S 2 with images of different sizes, channel 1 the larger (refused; FSL-104)",
        *("-S", "2", "-o", "seg", "@mni4_t1.nii.gz", "@mni_t1.nii.gz"),
        refused="FSL-104",
    )
    add(
        "err/S4-two-images",
        "-S 4 with two images (an option word read as an image)",
        *("-S", "4", "@mni4_t2.nii.gz", q),
    )
    add(
        "err/fsldir-unset-A",
        "FSLDIR unset with -A (no FSLDIR needed)",
        *("-A", "a", "b", "c", "-o", "seg", q),
        settings=(("FSLDIR", None),),
    )
    add(
        "err/S1-two-images",
        "-S 1 with two images (the last one segmented)",
        "-o",
        "seg",
        "@mni4_t2.nii.gz",
        q,
    )
    return c


def _prior_cases() -> dict[str, tuple[str, str, Run]]:
    """The tissue priors on MNI152NLin2009cAsym's 2 mm T1w: FSL's standard priors registered
    by ``-a`` (FLIRT's -applyxfm, run by FAST), ``-P`` (throughout), two classes, a class count
    the priors do not serve, ``-A`` (the template's own CSF, GM and WM maps), ``-A`` alone, a
    missing prior, a matrix FLIRT cannot read, the multi-channel segmentation."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    t1 = "@mni_t1.nii.gz"
    mat = "@std2mni.mat"
    tpl = ("@tpl_csf.nii.gz", "@tpl_gm.nii.gz", "@tpl_wm.nii.gz")
    add("priors/a-verbose", "-a with FSL's standard priors, -v", "-a", mat, "-v", "-o", "seg", t1)
    add(
        "priors/a-P-probs",
        "-a -P: the priors throughout, -p -g",
        "-a",
        mat,
        "-P",
        "-p",
        "-g",
        "-o",
        "seg",
        t1,
        **UNWRITTEN_SINGLE,
    )
    add("priors/a-n2", "-a -n 2: two classes", "-a", mat, "-n", "2", "-o", "seg", t1)
    add("priors/a-P-n2", "-a -P -n 2", "-a", mat, "-P", "-n", "2", "-o", "seg", t1)
    add(
        "priors/a-n4",
        "-a -n 4 (priors serve 2 or 3 classes only)",
        "-a",
        mat,
        "-n",
        "4",
        "-o",
        "seg",
        t1,
    )
    add(
        "priors/A-a",
        "-A (ds000005's own maps in the template's space) with -a (the identity) and -P",
        "-A",
        *tpl,
        "-a",
        "@ident.mat",
        "-P",
        "-o",
        "seg",
        t1,
    )
    add("priors/A-alone", "-A without -a (no priors)", "-A", *tpl, "-o", "seg", t1)
    add("priors/P-alone", "-P without -a (no priors)", "-P", "-o", "seg", "@mni4_t1.nii.gz")
    add(
        "priors/missing-prior",
        "-A naming a missing prior with -a",
        "-A",
        "nothere",
        tpl[1],
        tpl[2],
        "-a",
        "@ident.mat",
        "-o",
        "seg",
        "@mni_t1.nii.gz",
    )
    add(
        "priors/bad-matrix",
        "-a naming a missing matrix (FLIRT fails; FAST crashes: FSL-101)",
        *("-a", "nothere.mat", "-o", "seg", t1),
        never="FSL crashes (FSL-101)",
        fix_finishes="FSL-101",
    )
    add(
        "priors/name-blank",
        "-o with a blank: FLIRT's command split by the shell (FSL-102)",
        *("-a", mat, "-o", "my seg", "@mni4_t1.nii.gz"),
        never="FSL crashes (FSL-101)",
        fix_finishes="FSL-102",
    )
    add(
        "priors/multi-a-P",
        "-S 2 -a -P: the multi-channel segmentation with the priors",
        "-S",
        "2",
        "-a",
        mat,
        "-P",
        "-o",
        "seg",
        "@mni_t2.nii.gz",
        t1,
    )
    add(
        "priors/multi-a-n2",
        "-S 2 -a -n 2",
        "-S",
        "2",
        "-a",
        mat,
        "-n",
        "2",
        "-o",
        "seg",
        "@mni_t2.nii.gz",
        t1,
    )
    add(
        "multi/s2-opposite-contrasts",
        "-S 2 -B -b on a phantom of opposite contrasts: classes paired across the channels "
        "start empty; every voxel ends in class 1, the bias field NaN (FAST exits 0)",
        *("-S", "2", "-B", "-b", "-o", "seg", "@phantom_t2.nii.gz", "@phantom_t1.nii.gz"),
    )
    add(
        "multi/s2-manualseg-cycle",
        "-S 2 -s (T1w/T2w ratio, T1w) with the classes in a cyclic order (FSL-98)",
        "-S",
        "2",
        "-s",
        "@mni_means_cycle.txt",
        "-o",
        "seg",
        "@mni_ratio.nii.gz",
        t1,
    )
    return c


def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {
        **_real_cases(),
        **_multi_cases(),
        **_option_cases(),
        **_error_cases(),
        **_prior_cases(),
    }


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for run, node in fmriprep_calls():
        t = "standard" if run in STANDARD_RUNS else "full"
        if in_tier(t, tier):
            out.append(
                Case(
                    f"fmriprep/{run}",
                    "fmriprep",
                    "fMRIPrep's segmentation call",
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
        # a Run given whole (differential random testing's cases)
        _run_case(payload, checks)
    else:
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="FAST replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl fast` (crate larmorx-nc-fast, translated from FSL "
        f"{fsl_versions.NEWEST}'s source)",
        reference=f"fast from FSL {fsl_versions.NEWEST} (fsl-fast4 2111.3, fsl-newimage 2601.0, "
        "fsl-miscmaths 2412.6, fsl-utils 2412.6; the conda build at `~/fsl-6.0.7.23`, as built: "
        "glibc 2.35's maths and random numbers)",
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
            "and are reported only. The derived inputs are made once from ds000005's fMRIPrep "
            "run (`oracles/inputs/fsl-fast-replica/`)."
        ),
    )
