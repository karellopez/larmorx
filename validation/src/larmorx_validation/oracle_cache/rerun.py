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


def rerun(store: Store, entry: dict, *, timeout: float | None = 4 * 3600) -> Outcome:
    """Run ``entry`` again and compare its result with the stored one (a difference is kept as
    a dispute and logged as a finding)."""
    key = entry["key"]
    launcher = entry.get("launcher", {})
    oracle = entry["parts"]["oracle"]
    home = entry["parts"].get("env", {}).get("HOME")
    if home is not None and home != f"<directory {hashlib.sha256(b'').hexdigest()}>":
        return Outcome(key, "skipped", "its HOME held files (only an empty one is made again)")
    with tempfile.TemporaryDirectory(prefix="lx-oracle-rerun-") as t:
        root = Path(t)
        prep, values = _layout(store, entry, root)
        args = [_fill(a, values) for a in entry["template"]]
        family = entry.get("policy", {}).get("family")
        env = {k: v for k, v in os.environ.items() if not env_matters(k, family, os.environ)}
        for k, v in entry.get("rerun_env", {}).items():
            env[k] = _fill(v, values)
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
            inner = {k: _fill(v, values) for k, v in entry.get("rerun_env", {}).items()}
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


__all__ = ["Outcome", "rerun", "rerun_many"]
