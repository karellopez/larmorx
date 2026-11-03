# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room original's outputs on every case of the `fsl-topup` suite (its inputs as the
suite prepares them; FSL is not run), hashed, so two builds or two thread counts can be compared
bit for bit (sub-task 4: the speed work keeps every output identical, at every thread count).

  speed_identity.py run <out.json> <threads> [--jobs N] [--tier standard] [--only PREFIX]
  speed_identity.py compare <a.json> <b.json>

`run` runs `LARMORX_TOPUP_BIN` (a `larmorx` binary; else this Python's package) on each case in
both modes (reproduce and the default), at `<threads>` threads (`OMP_NUM_THREADS`; a case's own
`--nthr` wins), and records the SHA-256 of each output (an image's decompressed bytes, a text
file's bytes), of stdout and of stderr. `--profile`'s file holds durations and is left out.
Every case runs in its own temporary directory; run the whole script in a memory box.
"""

import gzip
import hashlib
import json
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as O
from larmorx_validation.parity import fsl_topup_replica as rep


def _digest(p: Path) -> str:
    b = p.read_bytes()
    if p.name.endswith(".gz"):
        b = gzip.decompress(b)
    return hashlib.sha256(b).hexdigest()[:20]


def _one(name: str, threads: int, mode: str) -> dict[str, str]:
    node = rep._source_node()
    with tempfile.TemporaryDirectory(prefix="lx-topup-speed_") as d_:
        d = Path(d_)
        if name.startswith("fmriprep/"):
            n = Path(dict(rep.fmriprep_calls())[name.split("/", 1)[1]])
            if not rep._prepare_fmriprep(n, d):
                return {"skipped": "inputs missing"}
            args, settings, prog = rep.fmriprep_args(n), {}, "topup"
        else:
            run = rep._all_cases()[name][2]
            try:
                args = rep._prepare(run, d, node)
            except Exception as e:  # an Outcome: the inputs are missing
                return {"skipped": str(e)[:80]}
            settings, prog = dict(run.settings), run.program
        before = {p.name for p in d.iterdir()}
        r = O.run_original(args, cwd=d, settings=settings, mode=mode, threads=threads, program=prog)
        out = {
            "exit": str(r.code),
            "stdout": hashlib.sha256(r.stdout.encode()).hexdigest()[:20],
            "stderr": hashlib.sha256(r.stderr.encode()).hexdigest()[:20],
        }
        for p in sorted(d.rglob("*")):
            if p.is_file() and p.name not in before and "profile" not in p.name:
                out[str(p.relative_to(d))] = _digest(p)
        return out


def run(path: str, threads: int, jobs: int, tier: str, only: str) -> None:
    names = [c.id for c in O.cases(tier) if c.payload[0] in ("run", "fmriprep")]
    names = [n for n in names if n.startswith(only)]
    work = [(n, m) for n in names for m in (defects.REPRODUCE, defects.FIX)]
    with ThreadPoolExecutor(jobs) as ex:
        res = list(ex.map(lambda w: _one(w[0], threads, w[1]), work))
    table = {f"{n} [{m}]": r for (n, m), r in zip(work, res)}
    Path(path).write_text(json.dumps(table, indent=1, sort_keys=True))
    skipped = sum("skipped" in r for r in res)
    print(f"{len(work)} runs ({len(names)} cases x 2 modes), {skipped} skipped -> {path}")


def compare(a: str, b: str) -> None:
    ta, tb = json.loads(Path(a).read_text()), json.loads(Path(b).read_text())
    diff = []
    for k in sorted(set(ta) | set(tb)):
        x, y = ta.get(k), tb.get(k)
        if x != y:
            keys = sorted(set(x or {}) | set(y or {}))
            bad = [f for f in keys if (x or {}).get(f) != (y or {}).get(f)]
            diff.append(f"{k}: {bad}")
    print(f"{len(set(ta) | set(tb))} runs compared, {len(diff)} differ")
    for line in diff:
        print("  ", line)


if __name__ == "__main__":
    if sys.argv[1] == "run":
        argv = sys.argv[2:]

        def opt(flag: str, default: str) -> str:
            return argv[argv.index(flag) + 1] if flag in argv else default

        run(
            argv[0],
            int(argv[1]),
            int(opt("--jobs", "1")),
            opt("--tier", "standard"),
            opt("--only", ""),
        )
    else:
        compare(sys.argv[2], sys.argv[3])
