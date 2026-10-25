# SPDX-License-Identifier: Apache-2.0
"""Running an original program through the cache.

``run(argv, env=..., policy=...)`` replaces ``subprocess.run(argv, capture_output=True, ...)``
in the parity suites. In the default mode (``use``) a run whose entry is complete is not run:
its outputs are copied back into this run's directories, and its exit status, standard output
and error are returned, with the stored run's paths replaced by this run's. Otherwise the
program runs, and what it did is stored (``policy``: what may be). The mode comes from
``LARMORX_ORACLE_CACHE_MODE`` (``use``, ``refresh``, ``off``), which ``python -m
larmorx_validation parity --oracle-cache`` sets for its worker processes too.
"""

from __future__ import annotations

import contextlib
import gzip
import os
import shutil
import signal
import subprocess
import time
from collections import Counter
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.oracle_cache import compare
from larmorx_validation.oracle_cache.key import (
    Prepared,
    Uncacheable,
    image_identity,
    prepare,
    program_identity,
    temp_root,
    under,
)
from larmorx_validation.oracle_cache.policy import CONFIRMATIONS, Policy, decide
from larmorx_validation.oracle_cache.store import Store, now

ENV_MODE = "LARMORX_ORACLE_CACHE_MODE"
MODES = ("use", "refresh", "off")
#: What a replayed stream holds in place of a path: ``\0LX:<name>\0``.
_OPEN, _CLOSE = b"\x00LX:", b"\x00"


def mode() -> str:
    value = os.environ.get(ENV_MODE, "use").strip() or "use"
    if value not in MODES:
        raise ValueError(f"{ENV_MODE}={value!r}: choose one of {MODES}")
    return value


@contextlib.contextmanager
def using(value: str):
    """``mode()`` is ``value`` inside the block (tests, the command line)."""
    saved = os.environ.get(ENV_MODE)
    os.environ[ENV_MODE] = value
    try:
        yield
    finally:
        if saved is None:
            os.environ.pop(ENV_MODE, None)
        else:
            os.environ[ENV_MODE] = saved


# ------------------------------------------------------------------------------------------------
# What happened: per case, then summed by the command line


@dataclass
class Tally:
    """The original runs of a case (or a record): ``served`` from the cache, ``ran``; why the
    runs that ran were not served (``fresh``: reason -> count); the time the served runs took
    when they were stored; the differences found."""

    served: int = 0
    ran: int = 0
    fresh: Counter = field(default_factory=Counter)
    saved_seconds: float = 0.0
    differences: list[dict] = field(default_factory=list)

    def add(self, other: Tally) -> Tally:
        self.served += other.served
        self.ran += other.ran
        self.fresh.update(other.fresh)
        self.saved_seconds += other.saved_seconds
        self.differences += other.differences
        return self

    def to_dict(self) -> dict:
        return {
            "served": self.served,
            "ran": self.ran,
            "fresh": dict(self.fresh),
            "saved_seconds": round(self.saved_seconds, 1),
            "differences": list(self.differences),
        }

    @classmethod
    def from_dict(cls, d: dict | None) -> Tally:
        if not d:
            return cls()
        return cls(
            d.get("served", 0),
            d.get("ran", 0),
            Counter(d.get("fresh", {})),
            d.get("saved_seconds", 0.0),
            list(d.get("differences", [])),
        )


@dataclass
class _Case:
    name: str = ""
    keys: set[str] = field(default_factory=set)
    tally: Tally = field(default_factory=Tally)


_case = _Case()


def begin_case(name: str) -> None:
    """A case starts: its runs are counted apart, and a command it runs twice runs twice."""
    global _case
    _case = _Case(name)


def end_case() -> Tally:
    """The case's tally (and a fresh one for what follows)."""
    global _case
    tally = _case.tally
    _case = _Case()
    return tally


def _fresh(reason: str) -> None:
    _case.tally.ran += 1
    _case.tally.fresh[reason] += 1


# ------------------------------------------------------------------------------------------------
# The result


