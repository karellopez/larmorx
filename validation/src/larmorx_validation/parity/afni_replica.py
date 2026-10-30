# SPDX-License-Identifier: Apache-2.0
"""What the parity suites of AFNI programs with only a replica share (3dAutomask, 3dUnifize):
running AFNI's program and the replica on the same files, and comparing exit statuses,
messages and every output, byte for byte.

AFNI runs as its own binary (``scripts/build_afni_oracle.sh``; the version is chosen by
:mod:`afni_versions`: ``LARMORX_AFNI_VERSION``, ``--afni-version``, else the implementation's
default; ``LARMORX_AFNI_BIN`` names another build's directory or one program), the replica the way users reach it: through larmorx's own dispatch
(``larmorx afni <program>`` with ``implementation="replica"``), in-process (the replicas are part
of larmorx since 2026-10-29; until then the GPL program ``larmorx-gpl``).

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
import tempfile
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import afni_versions, defects
from larmorx_validation.parity.afni_tshift import REPLICA_ENV as REPLICA_ENV
from larmorx_validation.parity.afni_tshift import replica_command as replica_command
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


def afni_binary(program: str, version: str | None = None) -> str | None:
    """The AFNI oracle ``program``: in ``LARMORX_AFNI_BIN`` (a directory, or a program whose
    directory holds the others), else the chosen version's build in the workspace
    (:mod:`afni_versions`), else on the PATH; ``version`` asks for that build only."""
    return afni_versions.binary(program, version)


@functools.cache
def afni_build(program: str, implementation: str | None = None, version: str | None = None) -> str:
    """The oracle's version and compiler, from its build record: the build ``implementation``'s
    cases run, or the build of ``version``."""
    binary = afni_binary(program, version)
    if version is None and implementation is not None and not os.environ.get("LARMORX_AFNI_BIN"):
        binary = afni_binary(program, afni_versions.selected(implementation))
    if binary is None:
        return "not found"
    info = Path(binary).parent.parent / "BUILD_INFO.txt"
    version = "unknown version"
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


def case_never(s: Scenario) -> str:
    """Why AFNI's runs of a case are never kept in the oracle cache: AFNI reads memory it never
    wrote (``undefined``), or the case's texts name a defect whose result varies."""
    from larmorx_validation.oracle_cache import never_for

    if s.undefined is not None:
        return f"AFNI reads memory it never wrote ({s.undefined})"
    return never_for(s.reason)


def _threads(env: dict[str, str]) -> int | None:
    """AFNI's OpenMP thread count in ``env`` (``None``: not set, the machine's)."""
    try:
        return int(env["OMP_NUM_THREADS"])
    except (KeyError, ValueError):
        return None


def run_afni(
    program: str,
    args: list[str],
    env: dict[str, str],
    cwd: Path,
    never: str = "",
    version: str | None = None,
) -> Run:
    """AFNI's ``program`` in ``cwd``, through the oracle cache (above one OpenMP thread, served
    once three runs agree; ``never``: why it always runs, :func:`case_never`); ``version``: that
    build (:mod:`afni_versions`), else the chosen one."""
    from larmorx_validation import oracle_cache

    policy = oracle_cache.Policy(threads=_threads(env), never=never, family="afni")
    p = oracle_cache.run(
        [afni_binary(program, version), *args], env=env, cwd=cwd, policy=policy, runner="afni"
    )
    return Run(p.returncode, p.stdout, p.stderr, cwd)


