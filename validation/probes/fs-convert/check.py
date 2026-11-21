# SPDX-License-Identifier: Apache-2.0
"""mri_convert's stats tables, CSV reader and command line against FreeSurfer 8.2.0's official code.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsconvert is
a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-volumes' example ``fs_convert_probe``, runs them
through ``fsconvert`` (``build.sh``: mri_convert.cpp with its main renamed and two print hooks,
linked with the unit oracle's build as the official program is) by the oracle-output cache at one
thread, runs the port on the same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and
compares the lines: ``reproduce`` must give FreeSurfer's on every case but those where FreeSurfer
reads memory it does not own or crashes (``SKIP``), which are counted, not compared; ``fix`` may
differ only where a defect it corrects changed the result (its notices). Prints the counts and the
hash the port's recorded test checks (``tests/fs_convert_official.rs``).

    python -P validation/probes/fs-convert/check.py --seed 1 --n 500 [--json out.json]
    python -P validation/probes/fs-convert/check.py --kind cnv --seed 1 --n 300
    python -P validation/probes/fs-convert/check.py --kind otl --seed 1 --n 120

``--kind cnv-heavy``: the conformed conversions (outputs of 256 voxels per axis), not in the
recorded test. ``--kind otl`` (relay 48): outline (.otl) series read and converted
(``SKIP_OTL``). ``--kind cnv``: whole conversions (``cnv`` cases: small MGH inputs, options with
arguments they accept, every written file compared through ``gzread`` with the command lines' time
stamps masked).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

# FreeSurfer reads memory it does not own or crashes: FS-1031 (a table name without an extension),
# FS-1032 (an empty table: its columns counted from memory contents), FS-1034 (a word past the
# reader's buffer), FS-1035 and FS-1036 (an empty CSV table, a short CSV row), FS-1037 (a .tac
# table of one column), FS-1038 (an option short of its arguments), FS-1039 (a name past its
# buffer), FS-1040 (--rescale-voxel's memory contents), FS-1041 (frames past their list), FS-1042
# (debug.gdb that cannot be created); FS-170 and FS-171 (a TSV table's names read from memory
# contents: an empty file, an empty first line)
SKIP = (
    "FS-170",
    "FS-171",
    "FS-1031",
    "FS-1032",
    "FS-1034",
    "FS-1035",
    "FS-1036",
    "FS-1037",
    "FS-1038",
    "FS-1039",
    "FS-1040",
    "FS-1041",
    "FS-1042",
)
# whole conversions: also the library's reads and writes outside a volume's buffer (FS-53),
# MRIcopy writing past a smaller destination (FS-163), and main's own crashes and writes past its
# buffers (FS-1043, FS-1044, FS-1046, FS-1047, FS-1049 to FS-1052)
SKIP_CNV = (
    *SKIP,
    "FS-53",
    "FS-163",
    "FS-1043",
    "FS-1044",
    "FS-1046",
    "FS-1047",
    "FS-1049",
    "FS-1050",
    "FS-1051",
    "FS-1052",
)


# outline (.otl) series: also the reader's crashes and memory reads (FS-1054: a file that is not
# a GDF file; FS-1057: labels read past the outline field; FS-1141: point coordinates never read;
# FS-1142: a slice count below 1; FS-1143: slice files past the open-file limit; FS-1144: a fill
# deeper than the stack; FS-1145: a field past the machine's memory). FS-1056's writes outside the
# 512 x 512 array are compared: the official build leaves that never-read array out.
SKIP_OTL = (
    *SKIP_CNV,
    "FS-1054",
    "FS-1057",
    "FS-1141",
    "FS-1142",
    "FS-1143",
    "FS-1144",
    "FS-1145",
)


# FreeSurfer prints memory contents in a line the case otherwise compares: FS-1053 (a new
# volume's printed validity flag); both sides' lines are compared with that line masked
MASK = {"FS-1053": (re.compile(r"valid   : -?[0-9]+"), "valid   : ?")}


def masked(line: str, ids: list[str]) -> str:
    for x in ids:
        if x in MASK:
            pat, rep = MASK[x]
            line = pat.sub(rep, line)
    return line


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-convert" / "fsconvert"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-convert/build.sh first")
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
            "fs_convert_probe",
            "-p",
            "larmorx-freesurfer-volumes",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_convert_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kind", choices=("opt", "cnv", "cnv-heavy", "otl"), default="opt")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=500)
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
    inputs = ws / "oracles" / "inputs" / "fs-convert"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = f"-{a.only.replace(',', '_')}" if a.only else ""
    kind = "" if a.kind == "opt" else f"{a.kind}-"
    cases_path = inputs / f"{kind}cases-{a.seed}-{a.n}{tag}.txt"
    gen = "cases" if a.kind == "opt" else f"{a.kind}-cases"
    text = subprocess.run(
        [str(port), gen, str(a.seed), str(a.n)], check=True, capture_output=True, text=True
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
        runner="fs-convert-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsconvert failed ({done.returncode}): {done.stderr[-2000:]}")
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
    if not len(official) == len(rep) == len(fix) == len(cases):
        raise SystemExit(
            f"line counts differ: official {len(official)}, reproduce {len(rep)}, fix {len(fix)}, "
            f"cases {len(cases)}"
        )
    equal, differ, skipped, skipped_same = Counter(), Counter(), Counter(), Counter()
    fix_differ, fix_unexplained, fix_ids, rep_ids = Counter(), Counter(), Counter(), Counter()
    hashed = hashlib.sha256()
    first_differ = []
    for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
        cmd = case.split()[0]
        for x in ids:
            rep_ids[x] += 1
        skip = {"opt": SKIP, "otl": SKIP_OTL}.get(a.kind, SKIP_CNV)
        if any(x in ids for x in skip):
            skipped[cmd] += 1
            if c == r:
                skipped_same[cmd] += 1
            hashed.update(b"SKIP\n")
            continue
        c, r, f = masked(c, ids), masked(r, ids), masked(f, ids)
        hashed.update(c.encode() + b"\n")
        if c == r:
            equal[cmd] += 1
        else:
            differ[cmd] += 1
            if len(first_differ) < a.show:
                first_differ.append({"case": case[:400], "official": c[:900], "port": r[:900]})
        if f != c:
            fix_differ[cmd] += 1
            if not fids:
                fix_unexplained[cmd] += 1
                if len(first_differ) < a.show:
                    first_differ.append({"case": case[:400], "official": c[:900], "fix": f[:900]})
            for x in fids:
                fix_ids[x] += 1
    summary = {
        "kind": a.kind,
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
