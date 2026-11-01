# SPDX-License-Identifier: Apache-2.0
"""Runs FSL 6.0.7.23's program and (with ``--rep``) the replica on a list of argument lines, each
in a fresh directory with the given files written first; prints FSL's output, or (``--rep``) for
each call whether the exit status, standard output and error (the replica's ``larmorx-nc:``
lines left out) and files written are FSL's, and the first difference.

Usage: python -P probe.py <work dir> <cases.py> [--rep] [--quiet]
(``LARMORX_FSL_623`` FSL 6.0.7.23, ``~/fsl-6.0.7.23`` by default; ``LARMORX_NC_BIN`` the replica's program,
the repository's ``crates-nc/target/release/larmorx-nc`` by default; ``MODE=fix`` runs the
replica's default instead of ``reproduce``.) ``cases.py`` defines ``PROGRAM``, ``FILES`` (name ->
text or a path to copy) and ``CASES`` (argument lists, or ``(arguments, standard input)``).
"""

import gzip
import os
import runpy
import shutil
import subprocess
import sys
from pathlib import Path

# FSL 6.0.7.23 (not $FSLDIR, which may name another release)
FSL = os.environ.get("LARMORX_FSL_623", str(Path.home() / "fsl-6.0.7.23"))
NC = os.environ.get(
    "LARMORX_NC_BIN",
    str(Path(__file__).resolve().parents[3] / "crates-nc/target/release/larmorx-nc"),
)
work = Path(sys.argv[1])
spec = runpy.run_path(sys.argv[2])
rep = "--rep" in sys.argv
quiet = "--quiet" in sys.argv
env = {
    "HOME": os.environ.get("HOME", "/tmp"),
    "PATH": "/usr/bin:/bin",
    "FSLDIR": FSL,
    "FSLOUTPUTTYPE": "NIFTI_GZ",
    "OPENBLAS_CORETYPE": "Haswell",
}


def prep(d):
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    for name, v in spec["FILES"].items():
        if isinstance(v, Path):
            shutil.copy(v, d / name)
        else:
            (d / name).write_text(v)


def files(d):
    return {f.name: f.read_bytes() for f in d.iterdir() if f.name not in spec["FILES"]}


def norm(b, name):
    if name.endswith(".gz"):
        b = gzip.decompress(b)
    if name.endswith((".nii", ".nii.gz")):
        b = b[:148] + b[228:]
    return b


bad = 0
for i, args in enumerate(spec["CASES"]):
    stdin = None
    if isinstance(args, tuple):
        args, stdin = args
    prog = spec["PROGRAM"]
    df, dr = work / f"c{i}" / "fsl", work / f"c{i}" / "rep"
    prep(df)
    pf = subprocess.run(
        [f"{FSL}/bin/{prog}", *args],
        cwd=df,
        env=env,
        capture_output=True,
        input=stdin,
        text=True,
        errors="replace",
        timeout=600,
    )
    fo = pf.stdout.replace(f"{FSL}/bin/{prog}", prog)
    fe = pf.stderr.replace(f"{FSL}/bin/{prog}", prog)
    line = f"=== [{i}] {prog} {' '.join(args)} -> exit {pf.returncode}"
    if not rep:
        print(line)
        print(fo, end="")
        print("--- stderr:", fe, end="")
        print("files:", sorted(files(df)))
        continue
    prep(dr)
    pr = subprocess.run(
        [NC, "fsl", prog, *args],
        cwd=dr,
        env={**env, "LARMORX_UPSTREAM_DEFECTS": os.environ.get("MODE", "reproduce")},
        capture_output=True,
        text=True,
        errors="replace",
        timeout=600,
        input=stdin,
    )
    re_ = "\n".join(ln for ln in pr.stderr.split("\n") if not ln.startswith("larmorx-nc:"))
    code_ok = pf.returncode == pr.returncode or (pf.returncode < 0 and pr.returncode == 1)
    out_ok = fo == pr.stdout
    err_ok = fe == re_ or (pf.returncode < 0 and re_.startswith(fe.rstrip("\n")))
    ff, fr = files(df), files(dr)
    files_ok = set(ff) == set(fr) and all(norm(ff[n], n) == norm(fr[n], n) for n in ff if n in fr)
    ok = code_ok and out_ok and err_ok and files_ok
    bad += not ok
    if ok and quiet:
        continue
    print(line, "OK" if ok else "DIFFER", f"(replica exit {pr.returncode})")
    if not out_ok:
        a, b = fo.splitlines(), pr.stdout.splitlines()
        for k in range(max(len(a), len(b))):
            x = a[k] if k < len(a) else "<end>"
            y = b[k] if k < len(b) else "<end>"
            if x != y:
                print(f"  stdout line {k + 1}: FSL {x!r}\n                  REP {y!r}")
                break
    if not err_ok:
        print("  FSL stderr:", repr(fe[-400:]))
        print("  REP stderr:", repr(pr.stderr[-400:]))
    if not files_ok:
        print(
            "  files",
            sorted(ff),
            sorted(fr),
            [n for n in ff if n in fr and norm(ff[n], n) != norm(fr[n], n)],
        )
if rep:
    print(f"{len(spec['CASES'])} cases, {bad} differ")
