# SPDX-License-Identifier: Apache-2.0
"""Two builds of larmorx on every antsAI and antsAffineInitializer case of a parity tier, in
both D13 modes and at several thread counts: whether the newer build gives the older one's
results bit for bit (exit status, standard output, standard error with every D13 notice, every
file byte for byte). The parity records compare larmorx with ANTs in ``reproduce`` and set the
notices aside; this checks the default mode and the notices too (the memory and speed part of
2026-10-28: nothing it changed may change a result).

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P compare_builds.py <work dir> <old python dir> <new python dir> \\
            [--tier standard] [--threads 1,4] [--jobs N] [--only <substring>]

Each build is a ``python/`` tree with its ``_core`` extension (``maturin build``, the wheel's
``larmorx/_core.abi3.so`` copied in). The old build runs at one thread (its results do not
depend on the count), the new one at each of ``--threads``. Prints one line per difference and a
summary; exits 1 when anything differs.
"""

from __future__ import annotations

import concurrent.futures as cf
import contextlib
import gzip
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def contents(d: Path) -> dict[str, bytes]:
    out = {}
    for f in sorted(d.iterdir()):
        raw = f.read_bytes()
        out[f.name] = gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw
    return out


def run(tree: Path, program: str, args: list[str], seed, mode: str, threads: int, cwd: Path):
    env = {
        k: v
        for k, v in os.environ.items()
        if k not in ("ANTS_RANDOM_SEED", "NSLOTS", "ITK_NUMBER_OF_THREADS_ENV_LIST")
    }
    env["PYTHONPATH"] = f"{tree}{os.pathsep}{env.get('PYTHONPATH', '')}"
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    env["LARMORX_UPSTREAM_DEFECTS"] = mode
    if seed is not None:
        env["ANTS_RANDOM_SEED"] = seed
    p = subprocess.run(
        [sys.executable, "-m", "larmorx", "ants", program, *args],
        capture_output=True, text=True, env=env, cwd=cwd, timeout=7200, check=False,
    )  # fmt: skip
    return p.returncode, p.stdout, p.stderr


def one(case, old: Path, new: Path, threads: list[int]) -> list[str]:
    from larmorx_validation.parity import ants_ai
    from larmorx_validation.parity.harness import Outcome

    c = case.payload
    if c.expect == "hang":
        return []  # ANTs never finishes; larmorx refuses at once, nothing computed
    problems = []
    with tempfile.TemporaryDirectory(prefix="lx-cmp-") as t:
        tmp = Path(t)
        for mode in ("reproduce", "fix"):
            runs = [("old", old, 1)] + [("new", new, n) for n in threads]
            seen = {}
            for who, tree, n in runs:
                out = tmp / f"{mode}-{who}-{n}"
                out.mkdir()
                try:
                    args, seed = ants_ai._resolve(c, out, tmp)
                except Outcome:
                    return []
                code, so, se = run(tree, c.program, args, seed, mode, n, out)
                text = (so.replace(str(out), "<out>"), se.replace(str(out), "<out>"))
                seen[(who, n)] = (code, text, contents(out))
            ref = seen[("old", 1)]
            for key, got in seen.items():
                if key == ("old", 1):
                    continue
                for what, a, b in zip(("exit", "output", "files"), ref, got, strict=True):
                    if a != b:
                        problems.append(f"{case.id} [{mode}, new at {key[1]}]: {what} differs")
                if got[:2] != ref[:2]:
                    # Exit statuses and the last lines each printed, to tell a run the
                    # machine stopped (a signal, the memory box) from a changed result.
                    tail = [(x[0], x[1][1].strip().splitlines()[-2:]) for x in (ref, got)]
                    problems.append(f"{case.id} [{mode}, new at {key[1]}]: old, new: {tail}")
    return problems


def main() -> int:
    args = sys.argv[1:]
    opts = {"--tier": "standard", "--threads": "1,4", "--jobs": "4", "--only": ""}
    for k in list(opts):
        if k in args:
            i = args.index(k)
            opts[k] = args[i + 1]
            del args[i : i + 2]
    work, old, new = (Path(a).resolve() for a in args)
    work.mkdir(parents=True, exist_ok=True)
    os.chdir(work)
    from larmorx_validation.parity import ants_affine_initializer, ants_ai

    cases = ants_ai.cases_for(opts["--tier"]) + ants_affine_initializer.cases_for(opts["--tier"])
    cases = [c for c in cases if opts["--only"] in c.id]
    threads = [int(x) for x in opts["--threads"].split(",")]
    # The generated inputs made once, before the cases run at once (two threads making the
    # same input race: one reads it before it is written).
    from larmorx_validation.parity.harness import Outcome

    with tempfile.TemporaryDirectory(prefix="lx-cmp-") as t:
        for c in cases:
            with contextlib.suppress(Outcome):
                ants_ai._resolve(c.payload, Path(t), Path(t))
    problems: list[str] = []
    done = 0
    with cf.ThreadPoolExecutor(int(opts["--jobs"])) as pool:
        futures = {pool.submit(one, c, old, new, threads): c for c in cases}
        for f in cf.as_completed(futures):
            c = futures[f]
            try:
                p = f.result()
                if p:
                    # Once more, alone: a difference that does not repeat was the machine's (a
                    # run stopped by a signal or the memory box), and is printed as such.
                    again = one(c, old, new, threads)
                    if not again:
                        for line in p:
                            print(f"transient, not repeated: {line}", flush=True)
                    p = again
            except Exception as e:  # a case that cannot run here is reported, not hidden
                p = [f"{c.id}: {type(e).__name__}: {e}"]
            done += 1
            for line in p:
                print(line, flush=True)
            problems += p
    print(f"{done} cases, {len(problems)} differences (modes reproduce and fix; new build at "
          f"{opts['--threads']} threads against the old one at 1)")  # fmt: skip
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
