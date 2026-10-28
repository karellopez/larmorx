# SPDX-License-Identifier: Apache-2.0
"""The largest real calls of the original programs, run again under the address-space limit:
each one's peak virtual size, and whether it gives its stored result.

The oracle cache holds every run of an original program the suites made: this re-runs chosen
entries (``oracle-cache verify``'s machinery, :func:`larmorx_validation.oracle_cache.rerun.rerun`)
in a copy of the cache (the entries copied, their blobs hard-linked: the workspace's cache is
never written), with the harness's runner replaced by one that samples every process of the run
(``/proc/<pid>/status``, ``VmPeak``, every 20 ms, the process and all it starts) and keeps each
process's largest virtual size. ``RLIMIT_AS`` is a per-process limit, so the number that matters
is the largest single process's ``VmPeak``.

Usage (from the checkout, the environment of ``scripts/orchestration/env.sh``, in a memory box)::

    python -P vm_peak.py <out> [--jobs N] [--keys KEY...]

Without ``--keys`` it measures the default list below: the largest inputs and longest runs per
program in the cache on 2026-10-28 (the data panel's 600-volume multiband-8 BOLD series through
antsApplyTransforms, mcflirt and ImageMath, the 96-slice multiband BOLD through 3dTshift with
OpenMP at 32 threads and unset, the 7 T and 1.5 T images through N4, DenoiseImage, Atropos,
antsAI, ResampleImage, BET's programs, fMRIPrep's ``bet`` through Apptainer). It writes
``<out>/vm_peak.json`` and prints a table.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation import address_space
from larmorx_validation.oracle_cache import core, rerun
from larmorx_validation.oracle_cache.store import Store

#: The entries measured by default (key prefixes, with what they are).
DEFAULT = {
    "e5426b10be70b83a": "antsApplyTransforms, the 600-volume multiband-8 BOLD series to T1w (4 threads)",
    "4eade692edcef013": "mcflirt, fMRIPrep's call on the 600-volume multiband-8 BOLD series",
    "87a1c2103e2dde7f": "mcflirt, the same series, another of fMRIPrep's calls",
    "5b6146744d764ab2": "ImageMath m on the 600-volume BOLD series (4 threads)",
    "6bd07e34225a5621": "3dTshift, 96-slice multiband BOLD, OpenMP at 32 threads",
    "a9f35b460fbdabdc": "3dTshift, 96-slice multiband BOLD, OpenMP threads unset",
    "4a525ebaf29e3c46": "SmoothImage, a 4D Philips multiband BOLD (4 threads)",
    "4867a6f801cc05bf": "MultiplyImages, a 4D Philips multiband BOLD (4 threads)",
    "0708419e6a43bf36": "DenoiseImage, the 1.5 T panel image (1 thread)",
    "ef6f46ffa658a612": "N4BiasFieldCorrection, sMRIPrep's brain extraction on ds005454 (1 thread)",
    "ad0ec497aa67feba": "Atropos, the child's T1w and T2w (1 thread)",
    "ccd877a89b785448": "antsAI, the blob alignment on a shifted T1w (1 thread)",
    "009765763648d9eb": "antsAffineInitializer, MNI 2 mm",
    "0a71ee335feb2413": "ResampleImage, the 7 T MP2RAGE to 1 mm B-spline (4 threads)",
    "2ad6c59878bb8414": "3dUnifize -EPI (OpenMP at 6 threads)",
    "e7b375f367952a9b": "betsurf with a T2 (the child)",
    "ec341e6bb50f335f": "bet -R, a 75-year-old's T1w",
    "6e473785a3d3b902": "fMRIPrep's bet through Apptainer, the 7 T T1w",
}

_SAMPLES: dict[str, dict] = {}
_lock = threading.Lock()


def _tree(pid: int) -> list[int]:
    out, todo = [], [pid]
    while todo:
        p = todo.pop()
        out.append(p)
        try:
            for task in os.listdir(f"/proc/{p}/task"):
                text = Path(f"/proc/{p}/task/{task}/children").read_text()
                todo += [int(c) for c in text.split()]
        except OSError:
            continue
    return out


def _vm_peak(pid: int) -> tuple[int, str] | None:
    try:
        text = Path(f"/proc/{pid}/status").read_text()
    except OSError:
        return None
    name, peak = "?", None
    for line in text.splitlines():
        if line.startswith("Name:"):
            name = line.split(None, 1)[1]
        elif line.startswith("VmPeak:"):
            peak = int(line.split()[1]) * 1024
    return None if peak is None else (peak, name)


def _sampled_execute(argv, env, cwd, timeout):
    """``core._execute`` with every process of the run sampled (its ``VmPeak``)."""
    start = time.perf_counter()
    p = subprocess.Popen(
        [str(a) for a in argv],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        cwd=None if cwd is None else str(cwd),
        preexec_fn=address_space.child_setup(),
    )
    streams: dict = {}

    def talk():
        try:
            streams["out"], streams["err"] = p.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            p.kill()
            streams["out"], streams["err"] = p.communicate()
            streams["timeout"] = True

    t = threading.Thread(target=talk)
    t.start()
    peaks: dict[int, tuple[int, str]] = {}
    while t.is_alive():
        for pid in _tree(p.pid):
            got = _vm_peak(pid)
            if got is not None and got[0] > peaks.get(pid, (0, ""))[0]:
                peaks[pid] = got
        time.sleep(0.02)
    t.join()
    top = max(peaks.values(), default=(0, "?"))
    with _lock:
        _SAMPLES[threading.current_thread().name] = {
            "vm_peak": top[0],
            "process": top[1],
            "processes": len(peaks),
        }
    code = None if streams.get("timeout") else p.returncode
    return core.Completed(
        [str(a) for a in argv], code, streams["out"], streams["err"], False,
        time.perf_counter() - start,
    )  # fmt: skip


def _copy(src: Store, dst: Store, entry: dict) -> None:
    """The entry and its blobs (hard links) into ``dst``."""
    r = entry["result"]
    shas = [r["stdout"], r["stderr"], *(o["sha256"] for o in r["outputs"])]
    for i in entry["inputs"]:
        shas.append(i["sha256"])
        shas += [h for _s, h in i.get("companions", [])]
    for sha in shas:
        a, b = src.blob_path(sha), dst.blob_path(sha)
        if b.exists() or not a.exists():
            continue
        b.parent.mkdir(parents=True, exist_ok=True)
        try:
            os.link(a, b)
        except FileExistsError:
            continue  # another job linked it
        except OSError:
            shutil.copy2(a, b)
    launcher = entry.get("launcher", {})
    if launcher.get("kind") == "exec" and not Path(launcher["program"]).is_file():
        # a run of a copy of the oracle in an agent's scratch directory, gone since: the
        # workspace's build of the same program (the same SHA-256) runs it
        same = _same_program(entry["parts"]["oracle"])
        if same is not None:
            entry = dict(entry, launcher=dict(launcher, program=str(same)))
    dst.write(entry)


def _same_program(oracle: dict) -> Path | None:
    from larmorx_validation.oracle_cache.hashing import sha256_file
    from larmorx_validation.oracle_cache.store import workspace

    ws = workspace()
    if ws is None:
        return None
    for candidate in sorted((ws / "oracles").glob(f"*/bin/{oracle.get('program')}")):
        if candidate.is_file() and sha256_file(candidate.resolve()) == oracle.get("sha256"):
            return candidate
    return None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("out", type=Path)
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--keys", nargs="+", help="entries (key prefixes) instead of the default list")
    args = ap.parse_args(argv)
    args.out.mkdir(parents=True, exist_ok=True)
    src = Store()
    dst = Store(args.out / "cache")
    dst.ensure()
    wanted = {k: DEFAULT.get(k, "") for k in args.keys} if args.keys else dict(DEFAULT)
    entries = {}
    for e in src.entries():
        for k in wanted:
            if e["key"].startswith(k):
                entries[k] = e
    core._execute = _sampled_execute
    print(f"address-space limit: {address_space.describe()}; {len(entries)} entries", flush=True)

    def one(k: str) -> dict:
        threading.current_thread().name = k
        e = entries[k]
        _copy(src, dst, e)
        o = rerun.rerun(dst, dst.read(e["key"]), timeout=4 * 3600)
        s = _SAMPLES.get(k, {})
        row = {
            "key": k,
            "what": wanted[k],
            "program": e["parts"]["oracle"].get("program"),
            "status": o.status,
            "detail": o.detail[:300],
            "seconds": round(o.seconds, 1),
            **s,
        }
        gib = s.get("vm_peak", 0) / (1 << 30)
        print(
            f"{o.status:8s} {k} {gib:7.2f} GiB ({s.get('process', '?')}, "
            f"{s.get('processes', 0)} processes) {o.seconds:7.1f} s  {wanted[k]}  {o.detail[:120]}",
            flush=True,
        )
        return row

    with ThreadPoolExecutor(args.jobs) as pool:
        rows = list(pool.map(one, [k for k in wanted if k in entries]))
    missing = [k for k in wanted if k not in entries]
    result = {
        "limit": address_space.limit(),
        "rows": rows,
        "missing": missing,
        "largest": max((r.get("vm_peak", 0) for r in rows), default=0),
    }
    (args.out / "vm_peak.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(
        f"largest single process: {result['largest'] / (1 << 30):.2f} GiB; "
        f"{sum(r['status'] == 'same' for r in rows)} of {len(rows)} the same"
        + (f"; not in the cache: {missing}" if missing else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
