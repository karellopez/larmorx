# SPDX-License-Identifier: Apache-2.0
"""The C library's reading of ``TZ`` beyond the probe's ``time`` group, as a black box.

Runs ``fs_core_probe batch`` (the C library's ``gmtime_r``, ``localtime_r`` and ``strftime``)
and the clean room's candidate on the same scans of times under more ``TZ`` and ``TZDIR``
values than ``compare.py``'s 24 zones (strings without rules, offsets out of range, Julian
and zero-based days, change times beyond a day, ``posixrules`` from another zone, leap
seconds, lookups) and prints, per case, whether the two agree. The cases marked ``partial``
are malformed rules for which glibc keeps a partly filled rule that the clean room does not
follow (``crates/larmorx-fs-core/src/clib/time.rs``): they are expected to differ.

    PYTHONPATH=validation/src scripts/orchestration/boxed.sh 2 python -P \\
        validation/investigations/freesurfer-cleanroom-core/tz_cases.py \\
        --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe \\
        --candidate <the candidate> --scratch <scratch>/tz

The probe's runs go through the oracle-output cache with a ``never`` policy (they read the
system's time-zone files); exit status 1 when an unexpected case differs.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ZONES = Path("/usr/share/zoneinfo")

# (TZ, uses the scratch TZDIR with Paris as posixrules, expected to differ)
CASES: list[tuple[str, bool, bool]] = [
    ("ABC5DEF", False, False),
    ("ABC5DEF,", False, False),
    ("ABC3DEF", False, False),
    ("ABC5DEF6", False, False),
    ("ABC3DEF2:30", False, False),
    ("ABC8DEF", False, False),
    ("<AB1>5<CD2>", False, False),
    ("EST5EDTx", False, False),
    ("ABC5DEF", True, False),
    ("ABC5DEF6", True, False),
    ("ABC2DEF1", True, False),
    ("Tok", True, False),
    ("Europe/Paris", True, False),
    ("Empty", True, False),
    (":", False, False),
    ("::UTC", False, False),
    ("UTC", False, False),
    ("Foo", False, False),
    ("Foo5", False, False),
    ("FOO+25", False, False),
    ("FOO-24:59:59", False, False),
    ("FOO24:00:01", False, False),
    ("AB5", False, False),
    ("<A>5", False, False),
    ("XYZ3:30", False, False),
    ("EST5:75", False, False),
    ("EST5:30:75", False, False),
    ("EST5EDT4,J60,J300", False, False),
    ("EST5EDT4,59,299", False, False),
    ("EST5EDT,J1,J365", False, False),
    ("EST5EDT,0,365", False, False),
    ("EST5EDT,J60/0,J59/0", False, False),
    ("EST5EDT,59/0,58/0", False, False),
    ("EST5EDT,J365/25,J1", False, False),
    ("EST5EDT,M3.2.0/-1,M11.1.0/26", False, False),
    ("EST5EDT,M3.2.0/167,M11.1.0/168", False, False),
    ("EST5EDT,M3.2.0/2:30:15,M11.1.0/-2:30", False, False),
    ("EST5EDT,M3.5.0,M11.5.6", False, False),
    ("EST5EDT,M3.2.0", False, False),
    ("EST5EDT,M4.1.0", False, False),
    ("EST5EDT,M3.2.0,M3.2.0", False, False),
    ("EST5EDT,M3.2.0,M11.1.0/2x", False, False),
    ("../zoneinfo/UTC", False, False),
    ("Europe/../Europe/Paris", False, False),
    ("right/UTC", False, False),
    ("right/Europe/Paris", False, False),
    (":/nonexistent", False, False),
    ("/usr/share/zoneinfo", False, False),
    ("Europe", False, False),
    ("Etc/GMT+5", False, False),
    # malformed rules: glibc's partly filled rule, not followed
    ("Foo5/", False, True),
    ("EST5EDT,Mx", False, True),
    ("EST5EDT,,M10.1.0", False, True),
    ("EST5EDT,M3.2.0,M11.1.0x", False, True),
    ("EST5EDT,M13.1.0,M11.1.0", False, True),
    ("EST5EDT,M3.6.0,M11.1.0", False, True),
    ("EST5EDT,M3.2.7,M11.1.0", False, True),
    ("EST5EDT,M3.2.0/x,M11.1.0", False, True),
]


def times() -> list[int]:
    t = list(range(1672531200, 1704067200, 1800))  # 2023, every half hour
    t += list(range(2140650000, 2140690000, 1800))  # posixrules' last transition, 2037
    t += list(range(-1689850000, -1689780000, 900))  # Paris's 1916 changes (std indicators)
    t += list(range(-1680440000, -1680380000, 900))
    t += list(range(-942050000, -941990000, 900))  # 1940 (wall indicator)
    t += list(range(-766670000, -766610000, 900))  # 1945
    t += [637934400, 953553600, -1000000000, 0, 4118083200, 2539987200, 2147483647]
    t += [1483228825, 1483228826, 1483228827, 1483228800]  # the leap second of 2016
    return t


def batch_file(scratch: Path) -> Path:
    lines = []
    for t in times():
        lines += [f"gmtime {t}", f"localtime {t}", f"strftimec {t}"]
    p = scratch / "tz-batch.txt"
    p.write_text("\n".join(lines) + "\n")
    return p


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--probe", required=True, type=Path)
    ap.add_argument("--candidate", required=True, type=Path)
    ap.add_argument("--scratch", required=True, type=Path)
    a = ap.parse_args()
    a.scratch.mkdir(parents=True, exist_ok=True)
    zd = a.scratch / "zd"
    zd.mkdir(exist_ok=True)
    shutil.copyfile(ZONES / "Europe/Paris", zd / "posixrules")
    shutil.copyfile(ZONES / "Asia/Tokyo", zd / "Tok")
    (zd / "Empty").write_bytes(b"")
    batch = batch_file(a.scratch)
    try:
        from larmorx_validation import oracle_cache
    except ImportError:
        oracle_cache = None
    bad = 0
    for tz, own_dir, partial in CASES:
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "TZ": tz}
        if own_dir:
            env["TZDIR"] = str(zd)
        argv = ["batch", str(batch)]
        if oracle_cache is not None:
            pol = oracle_cache.Policy(
                threads=1, family="freesurfer", never="reads the system's time-zone files"
            )
            r = oracle_cache.run(
                [str(a.probe), *argv],
                env=env,
                cwd=a.scratch,
                policy=pol,
                runner="freesurfer-cleanroom-tz",
            )
            want = r.stdout_bytes
        else:
            want = subprocess.run([str(a.probe), *argv], env=env, capture_output=True).stdout
        cenv = dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce")
        got = subprocess.run([str(a.candidate), *argv], env=cenv, capture_output=True).stdout
        same = want == got
        n = sum(1 for x, y in zip(want.split(b"\n"), got.split(b"\n"), strict=False) if x != y)
        tag = (
            "identical" if same else ("differs (partial rule, expected)" if partial else "DIFFERS")
        )
        label = f"TZ={tz!r}" + (" TZDIR=<posixrules=Paris>" if own_dir else "")
        print(f"{label:52s} {tag}{'' if same else f' ({n} of {len(times()) * 3} results)'}")
        if not same and not partial:
            bad += 1
    print(f"cases {len(CASES)}, unexpected differences {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
