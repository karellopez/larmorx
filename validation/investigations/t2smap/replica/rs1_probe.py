# SPDX-License-Identifier: Apache-2.0
"""The replica's gaps of the t2smap clean-room part (relay RS1, 2026-11-12), checked by running
tedana 26.0.3's program **directly** (its launcher, not the oracle-output cache: a replayed run
holds no empty directory and the orchestrator asked for direct runs) and the replica
(``larmorx-gpl tedana t2smap``) in both modes on small seeded inputs.

Each case prints tedana's exit status and standard error in a normalised form (paths written
``<W>``/``<site>``, a traceback's stack as ``<stack>``, chained tracebacks and Python's warnings
kept with their location and source line), then whether the replica's ``reproduce`` run gives
the same (exit, every line, every file's bytes) and what its default prints::

    <workspace>/oracles/python-newest/bin/python -P rs1_probe.py <work dir> \\
        --t2smap <workspace>/oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap \\
        --replica <target>/release/larmorx-gpl [--only <case>,...] [--registry]

``--registry`` runs tedana in this process instead (its ``_main``), with ``warnings``' filter
changes traced: each change prints ``##FILTERS <tedana frame>`` on standard error between
tedana's own lines, which shows where Python's warnings registry is reset during a run.
"""

from __future__ import annotations

import argparse
import gzip
import os
import re
import shutil
import subprocess
import sys
import traceback
from pathlib import Path

NPY_OFF = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
TES = ["-e", "12", "28", "44"]


def phantom(shape=(6, 7, 3), vols=4, tes=(12.0, 28.0, 44.0)):
    import numpy as np

    i, j, k = np.indices(shape)
    s0 = 800.0 + 10 * i + 3 * j + k
    t2 = 30.0 + i + j
    out = []
    for e, te in enumerate(tes):
        decay = np.ones(shape)
        for _ in range(round(te)):
            decay = decay * (1.0 - 1.0 / t2)
        series = (s0 * decay)[..., None] * (1.0 + 0.01 * ((np.arange(vols) + e) % 3 - 1))
        out.append(series.astype(np.float32))
    return out


def make_inputs(d: Path) -> None:
    import nibabel as nib
    import numpy as np

    d.mkdir(parents=True, exist_ok=True)
    aff = np.diag([-3.0, 3.0, 3.0, 1.0])
    echoes = phantom()
    shape = echoes[0].shape[:3]

    def save(x, name):
        nib.save(nib.Nifti1Image(x, aff), d / name)

    for e, x in enumerate(echoes, 1):
        save(x, f"e{e}.nii.gz")
        save(x[..., 0], f"e{e}-3d.nii.gz")  # a 3D echo
    ones = np.ones(shape, np.float32)
    save(ones, "mask.nii.gz")
    save(ones[:-1], "mask-grid.nii.gz")  # another grid
    save(np.zeros((*shape[:2], shape[2] + 1), np.float32), "mask-grid-empty.nii.gz")
    save(np.zeros(shape, np.float32), "mask-empty.nii.gz")
    # TED-31: every voxel without good signal (a negative first echo: §6.1's base count 0)
    save(-echoes[0], "dead-e1.nii.gz")
    # both infinities in one voxel's second echo (count 1 there; PAID's group 2 reads it)
    both = echoes[1].copy()
    both[2, 3, 1, 1] = np.inf
    both[2, 3, 1, 2] = -np.inf
    save(both, "both-e2.nii.gz")
    # TED-46: a block whose samples are 255 at every echo and volume (uint8 saturated)
    for e, x in enumerate(echoes, 1):
        u = np.clip(np.round(x / 4.0), 0, 255).astype(np.uint8)
        u[0:2, 0:3, :, :] = 255
        save(u, f"sat-e{e}.nii.gz")
    # numpy's scientific printing of the dropout thresholds (means of 3e5, 30, 3: a ratio > 1e3)
    sci = [x * f for x, f in zip(echoes, (400.0, 0.06, 0.009), strict=True)]
    for e, x in enumerate(sci, 1):
        save(x.astype(np.float32), f"sci-e{e}.nii.gz")
    # x-underflow-debug's form: later echoes a thousandth of the first within 0.1 ms
    fast = [x.copy() for x in echoes]
    fast[1][0:3, 0:4, :, :] = echoes[0][0:3, 0:4, :, :] * 1e-3
    fast[2][0:3, 0:4, :, :] = echoes[0][0:3, 0:4, :, :] * 0.9e-3
    for e in (2, 3):
        save(fast[e - 1], f"fast-e{e}.nii.gz")


def echoes(*names: str) -> list[str]:
    return ["-d", *(f"<I>/{n}.nii.gz" for n in names)]


