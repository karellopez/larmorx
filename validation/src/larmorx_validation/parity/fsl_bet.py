# SPDX-License-Identifier: Apache-2.0
"""Parity of the BET replicas (``larmorx-nc fsl bet``, ``bet2`` and ``betsurf``, run by larmorx
as a separate process) with FSL 6.0.7.17's ``bet``, ``bet2`` and ``betsurf``: exit status,
messages and every output (brain, mask, outline, skull, mesh, the skull and scalp surfaces and
their masks), byte for byte.

FSL runs from its own installation (``~/fsl``, or ``LARMORX_FSL_DIR``), never on the ``PATH``:
both programs run with ``PATH=/usr/bin:/bin``, ``FSLDIR`` set (the ``bet`` script calls
``${FSLDIR}/bin/...``), an empty ``HOME`` and each case's ``FSLOUTPUTTYPE``. The replica runs the
way users reach it: through larmorx's own dispatch (``larmorx fsl <tool>``), which runs the
``larmorx-nc`` program as a separate process (``LARMORX_NC_BIN``, else the ``crates-nc`` build).
This package is Apache-2.0 and never loads the replica. Every run of FSL goes through
:func:`oracle_run`.

What is compared:
- the exit status (where FSL aborts on an uncaught exception or a signal, the replica exits 1);
- standard output and error, byte for byte, without larmorx's own lines (upstream-defect notices
  and its ``(larmorx)`` lines) and with the ``bet`` script's own path (``$0``) read as ``bet``;
  where FSL crashes, the lines it printed before the crash;
- every file written: images by their bytes (decompressed), header and data; meshes and other
  text byte for byte. A mask that differs is also measured (Dice).

A fix pass then runs the replica's default (``upstream_defects="fix"``) on the same arguments and
reports which defects it corrected and what that changed (D13); the cases with a Python
equivalent also run ``lx.fsl.bet`` / ``lx.fsl.betsurf``, whose images must be the command line's.

Cases: fMRIPrep's real BET calls (``bet <boldref> <out> -f 0.20 -m``, nine in the five oracle
runs), also compared with fMRIPrep's own outputs and run by fMRIPrep 25.2.6's FSL (its image:
FSL's own variability between builds); every option of ``bet2``, ``bet`` and ``betsurf`` on
synthetic heads with a known brain; every storage type, scaling, orientation and file format FSL
reads; the variations ``-R``, ``-F``, ``-Z``; the messages and failures; the defects (FSL-40 to
-56); and real T1w, T2w and EPI images from children to older adults, 3T and 7T, a tumour and a
stroke (``larmorx-testdata``), with ``betsurf`` on matrices FLIRT computed.
"""

from __future__ import annotations

import contextlib
import functools
import gzip
import io
import os
import re
import shutil
import subprocess
import tempfile
import warnings
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    CheckList,
    Outcome,
    Suite,
)

TIERS = ("smoke", "standard", "full")
FMRIPREP_IMAGE = "oracles/images/fmriprep-25.2.6.sif"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
REPLICA = "crates-nc/target/{profile}/larmorx-nc{exe}"

EXIT = "exit status"
MESSAGES = "messages: FSL's, byte for byte"
CRASH = "messages before FSL's crash"
FILES = "files written"
FMRIPREP_OWN = "fMRIPrep's own output (as it ran)"
FMRIPREP_FSL = "fMRIPrep's FSL against the oracle"
RUN_AGAIN = "FSL run again"
PYTHON = "Python API"


# ------------------------------------------------------------------------------------------------
# The programs


def workspace() -> Path | None:
    """The workspace that holds ``oracles/`` (a parent of this file)."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def fsl_dir() -> Path | None:
    """FSL's installation: ``LARMORX_FSL_DIR``, else ``~/fsl`` (when it holds ``bin/bet2``)."""
    for candidate in (os.environ.get("LARMORX_FSL_DIR"), str(Path.home() / "fsl")):
        if candidate and (Path(candidate) / "bin" / "bet2").is_file():
            return Path(candidate)
    return None


@functools.cache
def fsl_version() -> str:
    d = fsl_dir()
    if d is None:
        return "not found"
    v = d / "etc" / "fslversion"
    version = v.read_text(encoding="utf-8").split(":")[0].strip() if v.is_file() else "unknown"
    return f"FSL {version} (fsl-bet2 2111.8, fsl-newimage 2203.12, fsl-meshclass 2111.0)"


def replica_binary() -> str | None:
    """The ``larmorx-nc`` program: ``LARMORX_NC_BIN``, else the release (then debug) build of
    ``crates-nc`` in this repository, else ``larmorx-nc`` on the PATH."""
    if env := os.environ.get("LARMORX_NC_BIN"):
        return env
    exe = ".exe" if os.name == "nt" else ""
    for profile in ("release", "debug"):
        for parent in Path(__file__).resolve().parents:
            candidate = parent / REPLICA.format(profile=profile, exe=exe)
            if candidate.is_file():
                return str(candidate)
    return shutil.which("larmorx-nc")


@functools.cache
def replica_version() -> str:
    binary = replica_binary()
    if binary is None:
        return "not found"
    out = subprocess.run([binary, "--version"], capture_output=True, text=True, encoding="utf-8")
    return out.stdout.strip() or "unknown version"


def fmriprep_image() -> Path | None:
    ws = workspace()
    image = None if ws is None else ws / FMRIPREP_IMAGE
    return image if image is not None and image.is_file() and shutil.which("apptainer") else None


@functools.cache
def generated_dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-fsl-gen-"))


@functools.cache
def home() -> Path:
    h = generated_dir() / "home"
    h.mkdir(exist_ok=True)
    return h


def environment(
    output_type: str = "NIFTI_GZ", extra: tuple[tuple[str, str | None], ...] = ()
) -> dict[str, str]:
    """The environment both programs run in: no FSL program on the PATH, ``FSLDIR`` for the
    ``bet`` script, an empty ``HOME``, the case's ``FSLOUTPUTTYPE`` and variables."""
    env = {
        "HOME": str(home()),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsl_dir() or ""),
        "FSLOUTPUTTYPE": output_type,
        "LC_ALL": "C",
        "OMP_NUM_THREADS": "2",
    }
    for name, value in extra:
        if value is None:
            env.pop(name, None)
        else:
            env[name] = value
    return env


@dataclass
class Run:
    code: int
    out: str
    err: str
    directory: Path


#: The extensions FSL's image layer adds to a name given without one (``bet in out`` reads
#: ``in.nii.gz``): the header-bearing files (an ``.img`` is read beside its ``.hdr``).
IMAGE_EXTENSIONS = (".nii.gz", ".nii", ".hdr", ".hdr.gz")

#: Defects whose FSL result depends on memory FSL never wrote: such runs are never cached
#: (the oracle cache's rule for undefined values).
UNDEFINED_MEMORY = ("FSL-43", "FSL-51", "FSL-53")


def implicit_inputs(args: list[str], cwd: Path) -> list[Path]:
    """The image files FSL reads that ``args`` name without their extension (every candidate
    that exists, as FSL's name rules look for them)."""
    out: list[Path] = []
    for a in args:
        if not a or a.startswith("-"):
            continue
        base = Path(a) if a.startswith("/") else cwd / a
        if base.is_file():
            continue
        for ext in IMAGE_EXTENSIONS:
            cand = Path(str(base) + ext)
            if cand.is_file() and cand not in out:
                out.append(cand)
    return out


def case_never(s: Scenario) -> str:
    """Why the case's runs of FSL are never served from the oracle cache ("" when they may be)."""
    if s.files:
        return (
            "files in the run's directory that bet may remove (FSL-46): the cache replays no "
            "removal"
        )
    text = " ".join((s.reason, s.fix_refuses or "", *s.silent_ok))
    for d in UNDEFINED_MEMORY:
        if d in text:
            return f"{d}: FSL reads memory it never wrote"
    return ""


