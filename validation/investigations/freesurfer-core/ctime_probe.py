# SPDX-License-Identifier: Apache-2.0
"""Compare larmorx-freesurfer-core's ``ctime::localtime`` + ``strftime("%c")`` with the C
library's (CPython's ``time.localtime`` and ``time.strftime`` call glibc's ``localtime_r`` and
``strftime``), over time zones and times (README.md beside this file).

Usage: python -P ctime_probe.py <probe binary> [--cases N]

The probe is the example ``ctime_probe`` of ``crates-freesurfer/larmorx-freesurfer-core``
(``cargo build --release --example ctime_probe``). Prints the counts and the first differences.
"""

import argparse
import os
import random
import subprocess
import time

ZONES = [
    "-",  # TZ unset: /etc/localtime
    "",
    "UTC0",
    "America/New_York",
    ":Europe/London",
    "Europe/Berlin",
    "Europe/Dublin",
    "Australia/Sydney",
    "Australia/Lord_Howe",
    "Asia/Kolkata",
    "Asia/Tehran",
    "America/Sao_Paulo",
    "Pacific/Chatham",
    "Pacific/Apia",
    "Africa/Casablanca",
    "Antarctica/Troll",
    "America/St_Johns",
    "EST5EDT",
    "EST5EDT,M3.2.0,M11.1.0",
    "AEST-10AEDT,M10.1.0,M4.1.0/3",
    "<+0530>-5:30",
    "CET-1CEST,M3.5.0,M10.5.0/3",
    "garbage",
    "Nowhere/Nothing",
]


def glibc(tz: str, t: int) -> str:
    if tz == "-":
        os.environ.pop("TZ", None)
    else:
        os.environ["TZ"] = tz
    time.tzset()
    return time.strftime("%c", time.localtime(t))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("probe")
    ap.add_argument("--cases", type=int, default=2000)
    a = ap.parse_args()
    rng = random.Random(20261112)
    cases = []
    for tz in ZONES:
        times = [0, -1, 2**31 - 1, 2**31, 1_773_000_000, 2_000_000_000, 4_102_444_800]
        times += [rng.randrange(-2_208_988_800, 4_102_444_800) for _ in range(a.cases)]
        # Around each hour of two years of transitions.
        base = rng.randrange(1_700_000_000, 1_900_000_000)
        times += [base + k * 3600 + rng.choice((-1, 0, 1)) for k in range(0, 17520, 7)]
        cases += [(tz, t) for t in times]
    lines = "".join(f"{'=' + tz if tz != '-' else '-'} {t}\n" for tz, t in cases)
    out = subprocess.run(
        [a.probe], input=lines, capture_output=True, text=True, check=True
    ).stdout.splitlines()
    differ = [(tz, t, o, glibc(tz, t)) for (tz, t), o in zip(cases, out, strict=True)]
    differ = [d for d in differ if d[2] != d[3]]
    print(f"cases {len(cases)}, zones {len(ZONES)}, differ {len(differ)}")
    by_zone: dict[str, int] = {}
    for d in differ:
        by_zone[d[0]] = by_zone.get(d[0], 0) + 1
    for z, n in sorted(by_zone.items()):
        print(f"  {z!r}: {n}")
    for d in differ[:10]:
        print("  ", d)


if __name__ == "__main__":
    main()
