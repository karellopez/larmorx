# SPDX-License-Identifier: Apache-2.0
"""Running a node's program again: with larmorx, with the oracle's own container, and with
the ANTs 2.6.5 oracle build (as built and with correctly rounded math).

Every run writes into its own directory under the comparison root,
``<compare>/<node path>/<variant>/``, with ``run.json`` (command, environment, exit status,
seconds). Reference runs (everything but larmorx) can be reused from an earlier invocation
(``--reuse``): their result does not change between invocations, and some take minutes.

The arguments are fMRIPrep's own (``command.txt``). Container paths are mapped to this
machine for larmorx and the host oracle; the container runs see the same paths as fMRIPrep did,
mounted read-only. Output arguments are rewritten to plain file names, so every run writes
into its own directory and nothing is ever written into the oracle run.
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import shutil
import subprocess
import time
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.stages.catalogue import CLI_PROGRAMS
from larmorx_validation.stages.oracle import Node, OracleRun


@dataclass
class RunResult:
    """One run of a program."""

    variant: str
    directory: Path
    code: int
    seconds: float
    outputs: list[Path]
    threads: int
    stderr: str = ""
    reused: bool = False

    @property
    def ok(self) -> bool:
        return self.code == 0 and all(p.exists() for p in self.outputs)


@dataclass
class Settings:
    """How a comparison runs."""

    run: OracleRun
    #: Root of the comparison outputs (outside git).
    compare: Path
    #: Re-run fMRIPrep's own tools in its container (timing on this machine, run-to-run
    #: reproducibility, the one-thread result).
    container: bool = True
    #: Run the ANTs 2.6.5 oracle build (as built and with CORE-MATH preloaded) where larmorx
    #: and fMRIPrep differ.
    ants_oracle: bool = True
    #: Reuse the one-thread reference runs of an earlier invocation (the runs whose times are
    #: reported, larmorx's and fMRIPrep's program at fMRIPrep's thread count, always run).
    reuse: bool = False
    #: larmorx's thread count; ``None``: the node's own.
    threads: int | None = None
    #: Only nodes whose path or stage contains one of these.
    select: Sequence[str] = ()
    log: object = field(default=None, repr=False)

    def say(self, text: str) -> None:
        if callable(self.log):
            self.log(text)


# ------------------------------------------------------------------------------------------------
# Arguments


def _bracket_values(text: str) -> list[str]:
    """``[ a, b ]`` -> ``["a", "b"]``; a plain value -> ``[value]``."""
    text = text.strip()
    if text.startswith("["):
        return [v.strip() for v in text.strip("[]").split(",") if v.strip()]
    return [text]


def _option(argv: Sequence[str], names: Sequence[str]) -> str | None:
    """The value of an option, with ANTs' ``[ ... ]`` values re-joined across words."""
    for i, a in enumerate(argv):
        if a in names and i + 1 < len(argv):
            value = argv[i + 1]
            if value.startswith("[") and "]" not in value:
                j = i + 2
                while j < len(argv) and "]" not in value:
                    value += " " + argv[j]
                    j += 1
            return value
    return None


def output_names(argv: Sequence[str]) -> list[str]:
    """The files a command line writes, as its arguments name them."""
    program = os.path.basename(argv[0])
    a = list(argv[1:])
    if program == "ImageMath":
        return [a[1]]
    if program == "ThresholdImage":
        return [a[2]]
    if program == "MultiplyImages":
        return [a[3]]
    if program in ("antsApplyTransforms", "DenoiseImage", "N4BiasFieldCorrection"):
        value = _option(a, ("-o", "--output"))
        if value is None:
            return []
        names = _bracket_values(value)
        if program == "antsApplyTransforms":
            # [out, 1] writes a composite transform: one file
            return names[:1]
        return names
    if program in ("SmoothImage", "ResampleImageBySpacing", "ResampleImage"):
        return [a[2]]
    if program == "3dTshift":
        value = _option(a, ("-prefix",))
        return [value] if value else []
    if program == "mcflirt":
        value = _option(a, ("-out", "-o"))
        return [value] if value else []
    return []


def _redirect_outputs(argv: Sequence[str], outputs: Sequence[str]) -> list[str]:
    """``argv`` with absolute output paths replaced by their file names."""
    out = list(argv)
    for name in outputs:
        if os.path.isabs(name):
            base = os.path.basename(name)
            out = [t.replace(name, base) for t in out]
    return out


def reference_outputs(run: OracleRun, node: Node, outputs: Sequence[str]) -> list[Path]:
    """Where fMRIPrep's node wrote ``outputs`` on this machine."""
    return [Path(run.host(o)) if os.path.isabs(o) else run.node_dir(node) / o for o in outputs]