def oracle_run(
    program: str,
    args: list[str],
    env: dict[str, str],
    cwd: Path,
    *,
    build: str = "oracle",
    binds: tuple[Path, ...] = (),
    never: str = "",
) -> Run:
    """Runs FSL's ``program``: the oracle installation (``build="oracle"``) or fMRIPrep
    25.2.6's FSL in its image (``build="fmriprep"``, with ``cwd`` and ``binds`` bound at their
    own paths). Every run of FSL in this suite goes through here, and through the oracle-output
    cache (:mod:`larmorx_validation.oracle_cache`): BET is single-threaded and draws no random
    numbers, so a run is served once it ran (crashes always run; ``never`` says why a case's
    runs always run, :func:`case_never`). The images it reads by a name without extension are
    keyed by their content (:func:`implicit_inputs`); every file it writes is in ``cwd``."""
    from larmorx_validation import oracle_cache

    policy = oracle_cache.Policy(threads=1, never=never, family="fsl")
    inputs = implicit_inputs(args, cwd)
    if build == "oracle":
        d = fsl_dir()
        assert d is not None
        p = oracle_cache.run(
            [str(d / "bin" / program), *args],
            env=env,
            cwd=cwd,
            outputs=[cwd],
            policy=policy,
            runner="fsl-bet-replica",
            inputs=inputs,
        )
        return Run(p.returncode, p.stdout, p.stderr, cwd)
    image = fmriprep_image()
    assert image is not None
    inner_fsl = "/app/.pixi/envs/fmriprep"
    bind = sorted({str(cwd), str(home()), *(str(b) for b in binds)})
    inner_env = {
        "HOME": str(home()),
        "FSLDIR": inner_fsl,
        "FSLOUTPUTTYPE": env.get("FSLOUTPUTTYPE", "NIFTI_GZ"),
        "PATH": "/usr/bin:/bin",
    }
    cmd = ["apptainer", "exec", "--cleanenv", "--no-home", "--pwd", str(cwd)]
    for b in bind:
        cmd += ["--bind", f"{b}:{b}"]
    for k in ("HOME", "FSLDIR", "FSLOUTPUTTYPE", "PATH"):
        cmd += ["--env", f"{k}={inner_env[k]}"]
    program_path = f"{inner_fsl}/bin/{program}"
    p = oracle_cache.run_apptainer(
        image,
        program_path,
        args,
        inner_env,
        [*cmd, str(image), program_path, *args],
        cwd=cwd,
        outputs=[cwd],
        policy=policy,
        runner="fsl-bet-replica",
        inputs=inputs,
    )
    return Run(p.returncode, p.stdout, p.stderr, cwd)


@contextlib.contextmanager
def process_environment(env: dict[str, str], cwd: Path | None = None) -> Iterator[None]:
    """This process's environment and working directory for an in-process larmorx run, which
    starts the replica's program with them; restored after."""
    saved, saved_cwd = dict(os.environ), os.getcwd()
    os.environ.clear()
    os.environ.update(env)
    if cwd is not None:
        os.chdir(cwd)
    try:
        yield
    finally:
        os.environ.clear()
        os.environ.update(saved)
        os.chdir(saved_cwd)


def run_larmorx(tool: str, args: list[str], mode: str, env: dict[str, str], cwd: Path) -> Run:
    """``larmorx fsl <tool>`` in process (it runs the ``larmorx-nc`` program as a separate
    process); ``mode`` is D13's choice."""
    from larmorx.cli import run

    env = dict(env, LARMORX_NC_BIN=str(replica_binary()))
    out, err = io.StringIO(), io.StringIO()
    with (
        process_environment(env, cwd),
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(out),
    ):
        code = run(["larmorx", "fsl", tool, *args], implementation="replica")
    return Run(code, out.getvalue(), err.getvalue(), cwd)


# ------------------------------------------------------------------------------------------------
# Messages

_TERMINATE = re.compile(
    r"terminate called after throwing an instance of [^\n]*\n(?:  what\(\): [^\n]*\n)?"
)
_BASH_SIGNAL = re.compile(r"^[^\n]*: line \d+: +\d+ (?:Aborted|Segmentation fault)[^\n]*\n", re.M)


def larmorx_own(line: str) -> bool:
    """A line larmorx adds: an upstream-defect notice or one of its own messages."""
    s = line.rstrip("\n")
    return s.startswith(("larmorx: ", "larmorx-nc: ")) or s.endswith("(larmorx)")


def fsl_text(text: str) -> str:
    """FSL's messages as compared: the ``bet`` script's own path (``$0``) read as ``bet``."""
    d = fsl_dir()
    if d is not None:
        text = text.replace(f"{d}/bin/bet", "bet")
    return text


def larmorx_text(text: str) -> str:
    return "".join(ln for ln in text.splitlines(keepends=True) if not larmorx_own(ln))


def first_difference(a: str, b: str) -> str:
    for i, (x, y) in enumerate(zip(a, b, strict=False)):
        if x != y:
            return f"at character {i}: FSL {a[max(0, i - 30) : i + 40]!r}, larmorx {b[max(0, i - 30) : i + 40]!r}"
    return f"FSL {len(a)} characters, larmorx {len(b)}: {a[len(b) :][:80]!r} / {b[len(a) :][:80]!r}"


def crashed(run: Run) -> bool:
    """Whether FSL aborted (an uncaught exception, a signal), directly or inside ``bet``."""
    return (
        run.code < 0
        or run.code in (134, 139)
        or bool(_TERMINATE.search(run.err))
        or bool(_BASH_SIGNAL.search(run.err))
    )


def before_crash(text: str) -> str:
    m = _TERMINATE.search(text)
    if m:
        text = text[: m.start()]
    return _BASH_SIGNAL.sub("", text)


def check_messages(checks: CheckList, fsl: Run, ours: Run, crash: bool) -> None:
    a_out, a_err = fsl_text(fsl.out), fsl_text(fsl.err)
    b_out, b_err = larmorx_text(ours.out), larmorx_text(ours.err)
    if crash:
        a_err = before_crash(a_err)
        name = CRASH
    else:
        name = MESSAGES
    same = a_out == b_out and a_err == b_err
    detail = (
        "the same"
        if same
        else (
            "stdout " + first_difference(a_out, b_out)
            if a_out != b_out
            else "stderr " + first_difference(a_err, b_err)
        )
    )
    checks(name, same, detail=detail)


# ------------------------------------------------------------------------------------------------
# Outputs


def content(path: Path) -> bytes:
    raw = path.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def is_image(name: str) -> bool:
    return name.endswith((".nii", ".nii.gz", ".hdr", ".hdr.gz", ".img", ".img.gz"))


def image_data(path: Path) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(path).dataobj.get_unscaled())


def dice(a: np.ndarray, b: np.ndarray) -> float:
    ma, mb = a != 0, b != 0
    s = int(ma.sum()) + int(mb.sum())
    return 1.0 if s == 0 else 2.0 * int((ma & mb).sum()) / s


def written(d: Path, inputs: tuple[str, ...] = ()) -> list[str]:
    return sorted(p.name for p in d.iterdir() if p.is_file() and p.name not in inputs)


@dataclass(frozen=True)
class FileComparison:
    """One file of two runs: bytes the same, and for an image whether the header is and how
    many values differ (``None`` when not an image or not comparable)."""

    same: bool
    header_same: bool
    differing: int | None
    total: int
    dice: float | None
    detail: str


def compare_file(pa: Path, pb: Path) -> FileComparison:
    ca, cb = content(pa), content(pb)
    if ca == cb:
        return FileComparison(True, True, 0, 0, None, "byte for byte")
    name = pa.name
    if is_image(name) and not name.endswith((".img", ".img.gz")):
        try:
            da, db = image_data(pa), image_data(pb)
        except Exception as e:
            return FileComparison(False, False, None, 0, None, f"differs ({type(e).__name__})")
        header_same = ca[:348] == cb[:348]
        if da.shape != db.shape:
            return FileComparison(
                False, header_same, None, int(da.size), None, f"FSL {da.shape}, larmorx {db.shape}"
            )
        n = int(np.count_nonzero(da.astype(np.float64) != db.astype(np.float64)))
        d = dice(da, db) if "mask" in name else None
        head = "the header differs" if not header_same else "the header is the same"
        detail = f"{head}; {n} of {da.size} values differ" + (
            f", Dice {d:.6f}" if d is not None else ""
        )
        return FileComparison(False, header_same, n, int(da.size), d, detail)
    return FileComparison(
        False,
        False,
        None,
        0,
        None,
        first_difference(ca.decode("latin-1")[:20000], cb.decode("latin-1")[:20000]),
    )


def compare_dirs(
    checks: CheckList, a: Path, b: Path, inputs: tuple[str, ...] = (), label: str = ""
) -> dict[str, FileComparison]:
    """Every file FSL wrote in ``a`` against the replica's in ``b``, a check each."""
    fa, fb = written(a, inputs), written(b, inputs)
    checks(
        f"{label}{FILES}", fa == fb, detail=", ".join(fa) if fa == fb else f"FSL {fa}, larmorx {fb}"
    )
    out: dict[str, FileComparison] = {}
    for name in fa:
        if name not in fb:
            continue
        c = compare_file(a / name, b / name)
        out[name] = c
        checks(f"{label}{name}", c.same, metric="bytes", value=c.dice, detail=c.detail)
    return out


def all_same(checks: CheckList, files: dict[str, FileComparison]) -> bool:
    return all(c.passed for c in checks.checks) and all(f.same for f in files.values())


# ------------------------------------------------------------------------------------------------
# Scenarios


