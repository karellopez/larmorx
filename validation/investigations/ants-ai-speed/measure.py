# SPDX-License-Identifier: Apache-2.0
"""Time, CPU time, peak memory and thread behaviour of antsAI and antsAffineInitializer: ANTs
2.6.5 with CORE-MATH preloaded (the build the records compare with) at one thread, and larmorx's
command (``python -m larmorx ants <program> ...``) in either D13 mode at any thread count, each
run its own process under ``/usr/bin/time -v``; every larmorx output compared with ANTs' byte for
byte.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P measure.py <work dir> <case> <who> [<who> ...] [--threads N] [--repeats R]

``case``: ``ai-head`` / ``init-head`` (``tests/parity/test_memory_peak.py``'s calls on its head
of 128 x 128 x 96 voxels), ``ai-7t`` / ``init-7t`` (the same calls on the data panel's 7 T
MP2RAGE: niworkflows' masked MNI template and the 7 T T1w of ``ds005454-sub-16``, as the template
registration's antsAffineInitializer gets them, ``oracles/inputs/fmriprep-registration``).
``who``: ``ants`` (one thread), ``ants-n`` (ANTs at ``--threads``), ``reproduce``, ``fix``.

Prints one line per run (exit status, wall seconds, user and system CPU seconds, CPU %, peak
MiB, voluntary and involuntary context switches); ``--json`` writes them.
"""

from __future__ import annotations

import gzip
import json
import os
import re
import subprocess
import sys
from pathlib import Path

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))

AI_ARGS = (
    "-d 3 -m Mattes[{fixed},{moving},32,Regular,0.25] -t Rigid[0.1] -s [30,0.12] -c 10 "
    "-o {out}/ai.mat"
)
INIT_ARGS = "3 {fixed} {moving} {out}/init.mat 15 0.1 0 10"

FIELDS = {
    "wall": r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)",
    "user": r"User time \(seconds\): ([\d.]+)",
    "sys": r"System time \(seconds\): ([\d.]+)",
    "cpu": r"Percent of CPU this job got: (\d+)%",
    "rss_kb": r"Maximum resident set size \(kbytes\): (\d+)",
    "voluntary": r"Voluntary context switches: (\d+)",
    "involuntary": r"Involuntary context switches: (\d+)",
}


def head_inputs(d: Path) -> dict[str, str]:
    """``test_memory_peak.py``'s head (``_more_inputs``): fixed, moving two voxels apart."""
    import nibabel as nib
    import numpy as np

    d.mkdir(parents=True, exist_ok=True)
    out = {"fixed": str(d / "fixed.nii.gz"), "moving": str(d / "moving.nii.gz")}
    if all(Path(p).is_file() for p in out.values()):
        return out
    rng = np.random.default_rng(11)
    shape = (128, 128, 96)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 64) ** 2 + (j - 64) ** 2 + ((k - 48) * 1.3) ** 2)
    head = np.where(r < 50, 400 + i, 20) + np.where(r < 25, 300, 0) + np.where(r < 12, 150, 0)
    img = (head + rng.normal(0, 10, shape)).astype(np.float32)
    a = np.diag([1.5, 1.5, 1.5, 1.0])
    nib.save(nib.Nifti1Image(img, a), out["fixed"])
    nib.save(nib.Nifti1Image(np.roll(img, 2, axis=0), a), out["moving"])
    return out


def seven_t_inputs() -> dict[str, str]:
    d = WORKSPACE / "oracles" / "inputs" / "fmriprep-registration" / "ds005454-sub-16-noreconall"
    out = {"fixed": str(d / "fixed_masked.nii.gz"), "moving": str(d / "moving_masked.nii.gz")}
    for p in out.values():
        if not Path(p).is_file():
            sys.exit(f"{p} is missing: made by larmorx_validation.parity.niworkflows_inputs")
    return out


def case(name: str, work: Path) -> tuple[str, str, dict[str, str]]:
    program, args = {
        "ai": ("antsAI", AI_ARGS),
        "init": ("antsAffineInitializer", INIT_ARGS),
    }[name.split("-")[0]]
    paths = head_inputs(work / "inputs") if name.endswith("-head") else seven_t_inputs()
    return program, args, paths


def measure(cmd: list[str], env: dict[str, str], cwd: Path) -> dict[str, float]:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd,
        check=False,
    )  # fmt: skip
    row: dict[str, float] = {"exit": p.returncode}
    for key, pattern in FIELDS.items():
        m = re.search(pattern, p.stderr)
        if not m:
            row[key] = float("nan")
            continue
        if key == "wall":
            s = 0.0
            for part in m.group(1).split(":"):
                s = s * 60 + float(part)
            row[key] = s
        else:
            row[key] = float(m.group(1))
    row["peak_mib"] = row.pop("rss_kb") / 1024
    if p.returncode != 0:
        print(p.stderr[-2000:], file=sys.stderr)
    return row


def contents(d: Path) -> dict[str, bytes]:
    out = {}
    for f in sorted(d.iterdir()):
        raw = f.read_bytes()
        out[f.name] = gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw
    return out


def main() -> int:
    args = sys.argv[1:]
    threads, repeats, json_out = 1, 1, None
    for flag in ("--threads", "--repeats", "--json"):
        if flag in args:
            i = args.index(flag)
            value = args[i + 1]
            del args[i : i + 2]
            if flag == "--threads":
                threads = int(value)
            elif flag == "--repeats":
                repeats = int(value)
            else:
                json_out = Path(value)
    work, name, *who = args
    work = Path(work)
    program, template, paths = case(name, work)
    from larmorx_validation.parity.ants_programs import ants_bin, crmath_preload

    base = {
        k: v
        for k, v in os.environ.items()
        if k not in ("RAYON_NUM_THREADS", "NSLOTS", "ITK_NUMBER_OF_THREADS_ENV_LIST")
    }
    base |= {"OMP_NUM_THREADS": "1", "ANTS_RANDOM_SEED": "42"}
    rows = []
    for w in who:
        for r in range(repeats):
            out = work / f"{name}-{w}-{threads}-{r}"
            out.mkdir(parents=True, exist_ok=True)
            for f in out.iterdir():
                f.unlink()
            words = [a.format(**paths, out=out) for a in template.split()]
            if w.startswith("ants"):
                n = threads if w == "ants-n" else 1
                cmd = [str(ants_bin() / program), *words]
                env = base | {
                    "LD_PRELOAD": str(crmath_preload()),
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(n),
                }
            else:
                n = threads
                cmd = [sys.executable, "-m", "larmorx", "ants", program, *words]
                env = base | {
                    "LARMORX_UPSTREAM_DEFECTS": w,
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(n),
                }
            row = measure(cmd, env, out) | {"who": w, "threads": n, "repeat": r}
            ref = work / f"{name}-ants-1-0"
            if not w.startswith("ants") and ref.is_dir():
                row["same_as_ants"] = contents(ref) == contents(out)
            load = os.getloadavg()
            row["load"] = [round(x, 2) for x in load]
            rows.append(row)
            print(
                f"{name} {w:9s} t={n:<2d} exit {row['exit']:.0f}  wall {row['wall']:8.2f} s  "
                f"user {row['user']:8.2f}  sys {row['sys']:6.2f}  cpu {row['cpu']:4.0f}%  "
                f"peak {row['peak_mib']:7.1f} MiB  vcs {row['voluntary']:.0f} "
                f"ivcs {row['involuntary']:.0f}  same {row.get('same_as_ants', '-')}  "
                f"load {row['load']}",
                flush=True,
            )
    if json_out is not None:
        json_out.write_text(json.dumps(rows, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