@dataclass
class Completed:
    """A run's result, as ``subprocess.run`` with ``capture_output=True`` gives it: the exit status
    (``None`` after a timeout), the bytes of both streams; ``cached`` when it came from the
    cache."""

    args: list[str]
    returncode: int | None
    stdout_bytes: bytes
    stderr_bytes: bytes
    cached: bool = False
    seconds: float = 0.0

    @property
    def timed_out(self) -> bool:
        return self.returncode is None

    def text(self, stream: str = "stdout", encoding: str = "utf-8", errors: str = "replace") -> str:
        """A stream as ``subprocess.run(..., text=True)`` decodes it (universal newlines)."""
        data = self.stdout_bytes if stream == "stdout" else self.stderr_bytes
        return data.decode(encoding, errors).replace("\r\n", "\n").replace("\r", "\n")

    @property
    def stdout(self) -> str:
        return self.text("stdout")

    @property
    def stderr(self) -> str:
        return self.text("stderr")


def _execute(
    argv: Sequence[str], env: dict[str, str], cwd: Path | None, timeout: float | None
) -> Completed:
    start = time.perf_counter()
    try:
        p = subprocess.run(
            [str(a) for a in argv],
            capture_output=True,
            env=env,
            cwd=None if cwd is None else str(cwd),
            timeout=timeout,
            check=False,
        )
        code, out, err = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired as e:
        code, out, err = None, e.stdout or b"", e.stderr or b""
    return Completed([str(a) for a in argv], code, out, err, False, time.perf_counter() - start)


# ------------------------------------------------------------------------------------------------
# Outputs: what changed in the run's places


def _snapshot(prep: Prepared) -> dict[Path, tuple]:
    files: dict[Path, tuple] = {}
    for path, recursive in prep.watched.values():
        if not path.is_dir():
            continue
        walker = os.walk(path) if recursive else [(str(path), [], None)]
        for root, _dirs, names in walker:
            if names is None:
                with os.scandir(root) as it:
                    names = [e.name for e in it if e.is_file(follow_symlinks=False)]
            for name in names:
                p = Path(root) / name
                try:
                    st = p.lstat()
                except OSError:
                    continue
                if p.is_file():
                    files[p] = (st.st_size, st.st_mtime_ns, st.st_ino)
    return files


def _changes(prep: Prepared, before: dict[Path, tuple]) -> tuple[list[Path], list[Path]]:
    after = _snapshot(prep)
    written = sorted(p for p, s in after.items() if before.get(p) != s)
    deleted = sorted(p for p in before if p not in after)
    return written, deleted


def _place_of(prep: Prepared, path: Path) -> tuple[str, str]:
    """``(place, path below it)`` of a file the run wrote: ``("in<n>", "")`` when it is one of
    its inputs (a run that rewrites a file it was given: replayed onto this run's input),
    else the longest place that holds it."""
    for n, item in enumerate(prep.inputs):
        if item.path == path:
            return f"in{n}", ""
    best = None
    for name, p in prep.places:
        if (path == p or under(path, p)) and (best is None or len(str(p)) > len(str(best[1]))):
            best = (name, p)
    if best is None:
        raise Uncacheable(f"it wrote {path}, outside the places the cache knows")
    rest = path.relative_to(best[1]).as_posix()
    return best[0], "" if rest == "." else rest


def _resolve_place(prep: Prepared, name: str, rest: str) -> Path:
    if name.startswith("in") and name[2:].isdigit():
        return prep.inputs[int(name[2:])].path
    for n, p in prep.places:
        if n == name:
            return p / rest if rest else p
    raise KeyError(name)


# ------------------------------------------------------------------------------------------------
# Streams: paths replaced by placeholders, and back


def _pairs(prep: Prepared, *, storing: bool) -> list[tuple[bytes, bytes]]:
    """``(this run's text, placeholder)`` pairs, longest text first: the places, the inputs
    (and the files beside them) and, when ``storing``, the inputs' file names long enough not
    to be met by chance (replaying knows every input's name)."""
    pairs: list[tuple[str, str]] = [(str(p), n) for n, p in prep.places]
    if prep.home is not None:
        pairs.append((str(prep.home), "home"))
    for i, item in enumerate(prep.inputs):
        pairs.append((str(item.path), f"in{i}"))
        for j, (_s, _h, c) in enumerate(item.companions):
            pairs.append((str(c), f"in{i}c{j}"))
        name = item.path.name
        if not storing or (len(name) >= 5 and "." in name):
            pairs.append((name, f"in{i}name"))
    out = [(t.encode(), _OPEN + n.encode() + _CLOSE) for t, n in pairs if len(t) >= 2]
    return sorted(set(out), key=lambda x: -len(x[0]))