def run_fmriprep_afni(
    program: str,
    args: list[str],
    env: dict[str, str],
    cwd: Path,
    binds: list[Path],
    never: str = "",
) -> Run:
    """``program`` from fMRIPrep 25.2.6's image (its own AFNI 25.2.09 build), as the user, with
    ``cwd`` and the inputs' directories bound at their own paths; through the oracle cache,
    keyed by the image's SHA-256."""
    from larmorx_validation import oracle_cache

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
    p = oracle_cache.run_apptainer(
        image,
        program,
        args,
        {**inner, "HOME": str(home())},
        [*cmd, str(image), program, *args],
        cwd=cwd,
        policy=oracle_cache.Policy(threads=_threads(inner), never=never, family="afni"),
        runner="afni-fmriprep",
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
    """``larmorx afni <program>`` in process, with ``implementation`` forced: the replica or the
    clean-room original; ``mode`` is D13's choice."""
    from larmorx.cli import run

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


def check_messages(
    checks: CheckList,
    afni: Run,
    ours: Run,
    explain: Callable[[Run], str | None] | None = None,
) -> None:
    """AFNI's and larmorx's standard output and error, byte for byte. ``explain(ours)`` gives the
    reason messages that differ are expected (AFNI-10: AFNI run on the input without its own
    header extension prints larmorx's messages), or ``None``."""
    a_err, l_err = afni_text(afni.err), larmorx_text(ours.err)
    detail = "identical"
    if afni.out != ours.out:
        detail = "stdout " + first_difference(afni.out, ours.out)
    elif a_err != l_err:
        detail = "stderr " + first_difference(a_err, l_err)
    for d in (afni.directory, ours.directory):
        detail = detail.replace(str(d), "<dir>")
    same = afni.out == ours.out and a_err == l_err
    reason = None if same or explain is None else explain(ours)
    if reason is not None:
        checks(MESSAGES, True, value=AFNI_10, detail=f"{reason} ({detail} with the extension)")
        return
    checks(MESSAGES, same, detail=detail)


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
    if not identical and n == 0:
        # Equal as numbers, not as bits: signed zeros or NaN payloads (random testing,
        # 2026-10-25: "0 of 294 values differ" said nothing of what did).
        try:
            width = f"u{va.dtype.itemsize}"
            ia = np.ascontiguousarray(va).view(width).ravel()
            ib = np.ascontiguousarray(vb).view(width).ravel()
            where = np.flatnonzero(ia != ib)
        except (TypeError, ValueError):
            where = np.array([], dtype=np.int64)
        if where.size:
            i = int(where[0])
            x, y = va.ravel()[i], vb.ravel()[i]
            detail = (
                f"{where.size} of {total} values differ in their bits only, equal as numbers "
                f"(signed zeros or NaN payloads; e.g. value {i}: AFNI {x!r}, larmorx {y!r})"
            )
        else:
            detail = "the data bytes differ outside the values"
    del same
    return Comparison(identical, ha == hb, n, total, max_abs, detail)


#: The tolerance of the clean-room originals' float32 outputs where the bits are not the spec's
#: to decide (PLAN.md §11.3, deterministic arithmetic: max |diff| ≤ 1e-5 × max |AFNI output|).
FLOAT_RTOL = 1e-5


#: The value of a header check whose difference AFNI-10 explains (``check_outputs``).
AFNI_10 = "AFNI-10"

#: The names' ends of the checks against fMRIPrep's own outputs (the oracle runs' node outputs).
FMRIPREP_OWN = "fMRIPrep's own output (as it ran)"
FMRIPREP_AFNI = "fMRIPrep's own output against AFNI's"


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


#: The check of a clean-room original's outputs against AFNI with correctly rounded maths.
CORRECTLY_ROUNDED = "values bit-identical to AFNI's with correctly rounded maths"


def crmath_preload() -> Path | None:
    """CORE-MATH's correctly rounded functions for ``LD_PRELOAD`` into AFNI
    (``scripts/build_crmath_preload.sh``), when the build has ``tanhf`` and ``cbrtf`` (3dUnifize's
    squash and ``-EPI``): ``LARMORX_CRMATH_PRELOAD``, else the workspace's
    ``oracles/core-math-preload``."""
    if env := os.environ.get("LARMORX_CRMATH_PRELOAD"):
        lib = Path(env)
    else:
        ws = workspace()
        if ws is None:
            return None
        lib = ws / "oracles/core-math-preload/libcrmath.so"
    info = lib.parent / "BUILD_INFO.txt"
    if not lib.is_file() or not info.is_file():
        return None
    functions = re.search(r"functions: (.+)", info.read_text(encoding="utf-8"))
    names = set(functions.group(1).split()) if functions else set()
    return lib if {"tanhf", "cbrtf"} <= names else None


def _correctly_rounded(
    checks: CheckList,
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    a_dir: Path,
    l_dir: Path,
    outputs: tuple[str, ...],
) -> None:
    """For a clean-room original whose float32 outputs differ from AFNI's in their last bits:
    AFNI again with CORE-MATH's correctly rounded functions preloaded (the original uses
    correctly rounded ``tanhf``; AFNI calls glibc's). Reported: how many values still differ."""
    differ = [
        n
        for n in outputs
        if (a_dir / n).is_file()
        and (l_dir / n).is_file()
        and nifti_parts(a_dir / n)[1] != nifti_parts(l_dir / n)[1]
    ]
    if not differ:
        return
    lib = crmath_preload()
    if lib is None:
        checks.add(
            Check(
                CORRECTLY_ROUNDED,
                True,
                metric="values that differ",
                value="not run",
                threshold="reported",
                detail="no CORE-MATH preload with tanhf and cbrtf (scripts/build_crmath_preload.sh)",
            )
        )
        return
    d = tmp / "afni-correctly-rounded"
    _setup(d, s)
    r = run_afni(program, args, dict(env, LD_PRELOAD=str(lib)), d, never=case_never(s))
    worst, details = 0, []
    for name in differ:
        if r.code != 0 or not (d / name).is_file():
            worst, details = -1, [f"AFNI with the preload: exit {r.code}"]
            break
        c = compare_files(d / name, l_dir / name)
        worst = max(worst, c.differing)
        details.append(f"{name}: {c.detail}")
    checks.add(
        Check(
            CORRECTLY_ROUNDED,
            True,
            metric="values that differ",
            value=worst,
            threshold="reported",
            detail="; ".join(details),
        )
    )


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
    writes the original's header byte for byte. The returned function's ``messages(ours)``
    explains messages that differ when that run prints the original's messages byte for byte
    (the copies' names read as the inputs'): AFNI's obliquity report comes from the matrix it
    reads, the extension's (2026-10-29, the data panel's 3dUnifize outputs)."""
    state: dict = {}

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
        r = run_afni(program, new_args, env, d, never=case_never(s))
        state["dir"] = d if r.code == 0 else None
        out, err = r.out, r.err
        for p, q in copies.items():
            out, err = out.replace(str(q), str(p)), err.replace(str(q), str(p))
        state["run"] = (r.code, out, err)
        return state["dir"]

    def messages(ours: Run) -> str | None:
        run_once()
        if "run" not in state:
            return None
        code, out, err = state["run"]
        if code != ours.code or out != ours.out or afni_text(err) != larmorx_text(ours.err):
            return None
        return (
            "the input carries AFNI's own header extension, whose matrix (7 significant digits) "
            "AFNI reads in place of the NIfTI header's, and so reports its obliquity from it "
            "(AFNI-10, which the clean-room original ignores): AFNI run on the input without "
            "its extension prints the original's messages byte for byte"
        )

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

    explain.messages = messages  # type: ignore[attr-defined]
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
    #: ``pass``, ``error`` (both refuse), ``crash`` (AFNI crashes, larmorx refuses),
    #: ``divergence`` (AFNI succeeds and larmorx refuses, for ``reason``) or ``guard`` (beyond
    #: any machine, AFNI-75: neither AFNI nor ``reproduce`` runs; the default must stop before
    #: allocating, naming the defect and the memory it would need).
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
    #: Defects only one mode meets, because another correction takes the other mode down a path
    #: where their condition does not arise (header slice code 2: AFNI-03's correction gives
    #: slice times, so AFNI-08's all-zero times arise in ``reproduce`` only).
    one_mode: tuple[str, ...] = ()
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
    #: ``guard``: text the refusal must hold (the GiB needed, the radius, the limit), and nothing
    #: written (``specs/3dUnifize.md`` §10).
    guard_text: tuple[str, ...] = ()


@dataclass(frozen=True)
class ToolSpec:
    program: str
    function: str
    scenarios: Callable[[], list[tuple[str, str, str, Scenario]]]
    inputs: Callable[[str], Path]
    #: ``replica`` (the bit-exact replica) or ``original`` (the clean-room original).
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


def _difftest_input(name: str) -> Path:
    """An input of a differential test's case (:mod:`larmorx_validation.difftest.inputs`)."""
    from larmorx_validation.difftest import inputs

    return inputs.path(name)