E3 = echoes("e1", "e2", "e3")
M = ["--mask", "<I>/mask.nii.gz"]
FAST = ["-e", "12", "12.05", "12.1"]
CASES: list[tuple[str, list[str]]] = [
    ("three-d", [*echoes("e1-3d", "e1-3d", "e1-3d"), *TES, *M]),
    ("mask-grid", [*E3, *TES, "--mask", "<I>/mask-grid.nii.gz"]),
    ("mask-grid-empty", [*E3, *TES, "--mask", "<I>/mask-grid-empty.nii.gz"]),
    ("missing-echo", ["-d", "<I>/e1.nii.gz", "<I>/e2.nii.gz", "<I>/nowhere.nii.gz", *TES, *M]),
    ("ted31-loglin", [*echoes("dead-e1", "e2", "e3"), *TES, *M, "--masktype", "none"]),
    (
        "ted31-curvefit",
        [*echoes("dead-e1", "e2", "e3"), *TES, *M, "--masktype", "none", "--fittype", "curvefit"],
    ),
    (
        "ted31-ts",
        [*echoes("dead-e1", "e2", "e3"), *TES, *M, "--masktype", "none", "--fitmode", "ts"],
    ),
    ("ted46-loglin", [*echoes("sat-e1", "sat-e2", "sat-e3"), *TES, *M]),
    ("ted46-curvefit", [*echoes("sat-e1", "sat-e2", "sat-e3"), *TES, *M, "--fittype", "curvefit"]),
    (
        "ted46-ts",
        [*echoes("sat-e1", "sat-e2", "sat-e3"), *TES, *M, "--fitmode", "ts", "--masktype", "none"],
    ),
    ("inf-both-paid", [*echoes("e1", "both-e2", "e3"), *TES, *M, "--combmode", "paid"]),
    ("inf-both", [*echoes("e1", "both-e2", "e3"), *TES, *M]),
    (
        "underflow-debug",
        [*echoes("e1", "fast-e2", "fast-e3"), *FAST, *M, "--masktype", "none", "--debug"],
    ),
    ("sci-thresholds", [*echoes("sci-e1", "sci-e2", "sci-e3"), *TES, *M]),
    # --fitmode ts with no volume left (TED-13 reproduced): the fit runs for no volume
    (
        "ts-no-volume",
        [*E3, *TES, *M, "--masktype", "none", "--fitmode", "ts", "--dummy-scans", "4"],
    ),
    (
        "ts-no-volume-te",
        [*E3, "-e", "12", "28", *M, "--masktype", "none", "--fitmode", "ts", "--dummy-scans", "4"],
    ),
    (
        "ted31-ts-curvefit",
        [
            *echoes("dead-e1", "e2", "e3"),
            *TES,
            *M,
            "--masktype",
            "none",
            "--fitmode",
            "ts",
            "--fittype",
            "curvefit",
        ],
    ),
    ("ted31-te", [*echoes("dead-e1", "e2", "e3"), "-e", "12", "28", *M, "--masktype", "none"]),
    ("all-no-volume", [*E3, *TES, *M, "--masktype", "none", "--dummy-scans", "4"]),
]


def environment() -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("NPY_", "OPENBLAS_", "OMP_"))}
    env.update(
        COLUMNS="80",
        TQDM_DISABLE="1",
        OPENBLAS_NUM_THREADS="1",
        OPENBLAS_CORETYPE="Haswell",
        NPY_DISABLE_CPU_FEATURES=NPY_OFF,
        OMP_NUM_THREADS="1",
    )
    return env


SITE = re.compile(r"\S*/site-packages/")


def normalise(text: str, work: Path) -> list[str]:
    """Paths ``<W>``/``<site>``; a traceback's stack as ``<stack>`` (Python's own frames, and the
    replica's one line naming its place); every other line kept."""
    text = text.replace(str(work), "<W>")
    text = SITE.sub("<site>/", text)
    out: list[str] = []
    in_trace = False
    for line in text.splitlines():
        if line.startswith("Traceback (most recent call last):"):
            out += [line, "  <stack>"]
            in_trace = True
            continue
        if in_trace:
            if line.startswith(" "):
                continue
            in_trace = False
        out.append(line)
    return out


def warnings_of(lines: list[str]) -> list[str]:
    """Python's warnings (location with the package's path, then the source line)."""
    out = []
    for i, line in enumerate(lines):
        m = re.match(r"^(\S+?\.py):(\d+): (\w+): (.*)$", line)
        if m:
            path = m.group(1).split("<site>/", 1)[-1]
            src = lines[i + 1].strip() if i + 1 < len(lines) else ""
            out.append(f"{path}:{m.group(2)}: {m.group(3)}: {m.group(4)} | {src}")
    return out


def tedana_lines(lines: list[str]) -> list[str]:
    """What tedana prints (the replica's ``larmorx-gpl:`` notices aside), each warning's
    location named from its package and its source line stripped."""
    out = []
    for line in lines:
        if line.startswith("larmorx-gpl:"):
            continue
        m = re.match(r"^(\S+?\.py):(\d+): (\w+): (.*)$", line)
        if m:
            line = f"{m.group(1).split('<site>/', 1)[-1]}:{m.group(2)}: {m.group(3)}: {m.group(4)}"
        out.append(line.strip() if line.startswith("  ") and "<stack>" not in line else line)
    return out


