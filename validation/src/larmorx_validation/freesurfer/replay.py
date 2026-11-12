# SPDX-License-Identifier: Apache-2.0
"""Replaying one recorded step of recon-all: the subject directory rebuilt as it was before the
step, a program run there, its files compared with the recorded ones.

- :func:`rebuild` makes ``<dest>/subjects/<id>/`` from a step's ``before`` map (every file's
  content from the recording's blobs), the directories that existed then (their birth times in
  the finished run), the links the finished run has where the state allows them, and the
  links beside the subject (``fsaverage``) that existed then; the step's command line and
  working directory with the recorded subjects directory replaced by the new one, and its
  environment: recon-all's own (``scripts/recon-all.env``, its FreeSurfer, locale, shell and
  path variables) with the step's recorded variables and the thread count.
- :func:`run_freesurfer` runs FreeSurfer's own program there through the oracle-output cache
  (``oracle_cache.run``, a launcher naming the installation, every file of the subject
  directory an input, the subjects directory the place it writes); :func:`run_command` runs any
  other program the same way, uncached (LARMORX's, later).
- :func:`compare_step` compares the subject directory after the run with the recorded state
  after the step (:mod:`larmorx_validation.freesurfer.files`).
- :func:`replay` does the three.

One thread by default (``threads=1``: ``OMP_NUM_THREADS``, ``FS_OMP_NUM_THREADS``,
``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`` and the programs' thread options); ``threads=None``
keeps the recording's settings.
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import socket
import subprocess
import time
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.freesurfer import files as fsfiles
from larmorx_validation.freesurfer.package import base, launcher
from larmorx_validation.freesurfer.recording import Recording, Step, skipped

#: The variables of recon-all's recorded environment a replayed step gets (the rest of the
#: machine's session is left out): exact names, and prefixes.
ENV_NAMES = frozenset(
    {
        "PATH", "LD_LIBRARY_PATH", "HOME", "USER", "LOGNAME", "SHELL", "LANG", "LANGUAGE", "TZ",
        "TMPDIR", "SUBJECTS_DIR", "PERL5LIB", "FUNCTIONALS_DIR", "LOCAL_DIR", "FIX_VERTEX_AREA",
        "HOST", "HOSTTYPE", "VENDOR", "OSTYPE", "MACHTYPE", "GROUP",
    }
)  # fmt: skip
ENV_PREFIXES = ("LC_", "FS", "FREESURFER", "MNI_", "MINC_", "OMP_", "GOMP_", "ITK_")
#: The variables that set a step's threads (recon-all's -openmp and -itkthreads).
THREAD_ENV = ("OMP_NUM_THREADS", "FS_OMP_NUM_THREADS", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
#: The thread options of the programs recon-all 8.2.0 runs, as the recordings show them
#: (mris_place_surface, mri_surf2volseg, rca-* --threads; mris_sphere -threads; ...).
THREAD_OPTIONS = ("--threads", "-threads", "--nthreads", "-nthreads", "-openmp", "--openmp", "-j")
#: A replay's root holds this file (a root without it is never emptied).
MARKER = ".larmorx-fs-replay"


def _keep(name: str) -> bool:
    # never the recorder's own variables: a nested fs_time would write into the recording
    if name.startswith("LARMORX_"):
        return False
    return name in ENV_NAMES or name.startswith(ENV_PREFIXES)


def _remap(text: str, old: str, new: str) -> str:
    return re.sub(re.escape(old) + r"(?=/|$|\s|:)", lambda _m: new, text) if old else text


@dataclass
class Rebuilt:
    """A step's subject directory, rebuilt, and how to run the step there."""

    recording: Recording
    step: Step
    root: Path
    subjects_dir: Path
    subject_dir: Path
    cwd: Path
    argv: list[str]
    env: dict[str, str]
    threads: int
    files: int = 0
    bytes: int = 0
    links: list[str] = field(default_factory=list)

    @property
    def inputs(self) -> list[Path]:
        """Every file of the rebuilt subject directory (the run's inputs, by content)."""
        out = []
        for rel in sorted(self.step.before):
            p = self.subject_dir / rel
            if p.is_file():
                out.append(p)
        return out


def step_env(
    rec: Recording, step: Step, subjects_dir: Path, cwd: Path, threads: int | None
) -> tuple[dict[str, str], int]:
    """The environment a replayed step runs in, and its thread count."""
    env = {k: v for k, v in rec.environment.items() if _keep(k)}
    env.update(step.env)
    for k, v in list(env.items()):
        env[k] = _remap(v, rec.subjects_dir, str(subjects_dir))
    env["SUBJECTS_DIR"] = str(subjects_dir)
    env["PWD"] = str(cwd)
    if threads is None:
        try:
            n = int(step.env.get("OMP_NUM_THREADS") or env.get("OMP_NUM_THREADS") or 1)
        except ValueError:
            n = 1
    else:
        n = threads
        for k in THREAD_ENV:
            env[k] = str(n)
    return env, n