def run_case(spec: ToolSpec, case: Case, checks: CheckList) -> None:
    """One case; every AFNI run in it uses the implementation's version
    (:func:`afni_versions.using`)."""
    with afni_versions.using(spec.implementation):
        _run_case(spec, case, checks)


def _run_case(spec: ToolSpec, case: Case, checks: CheckList) -> None:
    s: Scenario = case.payload
    program = spec.program
    impl = spec.implementation
    if afni_binary(program) is None:
        raise Outcome("skipped", f"the AFNI {program} oracle was not found (LARMORX_AFNI_BIN)")
    try:
        paths = [
            _difftest_input(name) if name.startswith("dt:") else spec.inputs(name)
            for name in s.inputs
        ]
    except FileNotFoundError as e:
        raise Outcome("skipped", f"input not available: {e}") from e
    env = environment(s.env)
    with tempfile.TemporaryDirectory(prefix="lx-afni-case-") as tmp_name:
        tmp = Path(tmp_name)
        args = _arguments(s, paths)
        a_dir, l_dir = tmp / "afni", tmp / "larmorx"
        _setup(a_dir, s)
        _setup(l_dir, s)
        if s.expect == "guard":
            ours = run_larmorx(program, args, defects.FIX, env, l_dir, impl)
            text = ours.err.replace(str(tmp), "<tmp>")
            checks(
                "larmorx refuses before allocating (AFNI-75)",
                ours.code != 0 and "(AFNI-75)" in text and "GiB" in text,
                detail=f"exit {ours.code}: {text[-600:]}",
            )
            if s.guard_text:
                missing = [t for t in s.guard_text if t not in text]
                checks(
                    "the refusal's words (the memory, the radius, the voxels, the limit)",
                    not missing,
                    detail=f"missing {missing}" if missing else "; ".join(s.guard_text),
                )
                written = _produced(l_dir, s)
                checks("nothing written", not written, detail=f"{written}")
            return
        afni = run_afni(program, args, env, a_dir, never=case_never(s))
        ours = run_larmorx(program, args, defects.REPRODUCE, env, l_dir, impl)
        a_out, l_out = _produced(a_dir, s), _produced(l_dir, s)
        _step(checks, program, s, args, env, tmp, afni, a_dir, a_out, paths)

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
        explain = (
            afni_10_explainer(program, s, args, env, tmp, paths, l_dir)
            if impl == "original"
            else None
        )
        check_messages(checks, afni, ours, explain.messages if explain else None)  # type: ignore[attr-defined]
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
        check_outputs(checks, a_dir, l_dir, tuple(a_out), spec.tolerance, explain)
        if spec.tolerance is not None and afni.code == 0:
            _correctly_rounded(checks, program, s, args, env, tmp, a_dir, l_dir, tuple(a_out))
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
                passed = c.identical
                if spec.tolerance is not None and not c.identical:
                    # A clean-room original's float32 values: the record's threshold, as
                    # against AFNI's oracle, and whether fMRIPrep's output is that oracle's.
                    vf = stored(path)
                    scale = float(np.max(np.abs(vf.astype(np.float64)))) if vf.size else 0.0
                    passed = bool(
                        vf.dtype.kind == "f"
                        and np.isfinite(c.max_abs)
                        and c.max_abs <= spec.tolerance * scale
                    )
                    if (a_dir / name).is_file():
                        oracle = compare_files(path, a_dir / name)
                        checks.add(
                            Check(
                                f"{name}: {FMRIPREP_AFNI}",
                                True,
                                metric="values that differ",
                                value=oracle.differing,
                                threshold="reported",
                                detail=oracle.detail,
                            )
                        )
                checks.add(
                    Check(
                        f"{name}: {FMRIPREP_OWN}",
                        passed,
                        metric="values that differ",
                        value=c.differing,
                        threshold="reported"
                        if spec.tolerance is None
                        else f"max |diff| ≤ {spec.tolerance:g} × max |fMRIPrep output|",
                        detail=c.detail,
                    )
                )
        if s.band:
            _band(checks, spec, s, args, env, tmp, a_dir, paths)
        if s.python is not None:
            _python(checks, spec, s, paths[0], l_dir)
        _fix_pass(checks, program, s, args, env, tmp, l_dir, ours, impl)


