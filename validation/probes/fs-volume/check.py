# SPDX-License-Identifier: Apache-2.0
"""fs.volume against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsvolume
is a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-volume's example ``fs_volume_probe``, runs them
through ``fsvolume`` (``build.sh``: FreeSurfer's volume functions from the unit oracle's build,
linked as the official programs are) by the oracle-output cache at one thread, runs the port on the
same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (the
result and the text FreeSurfer's code printed): ``reproduce`` must give FreeSurfer's on every case
but those where FreeSurfer reads or writes memory it does not own (``SKIP``), which are counted,
not compared (how many of them happen to agree is printed too); ``fix`` may differ only where a
defect it corrects changed the result (its notices). Prints the counts and the hash the port's
recorded test checks (``tests/fs_volume_official.rs``).

    python -P validation/probes/fs-volume/check.py --seed 1 --n 400 [--json out.json]
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

# memory FreeSurfer does not own: FS-288 (a buffer never written compared with the keys). FS-285
# and FS-286 (words written past their buffers) are compared: the official build's layout makes
# them deterministic, and reproduce follows it
# and the matrices FreeSurfer's eigen-systems never return on (FS-47, FS-48, ITK-112, ITK-115,
# ITK-116: the official code hangs, the port gives an error)
SKIP = (
    "FS-288",
    "FS-53",
    "FS-58",
    "FS-290",
    "FS-294",
    "FS-47",
    "FS-48",
    "ITK-112",
    "ITK-115",
    "ITK-116",
    "FS-300",
    "FS-302",
    "FS-303",
    "FS-304",
    "FS-305",
    "FS-306",
    "FS-307",
    "FS-308",
    "FS-309",
    "FS-310",
    "FS-314",
    "FS-315",
    "FS-322",
    "FS-323",
    "FS-324",
    "FS-325",
    "FS-327",
    "FS-330",
    "FS-463",
    "FS-465",
    "FS-467",
    "FS-163",
    "FS-469",
    "FS-470",
    "FS-471",
    "FS-473",
    "FS-475",
    "FS-479",
    "FS-482",
    "FS-484",
    "FS-485",
    "FS-486",
    "FS-487",
    "FS-488",
    "FS-490",
    "FS-491",
    "FS-493",
    "FS-496",
    "FS-497",
)


def heavy(case: str) -> bool:
    """``MRIconform`` resampling onto its 256^3 grid (every case of the command ``conform`` with kind
    0 but the grid itself): compared here, hashed as ``HEAVY`` in the recorded test (minutes each in
    a debug build)."""
    t = case.split()
    return t[:2] == ["conform", "0"] and t[2:5] != ["256", "256", "256"]


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-volume" / "fsvolume"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-volume/build.sh first")
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
            "fs_volume_probe",
            "-p",
            "larmorx-freesurfer-volume",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_volume_probe"


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
    inputs = ws / "oracles" / "inputs" / "fs-volume"
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
        runner="fs-volume-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsvolume failed ({done.returncode}): {done.stderr[-2000:]}")
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
    heavies = skipped_hashed = 0
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        for x in ids:
            rep_ids[x] += 1
        if heavy(case):
            hashed.update(b"HEAVY\n")
            heavies += 1
        elif any(x in ids for x in SKIP):
            hashed.update(b"SKIP\n")
            skipped_hashed += 1
        else:
            hashed.update(c.encode() + b"\n")
        if any(x in ids for x in SKIP):
            skipped[cmd] += 1
            if c == r:
                skipped_same[cmd] += 1
            continue
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
        "hashed_skip": skipped_hashed,
        "hashed_heavy": heavies,
        "first_differ": first_differ,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