def step_argv(rec: Recording, step: Step, subjects_dir: Path, threads: int | None) -> list[str]:
    """The step's command line for the replay: the subjects directory replaced, the program by
    its name, its thread options set to ``threads`` (``None``: as recorded)."""
    argv = [_remap(a, rec.subjects_dir, str(subjects_dir)) for a in step.argv]
    argv[0] = os.path.basename(argv[0])
    if threads is not None:
        for i in range(1, len(argv) - 1):
            if argv[i] in THREAD_OPTIONS and argv[i + 1].isdigit():
                argv[i + 1] = str(threads)
    return argv


def rebuild(rec: Recording, step: Step, dest: Path, *, threads: int | None = 1) -> Rebuilt:
    """The subject directory as it was before ``step``, under ``dest/subjects/``."""
    if step.final:
        raise ValueError("the final pseudo-step runs nothing")
    dest = Path(dest).absolute()
    if dest.exists():
        if not (dest / MARKER).is_file() and any(dest.iterdir()):
            raise FileExistsError(f"{dest} is not a replay's directory (no {MARKER}); not emptied")
        shutil.rmtree(dest)  # a replay's own rebuilt copy: regenerable from the recording
    subjects = dest / "subjects"
    subject = subjects / rec.subject
    subject.mkdir(parents=True)
    (dest / MARKER).write_text(f"{rec.root}\n{step.label}\n", encoding="utf-8")
    # directories that existed before the step (birth time), and the files' own directories
    for t, kind, rel, _text in rec.born:
        if kind == "d" and (t == 0.0 or t < step.start):
            (subjects / rel).mkdir(parents=True, exist_ok=True)
    n = size = 0
    for rel, sha in sorted(step.before.items()):
        if skipped(rel):
            continue
        blob = rec.blob(sha)
        target = subject / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(blob, target)
        n += 1
        size += target.stat().st_size
    links = []
    for rel, text in sorted(rec.links.items()):
        target_rel = os.path.normpath(os.path.join(os.path.dirname(rel), text))
        if (
            not os.path.isabs(text)
            and rel in step.before
            and step.before.get(target_rel) == step.before[rel]
        ):
            (subject / rel).unlink()
            (subject / rel).symlink_to(text)
            links.append(rel)
    for t, kind, rel, text in rec.born:
        if kind == "l" and "/" not in rel and rel != rec.subject and (t == 0.0 or t < step.start):
            link = subjects / rel
            if not link.exists() and not link.is_symlink():
                link.symlink_to(text)
                links.append(f"../{rel}")
    cwd_text = _remap(step.cwd, rec.subjects_dir, str(subjects))
    cwd = Path(cwd_text)
    cwd.mkdir(parents=True, exist_ok=True)
    env, nthreads = step_env(rec, step, subjects, cwd, threads)
    argv = step_argv(rec, step, subjects, threads)
    return Rebuilt(rec, step, dest, subjects, subject, cwd, argv, env, nthreads, n, size, links)


#: What a FreeSurfer program prints that changes from run to run (the cache's comparison of its
#: messages): the dates, times and memory figures of :data:`files.LOG_MASKS`.
STREAM_VOLATILE: tuple = (*fsfiles.DATES, *fsfiles.LOG_MASKS)


def run_freesurfer(rb: Rebuilt, *, timeout: float | None = None):
    """FreeSurfer's own program, run in the rebuilt directory through the oracle-output cache."""
    from larmorx_validation import oracle_cache

    fs_home = rb.env.get("FREESURFER_HOME")
    if not fs_home:
        raise RuntimeError("the recording names no FREESURFER_HOME")
    prog = launcher(rb.argv[0], fs_home)
    policy = oracle_cache.Policy(
        threads=rb.threads,
        family="freesurfer",
        formats="freesurfer",
        volatile=STREAM_VOLATILE,
    )
    return oracle_cache.run(
        [str(prog), *rb.argv[1:]],
        env=rb.env,
        cwd=rb.cwd,
        outputs=[rb.subjects_dir],
        policy=policy,
        timeout=timeout,
        runner="freesurfer-replay",
        inputs=rb.inputs,
    )


@dataclass
class Ran:
    """A run that did not go through the cache (:func:`run_command`)."""

    argv: list[str]
    returncode: int | None
    stdout_bytes: bytes
    stderr_bytes: bytes
    seconds: float
    cached: bool = False


