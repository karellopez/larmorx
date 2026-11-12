#!/usr/bin/python3 -I
# SPDX-License-Identifier: Apache-2.0
"""A stand-in for FreeSurfer's `fs_time` that records every step recon-all runs.

recon-all (8.2.0) runs each program as `$fs_time <program> <args>`. Installed as
`$FREESURFER_HOME/bin/fs_time` (FreeSurfer's own moved to `fs_time.orig`), this script:

- without `LARMORX_FS_RECORD` set, or when nested inside a recorded step, runs `fs_time.orig`
  with the same arguments and nothing else (the install behaves as shipped);
- with the single argument `--larmorx-final` (the run script, after recon-all ends), records only
  what changed since the last step;
- otherwise, around each call, compares the subject directory (`LARMORX_FS_SUBJDIR`) with its
  state after the previous step: what changed in between (commands recon-all runs without
  `fs_time`) is recorded as that step's `between`; then it runs `fs_time.orig`, and records what
  the step wrote (`outputs`) and deleted, with the directory's state before it (`before`), its
  command line, working directory, thread settings, times and exit status.

Every file's content is kept once, by SHA-256, in `<record>/blobs/` (a file a later step
overwrites keeps its earlier version), so the subject directory before any step can be rebuilt
from `steps/NNNN.json`'s `before` map. recon-all.log and the like are left out (they change at
every step). Standard library only; no output of its own on stdout or stderr.
"""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

SKIP_DIRS = {"tmp"}
SKIP_FILES = {
    "scripts/recon-all.log",
    "scripts/recon-all-status.log",
    "scripts/recon-all.cmd",
    "scripts/recon-all.done",
    "scripts/recon-all.error",
    "scripts/recon-all.local-copy",
    "scripts/build-stamp.txt",
}
ENV_KEYS = (
    "OMP_NUM_THREADS",
    "FS_OMP_NUM_THREADS",
    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS",
    "FS_LICENSE",
    "FREESURFER_HOME",
    "SUBJECTS_DIR",
)


def real_fs_time() -> str:
    return os.path.join(os.path.dirname(os.path.realpath(__file__)), "fs_time.orig")


def scan(root: str) -> dict:
    """relative path -> [size, mtime_ns] of every regular file (links followed as files)."""
    out = {}
    for d, dirs, files in os.walk(root):
        rel_d = os.path.relpath(d, root)
        dirs[:] = [x for x in dirs if not (rel_d == "." and x in SKIP_DIRS)]
        for f in files:
            p = os.path.join(d, f)
            rel = os.path.normpath(os.path.join(rel_d, f))
            if rel in SKIP_FILES or rel.startswith("scripts/IsRunning"):
                continue
            try:
                st = os.stat(p)
            except OSError:
                continue
            out[rel] = [st.st_size, st.st_mtime_ns]
    return out


def store(root: str, rel: str, blobs: str) -> str | None:
    p = os.path.join(root, rel)
    h = hashlib.sha256()
    try:
        with open(p, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
    except OSError:
        return None
    digest = h.hexdigest()
    dst = os.path.join(blobs, digest[:2], digest)
    if not os.path.exists(dst):
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        tmp = dst + ".part"
        shutil.copyfile(p, tmp)
        os.replace(tmp, dst)
    return digest


def changes(old: dict, new: dict, root: str, blobs: str):
    """(written: rel -> sha, deleted: [rel], new state with hashes)."""
    written, state = {}, {}
    for rel, (size, mtime) in new.items():
        prev = old.get(rel)
        if prev and prev[0] == size and prev[1] == mtime:
            state[rel] = prev
            continue
        digest = store(root, rel, blobs)
        if digest is None:
            continue
        state[rel] = [size, mtime, digest]
        if not prev or prev[2] != digest:
            written[rel] = digest
    deleted = sorted(set(old) - set(new))
    return written, deleted, state


def main() -> int:
    args = sys.argv[1:]
    record = os.environ.get("LARMORX_FS_RECORD")
    subj = os.environ.get("LARMORX_FS_SUBJDIR")
    if not record or not subj or os.environ.get("LARMORX_FS_NESTED") or args[:1] == ["ls"]:
        os.execv(real_fs_time(), [real_fs_time(), *args])
    blobs = os.path.join(record, "blobs")
    steps = os.path.join(record, "steps")
    os.makedirs(steps, exist_ok=True)
    state_file = os.path.join(record, "state.json")
    try:
        with open(state_file) as fh:
            last = json.load(fh)
    except FileNotFoundError:
        last = {"index": 0, "files": {}}
    os.makedirs(subj, exist_ok=True)
    between, between_deleted, before = changes(last["files"], scan(subj), subj, blobs)
    index = last["index"] + 1
    env = dict(os.environ, LARMORX_FS_NESTED="1")
    start = time.time()
    final = args == ["--larmorx-final"]
    status = 0 if final else subprocess.call([real_fs_time(), *args], env=env)
    end = time.time()
    written, deleted, after = changes(before, scan(subj), subj, blobs)
    entry = {
        "index": index,
        "argv": args,
        "cwd": os.getcwd(),
        "env": {k: os.environ[k] for k in ENV_KEYS if k in os.environ},
        "start": start,
        "end": end,
        "seconds": round(end - start, 3),
        "exit": status,
        "between": between,
        "between_deleted": between_deleted,
        "before": {rel: v[2] for rel, v in before.items()},
        "outputs": written,
        "deleted": deleted,
    }
    name = "final" if final else (os.path.basename(args[0]) if args else "none")
    with open(os.path.join(steps, f"{index:04d}-{name}.json"), "w") as fh:
        json.dump(entry, fh, indent=1, sort_keys=True)
    tmp = state_file + ".part"
    with open(tmp, "w") as fh:
        json.dump({"index": index, "files": after}, fh)
    os.replace(tmp, state_file)
    return status


if __name__ == "__main__":
    sys.exit(main())
