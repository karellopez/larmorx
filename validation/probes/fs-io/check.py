# SPDX-License-Identifier: Apache-2.0
"""fs.io against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsio is a
black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-io's example ``fs_io_probe``, runs them through
``fsio`` (``build.sh``: FreeSurfer's fio.cpp, bfileio.cpp and MRIio_old.cpp from the unit
oracle's build, linked as the official programs are) by the oracle-output cache at one thread,
runs the port on the same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares
the lines (the result and the text FreeSurfer's code printed). A ``?`` in the port's line is a
hexadecimal digit of a byte FreeSurfer takes from memory contents (FS-49, FS-167): it matches any
digit. ``reproduce`` must give FreeSurfer's line on every case but those the port marks ``SKIP``
(FreeSurfer crashes or reads for ever, or the port refuses what it does: FS-169, FS-172, FS-173,
FS-175), which are counted, not compared, and those it marks ``VARY`` (FreeSurfer's result
depends on memory it freed or never wrote: FS-168, FS-170), which are compared and counted apart;
``fix`` may differ only where a defect it corrects changed the result (its notices).
Prints the counts and the hash the port's recorded test checks (``tests/fs_io_official.rs``: the
``portable`` cases, which give the same results on every platform).

    python -P validation/probes/fs-io/check.py --seed 1 --n 1100 [--portable] [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

HEX = set("0123456789abcdef")


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-io" / "fsio"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-io/build.sh first")
    return exe


def port_binary(jobs: int) -> Path:
    manifest = REPO / "crates-freesurfer" / "Cargo.toml"
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--example",
            "fs_io_probe",
            "-p",
            "larmorx-freesurfer-io",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_io_probe"


def matches(official: str, port: str) -> bool:
    """Equal, a ``?`` of the port's matching any hexadecimal digit."""
    if len(official) != len(port):
        return False
    return all(o == p or (p == "?" and o in HEX) for o, p in zip(official, port, strict=True))


def masked(official: str, port: str) -> str:
    """The official line with the port's ``?`` digits in place (for the hash)."""
    if "?" not in port or len(official) != len(port):
        return official
    return "".join("?" if p == "?" and o in HEX else o for o, p in zip(official, port, strict=True))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=1100)
    ap.add_argument("--portable", action="store_true")
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-io"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "portable" if a.portable else "all"
    cases_path = inputs / f"cases-{tag}-{a.seed}-{a.n}.txt"
    gen = [str(port), "cases", str(a.seed), str(a.n)] + (["portable"] if a.portable else [])
    text = subprocess.run(gen, check=True, capture_output=True, text=True).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=7200,
        runner="fs-io-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsio failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()

    def port_lines(mode: str) -> list[tuple[str, list[str]]]:
        out = subprocess.run(
            [str(port), "run", str(cases_path), mode], check=True, capture_output=True, text=True
        ).stdout
        rows = []
        for line in out.splitlines():
            body, _, ids = line.partition("\t")
            rows.append((body, [x for x in ids.split(",") if x]))
        return rows

    rep, fix = port_lines("reproduce"), port_lines("fix")
    assert len(official) == len(rep) == len(fix) == len(cases)
    equal, differ, skipped, vary_equal, vary_differ = (
        Counter(),
        Counter(),
        Counter(),
        Counter(),
        Counter(),
    )
    fix_differ, fix_unexplained, fix_ids, rep_ids = Counter(), Counter(), Counter(), Counter()
    masked_lines = 0
    hashed = hashlib.sha256()
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        for x in ids:
            rep_ids[x] += 1
        if "SKIP" in ids:
            skipped[cmd] += 1
            hashed.update(b"SKIP\n")
            continue
        if "VARY" in ids:
            (vary_equal if matches(c, r) else vary_differ)[cmd] += 1
            hashed.update(b"SKIP\n")
            continue
        if "?" in r:
            masked_lines += 1
        hashed.update(masked(c, r).encode() + b"\n")
        if matches(c, r):
            equal[cmd] += 1
        else:
            differ[cmd] += 1
            if len(first_differ) < 5:
                first_differ.append({"case": case[:200], "official": c[:300], "port": r[:300]})
        if not matches(c, f):
            fix_differ[cmd] += 1
            real = [x for x in fids if x not in ("SKIP", "VARY")]
            if not real:
                fix_unexplained[cmd] += 1
            for x in real:
                fix_ids[x] += 1
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "portable": a.portable,
        "served": done.cached,
        "reproduce_equal": sum(equal.values()),
        "reproduce_equal_with_memory_digits": masked_lines,
        "reproduce_differ": dict(differ),
        "skipped": dict(skipped),
        "vary_equal": dict(vary_equal),
        "vary_differ": dict(vary_differ),
        "reproduce_notices": dict(rep_ids),
        "fix_differ": dict(fix_differ),
        "fix_differ_without_notice": dict(fix_unexplained),
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "first_differ": first_differ,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
