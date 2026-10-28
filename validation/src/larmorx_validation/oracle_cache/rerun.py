# SPDX-License-Identifier: Apache-2.0
"""Running a cached entry again, away from the suite that stored it: the drift detector
(``oracle-cache verify``) and the confirmations of runs above one thread (``oracle-cache
confirm``).

The entry holds what that needs: the arguments with placeholders, the inputs' bytes (blobs),
the places (directories, or a declared output file), the variables that matter, and how the
program was launched (directly, or in a container). Each place and input gets a fresh directory;
the program must still be the one the key names (its SHA-256 and its libraries', or the image's),
else the entry is skipped: a changed oracle has a new key, so its old entries are not drift.
"""

from __future__ import annotations

import hashlib
import os
import re
import shlex
import tempfile
from dataclasses import dataclass
from pathlib import Path

from larmorx_validation.oracle_cache import core
from larmorx_validation.oracle_cache.key import (
    Input,
    Prepared,
    env_matters,
    image_identity,
    program_identity,
)
from larmorx_validation.oracle_cache.policy import Policy
from larmorx_validation.oracle_cache.store import Store, now

_PLACEHOLDER = re.compile(r"\{(in\d+|cwd|out\d+p?|d\d+|home)\}")


@dataclass
class Outcome:
    key: str
    status: str  # "same", "differs", "skipped"
    detail: str = ""
    seconds: float = 0.0


def _layout(store: Store, entry: dict, root: Path) -> tuple[Prepared, dict[str, str]]:
    """Fresh places and inputs under ``root`` for ``entry``: the run's ``Prepared`` and the
    placeholders' values."""
    values: dict[str, str] = {}
    places: list[tuple[str, Path]] = []
    files: set[str] = set()
    kinds = {p["name"]: p for p in entry["places"]}
    for p in entry["places"]:
        if p["kind"] == "directory":
            d = root / p["name"]
            if any(c.isspace() for c in p.get("basename", "")):
                # a folder with a blank in its name, made so again (the program may read it)
                d = d / p["basename"]
            d.mkdir(parents=True, exist_ok=True)
            places.append((p["name"], d))
            values[p["name"]] = str(d)
    for p in entry["places"]:
        if p["kind"] == "file":
            parent = kinds.get(p["name"] + "p")
            base = Path(values[parent["name"]]) if parent else root / (p["name"] + "-dir")
            base.mkdir(parents=True, exist_ok=True)
            places.append((p["name"], base / p["basename"]))
            files.add(p["name"])
            values[p["name"]] = str(base / p["basename"])
    for d in entry["parts"].get("directories", []):  # the directories inside the places
        Path(_fill(d, values)).mkdir(parents=True, exist_ok=True)
    inputs = []
    for i, item in enumerate(entry["inputs"]):
        # where it was: inside its place, with its name (a program may write beside it, or
        # name it relative to its working directory); else a directory of its own
        if item.get("at"):
            path = Path(_fill(item["at"], values))
        else:
            path = root / f"in{i}" / f"input{item['suffix']}"
        path.parent.mkdir(parents=True, exist_ok=True)
        store.copy_blob(item["sha256"], path)
        stem = path.name[: len(path.name) - len(item["suffix"])] if item["suffix"] else path.name
        comps = []
        for suf, sha in item.get("companions", []):
            c = path.with_name(stem + suf)
            store.copy_blob(sha, c)
            comps.append((suf, sha, c))
        inputs.append(Input(path, item["sha256"], item["suffix"], tuple(comps), ""))
        values[f"in{i}"] = item.get("relative") or str(path)
        if written := item.get("written"):  # a path as written (a/sub/../x): its directories
            parts = _fill(written, values).split("/")
            for k in range(1, len(parts)):
                prefix = "/".join(parts[:k])
                if prefix and ".." not in parts[:k] and "." not in parts[:k]:
                    Path(prefix).mkdir(parents=True, exist_ok=True)
            values[f"in{i}"] = _fill(written, values)
    home = root / "home"
    home.mkdir()
    values["home"] = str(home)
    watched = {}
    for i, (where, recursive) in enumerate(entry.get("watch", [])):
        path = Path(_fill(where, values))
        if path.is_dir() and (path == root or path.is_relative_to(root)):
            watched[f"w{i}"] = (path, recursive)
    prep = Prepared(entry["parts"], places, inputs, [], watched, files, {}, home)
    return prep, values


def _fill(text: str, values: dict[str, str]) -> str:
    return _PLACEHOLDER.sub(lambda m: values[m.group(1)], text)


#: A place's path followed by a blank and more of the path, in an entry's arguments: a path with
#: a blank the key took apart before 2026-10-28.
_SPLIT_BLANK = re.compile(r"\{(?:cwd|out\d+p?|d\d+)\}/[^\s,\[\];=:@'\"(){}<>|]*\s+[^\s,\[\]]")
#: A printf pattern of a number in a place, in an entry's arguments (``{d0}/prior%02d.nii.gz``).
_PATTERN = re.compile(r"\{(cwd|out\d+p?|d\d+)\}/([^\s,\[\];]*%0?\d*d[^\s,\[\];]*)")


