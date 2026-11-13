# SPDX-License-Identifier: Apache-2.0
"""The variables of FreeSurfer's programs in the oracle-output cache's key (2026-11-13): which
names its source reads, and the proof that keying them changed no entry's key.

Usage::

    python -P freesurfer_env.py names <FreeSurfer's source tree>
    python -P freesurfer_env.py entries [--cache <cache>] [--list <paths>] [--keys-out <file>]
    python -P freesurfer_env.py recordings [--runs <workspace>/oracles/freesurfer-8.2.0/runs]

- ``names``: every name a ``getenv("...")`` call of the tree's C and C++ files spells out; each
  must be in the key of the family ``freesurfer`` (``key.env_matters``) or one of
  :data:`LEFT_OUT`. Exit status 1 when one is neither (a newer FreeSurfer reads another).
- ``entries``: the cache's entries of the family (``policy.family == "freesurfer"``): that each
  entry's key is the SHA-256 of its stored parts (the key's form is unchanged), that no entry's
  environment holds a name keyed since 2026-11-13 (:data:`key.FREESURFER_READS`), and that every
  entry was stored by a runner of :data:`RUNNERS`, whose environments are closed (they set the
  variables they name and nothing of the session's). ``--keys-out`` writes the keys of the
  entries whose runner's cases may set one of the new names (the fs.core probe's), for
  ``oracle-cache verify --keys @<file>``: each must replay identically in an environment without
  them, which is the run its unchanged key now names. ``--list`` reads the entries' paths from a
  file (one per line, relative to the cache) instead of scanning the whole cache.
- ``recordings``: the replay tool's environment of every step of every recording
  (``replay.step_env``, at one thread and at the recording's): none sets a new name.

A run that sets none of the new names has the same key as before (``validation/tests/
test_oracle_cache.py``, ``test_freesurfer_reads_*``); with these three checks, no entry stored
before the change has another key now.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter
from pathlib import Path

from larmorx_validation.oracle_cache import key
from larmorx_validation.oracle_cache.store import Store, workspace

#: The names FreeSurfer reads that the key leaves out on purpose (key.FREESURFER_READS says why).
LEFT_OUT = frozenset(
    {
        "USER", "LOGNAME", "USERNAME", "HOST", "HOSTNAME", "HOME", "PWD", "TERM",
        "ftp_proxy", "FTP_PROXY", "ftp_proxy_password", "ftp_proxy_user", "http_proxy",
        "HTTP_PROXY", "no_proxy",
    }
)  # fmt: skip

#: The runners that store entries of the family, and where each one's environment comes from
#: (all closed: none passes the session's variables on).
RUNNERS = {
    "freesurfer-replay": "the recording's recon-all environment and the step's six variables "
    "(replay.step_env; checked by `recordings`)",
    "freesurfer-unit": "a fixed environment (freesurfer_unit.call: PATH, OMP_NUM_THREADS, the C "
    "locale, HOME, FREESURFER_HOME, FS_LICENSE)",
    "freesurfer-unit-proof": "the same (freesurfer-unit-oracle/prove.py)",
    "freesurfer-core-probe": "PATH, LC_ALL, OMP_NUM_THREADS and the case's own variables "
    "(freesurfer-core-probe/compare.py); some cases set DIAG*, PROFILE, IMAGE_SIZE, TMPDIR, ...",
    "fs-maths-probe": "PATH, LC_ALL, LANG, OMP_NUM_THREADS",
    "fs-io-probe": "PATH, LC_ALL, LANG, OMP_NUM_THREADS",
    "fs-mri-probe": "PATH, LC_ALL, LANG, OMP_NUM_THREADS",
    "fs-surfio-probe": "PATH, LC_ALL, LANG, OMP_NUM_THREADS",
    "freesurfer-cleanroom-random": "PATH, LC_ALL, OMP_NUM_THREADS (FREESURFER_HOME, FS_LICENSE)",
    "freesurfer-cleanroom-glibc-random": "PATH, LC_ALL, OMP_NUM_THREADS (FREESURFER_HOME, "
    "FS_LICENSE)",
    "freesurfer-cleanroom-spline": "PATH, LC_ALL",
    "freesurfer-cleanroom-glibc-qsort": "PATH, LC_ALL",
}
#: The runners whose cases may set a new name: their entries are run again (``--keys-out``).
CASES_SET_THEM = ("freesurfer-core-probe",)

_GETENV = re.compile(rb'getenv\(\s*"([A-Za-z_0-9]+)"\s*\)')
_FAMILY = re.compile(rb'"family":\s*"freesurfer"')
_SOURCES = (".c", ".cpp", ".cxx", ".cc", ".h", ".hpp")


def new_name(name: str) -> bool:
    """Whether ``name`` is keyed since 2026-11-13 (one of key.FREESURFER_READS)."""
    for n in key.FREESURFER_READS:
        if name == n or (n.endswith("*") and name.startswith(n[:-1])):
            return True
    return False


def cmd_names(a: argparse.Namespace) -> int:
    names: set[str] = set()
    files = 0
    for p in Path(a.source).rglob("*"):
        if p.suffix in _SOURCES and p.is_file():
            files += 1
            names.update(m.group(1).decode() for m in _GETENV.finditer(p.read_bytes()))
    keyed = sorted(n for n in names if key.env_matters(n, "freesurfer"))
    out = sorted(n for n in names if n in LEFT_OUT and n not in keyed)
    missing = sorted(set(names) - set(keyed) - set(out))
    print(f"{files} source files, {len(names)} names read: {len(keyed)} keyed, {len(out)} left out")
    for n in missing:
        print(f"NOT KEYED: {n}")
    return 1 if missing else 0


def _paths(store: Store, listing: str | None):
    if listing:
        for line in Path(listing).read_text(encoding="utf-8").splitlines():
            if line.strip():
                p = Path(line.strip())
                yield p if p.is_absolute() else store.root / p
        return
    yield from sorted((store.root / "entries").glob("*/*.json"))


def cmd_entries(a: argparse.Namespace) -> int:
    store = Store(Path(a.cache) if a.cache else None)
    runners: Counter[str] = Counter()
    problems: list[str] = []
    rerun_keys: list[str] = []
    names: dict[str, set[str]] = {}
    for path in _paths(store, a.list):
        data = path.read_bytes()
        if not _FAMILY.search(data):
            continue
        entry = json.loads(data)
        if entry.get("policy", {}).get("family") != "freesurfer":
            continue
        runner = entry.get("runner", "")
        runners[runner] += 1
        blob = json.dumps(entry["parts"], sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        if hashlib.sha256(blob.encode()).hexdigest() != entry["key"]:
            problems.append(f"{entry['key'][:16]}: its key is not the SHA-256 of its parts")
        env = entry["parts"].get("env", {})
        names.setdefault(runner, set()).update(env)
        if held := sorted(n for n in env if new_name(n)):
            problems.append(f"{entry['key'][:16]} ({runner}): its key holds {', '.join(held)}")
        if runner not in RUNNERS:
            problems.append(f"{entry['key'][:16]}: runner {runner!r} is not reviewed here")
        if runner in CASES_SET_THEM:
            rerun_keys.append(entry["key"])
    print(f"{sum(runners.values())} entries of the family freesurfer in {store.root}")
    for runner, n in sorted(runners.items(), key=lambda kv: -kv[1]):
        print(f"  {n:6d} {runner}: keyed variables {' '.join(sorted(names[runner]))}")
    if a.keys_out:
        Path(a.keys_out).write_text("".join(k + "\n" for k in sorted(rerun_keys)), encoding="utf-8")
        print(f"{len(rerun_keys)} keys of {', '.join(CASES_SET_THEM)} written to {a.keys_out}")
    for p in problems:
        print(f"PROBLEM: {p}")
    print(f"{len(problems)} problems")
    return 1 if problems else 0


def cmd_recordings(a: argparse.Namespace) -> int:
    from larmorx_validation.freesurfer.recording import Recording
    from larmorx_validation.freesurfer.replay import step_env

    ws = workspace()
    runs = (
        Path(a.runs) if a.runs else (ws / "oracles" / "freesurfer-8.2.0" / "runs" if ws else None)
    )
    if runs is None or not runs.is_dir():
        print(f"no recordings in {runs}", file=sys.stderr)
        return 2
    steps = found = 0
    recordings = 0
    for root in sorted(p for p in runs.iterdir() if (p / "record" / "steps").is_dir()):
        rec = Recording(root)
        recordings += 1
        for step in rec.steps:
            for threads in (1, None):
                env, _n = step_env(
                    rec, step, Path("/replay/subjects"), Path("/replay/cwd"), threads
                )
                if held := sorted(n for n in env if new_name(n)):
                    found += 1
                    print(f"{rec.name} {step.index:04d}-{step.program}: sets {', '.join(held)}")
            steps += 1
    print(f"{recordings} recordings, {steps} steps: {found} environments set a new name")
    return 1 if found else 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    n = sub.add_parser("names", help="the names FreeSurfer's source reads, against the key")
    n.add_argument("source")
    e = sub.add_parser("entries", help="the cache's entries of the family")
    e.add_argument("--cache")
    e.add_argument("--list", help="the entries' paths, one per line (relative to the cache)")
    e.add_argument("--keys-out", help="write the keys to run again here")
    r = sub.add_parser("recordings", help="the replay tool's environments of every step")
    r.add_argument("--runs")
    a = ap.parse_args(argv)
    return {"names": cmd_names, "entries": cmd_entries, "recordings": cmd_recordings}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