# ------------------------------------------------------------------------------------------------
# Run bookkeeping


def _fresh(directory: Path) -> None:
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir(parents=True)


def _record(directory: Path, payload: dict) -> None:
    (directory / "run.json").write_text(json.dumps(payload, indent=1), encoding="utf-8")


def _reusable(directory: Path, argv: Sequence[str], threads: int) -> dict | None:
    f = directory / "run.json"
    if not f.is_file():
        return None
    try:
        rec = json.loads(f.read_text(encoding="utf-8"))
    except ValueError:
        return None
    if rec.get("argv") != list(argv) or rec.get("threads") != threads or rec.get("code") != 0:
        return None
    return rec


# ------------------------------------------------------------------------------------------------
# larmorx


def run_larmorx(
    s: Settings, node: Node, argv: Sequence[str], directory: Path, threads: int
) -> RunResult:
    """``larmorx <family> <program> <args>`` in-process (the Python console entry point, as a
    user runs it), in ``directory``, with ``threads`` threads."""
    from larmorx.cli import run as cli_run

    program = os.path.basename(argv[0])
    outputs = output_names(argv)
    host = [s.run.host_text(t) for t in _redirect_outputs(argv, outputs)[1:]]
    family = CLI_PROGRAMS.get(program, "mri" if program == "mcflirt" else "")
    name = "hmc" if program == "mcflirt" else program
    _fresh(directory)
    # D13: fMRIPrep's programs as they ran, so their defects reproduced.
    env = {
        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
        "OMP_NUM_THREADS": str(threads),
        "LARMORX_UPSTREAM_DEFECTS": "reproduce",
    }
    if program == "mcflirt":
        env["FSLOUTPUTTYPE"] = node.inputs.get("output_type", "NIFTI_GZ") or "NIFTI_GZ"
    saved = {k: os.environ.get(k) for k in env}
    cwd = os.getcwd()
    out, err = io.StringIO(), io.StringIO()
    os.environ.update(env)
    try:
        os.chdir(directory)
        t0 = time.perf_counter()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = cli_run(["larmorx", family, name, *host])
        seconds = time.perf_counter() - t0
    finally:
        os.chdir(cwd)
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    files = [directory / os.path.basename(o) for o in outputs]
    _record(
        directory,
        {
            "variant": "larmorx",
            "argv": ["larmorx", family, name, *host],
            "threads": threads,
            "env": env,
            "code": code,
            "seconds": seconds,
            "stderr": err.getvalue()[-2000:],
        },
    )
    return RunResult("larmorx", directory, code, seconds, files, threads, err.getvalue()[-500:])


# ------------------------------------------------------------------------------------------------
# The oracle's container


#: Runs a program inside the container and records its own time (without Apptainer's start).
_TIMED = (
    's=$(date +%s.%N); "$@" > .stdout 2> .stderr; rc=$?; e=$(date +%s.%N); '
    'echo "$rc $s $e" > .timing; exit $rc'
)


def container_command(
    s: Settings,
    directory: Path,
    argv: Sequence[str],
    env: dict[str, str],
    binds: Sequence[tuple[Path, str]] = (),
) -> list[str]:
    """``apptainer exec`` of ``argv`` in the oracle's image, with the run's mounts read-only and
    ``directory`` as the working directory (``/cmp``); ``binds`` adds read-only mounts."""
    image = s.run.image
    if image is None:
        raise FileNotFoundError("the oracle's container image was not found")
    home = s.compare / ".home"
    home.mkdir(parents=True, exist_ok=True)
    cmd = ["apptainer", "exec", "--cleanenv", "--home", f"{home}:/home/fmriprep", "--pwd", "/cmp"]
    env = {"TEMPLATEFLOW_HOME": "/templateflow", **env}
    for k, v in env.items():
        cmd += ["--env", f"{k}={v}"]
    for mount, host in sorted(s.run.mounts.items()):
        cmd += ["-B", f"{host}:{mount}:ro"]
    for host, mount in binds:
        cmd += ["-B", f"{host}:{mount}:ro"]
    cmd += ["-B", f"{directory}:/cmp", str(image)]
    return [*cmd, "bash", "-c", _TIMED, "bash", *argv]