def _read_patterns(entry: dict) -> list[str]:
    """The printf patterns in ``entry``'s arguments that name files it read: those none of its
    stored outputs match (``post%02d.nii.gz`` names what Atropos writes, ``prior%02d.nii.gz``
    what it reads)."""
    written = [(o["place"], o["path"]) for o in entry.get("result", {}).get("outputs", [])]
    out = []
    for a in entry.get("template", []):
        for m in _PATTERN.finditer(a):
            place, rel = m.group(1), m.group(2)
            p = re.search(r"%0?\d*d", rel)
            assert p is not None
            name = re.compile(re.escape(rel[: p.start()]) + r" *\d+" + re.escape(rel[p.end() :]))
            if not any(w == place and name.fullmatch(path) for w, path in written):
                out.append(m.group(0))
    return out


#: Entries superseded by a correction of their suite's key that the entry itself cannot show
#: (full key -> why): their suite keys the run differently now, so they are never reached.
SUPERSEDED: dict[str, str] = {
    # fsl-mcflirt-replica/edge/mat-exists (machine-move-proof.md)
    "51a04ae5838e3f0c3ea0ec835491fa7245e9da6a8dc48ae5fc4809f8ee9004d7": (
        "keyed before the files in mcflirt's working directory were inputs (2026-10-28): the "
        "file out.mat, which sends the matrices to out.mat+, is not among the entry's inputs"
    ),
}


def stale(entry: dict) -> str:
    """Why ``entry`` was keyed by rules the cache has since corrected, so that it cannot be run
    again as its suite would run it now (``""``: it can). Its suite's next run keys the run
    anew and stores a new entry; this one stays (nothing is deleted) and is never reached."""
    if why := SUPERSEDED.get(entry.get("key", "")):
        return why
    if entry.get("rules", 1) >= core.RULES:
        return ""  # stored by the current rules
    args = entry.get("template", [])
    parts = entry.get("parts", {})
    if _read_patterns(entry) and not any("series" in i for i in parts.get("inputs", [])):
        return (
            "keyed before files named by a pattern (prior%02d.nii.gz) were inputs "
            "(2026-10-28): the entry holds none of them"
        )
    if any(_SPLIT_BLANK.search(a) for a in args) and "text" not in parts:
        return (
            "keyed before a path with a blank was one path, its blank in the key (2026-10-28): "
            "an input so named is not among the entry's inputs"
        )
    return ""


def never_now(entry: dict, args: list[str], env: dict[str, str]) -> str:
    """Why the run of ``entry`` (its arguments ``args`` as this rerun fills them) is never cached
    under the current rules (``""``: it may be): ANTs' clock and memory rules, which the cache
    applies to every run of an ITK program (``policy.ants_never``)."""
    from larmorx_validation.oracle_cache.policy import ants_never

    if entry.get("policy", {}).get("family") != "itk":
        return ""
    program = str(entry["parts"]["oracle"].get("program", ""))
    return ants_never(Path(program).name, args, env)


def _container_env(entry: dict) -> dict[str, str]:
    """The variables the suite set inside the container (``--env``), as placeholders: from the
    launcher (entries since 2026-10-28), else from the command as it ran (``--env K=V``: ``HOME``
    as ``{home}``, a variable of the key as the key's, the rest as written)."""
    launcher = entry.get("launcher", {})
    if "env" in launcher:
        return dict(launcher["env"])
    keyed = entry.get("rerun_env", {})
    out = dict(keyed)
    try:
        words = shlex.split(entry.get("command", ""))
    except ValueError:
        return out
    home = str(Path.home())
    for k, w in enumerate(words[:-1]):
        if w == "--env" and "=" in words[k + 1]:
            name, value = words[k + 1].split("=", 1)
            if name == "HOME":
                out[name] = "{home}"
            elif name not in keyed:
                out[name] = home + value[1:] if value.startswith("~") else value
    return out