def _to_placeholders(data: bytes, prep: Prepared) -> bytes:
    if _OPEN in data:
        raise Uncacheable("its messages hold the cache's placeholder bytes")
    for text, ph in _pairs(prep, storing=True):
        data = data.replace(text, ph)
    return data


class _Unresolved(Exception):
    """A stored entry names a place or an input this run does not have."""


def _from_placeholders(data: bytes, prep: Prepared) -> bytes:
    if _OPEN not in data:
        return data
    names = {ph: text for text, ph in _pairs(prep, storing=False)}
    out, i = [], 0
    while (j := data.find(_OPEN, i)) >= 0:
        k = data.find(_CLOSE, j + len(_OPEN))
        ph = data[j : k + 1]
        if k < 0 or ph not in names:
            raise _Unresolved(ph.decode("utf-8", "replace"))
        out += [data[i:j], names[ph]]
        i = k + 1
    out.append(data[i:])
    return b"".join(out)


def _fragments(prep: Prepared) -> list[bytes]:
    """The random parts of this run's temporary paths (``lx-reg-k3j4h5``): stored bytes must
    not hold them."""
    root = temp_root()
    found = set()
    homes = [prep.home] if prep.home is not None else []
    for path in [p for _, p in prep.places] + [i.path for i in prep.inputs] + homes:
        try:
            rel = Path(os.path.abspath(path)).resolve().relative_to(root)
        except ValueError:
            continue
        if rel.parts:
            found.add(rel.parts[0])
    return sorted(f.encode() for f in found if len(f) >= 6)


def _holds(path: Path, fragments: list[bytes]) -> bytes | None:
    if not fragments:
        return None
    data = path.read_bytes()
    if path.name.endswith(".gz"):
        with contextlib.suppress(OSError, EOFError), gzip.open(path, "rb") as f:
            data = data + f.read(1 << 20)
    return next((f for f in fragments if f in data), None)


# ------------------------------------------------------------------------------------------------
# Entries


#: The events of an entry's history that are a run giving its stored result again.
AGREED = ("same", "confirmed", "refreshed", "verified")


def identical_runs(entry: dict) -> int:
    """How many runs gave the entry's result: the one that stored it, and every one since
    that agreed (a case that runs a command again is served it only from
    :data:`~larmorx_validation.oracle_cache.policy.CONFIRMATIONS` on)."""
    return 1 + sum(h.get("event") in AGREED for h in entry.get("history", []))


def _servable(store: Store, entry: dict) -> bool:
    if entry.get("disputed") or entry.get("confirmations", 0) < entry.get("required", 1):
        return False
    r = entry["result"]
    shas = [r["stdout"], r["stderr"], *(o["sha256"] for o in r["outputs"])]
    return all(store.has_blob(s) for s in shas)


def _replay(store: Store, entry: dict, prep: Prepared, argv: Sequence[str]) -> Completed:
    """The stored run, into this run's places (``_Unresolved`` before anything is written if
    the entry names a place or input this run lacks)."""
    r = entry["result"]
    out = _from_placeholders(store.get_bytes(r["stdout"]), prep)
    err = _from_placeholders(store.get_bytes(r["stderr"]), prep)
    try:
        deleted = [_resolve_place(prep, d["place"], d["path"]) for d in r.get("deleted", [])]
        targets = [(_resolve_place(prep, o["place"], o["path"]), o) for o in r["outputs"]]
    except KeyError as e:
        raise _Unresolved(str(e)) from None
    for path in deleted:
        with contextlib.suppress(OSError):
            path.unlink()
    for target, o in targets:
        store.copy_blob(o["sha256"], target)
    return Completed([str(a) for a in argv], r["returncode"], out, err, True, 0.0)


