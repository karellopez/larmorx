# SPDX-License-Identifier: Apache-2.0
"""mri_robust_register against FreeSurfer 8.2.0's official program, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example; its SKIP list names
the replica's defect handling).

Generates seeded cases with larmorx-freesurfer-volumes' example ``fs_robreg_probe`` (command lines
with the files they read: pairs of small head-like volumes of every type, extent and orientation,
masks, initial transforms and intensity scales, every option and output; the command line's own
paths and failures), runs the official ``mri_robust_register`` (``$FREESURFER_HOME/bin``, by name)
on each case through the oracle-output cache at one thread, in a folder of its own, runs the port
on the same cases with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines
(exit status, standard output, standard error, every file written; the line of the time taken
masked): ``reproduce`` must give FreeSurfer's on every case but those where FreeSurfer's result is
undefined (``SKIP``) and those the port does not run yet (``not ported yet``: the costs of
FreeSurfer's Powell registration, the entropy images, ``--test``, ``--debug``), which are counted,
not compared; ``fix`` may differ only where a defect it corrects changed the result (its notices).
Prints the counts and the hash the port's recorded test checks (``tests/fs_robreg_official.rs``).

The official side runs with ``FS_DISABLE_LANG=1``: FreeSurfer's start sets the numbers' locale
(``en_US``), which on a machine without that locale leaves ``errno`` at ``ENOENT`` and so prefixes
its first error message with ``error: No such file or directory`` (a fact of the machine, not
reproduced; ``docs/findings/freesurfer-info.md``). No case sets ``DIAG_NO``: FreeSurfer's licence
check prints the licence file's lines when it is above 0.

    python -P validation/probes/fs-robust-register/check.py --seed 1 --n 300 [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import os
import re
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

# FreeSurfer's result is undefined: FS-1070 (an option's argument past the last: a crash),
# FS-1071 (an --lta name past its buffer), FS-1072 (a morph read as an LTA); the library's:
# FS-1063, FS-1067, FS-1068, FS-1069 (a registration step or half-way maps of memory contents;
# FS-1073, the allocator's reuse FS-1063 meets most, is compared), FS-167 (a
# number read past the end of a file); the readers' and writers' (fs-volume's and fs-info's SKIP):
# FS-302 to FS-310 (an LTA or transform file FreeSurfer misreads or crashes on), FS-58, FS-288,
# FS-290, FS-294, FS-441
SKIP = (
    "FS-1070",
    "FS-1071",
    "FS-1072",
    "FS-1063",
    "FS-1067",
    "FS-1068",
    "FS-1069",
    # relay 48: the Powell registration's half-way geometry, memory FreeSurfer has freed (its
    # defined parts compared apart: DEFINED_BEFORE)
    "FS-1075",
    "FS-167",
    "FS-58",
    "FS-288",
    "FS-290",
    "FS-294",
    "FS-441",
    *(f"FS-{k}" for k in range(302, 311)),
)
NOT_PORTED = "not ported yet"
TIME = re.compile(r"Registration took \d+ minutes and \d+ seconds\.")
# relay 45: the entropy image's time (the port's debug build takes seconds), masked in the lines
# only (the cache's policy keeps TIME alone, so the recorded official runs stay its entries)
ENTROPY_TIME = re.compile(r" Entropy computation took \d+ seconds\.")

#: relay 48, ``--family halfway``: the official runs are never served from the cache (FS-1075: the
#: half-way geometry is memory contents, which may change from run to run)
NEVER = ""
#: FS-1075's defined parts: standard output up to the line FreeSurfer prints before it reads the
#: freed geometry, standard error up to the second half-way map, the files but the half-way ones
HALF_WAY_LINE = "Creating half way data ...\\x0a"
HALF_WAY_FILES = ("hm.lta", "hd.lta", "hm.mgz", "hd.mgz", "hw.mgz")

#: ``--preload``: a library for the official program's ``LD_PRELOAD`` (``decided_complex.sh``'s
#: decided ``cabs`` and ``csqrt``: the port's square roots of matrices under the user's D4); the
#: cache keys the library by its content.
PRELOAD = ""

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_robreg_probe"


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


def written(d: Path, inputs: set[str]) -> str:
    out = []
    for n, b in collect(d):
        if n in inputs:
            continue
        if b is None:
            out.append(f"D {n}")
            continue
        b = b.replace(str(d).encode(), b"<DIR>")
        if n.endswith((".lta", ".xfm", ".dat", ".txt")) or n == "noext":
            out.append(f"F {n} {escape(b)}")
        else:
            out.append(f"F {n} {fnv(b):016x} {len(b)}")
    return " ".join(out)


def slow_in_debug(line: str) -> bool:
    """A case of the Powell registration with a joint histogram's cost or SB (``--cost`` ``MI``,
    ``NMI``, ``ECC``, ``SCR``, ``SB``), which the recorded test's debug build hashes as ``SLOW``
    (``cases::histogram_or_sb_cost``; relay 47)."""
    t = line.split()
    for a, b in itertools.pairwise(t):
        if a == "2d2d636f7374":
            try:
                cost = bytes.fromhex(b).decode()
            except ValueError:
                continue
            if cost.upper() in ("MI", "NMI", "ECC", "SCR", "SB"):
                return True
    return False


def official_line(exe: Path, line: str, root: Path, k: int) -> str:
    t = line.split()
    assert t[0] == "rreg"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "larmorx", "TZ": "UTC0",
           "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
           **freesurfer_env()}  # fmt: skip
    if t[1] != "-":
        key, _, val = bytes.fromhex(t[1]).decode().partition("=")
        env[key] = val
    if PRELOAD:
        # relay 46: the decided cabs and csqrt (decided_complex.sh), the project's D4 decision
        env["LD_PRELOAD"] = PRELOAD
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
    names = {p.relative_to(d).as_posix() for p in inputs}
    for h in t[i + 1 : i + 1 + ndirs]:
        (d / bytes.fromhex(h).decode()).mkdir(parents=True, exist_ok=True)
        names.add(bytes.fromhex(h).decode())
    i += 1 + ndirs
    nargs = int(t[i])
    args = [("" if h == "@" else bytes.fromhex(h).decode()) for h in t[i + 1 : i + 1 + nargs]]
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1, family="freesurfer", formats="freesurfer", volatile=(TIME.pattern,)
        ).but_never(NEVER),
        timeout=120,
        runner="fs-robust-register-probe",
        inputs=inputs,
    )
    rc = done.returncode
    status = f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = escape(str(d).encode())

    def mask(s: str | bytes) -> str:
        b = s.encode("utf-8", "surrogateescape") if isinstance(s, str) else s
        return ENTROPY_TIME.sub(
            " Entropy computation took <T> seconds.",
            TIME.sub(
                "Registration took <T> minutes and <T> seconds.",
                escape(b).replace(dd, "<DIR>"),
                count=1,
            ),
            count=1,
        )

    files = written(d, names)
    return f"{status} | {mask(done.stdout)} | {mask(done.stderr)} | {files}"


def defined_before_half_way(line: str) -> str:
    """FS-1075's defined parts of a case's line (``<status> | <stdout> | <stderr> | <files>``):
    standard output up to ``Creating half way data ...``, standard error up to the end of the
    ``dsthw`` map, the files but the half-way ones (the exit status and the rest hang on the freed
    geometry)."""
    _, out, rest = line.split(" | ", 2)
    err, _, files = rest.rpartition(" | ")
    k = out.find(HALF_WAY_LINE)
    out = out if k < 0 else out[: k + len(HALF_WAY_LINE)]
    k = err.find("dsthw = [")
    if k >= 0:
        e = err.find("]", k)
        err = err[: e + 1] if e >= 0 else err
    kept = []
    for f in re.split(r" (?=[FD] )", files):
        name = f.split(" ", 2)[1] if f.count(" ") >= 1 else f
        if name not in HALF_WAY_FILES:
            kept.append(f)
    return f"{out} | {err} | {' '.join(kept)}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument(
        "--family",
        choices=("seeded", "powell", "histo", "halfway"),
        default="seeded",
        help="the cases: the prelude, the entropy images and seeded ones (--seed, --n), the "
        "Powell registration's 40 (relay 46), its joint-histogram and SB costs' 32 (relay 47), or "
        "its half-way outputs' 24 (relay 48: FS-1075, never served from the cache)",
    )
    ap.add_argument(
        "--preload",
        type=Path,
        help="a library for the official program's LD_PRELOAD (decided_complex.sh's: the decided "
        "cabs and csqrt); its case folders apart (cases-<seed>-<n>-preload.d)",
    )
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument(
        "--golden", action="store_true", help="the golden cases: their official SHA-256 only"
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_robust_register"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_robreg_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip
    inputs = ws / "oracles" / "inputs" / "fs-robust-register"
    inputs.mkdir(parents=True, exist_ok=True)
    global PRELOAD, NEVER
    PRELOAD = str(a.preload.resolve()) if a.preload else ""
    if a.family == "halfway":
        NEVER = "FS-1075: the half-way geometry is read from a freed image (memory contents)"
    tag = ("golden" if a.golden else a.family if a.family != "seeded" else f"{a.seed}-{a.n}") + (
        "-preload" if a.preload else ""
    )
    cases_path = inputs / f"cases-{tag}.txt"
    gen = (
        ["golden-cases"]
        if a.golden
        else [f"{a.family}-cases"]
        if a.family != "seeded"
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
    equal = differ = skipped = skipped_same = not_ported = 0
    fix_differ, fix_unexplained, fix_ids, rep_ids = 0, 0, Counter(), Counter()
    hashed = hashlib.sha256()
    # relay 47: the recorded test's debug build hashes the cases of a joint histogram's cost or SB
    # as SLOW (cases::histogram_or_sb_cost)
    hashed_debug = hashlib.sha256()
    first = []
    # relay 48: FS-1075's cases, their defined parts compared
    half_same = half_differ = 0
    for k, (case, c, (r, ids), (f, fids)) in enumerate(zip(cases, official, rep, fix, strict=True)):
        for x in ids:
            rep_ids[x] += 1
        if slow_in_debug(case):
            hashed_debug.update(b"SLOW\n")
        if NOT_PORTED in r:
            not_ported += 1
            hashed.update(b"NOTPORTED\n")
            if not slow_in_debug(case):
                hashed_debug.update(b"NOTPORTED\n")
            continue
        if any(x in ids for x in SKIP):
            skipped += 1
            skipped_same += c == r
            # FS-1075's defined parts, where no other undefined result came first
            if "FS-1075" in ids and not any(x in ids for x in SKIP if x != "FS-1075"):
                if defined_before_half_way(c) == defined_before_half_way(r):
                    half_same += 1
                else:
                    half_differ += 1
                    if len(first) < a.show:
                        first.append(
                            {"case": k, "fs1075_official": c[:1500], "fs1075_port": r[:1500]}
                        )
            hashed.update(b"SKIP\n")
            if not slow_in_debug(case):
                hashed_debug.update(b"SKIP\n")
            continue
        hashed.update(c.encode() + b"\n")
        if not slow_in_debug(case):
            hashed_debug.update(c.encode() + b"\n")
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
        # relay 47: which cases, and whether the official program ran with --preload's library
        "family": "golden" if a.golden else a.family,
        "preload": bool(a.preload),
        "cases": len(cases),
        "reproduce_equal": equal,
        "reproduce_differ": differ,
        "skipped": skipped,
        "skipped_but_identical": skipped_same,
        "fs1075_defined_identical": half_same,
        "fs1075_defined_differ": half_differ,
        "not_ported": not_ported,
        "reproduce_notices": dict(rep_ids),
        "fix_differ": fix_differ,
        "fix_differ_without_notice": fix_unexplained,
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "official_sha256_skip_debug": hashed_debug.hexdigest(),
        "first_differ": first,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained or half_differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
