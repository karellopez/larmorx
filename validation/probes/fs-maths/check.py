# SPDX-License-Identifier: Apache-2.0
"""fs.maths against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsmaths is
a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-core's example ``fs_maths_probe``, runs them
through ``fsmaths`` (``build.sh``: FreeSurfer's histograms, matrices, double matrices and affine
helpers from the unit oracle's build, linked as the official programs are) by the oracle-output
cache at one thread, runs the port on the same cases with ``--upstream-defects`` ``reproduce``
and ``fix``, and compares the lines: ``reproduce`` must give FreeSurfer's bits on every case, but
those where FreeSurfer reads outside its arrays (FS-22: the port notes it; such lines are
counted, not compared); ``fix`` may differ only where a defect it corrects changed the result
(its notices) or where the correctly rounded maths moved a last bit (``log`` in the entropy).
Prints the counts, the hash the port's recorded test checks (``tests/fs_maths_official.rs``),
and writes ``--json``.

    python -P validation/probes/fs-maths/check.py --seed 1 --n 470 [--json out.json]
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


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-maths" / "fsmaths"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-maths/build.sh first")
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
            "fs_maths_probe",
            "-p",
            "larmorx-freesurfer-core",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_maths_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=470)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-maths"
    inputs.mkdir(parents=True, exist_ok=True)
    cases_path = inputs / f"cases-{a.seed}-{a.n}.txt"
    text = subprocess.run(
        [str(port), "cases", str(a.seed), str(a.n)], check=True, capture_output=True, text=True
    ).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=3600,
        runner="fs-maths-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsmaths failed ({done.returncode}): {done.stderr[-2000:]}")
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
    equal, differ, skipped, skip_same = Counter(), Counter(), Counter(), 0
    fix_differ, fix_unexplained, fix_ids = Counter(), Counter(), Counter()
    hashed = hashlib.sha256()
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        if "FS-22" in ids:
            skipped[cmd] += 1
            skip_same += c == r
            hashed.update(b"SKIP\n")
            continue
        hashed.update(c.encode() + b"\n")
        if c == r:
            equal[cmd] += 1
        else:
            differ[cmd] += 1
            if len(first_differ) < 5:
                first_differ.append(case[:200])
        if f != c:
            fix_differ[cmd] += 1
            if not fids:
                fix_unexplained[cmd] += 1
            for x in fids:
                fix_ids[x] += 1
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "served": done.cached,
        "reproduce_equal": sum(equal.values()),
        "reproduce_differ": dict(differ),
        "fs22_skipped": dict(skipped),
        "fs22_skipped_official_equal_to_zero_fill": skip_same,
        "fix_differ": dict(fix_differ),
        "fix_differ_without_notice": dict(fix_unexplained),
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "first_differ": first_differ,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
