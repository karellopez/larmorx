# SPDX-License-Identifier: Apache-2.0
"""A batch of black-box comparisons (compare.py) of FSL's bet2/bet with the clean-room original
on the parity suite's phantoms. ``python -P cases.py <phantom dir> <work dir> [names...]``."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import compare  # noqa: E402


def cases(ph: Path) -> dict[str, tuple[str, list[str]]]:
    h = str(ph / "head.nii.gz")
    return {
        "default": ("bet2", [h, "out"]),
        "all": ("bet2", [h, "out", "-o", "-m", "-s", "-e", "-v"]),
        "f0.1": ("bet2", [h, "out", "-f", "0.1", "-m"]),
        "f0.9": ("bet2", [h, "out", "-f", "0.9", "-m"]),
        "g": ("bet2", [h, "out", "-g", "0.4", "-m", "-v"]),
        "r": ("bet2", [h, "out", "-r", "60", "-m", "-v"]),
        "c": ("bet2", [h, "out", "-c", "48", "56", "38", "-m", "-v"]),
        "w2": ("bet2", [h, "out", "-w", "2", "-m", "-v"]),
        "w0.5": ("bet2", [h, "out", "-w", "0.5", "-m", "-v"]),
        "t": ("bet2", [h, "out", "-t", "-m"]),
        "ct": ("bet2", [str(ph / "head-ct.nii.gz"), "out", "--ct", "-m", "-v"]),
        "d1d2": ("bet2", [h, "out", "--parameter_d1=5", "--parameter_d2=2", "-m", "-v"]),
        "t2": ("bet2", [str(ph / "head-t2.nii.gz"), "out", "-m", "-v"]),
        "neuro": ("bet2", [str(ph / "head-neuro.nii.gz"), "out", "-m", "-v"]),
        "i16s": ("bet2", [str(ph / "head-i16s.nii.gz"), "out", "-m"]),
        "u8": ("bet2", [str(ph / "head-u8.nii.gz"), "out", "-m"]),
        "obl": ("bet2", [str(ph / "head-obl.nii.gz"), "out", "-m"]),
        "q": ("bet2", [str(ph / "head-q.nii.gz"), "out", "-m"]),
        "n": ("bet2", [str(ph / "head-n.nii.gz"), "out", "-m"]),
        "n2": ("bet2", [str(ph / "head-n2.nii"), "out", "-m"]),
        "pair": ("bet2", [str(ph / "head-pair.hdr"), "out", "-m"]),
        "ana": ("bet2", [str(ph / "head-ana.hdr"), "out", "-m"]),
        "4d": ("bet2", [str(ph / "head-4d.nii.gz"), "out", "-m"]),
        "2d": ("bet2", [str(ph / "head-2d.nii.gz"), "out", "-m", "-v"]),
        "bm": ("bet2", [h, "out", f"--binaryMask={ph / 'head_mask.nii.gz'}", "-m", "-o", "-s", "-t"]),
        "bm-v": ("bet2", [h, "out", f"--binaryMask={ph / 'head_mask.nii.gz'}", "-v", "-e"]),
        "rand1-c": ("bet2", [str(ph / "rand1.nii.gz"), "out", "-c", "1", "2x", "3", "-v", "-m"]),
        "bet": ("bet", [h, "out"]),
        "bet-v": ("bet", [h, "out", "-m", "-v"]),
        "bet-opts": ("bet", [h, "out", "-o", "-m", "-s", "-e", "-t", "-f", "0.4", "-g", "0.1", "-r", "70", "-c", "48", "56", "38"]),
        "bet-R": ("bet", [str(ph / "head-off.nii.gz"), "out", "-R", "-m", "-v"]),
        "bet-R-n": ("bet", [h, "out", "-R", "-n", "-m"]),
        "bet-R-e": ("bet", [h, "out", "-R", "-e", "-m"]),
        "bet-F": ("bet", [str(ph / "head-4d.nii.gz"), "out", "-F", "-v"]),
        "bet-Z": ("bet", [str(ph / "slab.nii.gz"), "out", "-Z", "-m", "-v", "-s"]),
        "bet-Z-d": ("bet", [str(ph / "slab.nii.gz"), "out", "-Z", "-d"]),
        "bet-Z-large": ("bet", [h, "out", "-Z", "-m"]),
        "bet-f2": ("bet", [h, "out", "-f", "2"]),
        "bet-usage": ("bet", [h]),
        "bet-noinput": ("bet", ["/nonexistent/in", "out"]),
    }


def main(argv: list[str]) -> int:
    ph, work, *names = argv
    all_cases = cases(Path(ph))
    for name in names or list(all_cases):
        program, args = all_cases[name]
        r = compare.compare(Path(work) / name, program, args)
        bad = {
            k: v
            for k, v in r.items()
            if (k == "exit" and v[0] != v[1])
            or (k in ("stdout same", "stderr same") and not v)
            or k in ("stdout", "stderr")
            or (k == "files" and isinstance(v, tuple))
            or (isinstance(v, str) and not (" 0 of " in v or "distance p95 0.0000 max 0.0000" in v))
        }
        print(f"{name}: exit {r['exit']}", "OK" if not bad else "")
        for k, v in bad.items():
            print(f"    {k}: {v}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