def run_container(
    s: Settings,
    node: Node,
    argv: Sequence[str],
    directory: Path,
    threads: int,
    *,
    timed: bool = False,
    correctly_rounded: bool = False,
) -> RunResult | None:
    """The node's program, as fMRIPrep ran it, in fMRIPrep's own image, with ``threads``
    threads (``NSLOTS``, as nipype sets it); with ``correctly_rounded``, with CORE-MATH's
    functions preloaded (the image has the same glibc as this machine). A ``timed`` run (its
    time is reported) is never reused from an earlier invocation. ``None`` when the preload
    is missing."""
    from larmorx_validation.parity.ants_programs import crmath_preload

    outputs = output_names(argv)
    args = _redirect_outputs(argv, outputs)
    variant = f"container-{threads}t" + ("-cr" if correctly_rounded else "")
    files = [directory / os.path.basename(o) for o in outputs]
    reuse = s.reuse and not timed
    if reuse and (rec := _reusable(directory, args, threads)) and all(f.exists() for f in files):
        return RunResult(variant, directory, 0, rec["seconds"], files, threads, reused=True)
    env = {"NSLOTS": str(threads), "OMP_NUM_THREADS": "1", "MKL_NUM_THREADS": "1"}
    binds: list[tuple[Path, str]] = []
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            return None
        binds.append((lib.parent, "/crmath"))
        env["LD_PRELOAD"] = f"/crmath/{lib.name}"
    _fresh(directory)
    if node.program == "mcflirt" or node.program in ("fslmaths", "bet", "flirt"):
        env["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    p = subprocess.run(
        container_command(s, directory, args, env, binds),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=7200,
    )
    seconds = float("nan")
    timing = directory / ".timing"
    if timing.is_file():
        _, start, end = timing.read_text().split()
        seconds = float(end) - float(start)
    stderr = (
        (directory / ".stderr").read_text(errors="replace")
        if (directory / ".stderr").is_file()
        else p.stderr
    )
    _record(
        directory,
        {
            "variant": variant,
            "argv": list(args),
            "threads": threads,
            "env": env,
            "image": str(s.run.image),
            "code": p.returncode,
            "seconds": seconds,
            "stderr": stderr[-2000:],
        },
    )
    return RunResult(variant, directory, p.returncode, seconds, files, threads, stderr[-500:])


def run_container_python(
    s: Settings, directory: Path, script: str, args: Sequence[str], threads: int = 1
) -> subprocess.CompletedProcess:
    """A Python script inside the oracle's image (fMRIPrep's own code), in ``directory``."""
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "script.py").write_text(script, encoding="utf-8")
    env = {"OMP_NUM_THREADS": str(threads), "MKL_NUM_THREADS": "1", "PYTHONWARNINGS": "ignore"}
    cmd = container_command(s, directory, ["python", "/cmp/script.py", *args], env)
    p = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=7200
    )
    err = directory / ".stderr"
    if err.is_file():
        p.stderr = err.read_text(errors="replace")
    return p


def container_seconds(directory: Path) -> float:
    timing = directory / ".timing"
    if timing.is_file():
        _, start, end = timing.read_text().split()
        return float(end) - float(start)
    return float("nan")


# ------------------------------------------------------------------------------------------------
# The ANTs 2.6.5 oracle on this machine


