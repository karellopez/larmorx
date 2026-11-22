# SPDX-License-Identifier: Apache-2.0
"""fs_temp_file against FreeSurfer 8.2.0's official program, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example and names its defect
handling).

Takes the cases of larmorx-freesurfer-volumes' example ``fs_tempfile_probe``
(``crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_tempfile/cases.rs``: every option, the
five variables of the temporary folder in their order, folders that do not exist, cannot be
written or are files, suffixes that make no file, the parser's messages, the help and version
options), runs the official ``fs_temp_file`` (``$FREESURFER_HOME/bin``, by name) on each case in a
folder of its own through the oracle-output cache (never stored: the name holds six random
letters), runs the port on the same cases with ``--upstream-defects`` ``reproduce`` and ``fix``,
and compares the lines (exit status, standard output, standard error, every file and folder after
the run, a file made in ``/tmp``; the case's folder as ``<DIR>``, the six letters as ``XXXXXX``,
``--all-info``'s time, user and machine as ``*``): ``reproduce`` must give FreeSurfer's on every
case; ``fix`` may differ only where a defect it corrects changed the result (its notices).
Prints the hashes the port's recorded test checks (``tests/fs_temp_file_official.rs``): Unix's,
and the one for systems without Unix's modes (every mode as ``-``, the cases only Unix gives the
same way as ``UNIX-ONLY``).

The official side runs with ``FS_DISABLE_LANG=1`` and without ``DIAG_NO`` (FreeSurfer's licence
check would print the licence file). The machine must have no ``/scratch`` (``--scratch``'s cases).

    python -P validation/probes/fs-temp-file/check.py [--golden] [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
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

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_tempfile_probe"
NEVER = "fs_temp_file makes a name of six random letters (mkstemps): no two runs are alike"


def escape(b: bytes) -> str:
    return "".join(chr(c) if 0x20 <= c <= 0x7E and c != 0x5C else f"\\x{c:02x}" for c in b)


def unhex(s: str) -> str:
    return "" if s == "@" else bytes.fromhex(s).decode()


def mask_letters(s: str) -> str:
    return re.sub(r"tmp\.[A-Za-z0-9]{6}", "tmp.XXXXXX", s)


def mask_info(s: str) -> str:
    return re.sub(r"(TimeStamp|User|Machine|PlatformVersion): ([^ ]+)", r"\1: *", s)


def mode_of(st: os.stat_result, modes: bool) -> str:
    return f"{st.st_mode & 0o777:o}" if modes else "-"


def listing(d: Path, prefix: str, modes: bool) -> list[str]:
    out = []
    for e in sorted(d.iterdir(), key=lambda p: p.name.encode("utf-8", "surrogateescape")):
        name = prefix + e.name
        st = e.lstat()
        if stat.S_ISLNK(st.st_mode):
            out.append(f"L {name}")
        elif stat.S_ISDIR(st.st_mode):
            out.append(f"D {name} {mode_of(st, modes)}")
            out.extend(listing(e, name + "/", modes))
        else:
            out.append(f"F {name} {mode_of(st, modes)} {st.st_size}")
    return out


def setup(line: str, d: Path) -> tuple[list[str], dict[str, str], bool]:
    t = line.split(" ")
    assert t[0] == "tmpf", line
    place = lambda s: unhex(s).replace("<D>", str(d))  # noqa: E731
    i = 2

    def items() -> list[str]:
        nonlocal i
        n = int(t[i])
        v = [unhex(x) for x in t[i + 1 : i + 1 + n]]
        i += 1 + n
        return v

    for name in items():
        (d / name).mkdir(parents=True, exist_ok=True)
        os.chmod(d / name, 0o755)
    for name in items():
        (d / name).write_bytes(b"")
        os.chmod(d / name, 0o644)
    read_only = items()
    nlinks = int(t[i])
    for k in range(nlinks):
        os.symlink(place(t[i + 2 + 2 * k]), d / unhex(t[i + 1 + 2 * k]))
    i += 1 + 2 * nlinks
    nenv = int(t[i])
    env = dict(place(x).split("=", 1) for x in t[i + 1 : i + 1 + nenv])
    i += 1 + nenv
    nargs = int(t[i])
    args = [place(x) for x in t[i + 1 : i + 1 + nargs]]
    for name in read_only:
        os.chmod(d / name, 0o555)
    return args, env, t[1] == "u"


def official_lines(exe: Path, line: str, d: Path) -> tuple[str, str]:
    """The official program's line of one case in the folder ``d``: Unix's, and without modes."""
    if d.exists():
        for p in d.rglob("*"):
            if p.is_dir() and not p.is_symlink():
                os.chmod(p, 0o755)
        shutil.rmtree(d)
    d.mkdir(parents=True)
    args, case_env, unix = setup(line, d)
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", **freesurfer_env(), **case_env}  # fmt: skip
    if any(x.startswith("DIAG_NO") or x.lower() == "diag" for x in env):
        raise SystemExit("a case sets DIAG_NO: the licence check would print the licence file")
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="freesurfer", never=NEVER),
        timeout=15,
        runner="fs-temp-file-probe",
    )
    rc = done.returncode
    status = "TIMEOUT" if rc is None else f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = escape(str(d).encode())

    def mask(b: bytes) -> str:
        return mask_info(mask_letters(escape(b).replace(dd, "<DIR>")))

    ro = []
    for p in sorted(d.iterdir(), key=lambda p: p.name.encode()):
        if p.is_dir() and not p.is_symlink() and (p.stat().st_mode & 0o777) == 0o555:
            os.chmod(p, 0o755)
            ro.append(f"RO {p.name}")
    lines = done.stdout_bytes.decode("utf-8", "surrogateescape").split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    last = lines[-1].removesuffix("\r") if lines else ""
    outside = []
    if status == "EXIT 0" and not last.startswith(str(d)) and os.path.isabs(last):
        try:
            st = os.lstat(last)
        except OSError:
            st = None
        if st is not None:
            outside = [(mask_letters(escape(last.encode("utf-8", "surrogateescape"))), st)]
            os.remove(last)
    result = []
    for modes in (True, False):
        files = [*ro, *listing(d, "", modes)]
        files += [f"X {n} {mode_of(st, modes)} {st.st_size}" for n, st in outside]
        result.append(
            f"{status} | {mask(done.stdout_bytes)} | {mask(done.stderr_bytes)} | "
            f"{mask_letters(' '.join(files))}"
        )
    if unix:
        result[1] = "UNIX-ONLY"
    return result[0], result[1]


def sha(lines: list[str]) -> str:
    h = hashlib.sha256()
    for x in lines:
        h.update(x.encode() + b"\n")
    return h.hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument("--golden", action="store_true", help="the golden cases: their official hash")
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    if Path("/scratch").exists():
        raise SystemExit("/scratch exists on this machine: --scratch's cases assume it does not")
    exe = ws / FS_HOME / "bin" / "fs_temp_file"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_tempfile_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip
    inputs = ws / "oracles" / "inputs" / "fs-temp-file"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "golden" if a.golden else "all"
    cases_path = inputs / f"cases-{tag}.txt"
    text = subprocess.run(
        [EXAMPLE, "golden-cases" if a.golden else "cases"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    root = inputs / f"cases-{tag}.d"
    official = [official_lines(exe, c, root / str(k)) for k, c in enumerate(cases)]
    for p in root.rglob("*"):
        if p.is_dir() and not p.is_symlink():
            os.chmod(p, 0o755)
    shutil.rmtree(root)
    unix_lines = [u for u, _ in official]
    other_lines = [o for _, o in official]

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
        raise SystemExit(f"line counts differ: {len(official)}, {len(rep)}, {len(fix)}")
    equal = differ = 0
    fix_differ = fix_unexplained = 0
    fix_ids, rep_ids = Counter(), Counter()
    shown = 0
    for k, (o, (r, rids), (f, fids)) in enumerate(zip(unix_lines, rep, fix, strict=True)):
        rep_ids.update(set(rids))
        fix_ids.update(set(fids))
        if r == o:
            equal += 1
        else:
            differ += 1
            if shown < a.show:
                shown += 1
                print(f"case {k} DIFFERS\n  official  {o[:600]}\n  reproduce {r[:600]}")
        if f != r:
            fix_differ += 1
            if not fids:
                fix_unexplained += 1
                print(f"case {k}: fix differs without a notice\n  fix {f[:600]}")
    summary = {
        "cases": len(cases),
        "identical": equal,
        "different": differ,
        "fix_differs": fix_differ,
        "fix_unexplained": fix_unexplained,
        "reproduce_notices": dict(rep_ids),
        "fix_notices": dict(fix_ids),
        "unix_only": sum(x == "UNIX-ONLY" for x in other_lines),
        "OFFICIAL_SHA256": sha(unix_lines),
        "OFFICIAL_SHA256_NO_MODES": sha(other_lines),
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
