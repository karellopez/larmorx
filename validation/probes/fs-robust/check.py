# SPDX-License-Identifier: Apache-2.0
"""fs.robust against FreeSurfer 8.2.0's official code, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsrobust
is a black-box run of FreeSurfer's code).

Generates seeded cases with larmorx-freesurfer-robust's example ``fs_robust_probe``, runs them
through ``fsrobust`` (``build.sh``: FreeSurfer's robust-registration helpers compiled with the
official flags, linked as the official programs are) by the oracle-output cache at one thread, runs the port on the
same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (the
result and the text FreeSurfer's code printed): ``reproduce`` must give FreeSurfer's on every case
but those where FreeSurfer reads or writes memory it does not own (``SKIP``), which are counted,
not compared (how many of them happen to agree is printed too); ``fix`` may differ only where a
defect it corrects changed the result (its notices). Prints the counts and the hash the port's
recorded test checks (``tests/fs_robust_official.rs``).

    python -P validation/probes/fs-robust/check.py --seed 1 --n 400 [--json out.json]
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

# fs-volume's and fs-filter's SKIP (memory FreeSurfer does not own, crashes, hangs) and this
# probe's own: FS-966, FS-967 and ITK-115 (CostFunctions::orientation). fs-volume's comment:
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
    "FS-487",
    "FS-488",
    "FS-490",
    "FS-491",
    "FS-493",
    "FS-496",
    "FS-497",
    "FS-498",
    "FS-499",
    "FS-501",
    "FS-502",
    "FS-504",
    "FS-508",
    "FS-509",
    "FS-511",
    "FS-515",
    "FS-517",
    "FS-270",
    "FS-167",
    "FS-886",
    "FS-887",
    "FS-889",
    "FS-890",
    "FS-957",
    "FS-964",
    "FS-965",
    "FS-540",
    "FS-541",
    "FS-546",
    "FS-548",
    "FS-549",
    "FS-550",
    "FS-551",
    "FS-552",
    "FS-555",
    "FS-559",
    "FS-560",
    "FS-561",
    "FS-562",
    "FS-564",
    "FS-567",
    "FS-568",
    "FS-569",
    "FS-570",
    "FS-571",
    "FS-573",
    "FS-575",
    "FS-579",
    "FS-581",
    "FS-582",
    "FS-583",
    "FS-586",
    "FS-588",
    "FS-589",
    "FS-591",
    "FS-593",
    "FS-597",
    "FS-598",
    "FS-600",
    "FS-601",
    "FS-615",
    "FS-618",
    "FS-966",
    "FS-967",
    # the registration step's (3.2-03 part 1): a transformation set from a vector of another
    # length, the median of an empty list, the robust location's empty weights, the
    # least-squares location without a design matrix, fewer rows than parameters (memory
    # contents or a crash in FreeSurfer)
    "FS-1063",
    # FS-1063's allocator reuse (relay 44): the parameter there is the allocator's mangled
    # pointer, an address (the unit probe prints its bits); mri_robust_register's probe compares it
    "FS-1073",
    "FS-1064",
    "FS-1065",
    "FS-1066",
    "FS-1067",
    # VNL's QR solve of one row whose entry is 0 leaves the solution unwritten
    "ITK-113",
    # the registration's (3.2-03 part 2): the half-way maps before any iteration, a step of two
    # images of different sizes (a crash or memory contents in FreeSurfer)
    "FS-1068",
    "FS-1069",
)


# FS-968's lines (a transform distance from an SVD that did not converge): FreeSurfer's arbitrary
# number, which reproduce gives but for the NaNs' bits (the double SVD's NaN operand order is not
# ported yet: an open item in the crate's PROVENANCE.md). Compared with every NaN read as one NaN,
# hashed as NANBITS. SVDNAN (the port's marker, no defect): a double least-squares solve of a
# matrix with an infinite or NaN entry, the same open item (the double SVD's NaN operand orders).
NANBITS = ("FS-968", "SVDNAN")


def nan_canonical(line: str) -> str:
    """The line with every double or float token that is a NaN replaced by ``NaN``, and the NaNs'
    printed signs (``-nan`` in VNL's matrices on standard error) dropped."""
    out = []
    for tok in line.split(" "):
        if len(tok) == 16 and all(c in "0123456789abcdef" for c in tok):
            v = int(tok, 16)
            if (v >> 52) & 0x7FF == 0x7FF and v & ((1 << 52) - 1):
                tok = "NaN"
        out.append(tok)
    return re.sub(r" *-?nan", " nan", " ".join(out))


# The port's lines for a path of LAPACK it does not port (ZLAQR0, where ZLAHQR does not converge:
# a matrix with an infinite or NaN element, FS-969): counted and hashed as NOTPORTED, not compared.
NOTPORTED = "is not ported yet"


def heavy(case: str) -> bool:
    """No case of this probe is heavy (kept for check.py's shape)."""
    return False


def probe_binary(ws: Path) -> Path:
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-robust" / "fsrobust"
    if not exe.is_file():
        raise SystemExit(f"no {exe}: run validation/probes/fs-robust/build.sh first")
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
            "fs_robust_probe",
            "-p",
            "larmorx-freesurfer-robust",
            "--manifest-path",
            str(manifest),
            "-j",
            str(jobs),
        ],
        check=True,
        env=env,
    )
    return REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_robust_probe"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument(
        "--dump",
        type=Path,
        help="write every case's lines (official, reproduce, fix) as JSON lines",
    )
    ap.add_argument(
        "--only", help="comma-separated commands to keep (a check while porting; no official hash)"
    )
    ap.add_argument(
        "--family",
        choices=("helpers", "sqrtm", "regstep", "reg", "logm"),
        default="helpers",
        help="the seeded stream: the helpers (part 1), the matrix square root's (schur, sqrtm), "
        "the registration step's (3.2-03: rgauss, regress, trans, quat, rstep) or the "
        "registration's (3.2-03 part 2: rreg) or the matrix logarithm's and exponential's (3.2-15: "
        "logm, expm, geomean)",
    )
    ap.add_argument(
        "--cases",
        type=Path,
        help="a file of hand-written cases (hand_cases.py) instead of the seeded ones (no official hash)",
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = probe_binary(ws)
    port = port_binary(a.jobs)
    inputs = ws / "oracles" / "inputs" / "fs-robust"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = f"-{a.only.replace(',', '_')}" if a.only else ""
    fam = "" if a.family == "helpers" else f"-{a.family}"
    cases_path = inputs / f"cases{fam}-{a.seed}-{a.n}{tag}.txt"
    if a.cases:
        cases_path = inputs / f"hand-{a.cases.stem}.txt"
        text = a.cases.read_text(encoding="utf-8")
    else:
        sub = "cases" if a.family == "helpers" else f"cases-{a.family}"
        text = subprocess.run(
            [str(port), sub, str(a.seed), str(a.n)], check=True, capture_output=True, text=True
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
        runner="fs-robust-probe",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsrobust failed ({done.returncode}): {done.stderr[-2000:]}")
    official = done.stdout.splitlines()
    # FS-1311 (memory contents): the official matrix exponential reads its Pade coefficients from
    # uninitialized stack memory; its cases are compared with fsrobust-pade (MyMatrix.cpp with the
    # coefficients' arrays static, build.sh), the unpatched line counted apart
    pade_official: list[str] | None = None
    if a.family == "logm":
        pdone = oracle_cache.run(
            [exe.with_name("fsrobust-pade"), cases_path],
            env=env,
            policy=oracle_cache.Policy(threads=1, family="freesurfer"),
            timeout=7200,
            runner="fs-robust-probe",
        )
        if pdone.returncode != 0:
            raise SystemExit(f"fsrobust-pade failed ({pdone.returncode}): {pdone.stderr[-2000:]}")
        pade_official = pdone.stdout.splitlines()
        assert len(pade_official) == len(official)

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
    if a.dump:
        with a.dump.open("w", encoding="utf-8") as fh:
            for case, c, (r, ids), (f, fids) in zip(cases, official, rep, fix, strict=True):
                row = {
                    "case": case,
                    "official": c,
                    "reproduce": r,
                    "ids": ids,
                    "fix": f,
                    "fids": fids,
                }
                fh.write(json.dumps(row) + "\n")
    equal, differ, skipped, skipped_same = Counter(), Counter(), Counter(), Counter()
    notported = Counter()
    fix_differ, fix_unexplained, fix_ids, rep_ids = Counter(), Counter(), Counter(), Counter()
    hashed = hashlib.sha256()
    heavies = skipped_hashed = 0
    first_differ = []
    pade_cases = pade_unpatched_same = 0
    for k, (case, c, (r, ids), (f, fids)) in enumerate(zip(cases, official, rep, fix, strict=True)):
        cmd = case.split()[0]
        if pade_official is not None and "FS-1311" in ids:
            pade_cases += 1
            pade_unpatched_same += c == pade_official[k]
            c = pade_official[k]
        for x in ids:
            rep_ids[x] += 1
        if NOTPORTED in r:
            hashed.update(b"NOTPORTED\n")
            notported[cmd] += 1
            continue
        if heavy(case):
            hashed.update(b"HEAVY\n")
            heavies += 1
        elif any(x in ids for x in NANBITS):
            hashed.update(b"NANBITS\n")
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
        if c == r or (any(x in ids for x in NANBITS) and nan_canonical(c) == nan_canonical(r)):
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
        "family": a.family,
        "seed": a.seed,
        "cases": len(cases),
        "served": done.cached,
        "reproduce_equal": sum(equal.values()),
        "reproduce_equal_per_command": dict(equal),
        "reproduce_differ": dict(differ),
        "skipped": dict(skipped),
        "skipped_but_identical": dict(skipped_same),
        "not_ported": dict(notported),
        "reproduce_notices": dict(rep_ids),
        "fix_differ": dict(fix_differ),
        "fix_differ_without_notice": dict(fix_unexplained),
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "pade_reference_cases": pade_cases,
        "pade_unpatched_identical": pade_unpatched_same,
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