def run_ants_oracle(
    s: Settings, argv: Sequence[str], directory: Path, *, correctly_rounded: bool
) -> RunResult | None:
    """The program from the ANTs 2.6.5 oracle build, one thread, as built or with CORE-MATH's
    correctly rounded functions preloaded. ``None`` when the build (or the preload) is
    missing."""
    from larmorx_validation.parity.ants_programs import ants_bin, crmath_preload

    program = os.path.basename(argv[0])
    b = ants_bin()
    if b is None or not (b / program).is_file():
        return None
    env = {k: v for k, v in os.environ.items() if k not in ("NSLOTS", "LD_PRELOAD")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = "1"
    variant = "ants-2.6.5-1t" + ("-cr" if correctly_rounded else "")
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            return None
        env["LD_PRELOAD"] = str(lib)
    outputs = output_names(argv)
    args = [s.run.host_text(t) for t in _redirect_outputs(argv, outputs)[1:]]
    files = [directory / os.path.basename(o) for o in outputs]
    if (
        s.reuse
        and (rec := _reusable(directory, [program, *args], 1))
        and all(f.exists() for f in files)
    ):
        return RunResult(variant, directory, 0, rec["seconds"], files, 1, reused=True)
    _fresh(directory)
    t0 = time.perf_counter()
    p = subprocess.run(
        [str(b / program), *args],
        cwd=directory,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=7200,
    )
    seconds = time.perf_counter() - t0
    _record(
        directory,
        {
            "variant": variant,
            "argv": [program, *args],
            "threads": 1,
            "binary": str(b / program),
            "preload": env.get("LD_PRELOAD", ""),
            "code": p.returncode,
            "seconds": seconds,
            "stderr": p.stderr[-2000:],
        },
    )
    return RunResult(variant, directory, p.returncode, seconds, files, 1, p.stderr[-500:])


def host_mcflirt() -> Path | None:
    """FSL 6.0.7's ``mcflirt`` on this machine (``LARMORX_MCFLIRT``, else ``~/fsl/bin``)."""
    if env := os.environ.get("LARMORX_MCFLIRT"):
        return Path(env)
    candidate = Path.home() / "fsl" / "bin" / "mcflirt"
    return candidate if candidate.is_file() else None


def run_host_mcflirt(s: Settings, argv: Sequence[str], directory: Path) -> RunResult | None:
    """The host's FSL 6.0.7 ``mcflirt`` with fMRIPrep's arguments (a different FSL build)."""
    exe = host_mcflirt()
    if exe is None:
        return None
    outputs = output_names(argv)
    args = [s.run.host_text(t) for t in _redirect_outputs(argv, outputs)[1:]]
    files = [directory / os.path.basename(o) for o in outputs]
    mats = [directory / (os.path.basename(o) + ".mat") for o in outputs]
    if (
        s.reuse
        and (rec := _reusable(directory, ["mcflirt", *args], 1))
        and all(m.is_dir() for m in mats)
    ):
        return RunResult("fsl-6.0.7", directory, 0, rec["seconds"], mats, 1, reused=True)
    _fresh(directory)
    fsldir = exe.parent.parent
    env = {k: v for k, v in os.environ.items() if k != "LD_PRELOAD"}
    env.update({"FSLDIR": str(fsldir), "FSLOUTPUTTYPE": "NIFTI_GZ", "OMP_NUM_THREADS": "1"})
    t0 = time.perf_counter()
    p = subprocess.run(
        [str(exe), *args], cwd=directory, env=env, capture_output=True, text=True, timeout=7200
    )
    seconds = time.perf_counter() - t0
    _record(
        directory,
        {
            "variant": "fsl-6.0.7",
            "argv": ["mcflirt", *args],
            "threads": 1,
            "binary": str(exe),
            "code": p.returncode,
            "seconds": seconds,
            "stderr": p.stderr[-2000:],
        },
    )
    del files
    return RunResult("fsl-6.0.7", directory, p.returncode, seconds, mats, 1, p.stderr[-500:])


def fsl_version(exe: Path) -> str:
    v = exe.parent.parent / "etc" / "fslversion"
    return v.read_text().split(":")[0].strip() if v.is_file() else "unknown"


_VERSIONS_SCRIPT = r'''
"""The versions of the tools inside fMRIPrep's image."""
import json
import os
import re
import subprocess
import sys
from importlib.metadata import version

out = {"python": sys.version.split()[0]}
for name in ("fmriprep", "smriprep", "niworkflows", "sdcflows", "nipype", "nitransforms",
             "nibabel", "numpy", "scipy"):
    try:
        out[name] = version(name)
    except Exception:
        pass
meta = os.path.join(sys.prefix, "conda-meta")
pattern = re.compile(
    r"(ants|libitk|fsl-mcflirt|fsl-newimage|fsl-miscmaths|fsl-bet2|fsl-fast4|fsl-flirt|"
    r"libopenblas)-(\d[^-]*)-([^-]+)\.json$"
)
if os.path.isdir(meta):
    for f in sorted(os.listdir(meta)):
        m = pattern.match(f)
        if m:
            out[m.group(1)] = f"{m.group(2)} (build {m.group(3)})"
for cmd, key in (
    (["antsRegistration", "--version"], "ants_version"),
    (["3dvolreg", "-ver"], "afni"),
    (["ldd", "--version"], "glibc"),
):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        text = (p.stdout + p.stderr).strip().splitlines()
        out[key] = text[0].strip() if text else ""
    except Exception:
        pass
open("versions.json", "w").write(json.dumps(out))
'''


def container_versions(s: Settings) -> dict[str, str]:
    """The versions of the tools inside the oracle's image (cached next to the comparison)."""
    directory = s.compare / ".versions"
    record = directory / "versions.json"
    if not record.is_file():
        p = run_container_python(s, directory, _VERSIONS_SCRIPT, [])
        if p.returncode != 0 or not record.is_file():
            return {}
    return json.loads(record.read_text(encoding="utf-8"))


__all__ = [
    "RunResult",
    "Settings",
    "container_command",
    "container_seconds",
    "container_versions",
    "host_mcflirt",
    "output_names",
    "reference_outputs",
    "run_ants_oracle",
    "run_container",
    "run_container_python",
    "run_host_mcflirt",
    "run_larmorx",
]
