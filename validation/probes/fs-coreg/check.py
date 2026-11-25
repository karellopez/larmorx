# SPDX-License-Identifier: Apache-2.0
"""mri_coreg against FreeSurfer 8.2.0's official program, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example; its SKIP list names
the replica's defect handling).

Generates the cases of larmorx-freesurfer-volumes' example ``fs_coreg_probe`` (command lines with
the files they read: the command line's own paths and failures, the stand-alone options, every
registration option on a small "BOLD" volume and a "brain" reference, a subject's folder, then
seeded pairs of every type and orientation with random options), runs the official ``mri_coreg``
(``$FREESURFER_HOME/bin``, by name) on each case through the oracle-output cache at one thread
(``OMP_NUM_THREADS=1``), in a folder of its own, runs the port on the same cases with
``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (exit status, standard
output, standard error, every file written; the times and the machine's names masked):
``reproduce`` must give FreeSurfer's on every case but those where FreeSurfer's result is
undefined (``SKIP``) and those the port does not run yet (``is not ported yet``), which are
counted, not compared; ``fix`` may differ only where a defect it corrects changed the result (its
notices).

Each case's environment names ``FREESURFER_HOME`` a folder of the case with a small colour table
(``FSENVgetenv`` reads it; the licence file is ``FS_LICENSE``'s, read in place); ``@DIR@`` in an
assignment or an argument is the case's folder. The official side runs with ``FS_DISABLE_LANG=1``
(``docs/findings/freesurfer-info.md``) and its standard input from ``/dev/null``; no case sets a
diagnostic level (``DIAG_NO``, ``DIAG``, ``--diag``): FreeSurfer's licence check prints the
licence file's lines above 0.

    python -P validation/probes/fs-coreg/check.py --seed 1 --n 100 [--json out.json]
    python -P validation/probes/fs-coreg/check.py --golden
"""

from __future__ import annotations

import argparse
import hashlib
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

# FreeSurfer's result is undefined (memory contents, a crash's output, a run that never ends):
# FS-1313 (more than 10 --sep), FS-1314 (--threads without a number), FS-1316 (a number never
# read), FS-1318 (--lrrev without a left-right axis), FS-1319 (a brute force that never ends),
# FS-1320 (the percentile past the sorted values), FS-1321 (a sample outside a volume),
# FS-1322 (--dof above 12), FS-1323 (--sep 0 or less); the readers' and writers'
# (fs-robust-register's SKIP): FS-167, FS-58, FS-288, FS-290, FS-294, FS-441, FS-302 to FS-310
SKIP = (
    "FS-1313",
    "FS-1314",
    "FS-1316",
    "FS-1318",
    "FS-1319",
    "FS-1320",
    "FS-1321",
    "FS-1322",
    "FS-1323",
    "FS-167",
    "FS-58",
    "FS-288",
    "FS-290",
    "FS-294",
    "FS-441",
    *(f"FS-{k}" for k in range(302, 311)),
)
NOT_PORTED = "is not ported yet"
#: the times (both sides): Powell's, the whole run's (the head, the tail kept)
TIMES = (
    (re.compile(r"OptTimeSec +[0-9.]+ sec"), "OptTimeSec <T> sec"),
    (re.compile(r"OptTimeMin +[0-9.]+ min"), "OptTimeMin <T> min"),
    (re.compile(r"mri_coreg RunTimeSec +[0-9.]+ sec"), "mri_coreg RunTimeSec <T> sec"),
)
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_coreg_probe"
#: a timeout for one official run (the brute force on these volumes takes seconds)
TIMEOUT = 120
#: the port's notices of a case where the official program never ends (FS-1319: the brute force's
#: step does not advance; FS-1323: a negative separation): the official program is not run
NEVER_ENDS = ("FS-1319", "FS-1323")


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


