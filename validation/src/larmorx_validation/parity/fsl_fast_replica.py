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


@functools.cache
def inputs() -> Path:
    """The derived inputs (``oracles/inputs/fsl-fast-replica/``), made once, deterministically:
    ds000005's fMRIPrep T1w brain at half resolution; the multi-channel images
    (:func:`_multichannel_inputs`)."""
    import nibabel as nib

    ws = workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / SUITE
    stamp = out / "DONE-v3"
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
    _multichannel_inputs(ws, out)
    stamp.write_text("derived inputs of the fsl-fast-replica suite, version 3\n", encoding="utf-8")
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


def _run_case(run: Run, checks: CheckList) -> None:
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
            fix_pass(checks, PROGRAM, lambda t: _prepare(run, t), args, settings, b, replica)
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

            fix_pass(checks, PROGRAM, prepare, args, {}, b, replica)
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
        "multi/s0-error",
        "standard",
        "-S 0: fewer than two channels (FAST's error)",
        *("-S", "0", "@mni4_t1.nii.gz"),
    )
    return c


def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {**_real_cases(), **_multi_cases()}


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
