# SPDX-License-Identifier: Apache-2.0
"""What the parity suites of AFNI programs with only a replica share (3dAutomask, 3dUnifize):
running AFNI's program and the replica on the same files, and comparing exit statuses,
messages and every output, byte for byte.

AFNI runs as its own binary (``scripts/build_afni_oracle.sh``; ``LARMORX_AFNI_BIN`` names its
directory or one program), the replica the way users reach it: through larmorx's own dispatch
(``larmorx afni <program>`` with ``implementation="replica"``), which runs the ``larmorx-gpl``
program as a separate process (``LARMORX_GPL_BIN``, else the ``crates-gpl`` build). This package
is Apache-2.0 and never loads the replica.

Each program runs in a directory of its own, with the same environment: this process's without
AFNI's variables, ``HOME`` an empty directory (no ``.afnirc``), ``AFNI_DONT_LOGFILE=YES`` and
``AFNI_NIFTI_NOEXT=YES`` (larmorx never writes AFNI's history extension), plus each case's
variables. Outputs are named relative to the directory, inputs by absolute path.

What is compared:
- the exit status;
- standard output and error, byte for byte (colour codes included), without AFNI's version
  banner and compile date, larmorx's own lines (upstream-defect notices, ``-verb``'s
  implementation line) and the number in ``Elapsed = ... sec``; where AFNI crashes, the lines it
  printed before the signal against the replica's before its refusal;
- every output: its data (type, shape and every stored value, bit for bit) and its NIfTI header
  (the bytes before the data, decompressed for ``.nii.gz``).

A fix pass then runs the replica's default (``upstream_defects="fix"``) on the same arguments
and reports which defects it corrected and what that changed (D13); and the cases with a Python
equivalent run ``lx.afni.<function>(..., implementation="replica")`` too, whose outputs must be
the command line's, bit for bit.
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
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.afni_tshift import replica_binary
from larmorx_validation.parity.afni_tshift import replica_version as replica_version
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
)

ORACLE_DIR = "oracles/afni-25.2.09/bin"
FMRIPREP_IMAGE = "oracles/images/fmriprep-25.2.6.sif"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
TIERS = ("smoke", "standard", "full")

#: The checks' names (the summaries find them by name).
EXIT = "exit status"
MESSAGES = "messages: AFNI's, byte for byte"
CRASH = "messages before AFNI's crash"


@functools.cache
def workspace() -> Path | None:
    """The workspace that holds ``oracles/`` (a parent of this file)."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def afni_binary(program: str) -> str | None:
    """The AFNI oracle ``program``: in ``LARMORX_AFNI_BIN`` (a directory, or a program whose
    directory holds the others), else the workspace's ``oracles/afni-25.2.09`` build, else on
    the PATH."""
    if env := os.environ.get("LARMORX_AFNI_BIN"):
        path = Path(env)
        directory = path if path.is_dir() else path.parent
        candidate = directory / program
        return str(candidate) if candidate.is_file() else None
    ws = workspace()
    if ws is not None and (ws / ORACLE_DIR / program).is_file():
        return str(ws / ORACLE_DIR / program)
    return shutil.which(program)


@functools.cache
def afni_build(program: str) -> str:
    """The oracle's version, from its build record."""
    binary = afni_binary(program)
    if binary is None:
        return "not found"
    info = Path(binary).parent.parent / "BUILD_INFO.txt"
    version = "AFNI_25.2.09"
    if info.is_file():
        text = info.read_text(encoding="utf-8")
        m = re.search(r"AFNI version: (\S+)", text)
        version = m.group(1) if m else version
        m = re.search(r"compiler: (.+)", text)
        if m:
            version += f", {m.group(1).strip()}"
    return version


def fmriprep_image() -> Path | None:
    ws = workspace()
    image = None if ws is None else ws / FMRIPREP_IMAGE
    return image if image is not None and image.is_file() and shutil.which("apptainer") else None


@functools.cache
def generated_dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-afni-gen-"))


@functools.cache
def home() -> Path:
    """An empty home directory for both programs: AFNI reads no ``.afnirc`` from it."""
    h = generated_dir() / "home"
    h.mkdir(exist_ok=True)
    return h


def environment(extra: tuple[tuple[str, str | None], ...] = ()) -> dict[str, str]:
    """The environment both programs run in (see the module's documentation)."""
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))}
    # Two threads each (AFNI's OpenMP loop, the replica's): three parallel cases use six.
    env.update(
        HOME=str(home()), AFNI_DONT_LOGFILE="YES", AFNI_NIFTI_NOEXT="YES", OMP_NUM_THREADS="2"
    )
    for name, value in extra:
        if value is None:
            env.pop(name, None)
        else:
            env[name] = value
    return env


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


@dataclass(frozen=True)
class Run:
    code: int
    out: str
    err: str
    directory: Path


