# SPDX-License-Identifier: Apache-2.0
"""mri_add_xform_to_header against FreeSurfer 8.2.0's official program, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example; its SKIP list names
the replica's defect handling).

Generates seeded cases with larmorx-freesurfer-volumes' example ``fs_xform_probe`` (command lines
with the files and folders they read: MGZ and MGH volumes naming transforms that load, are missing
or fail, COR folders, transform files of every kind, odd names, outputs of every format and
folders), runs the official ``mri_add_xform_to_header`` (``$FREESURFER_HOME/bin``, by name) on each
case through the oracle-output cache at one thread, in a folder of its own, runs the port on the
same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (exit
status, standard output, standard error, every file and folder after the run): ``reproduce`` must
give FreeSurfer's on every case but those where FreeSurfer's result is undefined (``SKIP``), which
are counted, not compared; ``fix`` may differ only where a defect it corrects changed the result
(its notices). Prints the counts and the hash the port's recorded test checks
(``tests/fs_xform_official.rs``). ``--extra``: the cases only Linux's file names hold (not in the
recorded test: a path past 1,024 bytes, where macOS's stop; a name ending in a dot, which Windows
drops).

The official side runs with ``FS_DISABLE_LANG=1``: FreeSurfer's start sets the numbers' locale
(``en_US``), which on a machine without that locale leaves ``errno`` at ``ENOENT`` and so prefixes
its first error message with ``error: No such file or directory`` (a fact of the machine, not
reproduced; ``docs/findings/freesurfer-info.md``). No case sets ``DIAG_NO``: FreeSurfer's licence
check prints the licence file's lines when it is above 0.

    python -P validation/probes/fs-add-xform/check.py --seed 1 --n 400 [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

# FreeSurfer's result is undefined: FS-1154 (a transform name past the header's room), FS-1156
# (a COR-.info path past FreeSurfer's buffer); the library's: FS-58 (a COR folder's header,
# read without its slices, written as a volume: a crash), FS-167 (a number read past the end of a
# file), FS-441 (a volume without frames written: memory contents)
SKIP = ("FS-1154", "FS-1156", "FS-58", "FS-167", "FS-441")

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_xform_probe"


def escape(b: bytes) -> str:
    return "".join(chr(c) if 0x20 <= c <= 0x7E and c != 0x5C else f"\\x{c:02x}" for c in b)


def fnv(b: bytes) -> int:
    h = 0xCBF29CE484222325
    for c in b:
        h = ((h ^ c) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def collect(d: Path, prefix: str = "") -> list[tuple[str, bytes | None]]:
    out: list[tuple[str, bytes | None]] = []
    for e in sorted(d.iterdir(), key=lambda p: p.name.encode()):
        n = prefix + e.name
        if e.is_dir():
            out.append((n, None))
            out.extend(collect(e, n + "/"))
        else:
            out.append((n, e.read_bytes()))
    return out


def written(d: Path) -> str:
    out = []
    for n, b in collect(d):
        if b is None:
            out.append(f"D {n}")
            continue
        b = b.replace(str(d).encode(), b"<DIR>")
        if n.endswith((".xfm", ".info", ".txt")):
            out.append(f"F {n} {escape(b)}")
        else:
            out.append(f"F {n} {fnv(b):016x} {len(b)}")
    return " ".join(out)


def official_line(exe: Path, line: str, root: Path, k: int) -> str:
    t = line.split()
    assert t[0] == "xform"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", **freesurfer_env()}  # fmt: skip
    if t[1] != "-":
        key, _, val = bytes.fromhex(t[1]).decode().partition("=")
        env[key] = val
    # never a run whose licence check prints the licence file
    if any(x.startswith("DIAG_NO") or x.lower() == "diag" for x in env):
        raise SystemExit(f"case {k}: DIAG_NO would print the licence file")
    d = (root / f"case-{k}").resolve()
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    n = int(t[2])
    i = 3
    inputs = []
    for _ in range(n):
        name, h = t[i], t[i + 1]
        p = d / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(b"" if h == "@" else bytes.fromhex(h))
        inputs.append(p)
        i += 2
    ndirs = int(t[i])
    for h in t[i + 1 : i + 1 + ndirs]:
        (d / bytes.fromhex(h).decode()).mkdir(parents=True, exist_ok=True)
    i += 1 + ndirs
    nargs = int(t[i])
    args = [("" if h == "@" else bytes.fromhex(h).decode()) for h in t[i + 1 : i + 1 + nargs]]
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer"),
        timeout=120,
        runner="fs-add-xform-probe",
        inputs=inputs,
    )
    rc = done.returncode
    status = f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = escape(str(d).encode())

    def mask(s: str | bytes) -> str:
        b = s.encode("utf-8", "surrogateescape") if isinstance(s, str) else s
        return escape(b).replace(dd, "<DIR>")

    files = written(d)
    return f"{status} | {mask(done.stdout)} | {mask(done.stderr)} | {files}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument(
        "--golden", action="store_true", help="the golden cases: their official SHA-256 only"
    )
    ap.add_argument("--extra", action="store_true", help="the cases only Linux's names hold")
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_add_xform_to_header"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_xform_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip
    inputs = ws / "oracles" / "inputs" / "fs-add-xform"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "golden" if a.golden else "extra" if a.extra else f"{a.seed}-{a.n}"
    cases_path = inputs / f"cases-{tag}.txt"
    gen = (
        ["golden-cases"]
        if a.golden
        else ["extra-cases"]
        if a.extra
        else ["cases", str(a.seed), str(a.n)]
    )
    text = subprocess.run([EXAMPLE, *gen], check=True, capture_output=True, text=True).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    root = inputs / f"cases-{tag}.d"
    official = [official_line(exe, c, root, k) for k, c in enumerate(cases)]
    shutil.rmtree(root)
    if a.golden:
        h = hashlib.sha256()
        for c in official:
            h.update(c.encode() + b"\n")
        print(f"golden OFFICIAL_SHA256 {h.hexdigest()} ({len(official)} cases)")

    def port_lines(mode: str) -> list[tuple[str, list[str]]]:
        out = subprocess.run(
            [EXAMPLE, "run", cases_path, mode], check=True, capture_output=True, text=True
        ).stdout
        rows = []
        for line in out.splitlines():
            body, _, ids = line.partition("\t")
            rows.append((body, [x for x in ids.split(",") if x]))
        return rows

    rep, fix = port_lines("reproduce"), port_lines("fix")
    if not len(official) == len(rep) == len(fix) == len(cases):
        raise SystemExit(
            f"line counts differ: official {len(official)}, reproduce {len(rep)}, fix {len(fix)}"
        )
    equal = differ = skipped = skipped_same = 0
    fix_differ, fix_unexplained, fix_ids, rep_ids = 0, 0, Counter(), Counter()
    hashed = hashlib.sha256()
    first = []
    for k, (case, c, (r, ids), (f, fids)) in enumerate(zip(cases, official, rep, fix, strict=True)):
        for x in ids:
            rep_ids[x] += 1
        if any(x in ids for x in SKIP):
            skipped += 1
            skipped_same += c == r
            hashed.update(b"SKIP\n")
            continue
        hashed.update(c.encode() + b"\n")
        if c == r:
            equal += 1
        else:
            differ += 1
            if len(first) < a.show:
                first.append(
                    {"case": k, "args": case[-300:], "official": c[:1500], "port": r[:1500]}
                )
        if f != c:
            fix_differ += 1
            for x in fids:
                fix_ids[x] += 1
            if not fids:
                fix_unexplained += 1
                if len(first) < a.show:
                    first.append({"case": k, "official": c[:900], "fix": f[:900]})
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "reproduce_equal": equal,
        "reproduce_differ": differ,
        "skipped": skipped,
        "skipped_but_identical": skipped_same,
        "reproduce_notices": dict(rep_ids),
        "fix_differ": fix_differ,
        "fix_differ_without_notice": fix_unexplained,
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "first_differ": first,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
