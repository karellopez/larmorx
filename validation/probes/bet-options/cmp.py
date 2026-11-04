# SPDX-License-Identifier: Apache-2.0
"""Compares a bet probe's fsl/ and rep/ directories: status, messages (the script's path read as
its name, larmorx's own lines dropped), and every file (images by data and header without
descrip; other files by bytes)."""

import gzip
import os
import pathlib
import re
import sys

FSL = os.environ.get("LARMORX_FSL_DIR", str(pathlib.Path.home() / "fsl-6.0.7.23"))


def text(p: pathlib.Path, ours: bool) -> str:
    t = p.read_text(errors="replace")
    t = t.replace(f"{FSL}/bin/bet", "bet").replace(
        f"{FSL}/bin/standard_space_roi", "standard_space_roi"
    )
    t = re.sub(
        r"(: line \d+: +)\d+( (?:Aborted|Segmentation fault) +)(?:\(core dumped\) )?", r"\1PID\2", t
    )
    if ours:
        t = "".join(ln for ln in t.splitlines(keepends=True) if not ln.startswith("larmorx-nc: "))
    return t


def main() -> None:
    for tag in sys.argv[1:]:
        d = pathlib.Path(os.environ.get("PROBE_RUNS", pathlib.Path(__file__).parent / "runs")) / tag
        a, b = d / "fsl", d / os.environ.get("REPDIR", "rep")
        print(
            f"=== {tag}: status fsl {(a / 's.txt').read_text().strip()} rep {(b / 's.txt').read_text().strip()}"
            f"  time fsl {(a / 'time.txt').read_text().strip()} s rep {(b / 'time.txt').read_text().strip()} s"
        )
        for s in ("o.txt", "e.txt"):
            x, y = text(a / s, False), text(b / s, True)
            if x != y:
                print(f"  {s} DIFF:\n    fsl: {x[:700]!r}\n    rep: {y[:700]!r}")
        ours = [
            ln
            for ln in (b / "e.txt").read_text(errors="replace").splitlines()
            if ln.startswith("larmorx-nc: ")
        ]
        for ln in ours:
            print("  notice:", ln[:300])
        skip = {"o.txt", "e.txt", "s.txt", "time.txt", "in.nii.gz", "t2.nii.gz"}
        fa = sorted(p.name for p in a.iterdir() if p.name not in skip)
        fb = sorted(p.name for p in b.iterdir() if p.name not in skip)
        if fa != fb:
            print(
                "  files differ: fsl only",
                sorted(set(fa) - set(fb)),
                "rep only",
                sorted(set(fb) - set(fa)),
            )
        same = 0
        for n in sorted(set(fa) & set(fb)):
            x, y = (a / n).read_bytes(), (b / n).read_bytes()
            if n.endswith(".gz"):
                x, y = gzip.decompress(x), gzip.decompress(y)
            if n.endswith((".nii.gz", ".nii")) and len(x) > 352:
                x, y = x[:148] + x[228:], y[:148] + y[228:]
            if x == y:
                same += 1
            else:
                nd = sum(1 for i, j in zip(x, y, strict=False) if i != j)
                print(f"  {n}: DIFF ({len(x)} vs {len(y)} bytes, {nd} bytes differ)")
        print(f"  {same} of {len(set(fa) & set(fb))} common files identical")


main()