def _step(
    checks: CheckList,
    program: str,
    s: Scenario,
    args: list[str],
    env: dict[str, str],
    tmp: Path,
    afni: Run,
    a_dir: Path,
    a_out: list[str],
    paths: list[Path],
) -> None:
    """AFNI's own step from the previous version to the one compared (:mod:`afni_versions`):
    the previous build on the same arguments, in a directory set up alike (served from the
    oracle cache when an earlier record ran it); its exit status, messages and outputs against
    the compared build's. Reported, never a failure; a change without a known reason is a
    finding."""
    old = afni_versions.step_from()
    if old is None or s.undefined is not None:
        return
    o_dir = tmp / "afni-step"
    _setup(o_dir, s)
    prev = run_afni(program, args, env, o_dir, never=case_never(s), version=old)
    o_out = _produced(o_dir, s)
    msgs = tuple(
        (r.out + afni_text(r.err)).replace(str(d), "<dir>")
        for r, d in ((prev, o_dir), (afni, a_dir))
    )
    differences = {}
    for name in sorted(set(o_out) & set(a_out)):
        try:
            d = afni_versions.compare_output(o_dir / name, a_dir / name)
        except Exception as e:  # an output nibabel cannot read is reported
            d = f"not compared ({type(e).__name__})"
        if d is not None:
            differences[name] = d
    changed, detail = afni_versions.step_detail(
        (prev.code, afni.code),
        msgs,
        (o_out, a_out),
        differences,
        afni_versions.reasons(args, paths),
    )
    checks.add(
        Check(
            afni_versions.STEP_CHECK,
            True,
            metric="AFNI against AFNI",
            value="changed" if changed else "identical",
            threshold="reported",
            detail=detail,
        )
    )


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
                lambda d, t=t: run_afni(
                    spec.program,
                    args,
                    dict(env, OMP_NUM_THREADS=str(t)),
                    d,
                    never=case_never(s),
                ),
            )
        )
    if fmriprep_image() is not None:
        binds = sorted({p.parent for p in paths})
        variants.append(
            (
                "fMRIPrep 25.2.6's AFNI",
                lambda d: run_fmriprep_afni(spec.program, args, env, d, binds, never=case_never(s)),
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
    # A refusal happens where the program would use the option; an input it gives up on
    # earlier (a constant image, found by random testing) never gets there: then the default
    # runs, and the ordinary fix pass below checks it.
    if s.fix_refuses is not None and (r.code != 0 or s.fix_refuses in defects.notices_in(r.err)):
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
    one_mode_only = (set(entries) ^ set(reproduced_entries)) <= set(s.one_mode)
    if entries != reproduced_entries and not one_mode_only:
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

    def data_and_messages(r: CaseResult) -> list[Check]:
        return [*data_checks(r), *(c for c in r.checks if c.name == MESSAGES)]

    def identical(r: CaseResult) -> bool:
        reported = [c for c in r.checks if c.name.endswith(": values bit-identical to AFNI's")]
        return all(c.passed and c.value != AFNI_10 for c in data_and_messages(r)) and all(
            c.value == 0 for c in reported
        )

    with_outputs = [r for r in compared if data_checks(r)]
    bit = [r for r in with_outputs if identical(r)]
    afni_10 = [r for r in with_outputs if any(c.value == AFNI_10 for c in data_and_messages(r))]
    messages = [c for r in results for c in r.checks if c.name == MESSAGES]
    explained = [c for c in messages if c.value == AFNI_10]
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
        f"date, and larmorx's own lines, set aside): "
        f"{sum(c.passed for c in messages) - len(explained)} of {len(messages)} identical, in "
        "every case that ran both programs"
        + (
            f"; {len(explained)} explained by AFNI-10 (below: AFNI reports the obliquity of its "
            "header extension's matrix, and AFNI run on the input without its extension prints "
            "the original's messages byte for byte)."
            if explained
            else "."
        ),
    ]
    rounded = [c for r in with_outputs for c in r.checks if c.name == CORRECTLY_ROUNDED]
    if rounded:
        same = [c for c in rounded if c.value == 0]
        lines.append(
            f"- The other {len(rounded)} compared case(s) write float32 values within the threshold "
            "(AFNI's squash calls glibc's `tanhf`, which is not correctly rounded on some "
            "arguments; the original's is): with CORE-MATH's correctly rounded functions "
            f"preloaded into AFNI, {len(same)} of them are bit-identical to the original."
        )
    if afni_10:
        names = ", ".join(f"`{r.case}`" for r in afni_10)
        lines.append(
            f"- {len(afni_10)} case(s) whose input carries AFNI's own header extension, whose "
            "attributes AFNI reads in place of the NIfTI geometry (AFNI-10; the clean-room "
            "original ignores it, as 3dTshift's does): the data are bit-identical, and AFNI run "
            "on the input without its extension writes the original's header and prints its "
            f"messages byte for byte: {names}."
        )
    crashes = [r for r in results if any(c.name == CRASH for c in r.checks)]
    if crashes:
        lines.append(
            f"- {len(crashes)} case(s) where AFNI crashes ({crash_causes}): larmorx prints "
            "AFNI's messages up to the crash, then refuses with one line and exit status 1."
        )
    fmri = [c for r in results for c in r.checks if c.name.endswith(FMRIPREP_OWN)]
    if fmri:
        line = (
            f"- **fMRIPrep's own outputs** (the oracle runs' node outputs, as fMRIPrep 25.2.6 ran "
            f"them in its image): {sum(c.value == 0 for c in fmri)} of {len(fmri)} bit-identical "
            f"to the {implementation}'s"
        )
        rest = [r for r in results for c in r.checks if c.name.endswith(FMRIPREP_OWN) and c.value]
        if rest:
            # The clean-room original's float differences: fMRIPrep's output is AFNI's oracle
            # output, which AFNI with correctly rounded maths turns into the original's.
            explained = [
                r
                for r in rest
                if all(c.value == 0 for c in r.checks if c.name.endswith(FMRIPREP_AFNI))
                and any(c.name.endswith(FMRIPREP_AFNI) for c in r.checks)
                and all(c.value == 0 for c in r.checks if c.name == CORRECTLY_ROUNDED)
                and any(c.name == CORRECTLY_ROUNDED for c in r.checks)
            ]
            line += (
                f"; the other {len(rest)} within the threshold, and in {len(explained)} of them "
                "fMRIPrep's output is AFNI's oracle output bit for bit, which AFNI with correctly "
                "rounded maths turns into the original's (glibc's `tanhf`)"
            )
        lines.append(line + ".")
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
    lines += afni_versions.step_highlights(results)
    lines += defects.summary(results)
    return lines
