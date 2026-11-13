# SPDX-License-Identifier: Apache-2.0
"""fs.surfio against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fssurfio
is a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-surf's example ``fs_surfio_probe``, runs them
through ``fssurfio`` (``build.sh``: FreeSurfer's surface structure helpers and views from the unit
oracle's build, linked as the official programs are; ``metric``'s cases through ``fssurfio-mpstub``,
whose ``MRIScomputeMetricProperties`` is a stub) by the oracle-output cache at one thread, runs
the port on the same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the
lines (the result and the text FreeSurfer's code printed): ``reproduce`` must give FreeSurfer's on
every case (``SKIP`` lists the defects whose cases are counted, not compared: FreeSurfer reads
memory it never wrote, FS-121, FS-123, FS-06, FS-167, FS-49, FS-140, FS-144); ``fix`` may
differ only where a defect it corrects changed the result (its notices), or where the default's
correctly rounded ``sinf``/``cosf`` replace glibc's (a ``surf`` case that rotates, ``Z``) or its
correctly rounded ``atan2f`` (a ``posn`` case that computes the canonical coordinates' angles,
``cc`` or ``sv 3``): these name nothing, as every FreeSurfer computation that takes
``larmorx_core::math::Libm``'s quiet choice; counted as ``fix_libm``. Prints the counts and the hash the port's recorded test
checks (``tests/fs_surfio_official.rs``).

    python -P validation/probes/fs-surfio/check.py --seed 1 --n 930 [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import itertools
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

# FreeSurfer reads memory it never wrote or does not own in these cases (the metric properties'
# copy's unwritten normals and areas, FS-121; the faster computation's ripped faces and empty box,
# FS-123; the table arctangent of a NaN, FS-06; a number a file ends before, FS-167 and FS-49, the
# `sfile` and `cfile` readers; a decimation count that cannot be read, FS-140; a first VTK vertex
# that cannot be read, FS-144): counted, never compared
SKIP: tuple[str, ...] = ("FS-121", "FS-123", "FS-06", "FS-167", "FS-49", "FS-140", "FS-144")
SKIP_WHEN_REFUSED: tuple[str, ...] = ("FS-114",)


# the commands whose cases run on fssurfio-mpstub (MRIScomputeMetricProperties a stub)
STUB_COMMANDS = ("metric",)


def libm_case(cmd: str, toks: list[str]) -> bool:
    """Whether the case runs a function whose default takes correctly rounded maths where
    FreeSurfer's binary takes glibc's: a `surf` rotation (`Z`: `sinf`, `cosf`), a `posn`
    canonical-coordinates step (`cc`, or `sv 3`: `atan2f`)."""
    if cmd == "surf":
        return "Z" in toks
    if cmd == "posn":
        return "cc" in toks or any(a == "sv" and b == "3" for a, b in itertools.pairwise(toks))
    return False


def probe_binary(ws: Path, name: str = "fssurfio") -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-surfio" / name
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-surfio/build.sh first")
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
            "fs_surfio_probe",
            "-p",
            "larmorx-freesurfer-surf",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_surfio_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=450)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    stub = probe_binary(ws, "fssurfio-mpstub")
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-surfio"
    inputs.mkdir(parents=True, exist_ok=True)
    cases_path = inputs / f"cases-{a.seed}-{a.n}.txt"
    text = subprocess.run(
        [str(port), "cases", str(a.seed), str(a.n)], check=True, capture_output=True, text=True
    ).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    official: list[str] = [""] * len(cases)
    served = []
    for binary, on_stub in ((exe, False), (stub, True)):
        idx = [i for i, c in enumerate(cases) if (c.split()[0] in STUB_COMMANDS) == on_stub]
        if not idx:
            continue
        part = cases_path if not on_stub and len(idx) == len(cases) else None
        if part is None:
            part = inputs / f"cases-{a.seed}-{a.n}-{binary.name}.txt"
            part.write_text("".join(cases[i] + "\n" for i in idx), encoding="utf-8")
        done = oracle_cache.run(
            [binary, part],
            env=env,
            policy=oracle_cache.Policy(threads=1, family="freesurfer"),
            timeout=3600,
            runner="fs-surfio-probe",
        )
        if done.returncode != 0:
            raise SystemExit(f"{binary.name} failed ({done.returncode}): {done.stderr[-2000:]}")
        lines = done.stdout.splitlines()
        assert len(lines) == len(idx), (binary.name, len(lines), len(idx))
        for i, line in zip(idx, lines, strict=True):
            official[i] = line
        served.append(done.cached)

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
    fix_libm = Counter()
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
            if not fids and libm_case(cmd, case.split()):
                fix_libm[cmd] += 1
            elif not fids:
                fix_unexplained[cmd] += 1
            for x in fids:
                fix_ids[x] += 1
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "served": served,
        "reproduce_equal": sum(equal.values()),
        "reproduce_differ": dict(differ),
        "skipped": dict(skipped),
        "reproduce_notices": dict(rep_ids),
        "fix_differ": dict(fix_differ),
        "fix_differ_without_notice": dict(fix_unexplained),
        "fix_libm": dict(fix_libm),
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