def run_command(rb: Rebuilt, command: Sequence[str], *, timeout: float | None = None) -> Ran:
    """Another program in place of FreeSurfer's (LARMORX's, later): ``command`` is its command
    line before the step's arguments; ``{program}`` in it is the step's program."""
    prefix = [c.replace("{program}", rb.argv[0]) for c in command]
    argv = [*prefix, *rb.argv[1:]] if "{program}" in " ".join(command) else [*prefix, *rb.argv]
    start = time.perf_counter()
    try:
        p = subprocess.run(
            argv, cwd=rb.cwd, env=rb.env, capture_output=True, timeout=timeout, check=False
        )
        code, out, err = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired as e:
        code, out, err = None, e.stdout or b"", e.stderr or b""
    return Ran(argv, code, out, err, time.perf_counter() - start)


# ------------------------------------------------------------------------------------------------
# The comparison


_DATED = re.compile(r"\.Y\d{4}\.M\d\d\.D\d\d\.H\d\d\.M\d\d")


def _sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(4 << 20):
            h.update(block)
    return h.hexdigest()


def scan(subject: Path) -> dict[str, str]:
    """The subject directory as the recorder sees it: path -> SHA-256 (links as their files)."""
    out = {}
    for d, dirs, names in os.walk(subject):
        rel_d = os.path.relpath(d, subject)
        if rel_d == ".":
            dirs[:] = [x for x in dirs if x != "tmp"]
        for name in names:
            rel = os.path.normpath(os.path.join(rel_d, name))
            p = Path(d) / name
            if skipped(rel) or not p.is_file():
                continue
            out[rel] = _sha(p)
    return out


@dataclass
class FileResult:
    path: str
    replay_path: str
    comparison: fsfiles.Comparison


@dataclass
class StepReport:
    run: str
    stream: str
    step: str
    argv: list[str]
    threads: int
    exit_recorded: int
    exit_replayed: int | None
    seconds_recorded: float
    seconds_replayed: float
    served: bool
    files: list[FileResult]
    note: str = ""

    @property
    def verdict(self) -> str:
        if self.exit_replayed != self.exit_recorded:
            return "differs"
        verdicts = {f.comparison.verdict for f in self.files}
        if verdicts <= {fsfiles.IDENTICAL, fsfiles.PROVENANCE}:
            return "reproduced"
        if verdicts <= {fsfiles.IDENTICAL, fsfiles.PROVENANCE, fsfiles.LOG}:
            return "reproduced; a log differs"
        return "differs"

    def counts(self) -> dict[str, int]:
        out: dict[str, int] = {}
        for f in self.files:
            out[f.comparison.verdict] = out.get(f.comparison.verdict, 0) + 1
        return out

    def to_dict(self) -> dict:
        return {
            "run": self.run,
            "stream": self.stream,
            "step": self.step,
            "argv": self.argv,
            "threads": self.threads,
            "verdict": self.verdict,
            "exit": {"recorded": self.exit_recorded, "replayed": self.exit_replayed},
            "seconds": {
                "recorded": self.seconds_recorded,
                "replayed": round(self.seconds_replayed, 3),
            },
            "served_from_cache": self.served,
            "counts": self.counts(),
            "files": [
                {
                    "path": f.path,
                    "replay_path": f.replay_path,
                    "kind": f.comparison.kind,
                    "verdict": f.comparison.verdict,
                    "detail": f.comparison.detail,
                }
                for f in self.files
            ],
            "note": self.note,
        }


def _names(env: dict[str, str]) -> tuple[str, ...]:
    out = {env.get("USER", ""), env.get("LOGNAME", ""), env.get("HOST", "")}
    out.add(socket.gethostname())
    return tuple(sorted(n for n in out if len(n) >= 3))


