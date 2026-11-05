# SPDX-License-Identifier: Apache-2.0
"""dc_probe.py: GNU dc's (this machine's ``dc``, 1.4.1) prints of ``bet -S``'s eye threshold,
``10 k <mean> 3.5 <sd> * + p``, both streams apart: the expected values of the unit tests of
``larmorx_fsl::cli::bet_script`` (FSL-216, ``specs/bet.md`` §5.3 step 9). A black box: dc's
scales, its truncated products, ``.5`` without a leading zero, zero as ``0``, ``-`` for a negative
value, and a printed minus sign read as dc's subtraction (``dc: stack empty`` on standard error)."""

import subprocess

CASES = [
    ("523.418", "81.2345"),
    ("12.5", ".00000000001"),
    ("12.5", ".0000000001"),
    ("12.5", ".000000001"),
    ("1.000000", "2.000000"),
    ("0.5", "0"),
    ("0.000000", "0.000000"),
    ("0.000000", ".000001"),
    ("1.000000", "0.000000"),
    ("100", "0"),
    ("_5.000000", "1.000000"),
    ("_1.5", "1"),
    ("12345678901234567890.123456", "98765432109876543210.654321"),
    ("1951833.117037", "463525.237060"),
    ("-5.2", "81.2345"),
    ("-0.000000", "1.000000"),
    ("2.1e+06", "1"),
]

for mean, sd in CASES:
    r = subprocess.run(
        ["dc"], input=f"10 k {mean} 3.5 {sd} * + p\n", capture_output=True, text=True, check=False
    )
    print(f"{mean:>30} {sd:>30} -> stdout {r.stdout!r} stderr {r.stderr!r}")
