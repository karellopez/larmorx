# SPDX-License-Identifier: Apache-2.0
"""fs.mri against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsmri is
a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-mri's example ``fs_mri_probe``, runs them through
``fsmri`` (``build.sh``: FreeSurfer's MRI structure from the unit oracle's build, linked as the
official programs are) by the oracle-output cache at one thread, runs the port on the same cases
with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (the result and the
text FreeSurfer's code printed): ``reproduce`` must give FreeSurfer's on every case but those where
FreeSurfer reads or writes memory it does not own (FS-53, FS-163 where the port refuses the copy)
or crashes (FS-58, FS-165), which are counted, not compared; ``fix`` may differ only where a defect it corrects changed the result (its notices).
Prints the counts and the hash the port's recorded test checks (``tests/fs_mri_official.rs``).

    python -P validation/probes/fs-mri/check.py --seed 1 --n 400 [--json out.json]
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

# FS-198: FreeSurfer keeps referring to a matrix LTcopy freed (its line reads freed memory)
SKIP = ("FS-53", "FS-58", "FS-165", "FS-198")
# FS-163 (MRIcopy into a smaller destination) is skipped where the port refuses the copy (FreeSurfer
# writes outside the destination there); where both stay inside it the lines are compared
SKIP_WHEN_REFUSED = ("FS-163",)


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-mri" / "fsmri"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-mri/build.sh first")
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
            "fs_mri_probe",
            "-p",
            "larmorx-freesurfer-mri",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_mri_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-mri"
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
        runner="fs-mri-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsmri failed ({done.returncode}): {done.stderr[-2000:]}")
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
    equal, differ, skipped = Counter(), Counter(), Counter()
    fix_differ, fix_unexplained, fix_ids, rep_ids = Counter(), Counter(), Counter(), Counter()
    hashed = hashlib.sha256()
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        for x in ids:
            rep_ids[x] += 1
        refused = r.startswith("EXIT ")
        if any(x in ids for x in SKIP) or (refused and any(x in ids for x in SKIP_WHEN_REFUSED)):
            skipped[cmd] += 1
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
        "skipped": dict(skipped),
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