def _capture(
    store: Store, prep: Prepared, done: Completed, before: dict[Path, tuple], policy: Policy
) -> tuple[dict, list[tuple[dict, Path]]]:
    """The run's result for an entry (its blobs stored), and its output files."""
    written, deleted = _changes(prep, before)
    fragments = _fragments(prep)
    out = _to_placeholders(done.stdout_bytes, prep)
    err = _to_placeholders(done.stderr_bytes, prep)
    for stream, data in (("standard output", out), ("standard error", err)):
        if (f := next((f for f in fragments if f in data), None)) is not None:
            raise Uncacheable(
                f"its {stream} names a temporary path ({f.decode()}) in a form the cache cannot map"
            )
    outputs = []
    for path in written:
        place, rest = _place_of(prep, path)
        nifti = path.name.endswith((".nii", ".nii.gz"))
        if not (policy.nifti_extensions and nifti) and (f := _holds(path, fragments)) is not None:
            raise Uncacheable(f"its output {path.name} holds a temporary path ({f.decode()})")
        sha = store.put_file(path)
        outputs.append(
            ({"place": place, "path": rest, "sha256": sha, "size": path.stat().st_size}, path)
        )
    result = {
        "returncode": done.returncode,
        "stdout": store.put_bytes(out),
        "stderr": store.put_bytes(err),
        "outputs": [o for o, _ in outputs],
        "deleted": [dict(zip(("place", "path"), _place_of(prep, p), strict=True)) for p in deleted],
    }
    return result, outputs


def differences(
    store: Store,
    stored: dict,
    fresh: dict,
    fresh_files: dict[tuple[str, str], Path],
    volatile: Sequence[str] = (),
    nifti_extensions: bool = True,
) -> list[str]:
    """How a fresh run's result (its files at ``fresh_files``) differs from a stored one."""
    out = []
    if stored["returncode"] != fresh["returncode"]:
        out.append(f"exit status {stored['returncode']} stored, {fresh['returncode']} now")
    for stream in ("stdout", "stderr"):
        if stored[stream] == fresh[stream]:
            continue
        a = compare.mask(store.get_bytes(stored[stream]), volatile)
        b = compare.mask(store.get_bytes(fresh[stream]), volatile)
        if a != b:
            out.append(f"{stream}: {compare.first_line_difference(a, b)}")
    sa = {(o["place"], o["path"]): o for o in stored["outputs"]}
    sb = {(o["place"], o["path"]): o for o in fresh["outputs"]}
    if sorted(sa) != sorted(sb):
        out.append(f"files written: {sorted(sa)} stored, {sorted(sb)} now")
    for k in sorted(set(sa) & set(sb)):
        if sa[k]["sha256"] == sb[k]["sha256"]:
            continue
        same, why = compare.same_file(
            store.blob_path(sa[k]["sha256"]), fresh_files[k], nifti_extensions=nifti_extensions
        )
        if not same:
            out.append(f"{k[1] or k[0]}: {why}")
    return out


def _new_entry(
    prep: Prepared,
    result: dict,
    done: Completed,
    required: int,
    runner: str,
    policy: Policy,
    display: list[str],
    launcher: dict,
) -> dict:
    from larmorx_validation.oracle_cache import provenance

    return {
        "key": prep.key,
        "parts": prep.parts,
        "result": result,
        "inputs": [
            {
                "sha256": i.sha256,
                "suffix": i.suffix,
                "companions": [[s, h] for s, h, _ in i.companions],
                "at": prep.role_of(i.path),
                "relative": i.relative,
            }
            for i in prep.inputs
        ],
        "places": [
            {"name": n, "kind": "file" if n in prep.files else "directory", "basename": p.name}
            for n, p in prep.places
        ],
        "rerun_env": prep.rerun_env,
        "template": prep.template,
        "watch": [[w, r] for w, r in prep.watch_roles()],
        "launcher": launcher,
        "command": provenance.display(display),
        "runner": runner,
        "case": _case.name,
        "policy": {
            "family": policy.family,
            "threads": policy.threads,
            "volatile": list(policy.volatile),
            "nifti_extensions": policy.nifti_extensions,
        },
        "required": required,
        "confirmations": 1,
        "seconds": round(done.seconds, 3),
        "date": now(),
        "larmorx_commit": provenance.commit(),
        "history": [],
    }


def _store_inputs(store: Store, prep: Prepared) -> None:
    """The inputs' bytes, so that ``oracle-cache verify`` can run the entry again."""
    for i in prep.inputs:
        store.put_file(i.path, i.sha256)
        for _s, h, c in i.companions:
            store.put_file(c, h)