def compare_step(rb: Rebuilt, done) -> StepReport:
    """The rebuilt directory after the run against the recorded state after the step."""
    rec, step = rb.recording, rb.step
    expected = step.after()
    actual = scan(rb.subject_dir)
    before = step.before
    changed_ref = {rel for rel, sha in expected.items() if before.get(rel) != sha} | set(
        step.deleted
    )
    changed_rep = {rel for rel, sha in actual.items() if before.get(rel) != sha}
    changed_rep |= {rel for rel in before if rel not in actual and not skipped(rel)}
    # logs named by the time they were written: paired by their names without the time
    ref_dated = {_DATED.sub(".Y…", r): r for r in changed_ref if _DATED.search(r)}
    rep_dated = {_DATED.sub(".Y…", r): r for r in changed_rep if _DATED.search(r)}
    pairs: list[tuple[str | None, str | None]] = []
    for key in sorted(set(ref_dated) | set(rep_dated)):
        pairs.append((ref_dated.get(key), rep_dated.get(key)))
    plain = (changed_ref | changed_rep) - set(ref_dated.values()) - set(rep_dated.values())
    pairs += [(r, r) for r in sorted(plain)]
    ctx = fsfiles.Context(
        pairs=((rec.subjects_dir, str(rb.subjects_dir)),),
        names=tuple(sorted(set(_names(rec.environment)) | set(_names(rb.env)))),
    )
    results = []
    for ref, rep in pairs:
        a = rec.blob(expected[ref]) if ref is not None and ref in expected else None
        b = rb.subject_dir / rep if rep is not None and rep in actual else None
        c = fsfiles.compare(a, b, ctx, name=os.path.basename(rep or ref or ""))
        results.append(FileResult(ref or "", rep or "", c))
    return StepReport(
        run=rec.name,
        stream=rec.stream,
        step=step.label,
        argv=rb.argv,
        threads=rb.threads,
        exit_recorded=step.exit,
        exit_replayed=done.returncode,
        seconds_recorded=step.seconds,
        seconds_replayed=float(getattr(done, "seconds", 0.0)),
        served=bool(getattr(done, "cached", False)),
        files=results,
    )


def work_dir(rec: Recording, step: Step, threads: int | None) -> Path:
    t = "recorded" if threads is None else f"t{threads}"
    return base() / "work" / rec.name / f"{step.label}-{t}"


#: Beside the marker: how the replay's run ended (``--compare-only`` compares again from it).
RESULT = ".larmorx-fs-replay-result.json"


def _again(rec: Recording, step: Step, dest: Path, threads: int | None) -> tuple[Rebuilt, Ran]:
    """A replay already made in ``dest``, as it was left (its run's result from :data:`RESULT`)."""
    import json

    marker, result = dest / MARKER, dest / RESULT
    if not marker.is_file() or marker.read_text(encoding="utf-8").split("\n")[:2] != [
        str(rec.root),
        step.label,
    ]:
        raise FileNotFoundError(f"{dest} holds no replay of {rec.name} {step.label}")
    r = json.loads(result.read_text(encoding="utf-8"))
    subjects = dest / "subjects"
    cwd = Path(_remap(step.cwd, rec.subjects_dir, str(subjects)))
    env, n = step_env(rec, step, subjects, cwd, threads)
    rb = Rebuilt(
        rec,
        step,
        dest,
        subjects,
        subjects / rec.subject,
        cwd,
        step_argv(rec, step, subjects, threads),
        env,
        n,
    )
    return rb, Ran(
        r["argv"], r["returncode"], b"", r.get("tail", "").encode(), r["seconds"], r["cached"]
    )


def replay(
    rec: Recording,
    step: Step,
    *,
    threads: int | None = 1,
    dest: Path | None = None,
    command: Sequence[str] | None = None,
    timeout: float | None = None,
    compare_only: bool = False,
) -> tuple[StepReport, Rebuilt]:
    """Rebuild, run (FreeSurfer's program, or ``command``), compare; with ``compare_only`` only
    compare a replay made before in ``dest`` again (after a change of the comparison)."""
    import json

    dest = dest or work_dir(rec, step, threads)
    if compare_only:
        rb, done = _again(rec, step, dest, threads)
    else:
        rb = rebuild(rec, step, dest, threads=threads)
        done = (
            run_command(rb, command, timeout=timeout)
            if command
            else run_freesurfer(rb, timeout=timeout)
        )
        tail = (done.stderr_bytes or done.stdout_bytes)[-600:].decode("utf-8", "replace")
        (dest / RESULT).write_text(
            json.dumps(
                {
                    "argv": [str(a) for a in getattr(done, "args", getattr(done, "argv", []))],
                    "returncode": done.returncode,
                    "seconds": round(float(done.seconds), 3),
                    "cached": bool(getattr(done, "cached", False)),
                    "tail": tail,
                },
                indent=1,
            ),
            encoding="utf-8",
        )
    report = compare_step(rb, done)
    if done.returncode not in (0, step.exit):
        tail = (done.stderr_bytes or done.stdout_bytes)[-600:].decode("utf-8", "replace")
        report.note = f"its messages end: {tail}"
    return report, rb


__all__ = [
    "ENV_NAMES",
    "ENV_PREFIXES",
    "MARKER",
    "STREAM_VOLATILE",
    "THREAD_ENV",
    "THREAD_OPTIONS",
    "FileResult",
    "Ran",
    "Rebuilt",
    "StepReport",
    "compare_step",
    "rebuild",
    "replay",
    "run_command",
    "run_freesurfer",
    "scan",
    "step_argv",
    "step_env",
    "work_dir",
]
