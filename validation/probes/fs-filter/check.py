# SPDX-License-Identifier: Apache-2.0
"""fs.filter against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsfilter
is a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-filter's example ``fs_filter_probe``, runs them
through ``fsfilter`` (``build.sh``: FreeSurfer's filter functions from the unit oracle's build,
linked as the official programs are) by the oracle-output cache at one thread, runs the port on the
same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (the
result and the text FreeSurfer's code printed): ``reproduce`` must give FreeSurfer's on every case
but those where FreeSurfer reads or writes memory it does not own (``SKIP``), which are counted,
not compared (how many of them happen to agree is printed too); ``fix`` may differ only where a
defect it corrects changed the result (its notices). Prints the counts and the hash the port's
recorded test checks (``tests/fs_filter_official.rs``).

    python -P validation/probes/fs-filter/check.py --seed 1 --n 400 [--json out.json]
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

# memory FreeSurfer does not own: FS-540 (a neighbour read outside the volume's memory: memory
# contents or a crash), FS-541 (a masked voxel set from the limit before it is set, read through
# FS-539), FS-58 (a volume without voxels), FS-53 (a mask smaller than the volume read past its
# memory). FS-539's reads of the next row are compared: they are
# deterministic, and reproduce follows them
# FS-546 (a convolution's access outside the volumes' memory), FS-548 (a kernel volume read as
# floats past its memory), FS-549 (a kernel longer than the index tables), FS-550 (a crash), FS-551
# (a matrix written past its end), FS-552 (a crash), FS-555 (a Sobel filter writing outside its
# destination), FS-163 (a copy into a volume of fewer frames: FreeSurfer writes past it), FS-559
# (border labels written outside the destination), FS-560 (a histogram bin before its start)
SKIP = (
    "FS-540",
    "FS-541",
    "FS-58",
    "FS-53",
    "FS-546",
    "FS-548",
    "FS-549",
    "FS-550",
    "FS-551",
    "FS-552",
    "FS-555",
    "FS-163",
    "FS-559",
    "FS-560",
)


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-filter" / "fsfilter"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-filter/build.sh first")
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
            "fs_filter_probe",
            "-p",
            "larmorx-freesurfer-filter",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_filter_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument(
        "--only", help="comma-separated commands to keep (a check while porting; no official hash)"
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-filter"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = f"-{a.only.replace(',', '_')}" if a.only else ""
    cases_path = inputs / f"cases-{a.seed}-{a.n}{tag}.txt"
    text = subprocess.run(
        [str(port), "cases", str(a.seed), str(a.n)], check=True, capture_output=True, text=True
    ).stdout
    if a.only:
        keep = set(a.only.split(","))
        text = "".join(line for line in text.splitlines(True) if line.split()[0] in keep)
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, cases_path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=7200,
        runner="fs-filter-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsfilter failed ({done.returncode}): {done.stderr[-2000:]}")
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
    equal, differ, skipped, skipped_same = Counter(), Counter(), Counter(), Counter()
    fix_differ, fix_unexplained, fix_ids, rep_ids = Counter(), Counter(), Counter(), Counter()
    hashed = hashlib.sha256()
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        for x in ids:
            rep_ids[x] += 1
        if any(x in ids for x in SKIP):
            skipped[cmd] += 1
            if c == r:
                skipped_same[cmd] += 1
            hashed.update(b"SKIP\n")
            continue
        hashed.update(c.encode() + b"\n")
        if c == r:
            equal[cmd] += 1
        else:
            differ[cmd] += 1
            if len(first_differ) < a.show:
                first_differ.append({"case": case[:400], "official": c[:600], "port": r[:600]})
        if f != c:
            fix_differ[cmd] += 1
            if not fids:
                fix_unexplained[cmd] += 1
                if len(first_differ) < a.show:
                    first_differ.append({"case": case[:400], "official": c[:600], "fix": f[:600]})
            for x in fids:
                fix_ids[x] += 1
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "served": done.cached,
        "reproduce_equal": sum(equal.values()),
        "reproduce_equal_per_command": dict(equal),
        "reproduce_differ": dict(differ),
        "skipped": dict(skipped),
        "skipped_but_identical": dict(skipped_same),
        "reproduce_notices": dict(rep_ids),
        "fix_differ": dict(fix_differ),
        "fix_differ_without_notice": dict(fix_unexplained),
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "skipped_total": sum(skipped.values()),
        "first_differ": first_differ,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