def written(d: Path, inputs: list[str]) -> str:
    out = []
    for n, b in collect(d):
        if any(n == i or i.startswith(n + "/") for i in inputs):
            continue
        if b is None:
            out.append(f"D {n}")
            continue
        b = b.replace(str(d).encode(), b"<DIR>")
        if n.endswith((".lta", ".xfm", ".dat", ".txt")):
            out.append(f"F {n} {escape(b)}")
        else:
            out.append(f"F {n} {fnv(b):016x} {len(b)}")
    return " ".join(out)


def mask_machine(s: str) -> str:
    keys = ("sysname  ", "hostname ", "machine  ", "user     ")
    parts = s.split("\\x0a")
    for k, p in enumerate(parts):
        for key in keys:
            if p.startswith(key):
                parts[k] = key + "<M>"
    return "\\x0a".join(parts)


def mask(s: str) -> str:
    for t, rep in TIMES:
        s = t.sub(rep, s)
    return mask_machine(s)


def official_line(exe: Path, line: str, root: Path, k: int) -> str:
    t = line.split()
    assert t[0] == "coreg"
    d = (root / f"case-{k}").resolve()
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "larmorx", "TZ": "UTC0",
           "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
           **freesurfer_env()}  # fmt: skip
    if t[1] != "-":
        for a in bytes.fromhex(t[1]).decode().split(";"):
            if a:
                key, _, val = a.partition("=")
                env[key] = val.replace("@DIR@", str(d))
    # never a run whose licence check prints the licence file
    if any(x.startswith("DIAG") or x.lower() == "diag" for x in env):
        raise SystemExit(f"case {k}: a diagnostic setting would print the licence file")
    n = int(t[2])
    i = 3
    inputs = []
    names = []
    for _ in range(n):
        name, h = t[i], t[i + 1]
        p = d / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(bytes.fromhex(h))
        inputs.append(p)
        names.append(name)
        i += 2
    ndirs = int(t[i])
    for h in t[i + 1 : i + 1 + ndirs]:
        (d / bytes.fromhex(h).decode()).mkdir(parents=True, exist_ok=True)
        names.append(bytes.fromhex(h).decode())
    i += 1 + ndirs
    nargs = int(t[i])
    args = [bytes.fromhex(h).decode().replace("@DIR@", str(d)) for h in t[i + 1 : i + 1 + nargs]]
    if any("diag" in a.lower() for a in args):
        raise SystemExit(f"case {k}: a diagnostic option would print the licence file")
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1,
            family="freesurfer",
            formats="freesurfer",
            volatile=tuple(t.pattern for t, _ in TIMES),
        ),
        timeout=TIMEOUT,
        runner="fs-coreg-probe",
        inputs=inputs,
    )
    rc = done.returncode
    if rc is None:
        status = "TIMEOUT"
    else:
        status = f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = escape(str(d).encode())

    def m(s: str | bytes) -> str:
        b = s.encode("utf-8", "surrogateescape") if isinstance(s, str) else s
        return mask(escape(b).replace(dd, "<DIR>"))

    files = written(d, names)
    return f"{status} | {m(done.stdout)} | {m(done.stderr)} | {files}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument("--golden", action="store_true", help="the golden cases: their official SHA-256")
    ap.add_argument("--only", type=str, default="", help="comma-separated case numbers")
    ap.add_argument("--dump", type=Path, help="every case's official, reproduce and fix lines")
    a = ap.parse_args(argv)
    devnull = os.open(os.devnull, os.O_RDONLY)
    os.dup2(devnull, 0)
    os.close(devnull)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_coreg"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_coreg_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip
    inputs = ws / "oracles" / "inputs" / "fs-coreg"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "golden" if a.golden else f"{a.seed}-{a.n}"
    gen = ["golden-cases"] if a.golden else ["cases", str(a.seed), str(a.n)]
    text = subprocess.run([EXAMPLE, *gen], check=True, capture_output=True, text=True).stdout
    cases = text.splitlines()
    if a.only:
        keep = [int(x) for x in a.only.split(",")]
        cases = [cases[k] for k in keep]
        tag += "-only"
    cases_path = inputs / f"cases-{tag}.txt"
    cases_path.write_text("\n".join(cases) + "\n", encoding="utf-8")
    def port_lines(mode: str) -> list[tuple[str, list[str]]]:
        penv = dict(env, PROBE_JOBS=str(a.jobs))
        out = subprocess.run(
            [EXAMPLE, "run", cases_path, mode], check=True, capture_output=True, text=True,
            env=penv,
        ).stdout  # fmt: skip
        rows = []
        for line in out.splitlines():
            body, _, ids = line.partition("\t")
            rows.append((body, [x for x in ids.split(",") if x]))
        return rows

    rep, fix = port_lines("reproduce"), port_lines("fix")
    root = inputs / f"cases-{tag}.d"
    official = [
        "NOT RUN (the official program never ends)"
        if any(x in NEVER_ENDS for x in rep[k][1])
        else official_line(exe, c, root, k)
        for k, c in enumerate(cases)
    ]
    shutil.rmtree(root, ignore_errors=True)
    if a.golden:
        h = hashlib.sha256()
        for c in official:
            h.update(c.encode() + b"\n")
        print(f"golden OFFICIAL_SHA256 {h.hexdigest()} ({len(official)} cases)")
    if not len(official) == len(rep) == len(fix) == len(cases):
        raise SystemExit(
            f"line counts differ: official {len(official)}, reproduce {len(rep)}, fix {len(fix)}"
        )
    if a.dump:
        with a.dump.open("w", encoding="utf-8") as fh:
            for k, (c, (r, ids), (f, fids)) in enumerate(zip(official, rep, fix, strict=True)):
                fh.write(json.dumps({"k": k, "official": c, "reproduce": r, "ids": ids,
                                     "fix": f, "fix_ids": fids}) + "\n")  # fmt: skip
    counts: Counter[str] = Counter()
    differ = []
    fix_bad = []
    for k, (c, (r, ids), (f, fids)) in enumerate(zip(official, rep, fix, strict=True)):
        if NOT_PORTED in r:
            counts["not_ported"] += 1
            continue
        if any(x in SKIP for x in ids):
            counts["skipped"] += 1
            counts["skipped_but_identical"] += c == r
            continue
        if c == r:
            counts["reproduce_equal"] += 1
        else:
            counts["reproduce_differ"] += 1
            differ.append(k)
        if f != r:
            counts["fix_differ"] += 1
            if not fids:
                counts["fix_differ_without_notice"] += 1
                fix_bad.append(k)
    h = hashlib.sha256()
    for k, (c, (r, ids)) in enumerate(zip(official, rep, strict=True)):
        h.update(("SKIP" if any(x in SKIP for x in ids) or NOT_PORTED in r else c).encode())
        h.update(b"\n")
    summary = {
        "seed": a.seed,
        "family": "golden" if a.golden else "seeded",
        "cases": len(cases),
        **{k: counts.get(k, 0) for k in ("reproduce_equal", "reproduce_differ", "skipped",
                                          "skipped_but_identical", "not_ported", "fix_differ",
                                          "fix_differ_without_notice")},  # fmt: skip
        "official_sha256_skip": h.hexdigest(),
        "first_differ": differ[:20],
        "fix_differ_without_notice_cases": fix_bad[:20],
    }
    print(json.dumps(summary, indent=1))
    for k in differ[: a.show]:
        print(f"--- case {k}: {cases[k][:200]}")
        c, r = official[k], rep[k][0]
        j = next((i for i, (x, y) in enumerate(zip(c, r, strict=False)) if x != y), min(len(c), len(r)))
        print(f"    official:  ...{c[max(0, j - 200) : j + 300]}")
        print(f"    reproduce: ...{r[max(0, j - 200) : j + 300]}")
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