@dataclass(frozen=True)
class Scenario:
    """A case: the tool, its arguments (``{name}``: an input's path), the inputs (names given to
    :func:`input_path`), and what to expect."""

    tool: str
    args: tuple[str, ...]
    inputs: tuple[str, ...]
    tier: str = "smoke"
    output_type: str = "NIFTI_GZ"
    env: tuple[tuple[str, str | None], ...] = ()
    #: ``pass`` or ``error`` (both refuse) or ``crash`` (FSL crashes, the replica refuses).
    expect: str = "pass"
    #: Files in each program's directory before the run (name, text).
    files: tuple[tuple[str, str], ...] = ()
    #: The Python equivalent: ``(function, keyword arguments, {output file: result field})``.
    python: tuple[str, dict[str, Any], dict[str, str]] | None = None
    #: fMRIPrep's own outputs of this call: ``(node directory, {our file: its file})``.
    fmriprep: tuple[str, dict[str, str]] | None = None
    #: Also run FSL a second time and fMRIPrep's FSL (FSL's own variability).
    band: bool = False
    #: The defect whose correction makes the default refuse (exit 1, the defect named).
    fix_refuses: str | None = None
    #: Defects whose notice may be named without a visible change.
    silent_ok: tuple[str, ...] = ()
    #: Known brain mask (a phantom's): the file to measure against it, and the input's name.
    truth: tuple[str, str] | None = None
    reason: str = ""