def _settle(
    store: Store,
    prep: Prepared,
    done: Completed,
    before: dict[Path, tuple],
    decision,
    policy: Policy,
    runner: str,
    display: list[str],
    launcher: dict,
    refreshing: bool,
) -> str | None:
    """Store a fresh run, or compare it with the entry already stored for its key. Returns why
    the run was not stored, when that replaces the reason it ran."""
    if done.timed_out:
        return "timed out (never stored)"
    if done.returncode is not None and done.returncode < 0:
        try:
            name = signal.Signals(-done.returncode).name
        except ValueError:
            name = str(-done.returncode)
        return f"ended by a signal, {name} (never stored)"
    try:
        result, files = _capture(store, prep, done, before, policy)
    except Uncacheable as e:
        return f"not storable: {e}"
    fresh_files = {(o["place"], o["path"]): p for o, p in files}
    with store.lock(prep.key):
        entry = store.read(prep.key)
        if entry is None:
            store.write(
                _new_entry(prep, result, done, decision.required, runner, policy, display, launcher)
            )
            return None
        if entry.get("disputed") and not refreshing:
            return None  # known to differ: run, never served, not compared again
        diff = differences(
            store,
            entry["result"],
            result,
            fresh_files,
            policy.volatile,
            nifti_extensions=not policy.nifti_extensions,
        )
        event = {"date": now(), "seconds": round(done.seconds, 3), "case": _case.name}
        if diff:
            _dispute(store, prep, entry, result, diff, event, runner, refreshing)
            return None
        if entry.get("confirmations", 0) < entry.get("required", 1):
            entry["confirmations"] = entry.get("confirmations", 0) + 1
            entry["history"].append(event | {"event": "confirmed"})
        else:
            entry["history"].append(event | {"event": "refreshed" if refreshing else "same"})
        store.write(entry)
    return None


def _dispute(
    store: Store,
    prep: Prepared,
    entry: dict,
    result: dict,
    diff: list[str],
    event: dict,
    runner: str,
    refreshing: bool,
) -> None:
    """A fresh run differs from its stored entry: a finding. Both results are kept, the entry
    is never served again, and it is said loudly."""
    import sys

    record = {
        "event": "refresh" if refreshing else "fresh run",
        "case": _case.name,
        "runner": runner,
        "command": entry.get("command"),
        "differences": diff,
        "stored": entry["result"],
        "fresh": result,
    }
    where = store.dispute(prep.key, record)
    entry["disputed"] = {"date": now(), "differences": diff, "record": str(where)}
    entry["history"].append(event | {"event": "differs", "differences": diff})
    store.write(entry)
    _case.tally.differences.append(
        {
            "case": _case.name,
            "program": prep.parts["oracle"].get("program"),
            "differences": diff,
            "entry": str(store.entry_path(prep.key)),
            "dispute": str(where),
        }
    )
    print(
        f"\nORACLE CACHE: a run of {str(entry.get('command', '?'))[:200]} differs from its "
        f"stored entry ({'; '.join(diff)[:400]}); both kept: {where}",
        file=sys.stderr,
        flush=True,
    )


# ------------------------------------------------------------------------------------------------
# The entry points


def run(
    argv: Sequence[str | os.PathLike],
    *,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
    outputs: Sequence[Path] = (),
    policy: Policy | None = None,
    timeout: float | None = None,
    runner: str = "",
) -> Completed:
    """Run ``argv`` (an original program and its arguments), or replay it from the cache.

    ``env`` is the whole environment (default: this process's); ``cwd`` the working directory;
    ``outputs`` directories (or files) the program writes, beyond those its arguments name;
    ``policy`` whether the run may be cached (:mod:`~larmorx_validation.oracle_cache.policy`);
    ``runner`` the suite's name for the entry. The result is a :class:`Completed`."""
    argv = [str(a) for a in argv]
    env = dict(os.environ if env is None else env)
    policy = policy or Policy()
    program = Path(argv[0])
    if not program.is_absolute():
        found = shutil.which(argv[0], path=env.get("PATH"))
        program = Path(found) if found else program

    def oracle() -> dict:
        return program_identity(program, env)

    return _run(
        oracle,
        program.name,
        argv,
        argv[1:],
        env,
        cwd,
        outputs,
        policy,
        timeout,
        runner,
        {"kind": "exec", "program": str(program)},
        lambda: _execute(argv, env, cwd, timeout),
    )