def rerun(store: Store, entry: dict, *, timeout: float | None = 4 * 3600) -> Outcome:
    """Run ``entry`` again and compare its result with the stored one (a difference is kept as
    a dispute and logged as a finding)."""
    key = entry["key"]
    launcher = entry.get("launcher", {})
    oracle = entry["parts"]["oracle"]
    home = entry["parts"].get("env", {}).get("HOME")
    if home is not None and home != f"<directory {hashlib.sha256(b'').hexdigest()}>":
        return Outcome(key, "skipped", "its HOME held files (only an empty one is made again)")
    if why := stale(entry):
        return Outcome(key, "skipped", why)
    with tempfile.TemporaryDirectory(prefix="lx-oracle-rerun_") as t:
        root = Path(t)
        prep, values = _layout(store, entry, root)
        args = [_fill(a, values) for a in entry["template"]]
        family = entry.get("policy", {}).get("family")
        env = {k: v for k, v in os.environ.items() if not env_matters(k, family, os.environ)}
        for k, v in entry.get("rerun_env", {}).items():
            env[k] = _fill(v, values)
        if why := never_now(entry, args, env):
            return Outcome(key, "skipped", f"never cached under the current rules: {why}")
        cwd = Path(values["cwd"]) if entry["parts"].get("cwd") else None
        if launcher.get("kind") == "exec":
            program = Path(launcher["program"])
            if not program.is_file():
                return Outcome(key, "skipped", f"{program} is not here any more")
            if program_identity(program, env) != oracle:
                return Outcome(key, "skipped", "the program or its libraries changed (a new key)")
            argv = [str(program), *args]
            run_env = env
        elif launcher.get("kind") == "apptainer":
            image = Path(launcher["image"])
            if not image.is_file() or image_identity(image, launcher["program"]) != oracle:
                return Outcome(key, "skipped", "the container image changed or is not here")
            inner = {k: _fill(v, values) for k, v in _container_env(entry).items()}
            argv = _apptainer(image, launcher["program"], args, inner, cwd, root)
            run_env = dict(os.environ)
        else:
            return Outcome(key, "skipped", f"unknown launcher {launcher!r}")
        for name in ("LD_PRELOAD",):
            if name in env and not all(Path(p).is_file() for p in env[name].split()):
                return Outcome(key, "skipped", f"{name} names a file that is not here")
        before = core._snapshot(prep)
        done = core._execute(argv, run_env, cwd, timeout)
        if done.timed_out:
            return Outcome(key, "differs", "timed out", done.seconds)
        policy = Policy(
            entry.get("policy", {}).get("threads", 1),
            volatile=tuple(entry.get("policy", {}).get("volatile", ())),
            nifti_extensions=entry.get("policy", {}).get("nifti_extensions", ""),
            family=family,
        )
        try:
            result, files = core._capture(store, prep, done, before, policy)
        except core.Uncacheable as e:
            return Outcome(key, "differs", f"its result cannot be compared: {e}", done.seconds)
        fresh_files = {(o["place"], o["path"]): p for o, p in files}
        diff = core.differences(
            store,
            entry["result"],
            result,
            fresh_files,
            policy.volatile,
            nifti_extensions=not policy.nifti_extensions,
        )
        with store.lock(key):
            current = store.read(key) or entry
            event = {"date": now(), "seconds": round(done.seconds, 3), "event": "verified"}
            if diff:
                where = store.dispute(
                    key,
                    {
                        "event": "verify",
                        "command": entry.get("command"),
                        "differences": diff,
                        "stored": entry["result"],
                        "fresh": result,
                    },
                )
                current["disputed"] = {"date": now(), "differences": diff, "record": str(where)}
                current.setdefault("history", []).append(event | {"differences": diff})
                store.write(current)
                return Outcome(key, "differs", "; ".join(diff), done.seconds)
            if current.get("confirmations", 0) < current.get("required", 1):
                current["confirmations"] = current.get("confirmations", 0) + 1
                event["event"] = "confirmed"
            current.setdefault("history", []).append(event)
            store.write(current)
        return Outcome(key, "same", "", done.seconds)


def _apptainer(
    image: Path, program: str, args: list[str], inner: dict[str, str], cwd: Path | None, root: Path
) -> list[str]:
    """``apptainer exec`` of ``program`` with everything under ``root`` bound at its own path."""
    cmd = ["apptainer", "exec", "--cleanenv", "--no-home"]
    if cwd is not None:
        cmd += ["--pwd", str(cwd)]
    cmd += ["--bind", f"{root}:{root}"]
    for k, v in sorted(inner.items()):
        cmd += ["--env", f"{k}={v}"]
    return [*cmd, str(image), program, *args]


def _worker(root: str, key: str, timeout: float | None) -> Outcome:
    store = Store(Path(root))
    entry = store.read(key)
    if entry is None:
        return Outcome(key, "skipped", "the entry is gone")
    try:
        return rerun(store, entry, timeout=timeout)
    except Exception as e:  # report, never stop the others
        return Outcome(key, "skipped", f"{type(e).__name__}: {e}")


def rerun_many(
    store: Store, keys: list[str], *, jobs: int = 1, timeout: float | None = None, progress=None
) -> list[Outcome]:
    if jobs <= 1:
        out = []
        for k in keys:
            out.append(_worker(str(store.root), k, timeout))
            if progress:
                progress(out[-1])
        return out
    import multiprocessing
    from concurrent.futures import ProcessPoolExecutor

    with ProcessPoolExecutor(jobs, mp_context=multiprocessing.get_context("spawn")) as pool:
        futures = [pool.submit(_worker, str(store.root), k, timeout) for k in keys]
        out = []
        for f in futures:
            out.append(f.result())
            if progress:
                progress(out[-1])
        return out


__all__ = ["Outcome", "never_now", "rerun", "rerun_many", "stale"]
