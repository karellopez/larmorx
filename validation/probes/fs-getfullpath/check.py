# SPDX-License-Identifier: Apache-2.0
"""getfullpath against FreeSurfer 8.2.0's official script, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example and names its defect
handling).

Takes the cases of larmorx-freesurfer-volumes' example ``fs_getfullpath_probe``
(``crates-freesurfer/larmorx-freesurfer-volumes/tests/fs_getfullpath/cases.rs``: the scripts'
calls, then every way tcsh reads a name again: blanks, patterns, braces, ``~``, ``=N``, ``+N``
folders, leading dashes, backslashes, folders whose paths hold blanks or patterns), runs the
official ``getfullpath`` (``$FREESURFER_HOME/bin``, by name: ``/bin/tcsh`` with GNU coreutils, the
C locale) on each case in a folder of its own holding the example's fixture, through the
oracle-output cache (never stored: tcsh's patterns read the folder's whole listing, which the key
does not hold), runs the port on the same cases with ``--upstream-defects`` ``reproduce`` and
``fix``, and compares the lines (exit status, standard output, standard error; the case's folder
as ``<DIR>``): ``reproduce`` must give FreeSurfer's on every case but those whose tcsh form the
port's model does not reproduce (``UNMODELLED``, counted); ``fix`` may differ only with a notice.
Prints the hashes of the recorded test (``tests/getfullpath_official.rs``): every case's on Unix (a
case the model does not reproduce as ``UNMODELLED``), the plain cases' on every system. ``--seed
<s> --n <n>``: seeded random names made of the pieces tcsh reads again, instead of the fixed cases.

    python -P validation/probes/fs-getfullpath/check.py [--golden] [--json out.json]
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
from larmorx_validation.freesurfer_unit import FS_HOME  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_getfullpath_probe"
NEVER = "tcsh's patterns read the folder's whole listing, which the key does not hold"


def escape(b: bytes) -> str:
    return "".join(chr(c) if 0x20 <= c <= 0x7E and c != 0x5C else f"\\x{c:02x}" for c in b)


def unhex(s: str) -> str:
    return "" if s == "@" else bytes.fromhex(s).decode()


def make_fixture(d: Path, entries: list[str]) -> None:
    for name in entries:
        if name == "lnk":
            os.symlink("real", d / "lnk")
        elif name.endswith("/"):
            (d / name[:-1]).mkdir(parents=True, exist_ok=True)
        else:
            (d / name).write_bytes(b"")
    if (d / "noperm").is_dir():
        os.chmod(d / "noperm", 0o000)


def remove(d: Path) -> None:
    if (d / "noperm").is_dir():
        os.chmod(d / "noperm", 0o755)
    shutil.rmtree(d)


def official_line(exe: Path, line: str, d: Path, entries: list[str]) -> str:
    if d.exists():
        remove(d)
    d.mkdir(parents=True)
    make_fixture(d, entries)
    t = line.split(" ")
    assert t[0] == "gfp", line
    place = lambda s: unhex(s).replace("<D>", str(d))  # noqa: E731
    cwd = d / unhex(t[2])
    nenv = int(t[3])
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C"}
    env |= dict(place(x).split("=", 1) for x in t[4 : 4 + nenv])
    i = 4 + nenv
    args = [place(x) for x in t[i + 1 : i + 1 + int(t[i])]]
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=cwd,
        policy=oracle_cache.Policy(threads=1, family="freesurfer", never=NEVER),
        timeout=15,
        runner="fs-getfullpath-probe",
    )
    rc = done.returncode
    status = "TIMEOUT" if rc is None else f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    masks = [escape(str(d.resolve()).encode()), escape(str(d).encode())]

    def mask(b: bytes) -> str:
        s = escape(b)
        for m in masks:
            s = s.replace(m, "<DIR>")
        return s

    remove(d)
    return f"{status} | {mask(done.stdout_bytes)} | {mask(done.stderr_bytes)}"


def sha(lines: list[str]) -> str:
    h = hashlib.sha256()
    for x in lines:
        h.update(x.encode() + b"\n")
    return h.hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=8, help="differing cases to print")
    ap.add_argument("--golden", action="store_true", help="the golden cases: their official hash")
    ap.add_argument("--seed", type=int, help="seeded random names instead of the fixed cases")
    ap.add_argument("--n", type=int, default=300)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "getfullpath"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_getfullpath_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip

    def example(*words: str) -> str:
        return subprocess.run([EXAMPLE, *words], check=True, capture_output=True, text=True).stdout

    entries = [unhex(x.split(" ")[1]) for x in example("fixture").splitlines()]
    inputs = ws / "oracles" / "inputs" / "fs-getfullpath"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "golden" if a.golden else f"{a.seed}-{a.n}" if a.seed is not None else "all"
    cases_path = inputs / f"cases-{tag}.txt"
    if a.golden:
        text = example("golden-cases")
    elif a.seed is not None:
        text = example("cases", str(a.seed), str(a.n))
    else:
        text = example("cases")
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    root = inputs / f"cases-{tag}.d"
    official = [official_line(exe, c, root / str(k), entries) for k, c in enumerate(cases)]
    shutil.rmtree(root, ignore_errors=True)

    def port_lines(mode: str) -> list[tuple[str, list[str]]]:
        rows = []
        for line in example("run", str(cases_path), mode).splitlines():
            body, _, ids = line.partition("\t")
            rows.append((body, [x for x in ids.split(",") if x]))
        return rows

    rep, fix = port_lines("reproduce"), port_lines("fix")
    if not len(official) == len(rep) == len(fix) == len(cases):
        raise SystemExit(f"line counts differ: {len(official)}, {len(rep)}, {len(fix)}")
    equal = differ = unmodelled = 0
    fix_differ = fix_unexplained = 0
    rep_ids, plain_differ = Counter(), 0
    shown = 0
    for k, (c, o, (r, rids), (f, fids)) in enumerate(zip(cases, official, rep, fix, strict=True)):
        rep_ids.update(set(rids))
        is_plain = c.split(" ")[1] == "-"
        if "UNMODELLED" in rids:
            unmodelled += 1
            if shown < a.show:
                shown += 1
                print(f"case {k} UNMODELLED\n  official  {o[:400]}\n  port      {r[:400]}")
        elif r == o:
            equal += 1
        else:
            differ += 1
            if shown < a.show:
                shown += 1
                f = c.split(" ")
                shown_args = [unhex(x) for x in f[5 + int(f[3]) :]]
                where = unhex(c.split(" ")[2])
                print(f"case {k} DIFFERS {shown_args!r} in {where!r}\n  official  {o[:400]}\n"
                      f"  reproduce {r[:400]}")  # fmt: skip
        if is_plain and (f != o or r != o):
            plain_differ += 1
            print(f"case {k}: a plain case differs\n  fix {f[:400]}")
        if f != r:
            fix_differ += 1
            if not fids:
                fix_unexplained += 1
                print(f"case {k}: fix differs without a notice\n  fix {f[:400]}")
    plain = [o for c, o in zip(cases, official, strict=True) if c.split(" ")[1] == "-"]
    summary = {
        "cases": len(cases),
        "plain": len(plain),
        "identical": equal,
        "different": differ,
        "unmodelled": unmodelled,
        "plain_different": plain_differ,
        "fix_differs": fix_differ,
        "fix_unexplained": fix_unexplained,
        "reproduce_notices": dict(rep_ids),
        # the recorded test hashes a case the model does not reproduce as UNMODELLED
        "OFFICIAL_SHA256": sha(
            [
                "UNMODELLED" if "UNMODELLED" in rids else o
                for o, (_, rids) in zip(official, rep, strict=True)
            ]
        ),
        "OFFICIAL_SHA256_PLAIN": sha(plain),
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained or plain_differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