def run_apptainer(
    image: Path,
    program: str,
    args: Sequence[str],
    inner_env: dict[str, str],
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    outputs: Sequence[Path] = (),
    policy: Policy | None = None,
    timeout: float | None = None,
    runner: str = "",
) -> Completed:
    """``program`` of the container ``image``, run by ``command`` (the whole ``apptainer exec``
    command line the suite builds), keyed by the image's SHA-256, the program, its arguments
    and the environment set inside (``inner_env``)."""
    args = [str(a) for a in args]
    command = [str(c) for c in command]
    env = dict(os.environ)
    policy = policy or Policy()

    def oracle() -> dict:
        return image_identity(image, program)

    return _run(
        oracle,
        program,
        command,
        args,
        dict(inner_env),
        cwd,
        outputs,
        policy,
        timeout,
        runner,
        {"kind": "apptainer", "image": str(image), "program": program},
        lambda: _execute(command, env, cwd, timeout),
    )


def _run(
    oracle: Callable[[], dict],
    program: str,
    argv: list[str],
    args: list[str],
    env: dict[str, str],
    cwd: Path | None,
    outputs: Sequence[Path],
    policy: Policy,
    timeout: float | None,
    runner: str,
    launcher: dict,
    execute: Callable[[], Completed],
) -> Completed:
    current = mode()
    if current == "off":
        _fresh("the cache is off")
        return execute()
    decision = decide(policy, program)
    if not decision.cache:
        _fresh(f"never cached: {decision.reason}")
        return execute()
    store = Store()
    try:
        prep = prepare(
            oracle(),
            args,
            env,
            cwd=cwd,
            outputs=outputs,
            threads_known=policy.threads is not None,
            family=policy.family,
        )
    except Uncacheable as e:
        _fresh(f"not keyable: {e}")
        return execute()
    except Exception as e:  # the cache never breaks a parity run: it runs, uncached
        _warn(f"cannot key a run of {program} ({type(e).__name__}: {e}); it runs uncached")
        _fresh("not keyable: an error in the cache")
        return execute()
    store.ensure()
    repeated = prep.key in _case.keys
    _case.keys.add(prep.key)
    entry = store.read(prep.key)
    unresolved = ""
    if (
        current == "use"
        and entry is not None
        and (not repeated or identical_runs(entry) >= CONFIRMATIONS)
        and _servable(store, entry)
    ):
        try:
            done = _replay(store, entry, prep, argv)
        except _Unresolved as e:
            unresolved = f"its entry names {e}, which this run lacks"
        else:
            _case.tally.served += 1
            _case.tally.saved_seconds += entry.get("seconds", 0.0)
            return done
    if unresolved:
        reason = unresolved
    elif repeated:
        reason = f"the case runs it again (served once {CONFIRMATIONS} runs agree)"
    elif current == "refresh":
        reason = "refresh"
    elif entry is None:
        reason = "not in the cache yet"
    elif entry.get("disputed"):
        reason = "its entry is disputed (a run differed)"
    elif entry.get("confirmations", 0) < entry.get("required", 1):
        reason = f"above one thread: not yet {entry['required']} identical runs"
    else:
        reason = "its entry's files are missing"
    try:
        _store_inputs(store, prep)  # as they are before the run (it may write them)
        before = _snapshot(prep)
    except Exception as e:
        _warn(f"cannot store the inputs of {program} ({type(e).__name__}: {e})")
        _fresh("not storable: an error in the cache")
        return execute()
    done = execute()
    try:
        why = _settle(
            store,
            prep,
            done,
            before,
            decision,
            policy,
            runner,
            argv,
            launcher,
            refreshing=current == "refresh",
        )
    except Exception as e:  # the result stands; only the cache missed it
        _warn(f"cannot store a run of {program} ({type(e).__name__}: {e})")
        why = "not storable: an error in the cache"
    _fresh(why or reason)
    return done


def _warn(text: str) -> None:
    import sys

    print(f"oracle cache: {text}", file=sys.stderr, flush=True)


__all__ = [
    "ENV_MODE",
    "MODES",
    "Completed",
    "Tally",
    "begin_case",
    "differences",
    "end_case",
    "mode",
    "run",
    "run_apptainer",
    "using",
]