def run_afni(program: str, args: list[str], env: dict[str, str], cwd: Path) -> Run:
    p = subprocess.run(
        [afni_binary(program), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        cwd=cwd,
    )
    return Run(p.returncode, p.stdout, p.stderr, cwd)


def run_fmriprep_afni(
    program: str, args: list[str], env: dict[str, str], cwd: Path, binds: list[Path]
) -> Run:
    """``program`` from fMRIPrep 25.2.6's image (its own AFNI 25.2.09 build), as the user, with
    ``cwd`` and the inputs' directories bound at their own paths."""
    image = fmriprep_image()
    assert image is not None
    bind = sorted({str(cwd), *(str(b) for b in binds)})
    inner = {k: v for k, v in env.items() if k.startswith(("AFNI_", "OMP_"))}
    cmd = ["apptainer", "exec", "--cleanenv", "--no-home", "--pwd", str(cwd)]
    for b in bind:
        cmd += ["--bind", f"{b}:{b}"]
    for k, v in inner.items():
        cmd += ["--env", f"{k}={v}"]
    cmd += ["--env", f"HOME={home()}", "--bind", f"{home()}:{home()}"]
    p = subprocess.run(
        [*cmd, str(image), program, *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        cwd=cwd,
    )
    return Run(p.returncode, p.stdout, p.stderr, cwd)


def run_larmorx(
    program: str,
    args: list[str],
    mode: str,
    env: dict[str, str],
    cwd: Path,
    implementation: str = "replica",
) -> Run:
    """``larmorx afni <program>`` in process, with ``implementation`` forced: the replica (it
    runs the ``larmorx-gpl`` program as a separate process) or the clean-room original (in
    process); ``mode`` is D13's choice."""
    from larmorx.cli import run

    if implementation == "replica":
        env = dict(env, LARMORX_GPL_BIN=str(replica_binary()))
    out, err = io.StringIO(), io.StringIO()
    with (
        process_environment(env, cwd),
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(out),
    ):
        code = run(["larmorx", "afni", program, *args], implementation=implementation)
    return Run(code, out.getvalue(), err.getvalue(), cwd)


# ------------------------------------------------------------------------------------------------
# Messages

_AFNI_BUILD_LINES = re.compile(
    r"^(?:[^\n]*::)?(?:\x1b\[7m)?\+\+ 3d\w+: AFNI version=[^\n]*\n|^\*\* Program compile date = [^\n]*\n",
    re.M,
)
_ANSI = re.compile(r"\x1b\[[0-9;]*m")
_ELAPSED = re.compile(r"Elapsed = [0-9.]+ sec")


def larmorx_own(line: str) -> bool:
    """A line larmorx adds to AFNI's messages."""
    plain = _ANSI.sub("", line).rstrip()
    return (
        "upstream defect " in plain
        or "++ implementation: " in plain
        or "++ upstream defects: " in plain
        or plain.endswith("(larmorx)")
    )


def afni_text(text: str) -> str:
    return _ELAPSED.sub("Elapsed = N sec", _AFNI_BUILD_LINES.sub("", text))


def larmorx_text(text: str) -> str:
    kept = "".join(line for line in text.splitlines(keepends=True) if not larmorx_own(line))
    return _ELAPSED.sub("Elapsed = N sec", kept)


def first_difference(a: str, b: str) -> str:
    xs, ys = a.splitlines(), b.splitlines()
    k = next((i for i, (x, y) in enumerate(zip(xs, ys, strict=False)) if x != y), None)
    if k is None:
        k = min(len(xs), len(ys))
    x = xs[k] if k < len(xs) else "(nothing)"
    y = ys[k] if k < len(ys) else "(nothing)"
    return f"line {k + 1}: AFNI {x!r}, larmorx {y!r}"


def check_messages(checks: CheckList, afni: Run, ours: Run) -> None:
    a_err, l_err = afni_text(afni.err), larmorx_text(ours.err)
    detail = "identical"
    if afni.out != ours.out:
        detail = "stdout " + first_difference(afni.out, ours.out)
    elif a_err != l_err:
        detail = "stderr " + first_difference(a_err, l_err)
    for d in (afni.directory, ours.directory):
        detail = detail.replace(str(d), "<dir>")
    checks(MESSAGES, afni.out == ours.out and a_err == l_err, detail=detail)


def crashed(run: Run) -> bool:
    return "Fatal Signal" in run.err


def check_crash(checks: CheckList, afni: Run, ours: Run) -> None:
    """AFNI crashed (a signal, its crash banner, exit 1); larmorx must print what AFNI printed
    before the signal, then one line of its own, and exit with 1."""
    before = afni_text(afni.err.split("\nFatal Signal", 1)[0] + "\n")
    lines = larmorx_text(ours.err).splitlines(keepends=True)
    ours_before = "".join(lines[:-1]) if lines else ""
    checks(EXIT, afni.code == ours.code == 1, metric="exit status", value=ours.code, threshold=1)
    checks(
        CRASH,
        before.strip() == ours_before.strip(),
        detail="identical"
        if before.strip() == ours_before.strip()
        else first_difference(before, ours_before),
    )


def last_error(text: str) -> str:
    lines = [ln for ln in text.splitlines() if "ERROR" in ln or "Fatal" in ln]
    line = lines[-1] if lines else (text.splitlines()[-1] if text.strip() else "")
    return _ANSI.sub("", line)


# ------------------------------------------------------------------------------------------------
# Outputs


def nifti_parts(path: Path) -> tuple[bytes, bytes]:
    """A NIfTI file's header (the bytes before the data, extensions included) and data bytes,
    decompressed."""
    raw = path.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    if raw[:4] == b"\x5c\x01\x00\x00":
        offset = int(np.frombuffer(raw[108:112], "<f4")[0])
    elif raw[:4] == b"\x1c\x02\x00\x00":
        offset = int(np.frombuffer(raw[168:176], "<i8")[0])
    else:
        raise ValueError(f"{path.name} is not a little-endian NIfTI file")
    return raw[:offset], raw[offset:]


def stored(path: Path) -> np.ndarray:
    """The values as stored (before scaling), read with nibabel."""
    import nibabel as nib

    return np.asanyarray(nib.load(path).dataobj.get_unscaled())


@dataclass
class Comparison:
    identical: bool
    header: bool
    differing: int
    total: int
    max_abs: float
    detail: str


def compare_files(a: Path, b: Path) -> Comparison:
    ha, da = nifti_parts(a)
    hb, db = nifti_parts(b)
    va, vb = stored(a), stored(b)
    total = int(va.size)
    if va.dtype != vb.dtype or va.shape != vb.shape:
        return Comparison(
            False,
            ha == hb,
            total,
            total,
            float("inf"),
            f"AFNI {va.dtype} {va.shape}, larmorx {vb.dtype} {vb.shape}",
        )
    same = np.ascontiguousarray(va).view(np.uint8) == np.ascontiguousarray(vb).view(np.uint8)
    identical = da == db
    fa, fb = va.astype(np.float64), vb.astype(np.float64)
    with np.errstate(invalid="ignore"):
        diff = np.abs(fa - fb)
    n = int(np.count_nonzero(fa != fb))
    finite = diff[np.isfinite(diff)]
    max_abs = float(finite.max()) if finite.size else 0.0
    detail = (
        "bit-identical" if identical else f"{n} of {total} values differ, max |diff| {max_abs:.6g}"
    )
    del same
    return Comparison(identical, ha == hb, n, total, max_abs, detail)


#: The tolerance of the clean-room originals' float32 outputs where the bits are not the spec's
#: to decide (PLAN.md §11.3, deterministic arithmetic: max |diff| ≤ 1e-5 × max |AFNI output|).
FLOAT_RTOL = 1e-5


#: The value of a header check whose difference AFNI-10 explains (``check_outputs``).
AFNI_10 = "AFNI-10"


def has_afni_extension(path: Path) -> bool:
    """Whether a NIfTI-1 file carries AFNI's own header extension (code 4), whose attributes
    AFNI reads in place of the NIfTI header's geometry (AFNI-10)."""
    try:
        raw = path.read_bytes()
        if raw[:2] == b"\x1f\x8b":
            raw = gzip.decompress(raw)
    except OSError:
        return False
    if raw[:4] != b"\x5c\x01\x00\x00" or len(raw) < 360 or raw[348] == 0:
        return False
    offset = int(np.frombuffer(raw[108:112], "<f4")[0])
    pos = 352
    while pos + 8 <= offset:
        size, code = np.frombuffer(raw[pos : pos + 8], "<i4")
        if code == 4:
            return True
        if size <= 0:
            break
        pos += int(size)
    return False


def without_extensions(src: Path, dst: Path) -> Path:
    """A copy of the NIfTI-1 file ``src`` without its header extensions (the header's bytes
    unchanged but ``vox_offset``), uncompressed."""
    raw = src.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    offset = int(np.frombuffer(raw[108:112], "<f4")[0])
    header = bytearray(raw[:348])
    header[108:112] = np.float32(352.0).tobytes()
    dst.write_bytes(bytes(header) + b"\x00\x00\x00\x00" + raw[offset:])
    return dst


def check_outputs(
    checks: CheckList,
    afni_dir: Path,
    ours_dir: Path,
    outputs: tuple[str, ...],
    tolerance: float | None = None,
    explain_header: Callable[[str], str | None] | None = None,
) -> bool:
    """Every output of the case: present in both, data and header identical. With
    ``tolerance`` (a clean-room original's float32 outputs), the data may differ by at most
    ``tolerance`` × max |AFNI output|, and bit-identity is reported. ``explain_header(name)``
    gives the reason a header that differs is expected (AFNI-10), or ``None``. Returns whether
    every output was found in both."""
    ok = True
    for name in outputs:
        a, b = afni_dir / name, ours_dir / name
        if not (a.is_file() and b.is_file()):
            checks(
                f"{name}: written by both",
                False,
                detail=f"AFNI {a.is_file()}, larmorx {b.is_file()}",
            )
            ok = False
            continue
        c = compare_files(a, b)
        if tolerance is None or c.identical:
            checks.add(
                Check(
                    f"{name}: data",
                    c.identical,
                    metric="values that differ",
                    value=c.differing,
                    threshold=0,
                    detail=c.detail,
                )
            )
        else:
            va = stored(a)
            scale = float(np.max(np.abs(va.astype(np.float64)))) if va.size else 0.0
            limit = tolerance * scale
            within = va.dtype.kind == "f" and np.isfinite(c.max_abs) and c.max_abs <= limit
            checks.add(
                Check(
                    f"{name}: data",
                    bool(within),
                    metric="max |diff| / max |AFNI|",
                    value=c.max_abs / scale if scale else c.max_abs,
                    threshold=tolerance,
                    detail=c.detail,
                )
            )
            checks.add(
                Check(
                    f"{name}: values bit-identical to AFNI's",
                    True,
                    metric="values that differ",
                    value=c.differing,
                    threshold="reported",
                    detail=c.detail,
                )
            )
        reason = None if c.header or explain_header is None else explain_header(name)
        if reason is not None:
            checks.add(
                Check(
                    f"{name}: header bytes",
                    True,
                    metric="header bytes",
                    value=AFNI_10,
                    threshold="AFNI's header without its extension (AFNI-10)",
                    detail=reason,
                )
            )
        else:
            checks(f"{name}: header bytes", c.header, detail="identical" if c.header else "differ")
    return ok


def afni_10_explainer(
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    paths: list[Path],
    ours_dir: Path,
) -> Callable[[str], str | None]:
    """For a clean-room original, which ignores AFNI's own header extension (AFNI-10, as the
    3dTshift original does): when an input carries one, AFNI runs again on copies of the
    inputs without their extensions, and a header that differs is explained when that run
    writes the original's header byte for byte."""
    state: dict[str, Path | None] = {}

    def run_once() -> Path | None:
        if "dir" in state:
            return state["dir"]
        state["dir"] = None
        with_ext = [p for p in paths if has_afni_extension(p)]
        if not with_ext:
            return None
        d = tmp / "afni-without-extension"
        _setup(d, s)
        copies = {p: without_extensions(p, d / f"input{i}.nii") for i, p in enumerate(with_ext)}
        new_args = list(args)
        for p, q in copies.items():
            new_args = [a.replace(str(p), str(q)) for a in new_args]
        r = run_afni(program, new_args, env, d)
        state["dir"] = d if r.code == 0 else None
        return state["dir"]

    def explain(name: str) -> str | None:
        d = run_once()
        if d is None or not (d / name).is_file() or not (ours_dir / name).is_file():
            return None
        if nifti_parts(d / name)[0] != nifti_parts(ours_dir / name)[0]:
            return None
        return (
            "the input carries AFNI's own header extension, whose attributes AFNI reads in "
            "place of the NIfTI header's geometry (7 significant digits; AFNI-10, which the "
            "clean-room original ignores, as 3dTshift's does): AFNI run on the input without "
            "its extension writes the original's header byte for byte"
        )

    return explain


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Scenario:
    """A case: the arguments (``{0}``, ``{1}``: the inputs' paths), the inputs (names given to
    ``inputs``), the outputs to compare, and what to expect."""

    args: tuple[str, ...]
    inputs: tuple[str, ...]
    outputs: tuple[str, ...]
    tier: str = "smoke"
    #: ``pass``, ``error`` (both refuse), ``crash`` (AFNI crashes, larmorx refuses) or
    #: ``divergence`` (AFNI succeeds and larmorx refuses, for ``reason``).
    expect: str = "pass"
    env: tuple[tuple[str, str | None], ...] = ()
    #: Files in each program's directory before the run (name, text).
    files: tuple[tuple[str, str], ...] = ()
    reason: str = ""
    #: The Python equivalent: keyword arguments of ``lx.afni.<function>`` and how its outputs
    #: map to the case's files (``{"mask": "mask.nii", ...}``).
    python: dict[str, Any] | None = None
    python_outputs: dict[str, str] = field(default_factory=dict)
    #: AFNI's own variability: also run AFNI at these thread counts, and fMRIPrep's AFNI.
    band: tuple[int, ...] = ()
    #: fMRIPrep's own output of this call (its oracle run), compared with each output named.
    fmriprep: tuple[tuple[str, str], ...] = ()
    #: Defects whose notice may be named without a visible change.
    silent_ok: tuple[str, ...] = ()
    #: The defect whose correction makes the default refuse what AFNI does (exit status 1,
    #: the defect named): the fix pass checks that instead of comparing outputs.
    fix_refuses: str | None = None
    #: The defect whose correction makes the default write what AFNI refuses (exit status 0,
    #: the defect named, every output written): the fix pass checks that.
    fix_writes: str | None = None
    #: AFNI reads memory it never wrote in this case (the defect): what it does then depends
    #: on that memory, a crash or values from it. When AFNI crashes, its messages before the
    #: crash are compared; when it does not, every output's first ``defined`` values (the
    #: part that does not come from that memory) and its header. Either way the case is an
    #: expected divergence unless every value agrees.
    undefined: str | None = None
    defined: int = 0


@dataclass(frozen=True)
class ToolSpec:
    program: str
    function: str
    scenarios: Callable[[], list[tuple[str, str, str, Scenario]]]
    inputs: Callable[[str], Path]
    #: ``replica`` (the larmorx-gpl program) or ``original`` (the clean-room original).
    implementation: str = "replica"
    #: For float32 outputs that need not be bit-identical: the largest |diff| allowed, as a
    #: fraction of the largest |AFNI output| (``None``: bit-identity required).
    tolerance: float | None = None


def cases_of(spec: ToolSpec, tier: str) -> list[Case]:
    rank = TIERS.index(tier)
    return [
        Case(cid, category, desc, s)
        for cid, category, desc, s in spec.scenarios()
        if TIERS.index(s.tier) <= rank
    ]


def _arguments(s: Scenario, paths: list[Path]) -> list[str]:
    return [a.format(*[str(p) for p in paths]) if "{" in a else a for a in s.args]


def _setup(d: Path, s: Scenario) -> None:
    d.mkdir(parents=True, exist_ok=True)
    for name, text in s.files:
        (d / name).parent.mkdir(parents=True, exist_ok=True)
        (d / name).write_text(text, encoding="utf-8")


def _produced(d: Path, s: Scenario) -> list[str]:
    before = dict(s.files)
    out = []
    for name in s.outputs:
        f = d / name
        if f.is_file() and not (name in before and f.read_bytes() == before[name].encode()):
            out.append(name)
    return out


def run_case(spec: ToolSpec, case: Case, checks: CheckList) -> None:
    s: Scenario = case.payload
    program = spec.program
    impl = spec.implementation
    if afni_binary(program) is None:
        raise Outcome("skipped", f"the AFNI {program} oracle was not found (LARMORX_AFNI_BIN)")
    if impl == "replica" and replica_binary() is None:
        raise Outcome(
            "skipped", "the larmorx-gpl binary was not found (LARMORX_GPL_BIN, crates-gpl build)"
        )
    try:
        paths = [spec.inputs(name) for name in s.inputs]
    except FileNotFoundError as e:
        raise Outcome("skipped", f"input not available: {e}") from e
    env = environment(s.env)
    with tempfile.TemporaryDirectory(prefix="lx-afni-case-") as tmp_name:
        tmp = Path(tmp_name)
        args = _arguments(s, paths)
        a_dir, l_dir = tmp / "afni", tmp / "larmorx"
        _setup(a_dir, s)
        _setup(l_dir, s)
        afni = run_afni(program, args, env, a_dir)
        ours = run_larmorx(program, args, defects.REPRODUCE, env, l_dir, impl)
        a_out, l_out = _produced(a_dir, s), _produced(l_dir, s)

        if s.undefined is not None:
            _undefined(checks, program, s, args, env, tmp, afni, ours, a_dir, l_dir, impl)
            return
        if s.expect == "crash" or crashed(afni):
            check_crash(checks, afni, ours)
            if s.expect != "crash":
                checks("AFNI does not crash", False, detail=last_error(afni.err))
            if all(c.passed for c in checks.checks):
                raise Outcome(
                    BOTH_ERROR,
                    f"AFNI crashes (exit {afni.code}, {last_error(afni.err)}); larmorx refuses "
                    f"(exit {ours.code}: {last_error(ours.err)}), after the same messages",
                )
            return
        if s.expect == "divergence":
            ok = afni.code == 0 and ours.code != 0
            checks(
                "AFNI succeeds and larmorx refuses",
                ok,
                detail=f"AFNI exit {afni.code}; larmorx exit {ours.code}: {last_error(ours.err)}",
            )
            if ok:
                raise Outcome(EXPECTED, s.reason)
            return
        checks(
            EXIT, afni.code == ours.code, metric="exit status", value=ours.code, threshold=afni.code
        )
        check_messages(checks, afni, ours)
        checks(
            "the same outputs written",
            a_out == l_out,
            detail=f"AFNI {a_out}, larmorx {l_out}",
        )
        if s.expect == "error":
            if afni.code != 0 and all(c.passed for c in checks.checks):
                raise Outcome(
                    BOTH_ERROR,
                    f"AFNI exit {afni.code}: {last_error(afni.err)}; larmorx exit {ours.code}: "
                    f"{last_error(ours.err)} (the same messages)",
                )
            if afni.code == 0:
                checks("both reject the arguments", False, detail="AFNI succeeded")
            return
        explain = (
            afni_10_explainer(program, s, args, env, tmp, paths, l_dir)
            if impl == "original"
            else None
        )
        check_outputs(checks, a_dir, l_dir, tuple(a_out), spec.tolerance, explain)
        if s.expect == "pass" and afni.code != 0 and s.fix_writes is not None:
            _fix_writes(checks, program, s, args, env, tmp, impl)
            return
        if s.expect != "pass" or afni.code != 0:
            return
        for name, source in s.fmriprep:
            try:
                path = spec.inputs(source)
            except FileNotFoundError:
                continue
            if path.is_file() and (l_dir / name).is_file():
                c = compare_files(path, l_dir / name)
                checks.add(
                    Check(
                        f"{name}: fMRIPrep's own output (as it ran)",
                        c.identical,
                        metric="values that differ",
                        value=c.differing,
                        threshold="reported",
                        detail=c.detail,
                    )
                )
        if s.band:
            _band(checks, spec, s, args, env, tmp, a_dir, paths)
        if s.python is not None:
            _python(checks, spec, s, paths[0], l_dir)
        _fix_pass(checks, program, s, args, env, tmp, l_dir, ours, impl)


def _band(
    checks: CheckList,
    spec: ToolSpec,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    a_dir: Path,
    paths: list[Path],
) -> None:
    """AFNI's own variability: AFNI at other thread counts (``OMP_NUM_THREADS``) and fMRIPrep's
    build of AFNI, each against the oracle's output; reported, never a failure."""
    variants: list[tuple[str, Callable[[Path], Run]]] = []
    for t in s.band:
        variants.append(
            (
                f"AFNI at {t} thread(s)",
                lambda d, t=t: run_afni(spec.program, args, dict(env, OMP_NUM_THREADS=str(t)), d),
            )
        )
    if fmriprep_image() is not None:
        binds = sorted({p.parent for p in paths})
        variants.append(
            (
                "fMRIPrep 25.2.6's AFNI",
                lambda d: run_fmriprep_afni(spec.program, args, env, d, binds),
            )
        )
    for label, runner in variants:
        d = tmp / re.sub(r"\W+", "-", label)
        _setup(d, s)
        r = runner(d)
        if r.code != 0:
            checks.add(
                Check(
                    f"band: {label}",
                    True,
                    metric="exit status",
                    value=r.code,
                    threshold="reported",
                    detail=last_error(r.err),
                )
            )
            continue
        worst = 0
        details = []
        for name in s.outputs:
            if (a_dir / name).is_file() and (d / name).is_file():
                c = compare_files(a_dir / name, d / name)
                worst = max(worst, c.differing)
                details.append(f"{name}: {c.detail}")
        checks.add(
            Check(
                f"band: {label}",
                True,
                metric="values that differ from the oracle's",
                value=worst,
                threshold="reported",
                detail="; ".join(details),
            )
        )


def _python(checks: CheckList, spec: ToolSpec, s: Scenario, image: Path, l_dir: Path) -> None:
    """The Python API on the same input: every output the command line wrote, bit for bit (the
    values as :func:`larmorx.load` reads them)."""
    import warnings

    import larmorx as lx

    fn = getattr(lx.afni, spec.function)
    kwargs = dict(s.python or {})
    env = dict(os.environ)
    if spec.implementation == "replica":
        env["LARMORX_GPL_BIN"] = str(replica_binary())
    with process_environment(env), warnings.catch_warnings():
        warnings.simplefilter("ignore")
        result = fn(
            image, implementation=spec.implementation, upstream_defects="reproduce", **kwargs
        )
    bad = []
    for attr, name in s.python_outputs.items():
        img = getattr(result, attr)
        want = lx.load(l_dir / name)
        a, b = np.asarray(img.data), np.asarray(want.data)
        same = (
            a.dtype == b.dtype
            and a.shape == b.shape
            and np.array_equal(
                np.ascontiguousarray(a).view(np.uint8), np.ascontiguousarray(b).view(np.uint8)
            )
        )
        same = same and np.array_equal(np.asarray(img.affine), np.asarray(want.affine))
        if not same:
            bad.append(attr)
    checks(
        f"Python API: lx.afni.{spec.function} gives the command line's outputs",
        not bad,
        detail="bit-identical" if not bad else f"differ: {', '.join(bad)}",
    )


def _undefined(
    checks: CheckList,
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    afni: Run,
    ours: Run,
    a_dir: Path,
    l_dir: Path,
    impl: str = "replica",
) -> None:
    """A case where AFNI reads memory it never wrote (``Scenario.undefined``): AFNI crashes or
    writes values from that memory, depending on the memory; larmorx's ``reproduce`` mode
    fills the read deterministically (D13). Compared: the messages before AFNI's crash, or
    the exit status, the messages, every header and the outputs' defined part."""
    why = f"AFNI reads memory it never wrote ({s.undefined})"
    checks("larmorx completes (reproduce)", ours.code == 0, detail=last_error(ours.err))
    if crashed(afni):
        before = afni_text(afni.err.split("\nFatal Signal", 1)[0]).rstrip("\n")
        mine = larmorx_text(ours.err)
        ok = mine.startswith(before)
        checks(
            CRASH,
            ok,
            detail="larmorx prints what AFNI printed before its crash, then goes on"
            if ok
            else first_difference(before, mine[: len(before)]),
        )
        _fix_pass(checks, program, s, args, env, tmp, l_dir, ours, impl)
        if all(c.passed for c in checks.checks):
            raise Outcome(
                EXPECTED,
                f"{why}: this run crashed (Fatal Signal 11) after the same messages; larmorx "
                "fills the read with 0 in reproduce mode and corrects it by default",
            )
        return
    checks(EXIT, afni.code == ours.code, metric="exit status", value=ours.code, threshold=afni.code)
    check_messages(checks, afni, ours)
    beyond = 0
    for name in s.outputs:
        a, b = a_dir / name, l_dir / name
        if not (a.is_file() and b.is_file()):
            checks(f"{name}: written by both", False)
            continue
        ha, da = nifti_parts(a)
        hb, db = nifti_parts(b)
        checks(f"{name}: header bytes", ha == hb)
        size = stored(a).dtype.itemsize
        k = s.defined * size
        checks(
            f"{name}: the defined values ({s.defined})",
            da[:k] == db[:k],
            detail="bit-identical" if da[:k] == db[:k] else "they differ",
        )
        va = np.frombuffer(da[k:], dtype=stored(a).dtype)
        vb = np.frombuffer(db[k:], dtype=stored(b).dtype)
        n = int(
            np.count_nonzero(
                va.view(np.uint8).reshape(-1, size) != vb.view(np.uint8).reshape(-1, size)
            )
        )
        beyond += n
        checks.add(
            Check(
                f"{name}: values from memory AFNI never wrote",
                True,
                metric="values that differ",
                value=n,
                threshold="reported",
                detail=f"{n} of {va.size} differ",
            )
        )
    _fix_pass(checks, program, s, args, env, tmp, l_dir, ours, impl)
    if beyond and all(c.passed for c in checks.checks):
        raise Outcome(
            EXPECTED,
            f"{why}: {beyond} values come from that memory in AFNI's run; the defined values "
            "are bit-identical",
        )


def _fix_writes(
    checks: CheckList,
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    impl: str = "replica",
) -> None:
    """The fix pass of a case AFNI refuses and larmorx's default corrects (``fix_writes``): the
    default exits 0, names the defect and writes every output."""
    d = tmp / "fix"
    _setup(d, s)
    r = run_larmorx(program, args, defects.FIX, env, d, impl)
    named = defects.notices_in(r.err)
    written = _produced(d, s)
    ok = r.code == 0 and s.fix_writes in named and written == list(s.outputs)
    checks.add(
        Check(
            defects.FIX_PASS,
            ok,
            metric="values changed by the corrections",
            value=0,
            threshold="reported (named iff changed)",
            detail=f"{', '.join(named) or 'no defect named'}: no values; the outputs written "
            f"where AFNI refuses ({', '.join(written) or 'none'}, exit {r.code}) changed",
        )
    )


def _fix_pass(
    checks: CheckList,
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    reproduced_dir: Path,
    reproduced: Run,
    impl: str = "replica",
) -> None:
    d = tmp / "fix"
    _setup(d, s)
    r = run_larmorx(program, args, defects.FIX, env, d, impl)
    if s.fix_refuses is not None:
        named = defects.notices_in(r.err)
        checks.add(
            Check(
                defects.FIX_PASS,
                r.code == 1 and s.fix_refuses in named,
                metric="values changed by the corrections",
                value=0,
                threshold="reported (named iff changed)",
                detail=f"{', '.join(named) or 'no defect named'}: the default refuses (exit "
                f"{r.code}): {last_error(r.err)}",
            )
        )
        return
    if r.code != 0:
        checks(
            defects.FIX_PASS,
            False,
            detail=f"the fix pass failed (exit {r.code}): {last_error(r.err)}",
        )
        return
    change = defects.Change(0, 0, 0.0)
    other = []
    fixed_out, reproduced_out = _produced(d, s), _produced(reproduced_dir, s)
    for name in s.outputs:
        a, b = d / name, reproduced_dir / name
        if (name in fixed_out) != (name in reproduced_out):
            other.append(f"{name} written in one mode only")
            continue
        if name not in fixed_out:
            continue
        va, vb = stored(a), stored(b)
        change = change + defects.Change.of(va, vb)
        ha, _ = nifti_parts(a)
        hb, _ = nifti_parts(b)
        if ha != hb:
            other.append(f"{name} header")
    plain = [larmorx_text(x) for x in (r.err, reproduced.err)]
    if _ANSI.sub("", plain[0]) != _ANSI.sub("", plain[1]):
        other.append("messages")
    entries = defects.notices_in(r.err)
    defects.add_fix_check(
        checks, entries, change, other_changes=", ".join(other), silent_ok=s.silent_ok
    )
    reproduced_entries = defects.notices_in(reproduced.err)
    if entries != reproduced_entries:
        checks(
            "D13 notices in both modes",
            False,
            detail=f"fix: {entries}; reproduce: {reproduced_entries}",
        )


# ------------------------------------------------------------------------------------------------
# The record's summary


def highlights(
    results: list[CaseResult],
    program: str,
    function: str,
    crash_causes: str,
    implementation: str = "replica",
) -> list[str]:
    """The record's summary lines; ``crash_causes`` names what makes AFNI crash in its cases
    (e.g. "a missing option value, AFNI-40")."""
    compared = [r for r in results if r.status == PASS]

    def data_checks(r: CaseResult) -> list[Check]:
        return [c for c in r.checks if c.name.endswith((": data", ": header bytes"))]

    def identical(r: CaseResult) -> bool:
        reported = [c for c in r.checks if c.name.endswith(": values bit-identical to AFNI's")]
        return all(c.passed and c.value != AFNI_10 for c in data_checks(r)) and all(
            c.value == 0 for c in reported
        )

    with_outputs = [r for r in compared if data_checks(r)]
    bit = [r for r in with_outputs if identical(r)]
    afni_10 = [r for r in with_outputs if any(c.value == AFNI_10 for c in data_checks(r))]
    messages = [c for r in results for c in r.checks if c.name == MESSAGES]
    lines = [
        f"**Bit-identical to AFNI: {len(bit)} of {len(with_outputs)} compared cases that write "
        "outputs**: every output the same bytes (data and NIfTI header), with the same exit "
        "status and messages."
        + (
            f" The other {len(compared) - len(with_outputs)} compared case(s) write no output "
            "in either program (an existing output, a name with a blank, a directory that does "
            "not exist) and agree on the exit status and messages; the default mode's "
            "corrections there are in the D13 lines below."
            if len(compared) > len(with_outputs)
            else ""
        ),
        f"- Messages (standard output and error, byte for byte; AFNI's version banner and compile "
        f"date, and larmorx's own lines, set aside): {sum(c.passed for c in messages)} of "
        f"{len(messages)} identical, in every case that ran both programs.",
    ]
    if afni_10:
        names = ", ".join(f"`{r.case}`" for r in afni_10)
        lines.append(
            f"- {len(afni_10)} case(s) whose input carries AFNI's own header extension, whose "
            "attributes AFNI reads in place of the NIfTI geometry (AFNI-10; the clean-room "
            "original ignores it, as 3dTshift's does): the data are bit-identical, and AFNI run "
            f"on the input without its extension writes the original's header byte for byte: "
            f"{names}."
        )
    crashes = [r for r in results if any(c.name == CRASH for c in r.checks)]
    if crashes:
        lines.append(
            f"- {len(crashes)} case(s) where AFNI crashes ({crash_causes}): larmorx prints "
            "AFNI's messages up to the crash, then refuses with one line and exit status 1."
        )
    fmri = [c for r in results for c in r.checks if "fMRIPrep's own output" in c.name]
    if fmri:
        lines.append(
            f"- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran "
            f"them in its image): {sum(c.value == 0 for c in fmri)} of {len(fmri)} bit-identical "
            f"to the {implementation}'s."
        )
    band = [c for r in results for c in r.checks if c.name.startswith("band: ")]
    if band:
        kinds = sorted({c.name.removeprefix("band: ") for c in band})
        for k in kinds:
            cs = [c for c in band if c.name == f"band: {k}"]
            same = sum(c.value == 0 for c in cs)
            lines.append(
                f"- AFNI's own variability, {k}: {same} of {len(cs)} runs give the oracle's "
                "output bit for bit."
            )
    py = [c for r in results for c in r.checks if c.name.startswith("Python API")]
    if py:
        lines.append(
            f'- Python API (`lx.afni.{function}(..., implementation="{implementation}")`): '
            f"{sum(c.passed for c in py)} of {len(py)} cases give the command line's outputs bit "
            "for bit."
        )
    lines += defects.summary(results)
    return lines