def files_of(out: Path) -> dict[str, bytes]:
    got = {}
    if not out.is_dir():
        return got
    for p in sorted(out.rglob("*")):
        if p.is_file():
            data = p.read_bytes()
            if p.suffix == ".gz":
                data = gzip.decompress(data)
            got[str(p.relative_to(out))] = data
        elif p.is_dir():
            got[str(p.relative_to(out)) + "/"] = b""
    return got


def run(cmd: list[str], env: dict[str, str], cwd: Path) -> tuple[int, str]:
    p = subprocess.run(cmd, env=env, cwd=cwd, capture_output=True, text=True, timeout=900)
    return p.returncode, p.stderr


def registry_trace(args: list[str]) -> None:
    """tedana's ``_main`` in this process, with every change of ``warnings``' filters printed."""
    import warnings

    original = warnings._filters_mutated

    def traced(*a, **k):
        frames = [f for f in traceback.extract_stack()[:-1] if "/tedana/" in f.filename]
        where = f"{Path(frames[-1].filename).name}:{frames[-1].name}" if frames else "?"
        outer = traceback.extract_stack()[-3]
        sys.stderr.write(f"##FILTERS {where} via {Path(outer.filename).name}:{outer.name}\n")
        return original(*a, **k)

    warnings._filters_mutated = traced
    from tedana.workflows import t2smap

    try:
        t2smap._main(args)
    except BaseException as e:  # a probe: print what tedana raised
        sys.stderr.write(f"##RAISED {type(e).__name__}: {e}\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--t2smap")
    ap.add_argument("--replica")
    ap.add_argument("--only", default="")
    ap.add_argument("--registry", action="store_true")
    ap.add_argument("--show", action="store_true", help="print each run's normalised lines")
    a = ap.parse_args()
    work = a.work.resolve()
    inputs = work / "inputs"
    if not (inputs / "e1.nii.gz").is_file():
        make_inputs(inputs)
    only = set(filter(None, a.only.split(",")))
    env = environment()
    bad = 0
    for name, args in CASES:
        if only and name not in only:
            continue
        argv = [x.replace("<I>", str(inputs)) for x in args]
        d = work / name
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        if a.registry:
            _, err = run(
                [sys.executable, "-P", __file__, str(work), "--only", name, "--_inner"],
                env,
                d,
            )
            print(f"== {name} (tedana in process)")
            print("\n".join(normalise(err, work)))
            continue
        for who in ("t", "r", "f"):
            (d / who).mkdir()
        t = run([a.t2smap, *argv, "--out-dir", "out"], env, d / "t")
        r = run(
            [a.replica, "tedana", "t2smap", *argv, "--out-dir", "out"],
            dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce"),
            d / "r",
        )
        f = run(
            [a.replica, "tedana", "t2smap", *argv, "--out-dir", "out"],
            dict(env, LARMORX_UPSTREAM_DEFECTS="fix"),
            d / "f",
        )
        tl, rl, fl = (
            normalise(x[1].replace(str(d / w), "<run>"), work)
            for x, w in ((t, "t"), (r, "r"), (f, "f"))
        )
        same_exit = t[0] == r[0]
        same_lines = tedana_lines(tl) == tedana_lines(rl)
        tf, rf = files_of(d / "t" / "out"), files_of(d / "r" / "out")
        tf.pop("t2smap_call.sh", None)
        rf.pop("t2smap_call.sh", None)
        for k in ("dataset_description.json",):
            tf.pop(k, None)
            rf.pop(k, None)
        same_files = tf == rf
        ok = same_exit and same_lines and same_files
        bad += not ok
        print(
            f"{'ok  ' if ok else 'DIFF'} {name}: exit {t[0]}/{r[0]}/{f[0]} lines "
            f"{'=' if same_lines else '!='} files {'=' if same_files else '!='}"
        )
        if not same_lines:
            x, y = tedana_lines(tl), tedana_lines(rl)
            for i in range(max(len(x), len(y))):
                p = x[i] if i < len(x) else "<none>"
                q = y[i] if i < len(y) else "<none>"
                if p != q:
                    print(f"   line {i}: tedana  {p}")
                    print(f"   line {i}: replica {q}")
        if not same_files:
            names = sorted(set(tf) | set(rf))
            print("   files differ: " + ", ".join(n for n in names if tf.get(n) != rf.get(n)))
        if a.show:
            print("   -- tedana:\n" + "\n".join("   | " + x for x in tl))
        if a.show:
            print("   -- default:\n" + "\n".join("   | " + x for x in fl))
        print("   default: " + " / ".join(x for x in fl if x.startswith("larmorx-gpl:"))[:900])
        last = [x for x in fl if x and not x.startswith("larmorx-gpl:")]
        print(f"   default's last line: {last[-1] if last else ''}")
    if not a.registry:
        print(f"{bad} case(s) differ")
    return 1 if bad else 0


if __name__ == "__main__":
    if "--_inner" in sys.argv:
        argv = sys.argv[1:]
        argv.remove("--_inner")
        work = Path(argv[0]).resolve()
        name = argv[argv.index("--only") + 1]
        case = dict(CASES)[name]
        registry_trace(
            [x.replace("<I>", str(work / "inputs")) for x in case] + ["--out-dir", "out"]
        )
        sys.exit(0)
    sys.exit(main())
