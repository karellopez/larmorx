# SPDX-License-Identifier: Apache-2.0
"""t2smap's command line: larmorx-gpl's parser against tedana 26.0.3's, case by case.

tedana's parser (``tedana.workflows.t2smap._get_parser``) runs in-process with ``COLUMNS`` set per
case (argparse reads the width each time it formats); the replica's ``t2smap_parse`` example runs
as a program. Compared: the exit status (or the parsed values in Python's repr), standard output
and standard error, exactly. Usage:

    cargo build --release --manifest-path crates-gpl/Cargo.toml -p larmorx-gpl-tedana \\
        --example t2smap_parse
    <env>/bin/python -P validation/investigations/t2smap/replica/check_parser.py \\
        --parse <target>/release/examples/t2smap_parse --work <empty dir>
"""

from __future__ import annotations

import argparse
import contextlib
import io
import os
import subprocess
import sys
from pathlib import Path


def cases(f1: str, f2: str) -> list[tuple[str | None, list[str]]]:
    ok = ["-d", f1, f2, "-e", "0.015", "0.039"]
    out: list[tuple[str | None, list[str]]] = []
    for cols in [
        None,
        "20",
        "30",
        "40",
        "52",
        "60",
        "70",
        "79",
        "80",
        "81",
        "100",
        "120",
        "150",
        "200",
        "0",
        "-5",
        "abc",
        "1_00",
        " 90 ",
    ]:
        out.append((cols, ["-h"]))
        out.append((cols, []))
    for cols in [None, "11", "14", "40"]:
        out.append((cols, ["-v"]))
    argvs = [
        ["--help"],
        ["--he"],
        ["--version"],
        ["--vers"],
        ["-fv"],
        ["-h", "--bogus"],
        ["--bogus", "-h"],
        ["-d"],
        ["-d", "missing.nii"],
        ["-d", f1, "-e"],
        ["-d", f1, "-e", "x"],
        [*ok, "--fittype", "bad"],
        [*ok, "--fit", "loglin"],
        [*ok, "--fit=loglin"],
        [*ok, "--m", "x"],
        [*ok, "--n", "1"],
        [*ok, "--v"],
        [*ok, "--bogus"],
        [*ok, "-x"],
        [*ok, "extra"],
        [*ok, "--", "x"],
        ["-d", f1, "-e", "-1e-3"],
        ["-d", f1, "-e", "-0.5", "2"],
        [*ok, "--overwrite=1"],
        [*ok, "-f=1"],
        [*ok, "-fx"],
        [*ok, "-ff"],
        ["-d=" + f1, "-e", "1"],
        ["-d" + f1, "-e", "1"],
        [*ok, "--dummy-scans", "1.5"],
        [*ok, "--dummy-scans", "1_0"],
        [*ok, "--n-threads", " 3 "],
        [*ok, "--masktype", "decay", "dropout", "bad"],
        [*ok, "--masktype", "none"],
        [*ok, "--convention="],
        [*ok, "--convention=orig"],
        [*ok, "--prefix", "-x"],
        [*ok, "--prefix=-x"],
        [*ok, "--prefix", "sub_"],
        [*ok, "--exclude", "1:3"],
        [*ok, "--quiet"],
        [*ok, "--q"],
        ["-d", f1, "-d", f2, "-e", "1"],
        [*ok, "--mask", "missing"],
        [*ok, "--mask"],
        [*ok, "--mask", f1],
        ["-e", "inf", "nan", "-d", f1],
        ["-d", "-"],
        ["-d", ""],
        [*ok, "--out-dir"],
        [*ok, "--out-dir", "o", "--out-dir", "p"],
        [*ok, "--n-ind", "4"],
        [*ok, "--n-independent-echos", "x"],
        [*ok, "--verbose", "--debug", "--overwrite"],
        [*ok, "-e", "1", "2", "3"],
        [*ok, "--fitmode", "ts", "--combmode", "paid"],
        [*ok, "--combmode"],
        ["-e", "0.1", "0.2"],
        ["-d", f1, f2],
        [*ok, "-"],
        [*ok, "--dummy-scans", "-2"],
        [*ok, "--n-threads", "-1"],
        ["-dfoo"],
        ["--"],
        ["-d", f1, "--", "-e", "1"],
        [*ok, "--fittype"],
        ["-e", "1e5", "1E-3", "+2", " 3", "1_5.0", "-d", f1],
    ]
    out += [(None, a) for a in argvs]
    return out


def tedana_side(cols: str | None, argv: list[str]) -> str:
    from tedana.workflows.t2smap import _get_parser

    if cols is None:
        os.environ.pop("COLUMNS", None)
    else:
        os.environ["COLUMNS"] = cols
    o, e = io.StringIO(), io.StringIO()
    try:
        with contextlib.redirect_stdout(o), contextlib.redirect_stderr(e):
            ns = _get_parser().parse_args(argv)
    except SystemExit as x:
        return f"STATUS {x.code if x.code is not None else 0}\nSTDOUT\n{o.getvalue()}STDERR\n{e.getvalue()}"
    v = vars(ns)
    keys = [
        "data",
        "tes",
        "out_dir",
        "prefix",
        "convention",
        "verbose",
        "overwrite",
        "mask",
        "masktype",
        "dummy_scans",
        "exclude",
        "fittype",
        "fitmode",
        "combmode",
        "n_independent_echos",
        "n_threads",
        "debug",
        "quiet",
    ]
    return "STATUS ok\nSTDOUT\nSTDERR\n" + "".join(f"{k}={v[k]!r}\n" for k in keys)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--parse", required=True)
    ap.add_argument("--work", required=True, type=Path)
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    os.chdir(a.work)
    Path("e1.nii.gz").write_bytes(b"")
    Path("e2.nii.gz").write_bytes(b"")
    sys.argv[0] = "t2smap"
    bad = 0
    all_cases = cases("e1.nii.gz", "e2.nii.gz")
    for cols, argv in all_cases:
        theirs = tedana_side(cols, argv)
        env = dict(os.environ)
        env.pop("COLUMNS", None)
        if cols is not None:
            env["COLUMNS"] = cols
        ours = subprocess.run([a.parse, *argv], env=env, capture_output=True, text=True).stdout
        if ours != theirs:
            bad += 1
            print(f"DIFF COLUMNS={cols} {argv}\n--- tedana\n{theirs}--- replica\n{ours}")
    print(f"{len(all_cases)} cases, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
