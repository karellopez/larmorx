# SPDX-License-Identifier: Apache-2.0
"""The same bits from two builds of FLIRT's clean-room original, and at any thread count: every
`flirt` case of the fsl-flirt suite matching a pattern run by build A (one thread, or
`--threads-a`) and by build
B (at each thread count given), their exit statuses, streams and files compared byte for byte
(images by their decompressed bytes). A speed change of the original must leave every result
as it was (part 2, relay 3: the correlation ratio's bins summed in parallel). Development tool;
FSL is not run.

    PYTHONPATH=python:validation/src python -P same_bits.py --a <larmorx> --b <larmorx> \\
        [--threads 1,7] [--workers 2] [--case <regex>]

Run it inside scripts/orchestration/boxed.sh (workers × threads at most the agent's share).
"""

from __future__ import annotations

import argparse
import gzip
import os
import re
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep


def run(binary: str, run_: rep.Run, threads: int) -> tuple[int, str, str, dict[str, bytes]]:
    with tempfile.TemporaryDirectory(prefix="lx-same-") as d_:
        d = Path(d_)
        args = rep._prepare(run_, d)
        before = {p for p in d.rglob("*") if p.is_file()}
        env = {k: v for k, v in os.environ.items() if not k.startswith("LARMORX_UPSTREAM")}
        env.update({"FSLOUTPUTTYPE": "NIFTI_GZ", "OMP_NUM_THREADS": str(threads)})
        env.update({k: v for k, v in run_.settings if v is not None})
        env["LARMORX_UPSTREAM_DEFECTS"] = "reproduce"
        p = subprocess.run(
            [binary, "fsl", run_.program, *args],
            cwd=d,
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=900,
        )
        files = {}
        for f in sorted(x for x in d.rglob("*") if x.is_file() and x not in before):
            b = f.read_bytes()
            files[str(f.relative_to(d))] = gzip.decompress(b) if f.suffix == ".gz" else b
        out = p.stdout.replace(str(d), "<run>")
        err = re.sub(r"0x[0-9a-f]+", "0x", p.stderr.replace(str(d), "<run>"))
        return p.returncode, out, err, files


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--a", required=True)
    ap.add_argument("--b", required=True)
    ap.add_argument("--threads", default="1,7")
    ap.add_argument("--threads-a", type=int, default=1)
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--case", default=".")
    o = ap.parse_args()
    pat = re.compile(o.case)
    threads = [int(t) for t in o.threads.split(",")]
    cases = [
        (name, r)
        for name, (_t, _d, r) in rep._all_cases().items()
        if r.program == "flirt" and pat.search(name) and not (r.hang or r.crash)
    ]

    def one(item: tuple[str, rep.Run]) -> str | None:
        name, r = item
        want = run(o.a, r, o.threads_a)
        for t in threads:
            got = run(o.b, r, t)
            if got != want:
                what = [
                    k
                    for k, a, b in zip(
                        ("exit", "stdout", "stderr", "files"), want, got, strict=True
                    )
                    if a != b
                ]
                return f"{name} ({t} threads): {', '.join(what)} differ"
        return None

    with ThreadPoolExecutor(o.workers) as ex:
        bad = [m for m in ex.map(one, cases) if m]
    print(f"{len(cases)} cases, {len(bad)} with a difference")
    for m in bad:
        print("  " + m)


if __name__ == "__main__":
    main()