def _fmriprep_calls() -> list[tuple[str, Path, Path]]:
    """``(id, input, node directory)`` of every ``bet`` call of the oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    root = ws / FMRIPREP_RUNS
    for cmd in sorted(root.glob("*/work/**/skullstrip_first_pass/command.txt")):
        node = cmd.parent
        words = cmd.read_text(encoding="utf-8").split()
        if not words or words[0] != "bet":
            continue
        run = node.relative_to(root).parts[0]
        source = node.parent / "n4_correct" / words[1]
        if not source.is_file():
            hits = sorted(root.glob(f"{run}/**/{words[1]}"))
            source = hits[0] if hits else source
        wf = node.parent.name.replace("_wf", "")
        bold = next(
            (
                p
                for p in node.parts
                if p.startswith("bold_") and p.endswith("_wf") and p != "bold_fit_wf"
            ),
            "bold",
        )
        out.append(
            (
                f"{run.split('-sub')[0]}-{bold.removeprefix('bold_').removesuffix('_wf')}-{wf}",
                source,
                node,
            )
        )
    return out


def _scenarios() -> list[tuple[str, str, str, Scenario]]:
    # fmt: off
    out: list[tuple[str, str, str, Scenario]] = []
    S = Scenario

    def add(cid: str, category: str, desc: str, s: Scenario) -> None:
        out.append((f"{category}/{cid}", category, desc, s))

    # --- fMRIPrep's calls ------------------------------------------------------------------------
    for i, (cid, source, node) in enumerate(_fmriprep_calls()):
        theirs = {p.name for p in node.glob("*_brain*.nii.gz")}
        brain = next((n for n in theirs if n.endswith("_brain.nii.gz")), None)
        mask = next((n for n in theirs if n.endswith("_brain_mask.nii.gz")), None)
        mapping = {k: v for k, v in (("out.nii.gz", brain), ("out_mask.nii.gz", mask)) if v}
        add(cid, "fmriprep", f"fMRIPrep's call `bet <boldref> <out> -f 0.20 -m` ({source.name})",
            S("bet", ("{0}", "out", "-f", "0.20", "-m"), (str(source),),
              tier="smoke" if i == 0 else "standard",
              python=("bet", {"frac": 0.2, "mask": True}, {"out.nii.gz": "brain", "out_mask.nii.gz": "mask"}),
              fmriprep=(str(node), mapping), band=True))

    # --- bet2's options, on the synthetic head ---------------------------------------------------
    h = "head"
    add("default", "bet2", "bet2's defaults (-f 0.5)", S("bet2", ("{0}", "out"), (h,), truth=("out.nii.gz", h)))
    add("all-outputs", "bet2", "-o -m -s -e -v: every output, verbose",
        S("bet2", ("{0}", "out", "-o", "-m", "-s", "-e", "-v"), (h,), truth=("out_mask.nii.gz", h),
          python=("bet", {"mask": True, "outline": True, "skull": True, "mesh": True},
                  {"out.nii.gz": "brain", "out_mask.nii.gz": "mask", "out_overlay.nii.gz": "outline", "out_skull.nii.gz": "skull"})))
    for f in ("0.1", "0.3", "0.7", "0.9"):
        add(f"f-{f}", "bet2", f"-f {f}", S("bet2", ("{0}", "out", "-f", f, "-m"), (h,),
                                          tier="smoke" if f == "0.3" else "standard", truth=("out_mask.nii.gz", h)))
    for g in ("-0.4", "0.4"):
        add(f"g{g}", "bet2", f"-g {g}", S("bet2", ("{0}", "out", "-g", g, "-m"), (h,), tier="standard"))
    add("r", "bet2", "-r 60 (the radius)", S("bet2", ("{0}", "out", "-r", "60", "-m"), (h,), tier="standard"))
    add("c", "bet2", "-c: the centre in voxels", S("bet2", ("{0}", "out", "-c", "48", "56", "38", "-m"), (h,)))
    add("w", "bet2", "-w 2 and -w 0.5 (smoothness)", S("bet2", ("{0}", "out", "-w", "2", "-m"), (h,), tier="standard"))
    add("w-0.5", "bet2", "-w 0.5", S("bet2", ("{0}", "out", "-w", "0.5", "-m"), (h,), tier="standard"))
    add("t", "bet2", "-t: thresholding", S("bet2", ("{0}", "out", "-t", "-m"), (h,), tier="standard"))
    add("n", "bet2", "-n: no brain image", S("bet2", ("{0}", "out", "-n", "-m"), (h,)))
    add("ct", "bet2", "--ct: CT intensities", S("bet2", ("{0}", "out", "--ct", "-m"), ("head-ct",), tier="standard"))
    add("d1d2", "bet2", "the hidden --parameter_d1/d2", S("bet2", ("{0}", "out", "--parameter_d1=5", "--parameter_d2=2", "-m"), (h,), tier="standard"))
    add("short-run", "bet2", "a run of one-letter options (-mno)", S("bet2", ("{0}", "out", "-mno"), (h,), tier="standard"))
    add("repeated", "bet2", "an option given twice keeps the last", S("bet2", ("{0}", "out", "-f", "0.2", "-f", "0.6", "-m"), (h,), tier="standard"))
    add("t2", "bet2", "the T2-like head", S("bet2", ("{0}", "out", "-m"), ("head-t2",)))
    add("help", "bet2", "-h", S("bet2", ("-h",), ()))
    add("usage", "bet2", "no arguments", S("bet2", (), (), expect="error"))
    add("one-argument", "bet2", "one argument", S("bet2", ("{0}",), (h,), expect="error"))
    add("dash-names", "bet2", "an output name starting with -", S("bet2", ("{0}", "-m"), (h,), expect="error"))
    add("unknown-option", "bet2", "an unknown option", S("bet2", ("{0}", "out", "-x"), (h,), expect="error"))
    add("missing-value", "bet2", "-f without its value", S("bet2", ("{0}", "out", "-f"), (h,), expect="error"))
    add("bad-number", "bet2", "-f with a word", S("bet2", ("{0}", "out", "-f", "abc"), (h,), expect="error"))
    add("long-without-equals", "bet2", "--parameter_d1 5: long options need =", S("bet2", ("{0}", "out", "--parameter_d1", "5"), (h,), expect="error"))
    add("f-out-of-range", "bet2", "-f 1.5", S("bet2", ("{0}", "out", "-f", "1.5"), (h,), expect="error"))
    add("g-out-of-range", "bet2", "-g -2", S("bet2", ("{0}", "out", "-g", "-2"), (h,), expect="error"))
    add("unrecognised-token", "bet2", "a third word", S("bet2", ("{0}", "out", "extra"), (h,), expect="error"))
    add("missing-input", "bet2", "an input that does not exist (FSL aborts)",
        S("bet2", ("/nonexistent/in", "out"), (), expect="crash"))
    add("img-output", "bet2", "an output name with .img and no .hdr (FSL-40: FSL aborts)",
        S("bet2", ("{0}", "out.img"), (h,), expect="crash"))
    add("hdr-in-name", "defects", "an output name with .hdr inside (FSL-40)", S("bet2", ("{0}", "a.hdrb", "-m"), (h,)))
    add("constant", "bet2", "a constant image (FSL-41: FSL reads an empty list and crashes; reproduce fills the median with 0)",
        S("bet2", ("{0}", "out", "-m"), ("constant",), expect="crash-filled", fix_refuses="FSL-41", tier="standard"))
    add("nan-f", "defects", "-f nan (FSL-42; FSL crashes in its fill, FSL-43)", S("bet2", ("{0}", "out", "-f", "nan", "-m"), (h,), expect="crash-filled", fix_refuses="FSL-42", tier="standard"))
    add("negative-r", "defects", "-r -50 (FSL-42)", S("bet2", ("{0}", "out", "-r", "-50", "-m"), (h,), fix_refuses="FSL-42", tier="standard"))
    add("zero-w", "defects", "-w 0 (FSL-42; FSL crashes in its fill, FSL-43)", S("bet2", ("{0}", "out", "-w", "0", "-m"), (h,), expect="crash-filled", fix_refuses="FSL-42", tier="standard"))

    # --- Storage types, scaling, orientation and formats ----------------------------------------
    for img in ("head-i16", "head-u8", "head-i8", "head-u16", "head-i32", "head-f64", "head-i16-scaled",
                "head-neuro", "head-qform-only", "head-sform-only", "head-noxform", "head-oblique",
                "head-nifti2", "head-uncompressed"):
        add(img, "formats", f"{img}: brain and mask", S("bet2", ("{0}", "out", "-m"), (img,),
                                                        tier="smoke" if img in ("head-i16", "head-neuro") else "standard"))
    add("pair-input", "formats", "a NIfTI pair input (.hdr/.img)", S("bet2", ("{0}", "out", "-m"), ("head-pair",), tier="standard"))
    add("analyze-input", "formats", "an ANALYZE 7.5 input", S("bet2", ("{0}", "out", "-m"), ("head-analyze",), tier="standard"))
    for t in ("NIFTI", "NIFTI_PAIR", "NIFTI2", "NIFTI2_GZ", "NIFTI_PAIR_GZ", "ANALYZE", "ANALYZE_GZ"):
        add(f"output-{t}", "formats", f"FSLOUTPUTTYPE={t}", S("bet2", ("{0}", "out", "-m", "-s"), (h,), output_type=t,
                                                               tier="smoke" if t == "NIFTI" else "standard"))
    add("output-unset", "formats", "FSLOUTPUTTYPE unset (FSL stops)", S("bet2", ("{0}", "out", "-m"), (h,), env=(("FSLOUTPUTTYPE", None),), expect="error", tier="standard"))
    add("output-bad", "formats", "FSLOUTPUTTYPE=JPEG", S("bet2", ("{0}", "out", "-m"), (h,), output_type="JPEG", expect="error", tier="standard"))
    add("4d-input", "formats", "a 4D input: its first volume, with FSL's warning", S("bet2", ("{0}", "out", "-m"), ("head-4d",)))
    add("2d", "defects", "a single slice: the mask's fill starts below the image (FSL-43)",
        S("bet2", ("{0}", "out", "-m"), ("head-2d",), tier="standard", silent_ok=("FSL-43",)))
    add("extensions", "defects", "two extensions of different sizes kept (FSL_LOAD_NIFTI_EXTENSIONS=1, FSL-56)",
        S("bet2", ("{0}", "out", "-m"), ("head-ext",), env=(("FSL_LOAD_NIFTI_EXTENSIONS", "1"),), output_type="NIFTI", tier="standard"))
    add("extensions-dropped", "formats", "extensions in the input, not kept by default", S("bet2", ("{0}", "out", "-m"), ("head-ext",), output_type="NIFTI", tier="standard"))

    # --- The bet script ---------------------------------------------------------------------------
    add("default", "bet", "bet's defaults", S("bet", ("{0}", "out"), (h,)))
    add("options", "bet", "every bet2 option bet passes on", S("bet", ("{0}", "out", "-o", "-m", "-s", "-e", "-t", "-f", "0.4", "-g", "0.1", "-r", "70", "-c", "48", "56", "38"), (h,), tier="standard"))
    add("verbose", "bet", "-v: the script's variables, then bet2's messages", S("bet", ("{0}", "out", "-m", "-v"), (h,)))
    add("robust", "bet", "-R: bet2 again from the brain's centre", S("bet", ("{0}", "out", "-R", "-m"), ("head-offcentre",),
                                                                      python=("bet", {"variation": "robust", "mask": True}, {"out.nii.gz": "brain", "out_mask.nii.gz": "mask"})))
    add("robust-n", "bet", "-R -n: the script removes -n", S("bet", ("{0}", "out", "-R", "-n", "-m"), (h,), tier="standard"))
    add("robust-verbose", "bet", "-R -v", S("bet", ("{0}", "out", "-R", "-v"), (h,), tier="standard"))
    add("fmri", "bet", "-F: the first volume's mask, dilated, on every volume", S("bet", ("{0}", "out", "-F"), ("head-4d",),
                                                                                    python=("bet", {"variation": "fmri"}, {"out.nii.gz": "brain", "out_mask.nii.gz": "mask"})))
    add("fmri-verbose", "bet", "-F -v -f 0.4", S("bet", ("{0}", "out", "-F", "-v", "-f", "0.4"), ("head-4d",), tier="standard"))
    add("small-fov", "bet", "-Z: a slab padded to 100 mm (FSL-45: the crop back needs imglob)", S("bet", ("{0}", "out", "-Z", "-m"), ("slab",)))
    add("small-fov-verbose", "bet", "-Z -v -s", S("bet", ("{0}", "out", "-Z", "-m", "-v", "-s"), ("slab",), tier="standard"))
    add("small-fov-large", "bet", "-Z on an image already 100 mm high: no padding", S("bet", ("{0}", "out", "-Z", "-m"), (h,), tier="standard"))
    add("debug", "bet", "-d: the temporary files kept", S("bet", ("{0}", "out", "-Z", "-d"), ("slab",), tier="standard"))
    for v, need in (("-S", "eyes"), ("-B", "bias"), ("-A", "surfaces")):
        add(f"pending{v}", "bet", f"{v}: needs FSL programs larmorx does not replicate yet ({need})",
            S("bet", ("{0}", "out", v), (h,), expect="pending", tier="standard"))
    add("pending-A2", "bet", "-A2 <T2>: pending (FLIRT)", S("bet", ("{0}", "out", "-A2", "{1}"), (h, "head-t2"), expect="pending", tier="standard"))
    add("usage", "bet", "bet without an output", S("bet", ("{0}",), (h,), expect="error"))
    add("no-input", "bet", "an input that does not exist", S("bet", ("/nonexistent/in", "out"), (), expect="error"))
    add("unknown-option", "bet", "an unknown option", S("bet", ("{0}", "out", "-w", "2"), (h,), expect="error"))
    add("missing-value", "bet", "-f without its value", S("bet", ("{0}", "out", "-f"), (h,), expect="error"))
    add("c-short", "bet", "-c with two numbers", S("bet", ("{0}", "out", "-c", "1", "2"), (h,), expect="error"))
    add("bet2-fails", "bet", "bet2 refuses (-f 2): the script's ERR trap", S("bet", ("{0}", "out", "-f", "2"), (h,), expect="error"))
    add("blank-in-output", "defects", "an output name with a blank (FSL-44)", S("bet", ("{0}", "my out", "-m"), (h,), tier="standard"))
    add("empty-argument", "defects", "an empty argument ends the options (FSL-44)", S("bet", ("{0}", "out", "", "-m"), (h,), tier="standard"))
    add("cleanup", "defects", "the cleanup glob removes out_tmp* files it did not create (FSL-46)",
        S("bet", ("{0}", "out", "-F"), ("head-4d",), files=(("out_tmp_notes.txt", "keep me\n"),), tier="standard"))

    # --- betsurf ------------------------------------------------------------------------------------
    sb = ("head-mesh", "head-mat")
    add("t1only", "betsurf", "--t1only -o -m -s: bet -A's call", S("betsurf", ("--t1only", "-o", "-m", "-s", "{0}", "{1}", "{2}", "out"), ("head", *sb),
                                                                   python=("betsurf", {"skull_mask": True, "outlines": True}, {"out_inskull_mask.nii.gz": "inskull_mask", "out_skull_mask.nii.gz": "skull_mask"})))
    add("t1t2", "betsurf", "-o -m -s with the T2: bet -A2's call", S("betsurf", ("-o", "-m", "-s", "{0}", "{1}", "{2}", "{3}", "out"), ("head", "head-t2", *sb)))
    add("verbose", "betsurf", "-v -m", S("betsurf", ("-v", "-m", "{0}", "{1}", "{2}", "{3}", "out"), ("head", "head-t2", *sb), tier="standard"))
    add("precision", "betsurf", "-p 1: retessellated", S("betsurf", ("-1", "-p", "1", "-m", "{0}", "{1}", "{2}", "out"), ("head", *sb), tier="standard"))
    add("surfaces-only", "betsurf", "no option: the meshes only", S("betsurf", ("{0}", "{1}", "{2}", "{3}", "out"), ("head", "head-t2", *sb), tier="standard"))
    add("help", "betsurf", "-h", S("betsurf", ("-h",), ()))
    add("too-few", "betsurf", "too few arguments", S("betsurf", ("{0}", "{1}", "{2}"), ("head", *sb), expect="error"))
    add("too-few-t1only", "defects", "--t1only with too few arguments exits 0 (FSL-54)", S("betsurf", ("--t1only", "{0}", "{1}"), ("head", "head-mesh"), tier="standard"))
    add("no-matrix", "betsurf", "a matrix that cannot be opened", S("betsurf", ("{0}", "{1}", "{2}", "/nonexistent.mat", "out"), ("head", "head-t2", "head-mesh"), expect="error"))
    add("no-matrix-t1only", "defects", "--t1only with a missing matrix exits 0 (FSL-54)", S("betsurf", ("-1", "{0}", "{1}", "/nonexistent.mat", "out"), ("head", "head-mesh"), tier="standard"))
    add("no-mesh", "betsurf", "a mesh that cannot be opened", S("betsurf", ("{0}", "{1}", "/nonexistent.vtk", "{2}", "out"), ("head", "head-t2", "head-mat"), expect="error"))
    add("dash-argument", "betsurf", "a word starting with - among the files", S("betsurf", ("{0}", "-o", "{1}", "{2}", "{3}", "out"), ("head", "head-t2", *sb), expect="error"))
    add("bad-option", "betsurf", "an unknown option", S("betsurf", ("-x", "{0}"), ("head",), expect="error"))
    add("p-bad", "betsurf", "-p with a word", S("betsurf", ("-p", "x", "{0}"), ("head",), expect="error"))
    add("short-matrix", "defects", "a matrix of 6 numbers (FSL-53)", S("betsurf", ("-1", "-m", "{0}", "{1}", "{2}", "out"), ("head", "head-mesh", "short-mat"),
                                                                       fix_refuses="FSL-53", tier="standard"))
    for name, desc in (("mesh-unread", "a coordinate that does not read"), ("mesh-missing-vertex", "a face naming a vertex the mesh does not have"),
                       ("mesh-truncated", "a mesh file that ends after two vertices")):
        add(name, "defects", f"{desc} (FSL-52: FSL crashes; reproduce fills what FSL leaves undefined)",
            S("betsurf", ("-1", "-m", "{0}", "{1}", "{2}", "out"), ("head", name, "head-mat"), expect="crash-filled", fix_refuses="FSL-52", tier="standard"))
    add("t2-other-grid", "defects", "a T2 on another grid (FSL-48)", S("betsurf", ("-m", "{0}", "{1}", "{2}", "{3}", "out"), ("head", "head-t2-small", *sb),
                                                                         fix_refuses="FSL-48", tier="standard"))
    add("off-mesh", "betsurf", "an OFF mesh", S("betsurf", ("-1", "-m", "{0}", "{1}", "{2}", "out"), ("head", "head-mesh-off", "head-mat"), tier="standard"))
    add("fs-mesh", "betsurf", "a FreeSurfer ASCII mesh", S("betsurf", ("-1", "-m", "{0}", "{1}", "{2}", "out"), ("head", "head-mesh-fs", "head-mat"), tier="standard"))

    # --- Real data ------------------------------------------------------------------------------------
    for name, desc in REAL_T1:
        add(name, "real", f"{desc}: bet's defaults and -m", S("bet", ("{0}", "out", "-m"), (f"real:{name}",), tier="standard", band=True))
        add(f"{name}-R", "real", f"{desc}: -R -m -s -e", S("bet", ("{0}", "out", "-R", "-m", "-s", "-e"), (f"real:{name}",), tier="full"))
    for name, desc in REAL_SURF:
        add(f"{name}-surf", "real", f"{desc}: betsurf --t1only -o -m -s on FLIRT's matrix", S("betsurf", ("--t1only", "-o", "-m", "-s", "{0}", "{1}", "{2}", "out"),
                                                                                         (f"real:{name}", f"mesh:{name}", f"mat:{name}"), tier="full"))
    for name, desc in REAL_T2:
        add(f"{name}-surf-t2", "real", f"{desc}: betsurf -o -m -s with the T2 FLIRT registered", S("betsurf", ("-o", "-m", "-s", "{0}", "{1}", "{2}", "{3}", "out"),
                                                                                                  (f"real:{name}", f"t2:{name}", f"mesh:{name}", f"mat:{name}"), tier="full"))
    add("bold-F", "real", "a BOLD series (ds000005): bet -F", S("bet", ("{0}", "out", "-F"), ("real:bold-ds000005",), tier="standard"))
    add("bold-first", "real", "a BOLD series (ds000005): bet2 on its first volume (FSL's warning)", S("bet2", ("{0}", "out", "-f", "0.3", "-m"), ("real:bold-ds000005",), tier="standard"))
    # fmt: on
    return out


#: Real T1-weighted images (``larmorx-testdata``): ``(name, description)``.
REAL_T1 = (
    ("child-3y", "a child of 3.5 years (ds000228)"),
    ("child-7y", "a child of 7.5 years with motion (ds000256)"),
    ("adult-ds000005", "an adult, the fMRIPrep oracle run's T1w (ds000005)"),
    ("older-75y", "an adult of 75 years at 1.5 T (ds002422)"),
    ("older-82y", "an adult of 82 years (ds003592)"),
    ("7t", "7 T, MP2RAGE (ds005454)"),
    ("tumour", "a tumour (ds005003)"),
    ("stroke", "a stroke (ds004884)"),
    ("philips", "a Philips scanner (ds002603)"),
)
REAL_SURF = (("adult-ds000005", "an adult (ds000005)"), ("older-82y", "an adult of 82 years"))
REAL_T2 = (
    ("child-7y", "a child of 7.5 years (ds000256)"),
    ("tumour", "a tumour (ds005003)"),
    ("philips", "a Philips scanner (ds002603)"),
)


# ------------------------------------------------------------------------------------------------
# Inputs


def _rng_ints(seed: int, n: int, lo: int, hi: int) -> np.ndarray:
    return np.random.default_rng(seed).integers(lo, hi, size=n)


#: The synthetic head's grid: 2 mm voxels.
HEAD_SHAPE = (96, 112, 84)


def _head(
    levels: tuple[int, int, int, int, int], seed: int, shape=HEAD_SHAPE, centre=None
) -> tuple[np.ndarray, np.ndarray]:
    """A head of ellipsoidal shells (brain, CSF, skull, scalp, air) with a ramp and noise, and
    its brain mask; 2 mm voxels."""
    nx, ny, nz = shape
    cx, cy, cz = centre if centre is not None else ((nx - 1) / 2, (ny - 1) / 2, (nz - 1) / 2)
    i, j, k = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
    # semi-axes of the brain in voxels: 34 x 42 x 30 (68 x 84 x 60 mm)
    r = ((i - cx) / 34) ** 2 + ((j - cy) / 42) ** 2 + ((k - cz) / 30) ** 2
    out = np.full(shape, levels[4], dtype=np.float64)
    out[r < 1.62] = levels[3]
    out[r < 1.38] = levels[2]
    out[r < 1.18] = levels[1]
    out[r < 1.0] = levels[0]
    noise = _rng_ints(seed, out.size, -20, 21).reshape(shape)
    out = out * (90 + i * 20 / nx) / 100 + noise
    return out.astype(np.float32), (r < 1.0).astype(np.uint8)


T1_LEVELS = (700, 150, 60, 800, 10)
T2_LEVELS = (400, 1500, 40, 300, 10)
AFFINE = np.array([[-2.0, 0, 0, 95.0], [0, 2.0, 0, -111.0], [0, 0, 2.0, -83.0], [0, 0, 0, 1]])


def _save(
    data: np.ndarray,
    path: Path,
    affine=AFFINE,
    *,
    kind: str = "nifti1",
    qform: int = 1,
    sform: int = 1,
    slope=None,
) -> Path:
    import nibabel as nib

    if kind == "nifti2":
        img = nib.Nifti2Image(data, affine)
    elif kind == "pair":
        img = nib.Nifti1Pair(data, affine)
    elif kind == "analyze":
        img = nib.AnalyzeImage(data, affine)
    else:
        img = nib.Nifti1Image(data, affine)
    if kind != "analyze":
        img.set_qform(affine, code=qform)
        img.set_sform(affine, code=sform)
        img.header.set_xyzt_units("mm", "sec")
        if slope is not None:
            img.header.set_slope_inter(*slope)
    nib.save(img, path)
    return path


@functools.cache
def _phantoms() -> dict[str, Path]:
    d = generated_dir() / "phantoms"
    d.mkdir(exist_ok=True)
    t1, truth = _head(T1_LEVELS, 1)
    t2, _ = _head(T2_LEVELS, 2)
    p: dict[str, Path] = {}
    p["head"] = _save(t1, d / "head.nii.gz")
    p["truth:head"] = _save(truth, d / "head_truth.nii.gz")
    p["head-t2"] = _save(t2, d / "head-t2.nii.gz")
    p["head-uncompressed"] = _save(t1, d / "head-u.nii")
    off, _ = _head(T1_LEVELS, 3, centre=(40.0, 50.0, 36.0))
    p["head-offcentre"] = _save(off, d / "head-off.nii.gz")
    p["head-i16"] = _save(np.round(t1).astype(np.int16), d / "head-i16.nii.gz")
    p["head-u8"] = _save(np.clip(np.round(t1 / 4), 0, 255).astype(np.uint8), d / "head-u8.nii.gz")
    p["head-i8"] = _save(np.clip(np.round(t1 / 8), -128, 127).astype(np.int8), d / "head-i8.nii.gz")
    p["head-u16"] = _save(np.round(t1 * 10).astype(np.uint16), d / "head-u16.nii.gz")
    p["head-i32"] = _save(np.round(t1 * 1000).astype(np.int32), d / "head-i32.nii.gz")
    p["head-f64"] = _save(t1.astype(np.float64), d / "head-f64.nii.gz")
    p["head-i16-scaled"] = _save(
        np.round(t1).astype(np.int16), d / "head-i16s.nii.gz", slope=(0.5, 10.0)
    )
    neuro = AFFINE.copy()
    neuro[0, 0], neuro[0, 3] = 2.0, -95.0
    p["head-neuro"] = _save(t1[::-1], d / "head-neuro.nii.gz", neuro)
    p["head-qform-only"] = _save(t1, d / "head-q.nii.gz", sform=0)
    p["head-sform-only"] = _save(t1, d / "head-s.nii.gz", qform=0)
    p["head-noxform"] = _save(t1, d / "head-n.nii.gz", qform=0, sform=0)
    c, s = np.cos(0.2), np.sin(0.2)
    rot = np.array([[1, 0, 0, 0], [0, c, -s, 0], [0, s, c, 0], [0, 0, 0, 1.0]])
    p["head-oblique"] = _save(t1, d / "head-obl.nii.gz", rot @ AFFINE)
    p["head-nifti2"] = _save(t1, d / "head-n2.nii", kind="nifti2")
    p["head-pair"] = _save(t1, d / "head-pair.hdr", kind="pair")
    p["head-analyze"] = _save(np.round(t1).astype(np.int16), d / "head-ana.hdr", kind="analyze")
    p["head-4d"] = _save(
        np.stack([_head(T1_LEVELS, 10 + v)[0] for v in range(3)], axis=-1), d / "head-4d.nii.gz"
    )
    p["head-2d"] = _save(t1[:, :, 42:43], d / "head-2d.nii.gz")
    p["slab"] = _save(t1[:, :, 27:57], d / "slab.nii.gz")
    p["constant"] = _save(np.full((40, 40, 30), 100.0, dtype=np.float32), d / "constant.nii.gz")
    ct = np.where(truth > 0, 40.0, -1000.0).astype(np.float32)
    ct[(t1 > 700) & (truth == 0)] = 1200.0
    p["head-ct"] = _save(ct + _rng_ints(9, ct.size, -5, 6).reshape(ct.shape), d / "head-ct.nii.gz")
    import nibabel as nib

    img = nib.Nifti1Image(t1, AFFINE)
    img.header.extensions.append(nib.nifti1.Nifti1Extension(6, b"first extension, of comments"))
    img.header.extensions.append(
        nib.nifti1.Nifti1Extension(6, b"second, longer extension: " + b"x" * 40)
    )
    nib.save(img, d / "head-ext.nii")
    p["head-ext"] = d / "head-ext.nii"
    small, _ = _head(T2_LEVELS, 4, shape=(80, 96, 70))
    p["head-t2-small"] = _save(small, d / "head-t2-small.nii.gz")
    # betsurf's inputs: FSL's own bet2 mesh of the head, and a matrix to MNI152's 2 mm grid
    mesh_dir = d / "mesh"
    mesh_dir.mkdir(exist_ok=True)
    run = oracle_run("bet2", [str(p["head"]), "head", "-e", "-n"], environment(), mesh_dir)
    assert run.code == 0, run.err
    p["head-mesh"] = mesh_dir / "head_mesh.vtk"
    p["head-mat"] = d / "head_to_std.mat"
    p["head-mat"].write_text("1 0 0 -8\n0 1 0 4\n0 0 1 -12\n0 0 0 1\n", encoding="utf-8")
    p["short-mat"] = d / "short.mat"
    p["short-mat"].write_text("1 0 0 -8\n0 1\n", encoding="utf-8")
    p["head-mesh-off"] = d / "head_mesh.off"
    p["head-mesh-off"].write_text(_vtk_to(p["head-mesh"], "off"), encoding="utf-8")
    p["head-mesh-fs"] = d / "head_mesh.asc"
    p["head-mesh-fs"].write_text(_vtk_to(p["head-mesh"], "fs"), encoding="utf-8")
    # FSL-52: mesh files FSL reads without checks (it crashes on each)
    lines = p["head-mesh"].read_text(encoding="utf-8").splitlines()
    pi = next(i for i, ln in enumerate(lines) if ln.startswith("POINTS"))
    fi = next(i for i, ln in enumerate(lines) if ln.startswith("POLYGONS"))
    bad = {
        "mesh-unread": [*lines[: pi + 1], "1 x 2", *lines[pi + 2 :]],
        "mesh-missing-vertex": [*lines[: fi + 1], "3 0 1 99999", *lines[fi + 2 :]],
        "mesh-truncated": lines[: pi + 3],
    }
    for name, body in bad.items():
        p[name] = d / f"{name}.vtk"
        p[name].write_text("\n".join(body) + "\n", encoding="utf-8")
    return p


def _vtk_to(vtk: Path, kind: str) -> str:
    words = vtk.read_text(encoding="utf-8").split()
    k = words.index("POINTS")
    n = int(words[k + 1])
    pts = words[k + 3 : k + 3 + 3 * n]
    k = words.index("POLYGONS")
    m = int(words[k + 1])
    faces = words[k + 3 : k + 3 + 4 * m]
    rows = [pts[3 * i : 3 * i + 3] for i in range(n)]
    tris = [faces[4 * i + 1 : 4 * i + 4] for i in range(m)]
    if kind == "off":
        return (
            "OFF\n"
            + f"{n} {m} 0\n"
            + "".join(" ".join(r) + "\n" for r in rows)
            + "".join("3 " + " ".join(t) + "\n" for t in tris)
        )
    return (
        "#!ascii surface\n"
        + f"{n} {m}\n"
        + "".join(" ".join(r) + " 0\n" for r in rows)
        + "".join(" ".join(t) + " 0\n" for t in tris)
    )


def _testdata(rel: str) -> Path:
    base = Path(
        os.environ.get("LARMORX_TESTDATA_CACHE", Path.home() / ".cache/larmorx-testdata/data")
    )
    p = base / rel
    if not p.is_file():
        raise FileNotFoundError(rel)
    return p


#: The real images, in the test-data cache.
REAL_FILES = {
    "child-3y": "openneuro/ds000228/sub-pixar008/anat/sub-pixar008_T1w.nii.gz",
    "child-7y": "openneuro/ds000256/sub-CTS203/anat/sub-CTS203_T1w.nii.gz",
    "t2:child-7y": "openneuro/ds000256/sub-CTS203/anat/sub-CTS203_T2w.nii.gz",
    "adult-ds000005": "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz",
    "older-75y": "openneuro/ds002422/sub-02/anat/sub-02_T1w.nii",
    "older-82y": "openneuro/ds003592/sub-280/ses-1/anat/sub-280_ses-1_T1w.nii.gz",
    "7t": "openneuro/ds005454/sub-16/anat/sub-16_acq-mp2rage_T1w.nii.gz",
    "tumour": "openneuro/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T1w.nii.gz",
    "t2:tumour": "openneuro/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T2w.nii.gz",
    "stroke": "openneuro/ds004884/sub-M2079/ses-217/anat/sub-M2079_ses-217_acq-tfl3p2_run-3_T1w.nii.gz",
    "philips": "openneuro/ds002603/sub-01/anat/sub-01_acq-SaggitalNsig1Mm1_T1w.nii.gz",
    "t2:philips": "openneuro/ds002603/sub-01/anat/sub-01_acq-Nsig1Mm1_T2w.nii.gz",
    "bold-ds000005": "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz",
}


def _real(name: str) -> Path:
    rel = REAL_FILES.get(name)
    if rel is None:
        raise FileNotFoundError(name)
    p = _testdata(rel)
    return p


@functools.cache
def _derived(kind: str, name: str) -> Path:
    """betsurf's inputs for a real image, made with FSL itself (once per run of the suite):
    ``mesh`` (bet2 -e), ``mat`` (FLIRT to MNI152 2 mm, as bet -A runs it), ``t2`` (FLIRT of the
    T2 to the T1, sinc, as bet -A2 runs it)."""
    t1 = _real(name)
    d = derived_dir(t1)
    env = environment()
    fsl = fsl_dir()
    assert fsl is not None
    if kind == "mesh":
        if not (d / "t1_mesh.vtk").is_file():
            r = oracle_run("bet2", [str(t1), "t1", "-e", "-n"], env, d)
            assert r.code == 0, r.err
        return d / "t1_mesh.vtk"
    if kind == "mat":
        if not (d / "t1_to_std.mat").is_file():
            r = oracle_run(
                "flirt",
                [
                    "-ref",
                    str(fsl / "data/standard/MNI152_T1_2mm"),
                    "-in",
                    str(t1),
                    "-omat",
                    "t1_to_std.mat",
                ],
                env,
                d,
            )
            assert r.code == 0, r.err
        return d / "t1_to_std.mat"
    if kind == "t2":
        if not (d / "t2_to_t1.nii.gz").is_file():
            t2 = _real(f"t2:{name}")
            r = oracle_run(
                "flirt",
                [
                    "-in",
                    str(t2),
                    "-ref",
                    str(t1),
                    "-out",
                    "t2_to_t1",
                    "-omat",
                    "t2_to_t1.mat",
                    "-interp",
                    "sinc",
                ],
                env,
                d,
            )
            assert r.code == 0, r.err
        return d / "t2_to_t1.nii.gz"
    raise ValueError(kind)


def derived_dir(source: Path) -> Path:
    """Where FSL's derived inputs of ``source`` are kept: ``oracles/inputs/fsl-bet/<the input's
    SHA-256>/`` in the workspace (outside git, and outside the oracle cache's directory, as the
    suites keep their inputs), so that FLIRT runs once per input."""
    import hashlib

    digest = hashlib.sha256(source.read_bytes()).hexdigest()[:20]
    ws = workspace()
    base = ws / "oracles" / "inputs" / "fsl-bet" if ws is not None else generated_dir() / "derived"
    d = base / digest
    d.mkdir(parents=True, exist_ok=True)
    return d


def input_path(name: str) -> Path:
    """The file of an input name: a phantom, a real image, or what FSL derives from one."""
    if name.startswith("/"):
        return Path(name)
    kind, _, rest = name.partition(":")
    if kind == "real":
        return _real(rest)
    if kind in ("mesh", "mat", "t2"):
        return _derived(kind, rest)
    return _phantoms()[name]


# ------------------------------------------------------------------------------------------------
# Running a case


def cases_of(tier: str) -> list[Case]:
    rank = TIERS.index(tier)
    return [
        Case(cid, category, desc, s)
        for cid, category, desc, s in _scenarios()
        if TIERS.index(s.tier) <= rank
    ]


def _arguments(s: Scenario, paths: list[Path]) -> list[str]:
    return [a.format(*[str(p) for p in paths]) if "{" in a else a for a in s.args]


def _prepare(d: Path, s: Scenario) -> None:
    d.mkdir(parents=True, exist_ok=True)
    for name, text in s.files:
        (d / name).write_text(text, encoding="utf-8")


def run_case(case: Case, checks: CheckList) -> None:
    s: Scenario = case.payload
    if fsl_dir() is None:
        raise Outcome("skipped", "FSL is not installed (~/fsl, LARMORX_FSL_DIR)")
    if replica_binary() is None:
        raise Outcome("skipped", "the larmorx-nc program is not built (crates-nc)")
    try:
        paths = [input_path(n) for n in s.inputs]
    except FileNotFoundError as e:
        raise Outcome("skipped", f"input not available: {e}") from e
    args = _arguments(s, paths)
    env = environment(s.output_type, s.env)
    inputs = tuple(name for name, _ in s.files)
    with tempfile.TemporaryDirectory(prefix="lx-fsl-") as tmp:
        root = Path(tmp)
        f_dir, r_dir = root / "fsl", root / "larmorx"
        _prepare(f_dir, s)
        _prepare(r_dir, s)
        if s.expect == "pending":
            ours = run_larmorx(s.tool, args, "reproduce", env, r_dir)
            refused = ours.code == 1 and "pending" in ours.err and "(larmorx)" in ours.err
            checks("refused as pending", refused, detail=larmorx_err_line(ours))
            raise Outcome(
                EXPECTED,
                "needs FSL programs larmorx does not replicate yet: " + larmorx_err_line(ours),
            )
        fsl = oracle_run(s.tool, args, env, f_dir, never=case_never(s))
        ours = run_larmorx(s.tool, args, "reproduce", env, r_dir)
        crash = crashed(fsl)
        filled = crash and s.expect == "crash-filled"
        if filled:
            checks(
                EXIT,
                ours.code == 0,
                detail=f"FSL crashed ({fsl.code}); larmorx fills what FSL leaves undefined and "
                f"finishes ({ours.code})",
            )
        elif crash:
            checks(EXIT, ours.code == 1, detail=f"FSL crashed ({fsl.code}), larmorx {ours.code}")
        else:
            checks(EXIT, fsl.code == ours.code, detail=f"FSL {fsl.code}, larmorx {ours.code}")
        check_messages(checks, fsl, ours, crash)
        if filled:
            _filled(checks, s, args, env, ours, r_dir, root, inputs)
        else:
            compare_dirs(checks, f_dir, r_dir, inputs)
        identical = all(c.passed for c in checks.checks)
        if s.truth is not None:
            _truth(checks, f_dir, r_dir, s)
        if s.fmriprep is not None:
            _fmriprep_own(checks, r_dir, s)
        if s.band:
            _band(checks, s, args, env, f_dir, root, paths)
        fix_dir = root / "fix"
        _prepare(fix_dir, s)
        fixed = run_larmorx(s.tool, args, "fix", env, fix_dir)
        _fix_pass(checks, s, ours, fixed, r_dir, fix_dir, inputs)
        if s.python is not None and identical and fsl.code == 0:
            _python(checks, s, paths, f_dir)
        if filled and all(c.passed for c in checks.checks):
            raise Outcome(
                EXPECTED,
                f"FSL crashes (status {fsl.code}); with upstream_defects='reproduce' larmorx fills what FSL "
                f"leaves undefined deterministically, names {s.fix_refuses} and writes its "
                "outputs; the default refuses",
            )
        if crash and all(c.passed for c in checks.checks):
            raise Outcome(BOTH_ERROR, f"FSL crashes, larmorx refuses: {larmorx_err_line(ours)}")
        if fsl.code != 0 and not crash and all(c.passed for c in checks.checks):
            raise Outcome(BOTH_ERROR, f"both refuse (exit {fsl.code}) with {last_line(fsl)}")


def _filled(
    checks: CheckList,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    ours: Run,
    r_dir: Path,
    root: Path,
    inputs: tuple[str, ...],
) -> None:
    """Where FSL crashes on what it leaves undefined and ``reproduce`` fills it (D13's failure
    cases): the replica names the defect as reproduced, writes its outputs, and gives the same
    bytes when run again (the fill is deterministic). FSL's outputs are not compared: it wrote
    none, or ones that depend on memory it never set."""
    entry = s.fix_refuses or ""
    named = f"upstream defect {entry} (" in ours.err and ") reproduced: " in ours.err
    checks(
        "what FSL leaves undefined filled, and named",
        named,
        detail=next(
            (ln for ln in ours.err.splitlines() if f"upstream defect {entry} (" in ln),
            "no notice",
        ),
    )
    written = sorted(p.name for p in r_dir.iterdir() if p.name not in inputs)
    checks("outputs written", bool(written), detail=", ".join(written) or "none")
    again = root / "larmorx-again"
    _prepare(again, s)
    run_larmorx(s.tool, args, "reproduce", env, again)
    sub = CheckList()
    compare_dirs(sub, r_dir, again, inputs)
    same = all(c.passed for c in sub.checks)
    checks(
        "the fill is deterministic (run twice)",
        same,
        detail="bit-identical" if same else "; ".join(c.detail for c in sub.checks if not c.passed),
    )


def larmorx_err_line(run: Run) -> str:
    lines = [ln for ln in run.err.splitlines() if ln.endswith("(larmorx)")]
    return lines[-1] if lines else last_line(run)


def last_line(run: Run) -> str:
    """What a refusal says: its usage, or its last line (generated paths shortened)."""
    text = run.err + run.out
    if "Usage:" in text or "Usage: " in text:
        return "FSL's usage"
    lines = [ln for ln in text.splitlines() if ln.strip()]
    return fsl_text(lines[-1]).replace(str(generated_dir()), "<generated>") if lines else ""


def _truth(checks: CheckList, f_dir: Path, r_dir: Path, s: Scenario) -> None:
    """A phantom's known brain: the Dice of FSL's mask (and so the replica's) with it."""
    name, image = s.truth  # type: ignore[misc]
    truth = image_data(input_path(f"truth:{image}"))
    for who, d in (("FSL", f_dir), ("larmorx", r_dir)):
        f = d / name
        if f.is_file():
            m = image_data(f)
            checks(
                f"Dice with the known brain ({who})",
                True,
                metric="Dice",
                value=round(dice(m, truth), 6),
                threshold="reported",
            )


def _fmriprep_own(checks: CheckList, r_dir: Path, s: Scenario) -> None:
    node, mapping = s.fmriprep  # type: ignore[misc]
    for ours, theirs in mapping.items():
        a, b = Path(node) / theirs, r_dir / ours
        if not a.is_file() or not b.is_file():
            continue
        da, db = image_data(a), image_data(b)
        same = da.shape == db.shape and da.dtype == db.dtype and np.array_equal(da, db)
        head = "header the same" if content(a)[:348] == content(b)[:348] else "header differs"
        checks(
            f"{ours}: {FMRIPREP_OWN}",
            same,
            metric="values",
            threshold="reported",
            detail=(
                "values bit-identical"
                if same
                else f"{int(np.count_nonzero(da != db))} values differ"
            )
            + f"; {head}",
        )


def _band(
    checks: CheckList,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    f_dir: Path,
    root: Path,
    paths: list[Path],
) -> None:
    """FSL's own variability: the oracle run again, and fMRIPrep's FSL build."""
    again = root / "again"
    _prepare(again, s)
    oracle_run(s.tool, args, env, again, never=case_never(s))
    sub = CheckList()
    compare_dirs(sub, f_dir, again)
    same = all(c.passed for c in sub.checks)
    checks(
        RUN_AGAIN,
        same,
        metric="files",
        threshold="reported",
        detail="bit-identical" if same else "; ".join(c.detail for c in sub.checks if not c.passed),
    )
    if fmriprep_image() is None:
        return
    fm = root / "fmriprep-fsl"
    _prepare(fm, s)
    r = oracle_run(
        s.tool,
        args,
        env,
        fm,
        build="fmriprep",
        binds=tuple(p.parent for p in paths),
        never=case_never(s),
    )
    sub = CheckList()
    files = compare_dirs(sub, f_dir, fm)
    names = sub.checks[0].passed
    # a file has other values when its data differ, not only its header (newimage's version)
    differing = [
        n for n, f in files.items() if not f.same and (f.differing is None or f.differing > 0)
    ]
    headers = [n for n, f in files.items() if not f.header_same]
    detail = (
        f"exit {r.code}; "
        + ("the same files" if names else sub.checks[0].detail)
        + (f"; other values in {', '.join(differing)}" if differing else "; the same values")
        + (f"; other headers in {', '.join(headers)}" if headers else "")
    )
    checks(
        FMRIPREP_FSL,
        True,
        metric="files with other values",
        value=len(differing) + (0 if names else 1),
        threshold="reported",
        detail=detail,
    )


def _images(d: Path, inputs: tuple[str, ...]) -> dict[str, np.ndarray]:
    """The images a run wrote, as stored; one that nibabel cannot read (FSL-56's offset) is left
    to the byte comparison."""
    out = {}
    for n in written(d, inputs):
        if is_image(n) and not n.endswith((".img", ".img.gz")):
            try:
                out[n] = image_data(d / n)
            except Exception:
                continue
    return out


def _fix_pass(
    checks: CheckList,
    s: Scenario,
    ours: Run,
    fixed: Run,
    r_dir: Path,
    fix_dir: Path,
    inputs: tuple[str, ...],
) -> None:
    entries = defects.notices_in(fixed.err)
    if s.fix_refuses is not None:
        ok = fixed.code == 1 and s.fix_refuses in entries
        checks(
            defects.FIX_PASS,
            ok,
            metric="the default refuses",
            value=1,
            threshold="reported (named iff changed)",
            detail=f"{', '.join(entries) or 'no defect named'}: the default refuses ({larmorx_err_line(fixed)})",
        )
        return
    if ours.code != 0 and fixed.code != 0:
        defects.add_fix_check(checks, entries, defects.Change(0, 0, 0.0), silent_ok=s.silent_ok)
        return
    a, b = _images(r_dir, inputs), _images(fix_dir, inputs)
    change = defects.Change(0, 0, 0.0)
    for name in sorted(set(a) & set(b)):
        change = change + defects.Change.of(b[name], a[name])
    others = []
    if sorted(a) != sorted(b) or written(r_dir, inputs) != written(fix_dir, inputs):
        others.append(f"files {written(r_dir, inputs)} became {written(fix_dir, inputs)}")
    if ours.code != fixed.code:
        others.append(f"exit status {ours.code} became {fixed.code}")
    for name in sorted(set(written(r_dir, inputs)) & set(written(fix_dir, inputs))):
        if (not is_image(name) or name not in a or name not in b) and content(
            r_dir / name
        ) != content(fix_dir / name):
            others.append(f"{name} changed")
    kept_r = [n for n in inputs if (r_dir / n).is_file()]
    kept_f = [n for n in inputs if (fix_dir / n).is_file()]
    if kept_r != kept_f:
        others.append(f"the files present before the run: {kept_r} kept, {kept_f} by the default")
    defects.add_fix_check(
        checks, entries, change, other_changes="; ".join(others), silent_ok=s.silent_ok
    )


def _python(checks: CheckList, s: Scenario, paths: list[Path], f_dir: Path) -> None:
    import larmorx as lx

    function, kwargs, outputs = s.python  # type: ignore[misc]
    env = dict(os.environ, LARMORX_NC_BIN=str(replica_binary()))
    with process_environment(env), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if function == "bet":
            result = lx.fsl.bet(str(paths[0]), upstream_defects="reproduce", **kwargs)
        else:
            t2 = None if len(paths) == 3 else paths[1]
            mesh, mat = paths[-2], paths[-1]
            result = lx.fsl.betsurf(
                str(paths[0]), str(mesh), str(mat), t2=t2, upstream_defects="reproduce", **kwargs
            )
    for name, attr in outputs.items():
        img = getattr(result, attr)
        theirs = image_data(f_dir / name)
        same = (
            img is not None
            and img.data.shape == theirs.shape
            and np.array_equal(np.asarray(img.data), theirs)
        )
        checks(
            f"{PYTHON}: {attr}", same, detail="the same values" if same else "differs from FSL's"
        )


# ------------------------------------------------------------------------------------------------
# The suite


def highlights(results: list[CaseResult]) -> list[str]:
    n = len(results)
    passed = sum(r.status == PASS for r in results)
    both = sum(r.status == BOTH_ERROR for r in results)
    expected = sum(r.status == EXPECTED for r in results)
    files = sum(1 for r in results for c in r.checks if c.metric == "bytes")
    same = sum(1 for r in results for c in r.checks if c.metric == "bytes" and c.passed)
    lines = [
        f"- **{passed} of {n} cases bit-identical to FSL** (exit status, messages, every file); "
        f"{both} where both refuse or FSL crashes and the replica refuses; {expected} pending "
        f"(options that run FSL programs larmorx does not replicate yet).",
        f"- **{same} of {files} files** written byte for byte as FSL writes them.",
    ]
    own = [c for r in results for c in r.checks if c.name.endswith(FMRIPREP_OWN)]
    if own:
        lines.append(
            f"- fMRIPrep's own BET outputs: {sum(c.passed for c in own)} of {len(own)} with the same values as the replica's."
        )
    again = [c for r in results for c in r.checks if c.name == RUN_AGAIN]
    if again:
        lines.append(
            f"- FSL run again: {sum(c.passed for c in again)} of {len(again)} runs bit-identical (FSL is deterministic)."
        )
    fm = [c for r in results for c in r.checks if c.name == FMRIPREP_FSL]
    if fm:
        lines.append(
            f"- fMRIPrep 25.2.6's FSL (fsl-newimage 2501.4, fsl-miscmaths 2412.4): {sum(c.value == 0 for c in fm)} "
            f"of {len(fm)} runs with the oracle's values."
        )
    lines += defects.summary(results)
    return lines


def suite(implementation: str = "replica") -> Suite:
    """The BET suite for the replica (``larmorx-nc``): ``larmorx fsl bet|bet2|betsurf`` and
    ``lx.fsl.bet`` / ``lx.fsl.betsurf``."""
    if implementation != "replica":
        raise ValueError(
            "BET has only its replica so far (its clean-room original is to be written from specs/bet.md)"
        )
    tool = (
        f"`larmorx-nc fsl bet`, `bet2` and `betsurf` ({replica_version()}; crate larmorx-nc-bet, the "
        "replica translated from FSL 6.0.7.17's source under the FSL Licence), run by larmorx as a separate "
        "process: `larmorx fsl <tool>` and `lx.fsl.bet` / `lx.fsl.betsurf`"
    )
    return Suite(
        name="fsl-bet-replica",
        title="BET (replica)",
        tool=tool,
        reference=f"bet, bet2 and betsurf from {fsl_version()}, the conda installation in `~/fsl`",
        thresholds=[
            (
                "Exit status",
                "the same (both succeed, or both refuse); where FSL crashes, larmorx exits 1",
            ),
            (
                "Messages",
                "standard output and error byte for byte (larmorx's own lines set aside; the bet script's path read as `bet`)",
            ),
            (
                "Every file written",
                "byte for byte (images decompressed): header and data, meshes as text",
            ),
        ],
        cases=cases_of,
        run_case=run_case,
        packages=("nibabel",),
        highlights=highlights,
        notes=(
            "Both programs run in directories of their own on the same input files, with `PATH=/usr/bin:/bin` "
            "(FSL never on it), `FSLDIR` set, an empty `HOME` and the case's `FSLOUTPUTTYPE`. The replica is "
            "compared in `reproduce` mode; the fix pass runs its default. The fMRIPrep cases also compare the "
            "replica with fMRIPrep's own outputs of the node and run FSL again and fMRIPrep 25.2.6's FSL (FSL's "
            "own variability). The `-S`, `-B`, `-A` and `-A2` variations of `bet` run FSL programs larmorx does "
            "not replicate yet (standard_space_roi, FLIRT, FAST): the replica refuses them, naming what is missing."
        ),
    )
