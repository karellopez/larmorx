# SPDX-License-Identifier: Apache-2.0
"""The evidence of tedana's ``t2smap`` replica (``larmorx-gpl tedana t2smap``, the GPL package's
program, translated from tedana 26.0.3), against tedana itself and fMRIPrep 25.2.6's own files.

- **fMRIPrep's multi-echo calls** (the oracle runs' ``bold_t2smap_wf/t2smap_node``: fMRIPrep's
  resampled echoes, its BOLD mask dilated by two voxels, the echo times in ms, ``--fittype
  curvefit``): tedana 26.0.3 in the newest kept environment (numpy 2.5.4, SciPy 1.18.1; numpy's
  OpenBLAS on its Haswell kernels, numpy's AVX-512 kernels off, one BLAS thread: larmorx's
  reference setup, decision D2), the replica in its reproduce mode at 1 and 14 threads and in
  its default mode, and tedana's own variability: the shared environment's releases (numpy
  2.3.5, SciPy 1.15.3), numpy's AVX-512 kernels on, ``--n-threads 4``, and tedana 25.0.1 in
  fMRIPrep's image on this machine; fMRIPrep's own files (tedana 25.0.1, in its run) beside them.
  ds003592's maps and combined series and ds000258's maps are shown; every call in a table.
- The record's cases, tedana's own spread (the record's ``spread/`` cases), the defects TED-01
  to TED-18 with the maths entries, the memory peaks (a call in a process of its own), random
  testing's trial and its check, and the benchmark.
- fMRIPrep's own files against tedana 25.0.1 run here in fMRIPrep's image with numpy's AVX-512
  kernels off and on (fMRIPrep's runs had them on), on its four curve-fit calls (2026-11-12).
- The clean-room original's part (``tedana_t2smap_clean_room.extend``, its implementer's module:
  its worked example, its record's cases as ``original/<case>``, its tables), and its series on
  the speed chart from its benchmark.

tedana runs as its program through the oracle-output cache (the parity suite's launchers,
``parity.tedana_t2smap_replica``), the replica as the ``larmorx-gpl`` program (``LARMORX_GPL_BIN``
or crates-gpl's release build). Every image is shown inside fMRIPrep's mask (the BOLD mask
dilated by two voxels, the region t2smap computes over), dilated by one more voxel: no face can
show.

    python -m larmorx_validation.evidence.tedana_t2smap   (computes the cache entries)
    python -m larmorx_validation.evidence.tedana_t2smap --refresh   (brings them to this commit)
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "tedana-t2smap"
RECORD = "tedana-t2smap-replica"
BENCH = "tedana-t2smap"
#: Random testing of this part: the trial, and the short check of the final build.
TRIALS = ("2026-11-08-t2smap-replica", "2026-11-08-t2smap-replica-check")
FINDINGS = "docs/findings/tedana-t2smap.md"
#: fMRIPrep's calls shown as examples (the record's case names, without ``fmriprep/``).
SHOWN = ("ds003592-sub-280-rest-curvefit", "ds000258-sub-21262-rest-curvefit")
#: The call whose optimally combined series is shown, and how many of its volumes.
SERIES = "ds003592-sub-280-rest-curvefit"
SERIES_SHOWN = 3
#: The memory rule's calls (tests/parity/test_memory_peak.py's T2SMAP_CASES).
MEMORY_CALLS = ("ds003592-sub-280-rest-curvefit", "ds003592-sub-280-rest-loglin")
#: The settings every run shares (the parity suite's): what RUN_INFO records of each run.
SETTINGS = {
    "OPENBLAS_CORETYPE": "Haswell",
    "NPY_DISABLE_CPU_FEATURES": "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL "
    "AVX512_ICL AVX512_SPR",
    "OPENBLAS_NUM_THREADS": "1",
    "COLUMNS": "80",
    "TQDM_DISABLE": "1",
}
#: The runs of a call's entry: label, program, settings (see ``_go``).
RUNS = [
    ("tedana", "tedana", {"env": "newest"}),
    ("replica", "replica", {"mode": "reproduce", "threads": 1}),
    ("replica-14", "replica", {"mode": "reproduce", "threads": 14}),
    ("replica-fix", "replica", {"mode": "fix", "threads": 14}),
    ("tedana-avx512", "tedana", {"env": "newest", "avx512": True}),
    ("tedana-shared", "tedana", {"env": "shared"}),
    ("tedana-threads4", "tedana", {"env": "newest", "threads": 4}),
    ("tedana-25", "fmriprep-tedana", {}),
    ("tedana-25-avx512", "fmriprep-tedana", {"avx512": True}),
]
#: The maps an example shows: output id, file (tedana's BIDS names), label, unit.
MAPS = [
    ("t2s", "T2starmap.nii.gz", "T2* (s)", "s"),
    ("s0", "S0map.nii.gz", "S0", ""),
    ("t2s-limited", "desc-limited_T2starmap.nii.gz", "T2*, limited (s)", "s"),
    ("s0-limited", "desc-limited_S0map.nii.gz", "S0, limited", ""),
    ("mask", "desc-adaptiveGoodSignal_mask.nii.gz", "Adaptive mask", ""),
    ("failures", "desc-decayFitFailures_mask.nii.gz", "Fit failures", ""),
]
#: Every file the table of files compares (images and the confounds table), in tedana's order.
#: The RMSE map holds NaN outside the voxels of two good echoes or more: the viewer shows no
#: NaN, so that map is compared in the table only.
FILES = (
    "desc-adaptiveGoodSignal_mask.nii.gz",
    "desc-decayFitFailures_mask.nii.gz",
    "S0map.nii.gz",
    "desc-rmse_statmap.nii.gz",
    "desc-confounds_timeseries.tsv",
    "desc-limited_S0map.nii.gz",
    "desc-limited_T2starmap.nii.gz",
    "desc-optcom_bold.nii.gz",
    "T2starmap.nii.gz",
)
#: The table of files' comparisons: label, run A, run B.
FILE_PAIRS = [
    ("replica − tedana 26.0.3", "replica", "tedana"),
    ("14 − 1 thread", "replica-14", "replica"),
    ("default − reproduce", "replica-fix", "replica-14"),
    ("tedana: AVX-512 on − off", "tedana-avx512", "tedana"),
    ("tedana: shared − newest", "tedana-shared", "tedana"),
    ("tedana: 4 threads − 1", "tedana-threads4", "tedana"),
    ("tedana 25.0.1 − 26.0.3", "tedana-25", "tedana"),
    ("tedana 25.0.1: AVX-512 on − off", "tedana-25-avx512", "tedana-25"),
]
OPTCOM = "desc-optcom_bold.nii.gz"
#: Why tedana 26.0.3's outputs differ from tedana 25.0.1's (fMRIPrep 25.2.6's) on its calls.
VERSION_STEP = (
    "tedana 26.0.x caps only the limited T2* map (TED-05; 25.0.1 capped the full map, as "
    "larmorx's default does), loads the data as float32 (25.0.1: float64), applies the T2* floors "
    "to the full map and no longer clamps negative values; and fMRIPrep's image has other "
    "libraries (numpy 2.2.6, which sums more than 8,192 elements in chunks, SciPy 1.15.2, "
    "conda-forge's OpenBLAS)"
)
#: Why tedana's outputs change from the shared environment's releases to the newest.
RELEASE_STEP = {
    "curvefit": "SciPy 1.15.3 → 1.18.1 changed least_squares' trust-region reflective method "
    "and its finite differences (the order of their steps, the SVD factors' memory order): the "
    "curve fit's last bits, which the full T2* map magnifies where T2* is large",
    "loglin": "numpy 2.3.5 → 2.5.4 brought LAPACK 3.12.0 → 3.12.1 (dgelsd's QR applies its "
    "reflectors another way): the last bits of a voxel's log-linear slope now and then",
}
#: fMRIPrep's own files of a t2smap node (the two it uses).
FMRIPREP_FILES = {"t2s": "T2starmap.nii.gz", "optcom": OPTCOM}
#: The panels of runs: id (= run label), role, label, sub.
PANELS = {
    "tedana": ("original", "tedana 26.0.3",
               "numpy 2.5.4, SciPy 1.18.1, AVX-512 kernels off, one thread: larmorx's reference"),
    "replica": ("larmorx", "larmorx (replica), 1 thread", "reproduce mode"),
    "replica-14": ("larmorx", "larmorx (replica), 14 threads", "reproduce mode"),
    "replica-fix": ("larmorx", "larmorx (replica), 14 threads", "default mode (D13 corrections)"),
    "tedana-avx512": ("original", "tedana 26.0.3", "numpy's AVX-512 kernels on (this CPU's default)"),
    "tedana-shared": ("original", "tedana 26.0.3",
                      "numpy 2.3.5, SciPy 1.15.3: the shared environment's releases"),
    "fmriprep": ("fmriprep", "fMRIPrep 25.2.6's own file",
                 "tedana 25.0.1 in fMRIPrep's image, in the oracle run"),
}  # fmt: skip


def _suite():
    from larmorx_validation.parity import tedana_t2smap_replica as tt

    return tt


# ------------------------------------------------------------------------------- the calls


def call_args(name: str) -> list[str]:
    """fMRIPrep's t2smap call ``name`` (its node's ``command.txt``, paths on this machine)."""
    for n, a in _suite()._investigation("check_stages").real_cases():
        if n == name:
            return a
    raise FileNotFoundError(f"fMRIPrep's t2smap call {name} (its oracle run)")


def parse(args: list[str]) -> dict:
    """The echoes, echo times, mask and fit type of a t2smap command line."""
    out: dict = {"echoes": [], "tes": [], "mask": None, "fittype": "loglin"}
    key = None
    for a in args:
        if a.startswith("-"):
            key = {"-d": "echoes", "-e": "tes", "--mask": "mask", "--fittype": "fittype"}.get(a)
            continue
        if key == "echoes":
            out["echoes"].append(Path(a))
        elif key == "tes":
            out["tes"].append(float(a))
        elif key in ("mask", "fittype"):
            out[key] = Path(a) if key == "mask" else a
    return out


def node_dir(name: str) -> Path:
    d = _suite()._node_dir(name)
    if d is None:
        raise FileNotFoundError(f"fMRIPrep's t2smap node of {name}")
    return d


def _dataset(name: str) -> dict:
    """A call's dataset as the credits name it (from larmorx-testdata's catalog)."""
    import larmorx_testdata as td

    ds, sub = name.split("-")[0], "sub-" + name.split("-")[2]
    c = td.catalog().collections[f"openneuro-{ds}"]
    title = re.sub(r"\s*\(OpenNeuro[^)]*\)\s*$", "", c.title)
    return {"dataset": f"OpenNeuro {ds} ({title})", "subject": sub, "licence": c.licence}


def _fmriprep_host(name: str) -> str:
    """Where fMRIPrep's own run of a call was made (its RUN_INFO.txt's host line)."""
    import os

    run = node_dir(name).relative_to(common.oracles() / "fmriprep-25.2.6").parts[0]
    text = (common.fmriprep_run(run) / "RUN_INFO.txt").read_text(encoding="utf-8")
    m = re.search(r"^host: .*?(\d+) CPUs", text, re.M)
    cpus = int(m.group(1)) if m else None
    if cpus == os.cpu_count():
        return "made on this machine, where numpy uses its AVX-512 kernels by default (NP-04)"
    if run in NO_AVX512_RUNS:
        return (
            f"made on {NO_AVX512_RUNS[run]} ({cpus} logical CPUs), where numpy computes float64 "
            "exp and log with glibc's functions"
        )
    return f"made on another machine ({cpus} CPUs)"


def _task(name: str) -> str:
    return name.split("-")[3]


# --------------------------------------------------------------------------- the runs


def _record(e: fcache.Entry, label: str, argv: list[str], env: dict, r, seconds: float) -> None:
    run = fcache.Run(
        label, [str(a) for a in argv], env, r.code, round(seconds, 2), r.stderr[-2000:]
    )
    e.runs.append(run)
    (e.dir / f"{label}.log").write_text(
        f"$ {shlex.join(run.argv)}\n# env: {env}\n# exit {r.code}, {run.seconds} s\n"
        f"--- stdout\n{r.stdout}\n--- stderr\n{r.stderr}",
        encoding="utf-8",
    )
    if r.code != 0:
        raise RuntimeError(f"{e.case}/{label} exited with {r.code}:\n{r.stderr[-2000:]}")


def _go(e: fcache.Entry, label: str, program: str, kw: dict, args: list[str]):
    """One run of a call's entry into ``<entry>/<label>/out``: tedana through the oracle-output
    cache, tedana 25.0.1 in fMRIPrep's image (the same), or the replica program."""
    tt = _suite()
    cwd = e.dir / label
    if cwd.exists():
        shutil.rmtree(cwd)
    cwd.mkdir(parents=True)
    t0 = time.perf_counter()
    if program == "tedana":
        env, threads = kw["env"], kw.get("threads", 1)
        full = [*args, "--n-threads", str(threads)] if threads != 1 else list(args)
        r = tt.run_tedana(env, full, cwd, avx512=kw.get("avx512", False), threads=threads)
        settings = dict(SETTINGS)
        if kw.get("avx512"):
            settings.pop("NPY_DISABLE_CPU_FEATURES")
        if threads != 1:
            settings["OPENBLAS_NUM_THREADS"] = str(threads)
        argv = [str(tt.launcher(env)), *full, "--out-dir", "out"]
    elif program == "fmriprep-tedana":
        binds = sorted({p.parent for p in parse(args)["echoes"]} | {parse(args)["mask"].parent})
        r = tt.run_fmriprep_tedana(args, cwd, binds, avx512=kw.get("avx512", False))
        settings = dict(SETTINGS)
        if kw.get("avx512"):
            settings.pop("NPY_DISABLE_CPU_FEATURES")
        argv = ["apptainer", "exec", "--cleanenv", str(tt.workspace() / tt.FMRIPREP_IMAGE),
                "t2smap", *args, "--out-dir", "out"]  # fmt: skip
    else:
        r = tt.run_replica("newest", args, cwd, kw["mode"], threads=kw["threads"])
        settings = {**SETTINGS, "LARMORX_UPSTREAM_DEFECTS": kw["mode"],
                    "LARMORX_TEDANA_LIBRARIES": "newest"}  # fmt: skip
        if kw["threads"] == 1:
            # the parity suite's run_replica says the one thread there (without t2smap's own
            # --n-threads the program takes OMP_NUM_THREADS, else every logical CPU)
            settings["OMP_NUM_THREADS"] = "1"
        extra = [] if kw["threads"] == 1 else ["--n-threads", str(kw["threads"])]
        argv = ["larmorx-gpl", "tedana", "t2smap", *args, *extra, "--out-dir", "out"]
    _record(e, label, argv, settings, r, time.perf_counter() - t0)
    return r


def _versions() -> dict[str, str]:
    tt = _suite()
    return {
        "tedana": f"tedana {tt.TEDANA} in oracles/python-newest ({tt.RELEASES['newest']}: "
        f"larmorx's reference) and in the shared .venv ({tt.RELEASES['shared']})",
        "fmriprep": f"tedana {tt.FMRIPREP_TEDANA} in fMRIPrep 25.2.6's image",
        "replica": tt.replica_version(),
    }


def _compute_call(name: str):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.parity import defects

        args = call_args(name)
        p = parse(args)
        for i, f in enumerate(p["echoes"]):
            e.input(f"echo {i + 1} (fMRIPrep's)", f)
        e.input("mask (fMRIPrep's dilated BOLD mask)", p["mask"])
        node = node_dir(name)
        for k, f in FMRIPREP_FILES.items():
            e.input(f"fMRIPrep's own {k}", node / f)
        named = {}
        for label, program, kw in RUNS:
            r = _go(e, label, program, kw, args)
            if program == "replica":
                named[label] = defects.notices_in(r.stderr)
        e.finish(
            versions=_versions(),
            node=str(node.relative_to(common.oracles() / "fmriprep-25.2.6")),
            named=named,
            settings="fMRIPrep 25.2.6's t2smap call of the run (its node's command.txt), with "
            "--out-dir out in each run's folder; tedana through the oracle-output cache (the "
            "parity suite's launchers); the replica as the larmorx-gpl program; "
            + ", ".join(f"{k}={v}" for k, v in SETTINGS.items())
            + " unless a run's own settings say otherwise",
        )

    return compute


def _call_description(name: str) -> str:
    return (
        f"fMRIPrep's t2smap call {name}: tedana 26.0.3 (reference setup, AVX-512 on, shared "
        "releases, 4 threads), tedana 25.0.1 in fMRIPrep's image (numpy's AVX-512 kernels off "
        "and on), the replica (1 and 14 threads, both modes)"
    )


def call_entry(ctx: core.Context, name: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"call-{name}",
        _call_description(name),
        _compute_call(name),
    )


_PEAK = re.compile(r"Maximum resident set size \(kbytes\): (\d+)")


def _compute_memory(e: fcache.Entry) -> None:
    tt = _suite()
    peaks: dict[str, dict[str, float]] = {}
    exe = tt.replica_binary()
    if exe is None or not Path("/usr/bin/time").is_file():
        raise FileNotFoundError("larmorx-gpl or /usr/bin/time")
    for name in MEMORY_CALLS:
        args = call_args(name)
        peaks[name] = {}
        for who, argv, extra in (
            ("tedana", [str(tt.launcher("newest")), *args], {}),
            ("larmorx-reproduce", [str(exe), "tedana", "t2smap", *args],
             {"LARMORX_UPSTREAM_DEFECTS": "reproduce", "LARMORX_TEDANA_LIBRARIES": "newest",
              "OMP_NUM_THREADS": "1"}),
            ("larmorx-fix", [str(exe), "tedana", "t2smap", *args],
             {"LARMORX_UPSTREAM_DEFECTS": "fix", "LARMORX_TEDANA_LIBRARIES": "newest",
              "OMP_NUM_THREADS": "1"}),
        ):  # fmt: skip
            label = f"{name}-{who}"
            cwd = e.dir / label
            if cwd.exists():
                shutil.rmtree(cwd)
            cwd.mkdir(parents=True)
            env = {**tt._base_env(), **extra}
            full = ["/usr/bin/time", "-v", *argv, "--out-dir", "out"]
            t0 = time.perf_counter()
            pr = subprocess.run(full, cwd=cwd, env=env, capture_output=True, text=True,
                                timeout=3600, check=False)  # fmt: skip
            r = tt.Run(pr.returncode, pr.stdout, pr.stderr, cwd)
            shown = ["/usr/bin/time", "-v", *(["larmorx-gpl", *argv[1:]] if who != "tedana"
                                              else argv), "--out-dir", "out"]  # fmt: skip
            _record(e, label, shown, {**SETTINGS, **extra}, r, time.perf_counter() - t0)
            m = _PEAK.search(pr.stderr)
            peaks[name][who] = int(m.group(1)) / 1024 if m else float("nan")
            shutil.rmtree(cwd / "out", ignore_errors=True)
    e.finish(
        versions=_versions(),
        peaks_mib=peaks,
        settings="each call in a process of its own, one thread (the replica: OMP_NUM_THREADS=1, "
        "since without t2smap's own --n-threads it takes every logical CPU, 2026-11-11), its peak "
        "resident memory from /usr/bin/time -v; tedana's launcher run directly (not through the "
        "oracle-output cache), the replica as the larmorx-gpl program",
    )


MEMORY_DESCRIPTION = (
    "the memory rule's calls (fMRIPrep's ds003592 call, curvefit and loglin): tedana 26.0.3 "
    "and the replica in both modes, one thread, peak resident memory"
)


def memory_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, "memory", MEMORY_DESCRIPTION, _compute_memory)


def _inputs_current(info: dict) -> bool:
    """Every input of an entry still the file it was (its SHA-256)."""
    from larmorx_validation.figures.core import sha256

    for v in info.get("inputs", {}).values():
        m = re.fullmatch(r"(.*) \(sha256 ([0-9a-f]{64})\)", v)
        if m is None or not Path(m[1]).is_file() or sha256(Path(m[1])) != m[2]:
            return False
    return True


def refresh(ctx: core.Context) -> None:
    """Bring the shown calls' entries and the memory entry up to this commit (relay RS3,
    2026-11-12): an entry whose inputs changed (ds000258's oracle run made again on 2026-11-09
    with OpenBLAS's Haswell kernels: its mask and fMRIPrep's own files) is computed again; in the
    others the replica's runs are made again with this commit's replica and the runs added to
    ``RUNS`` since are made, tedana's runs kept (the oracle-output cache serves them anyway);
    the memory entry is computed again (the replica measured there was larmorx-gpl 0.0.1)."""
    from larmorx_validation.figures.core import git_commit
    from larmorx_validation.parity import defects

    fctx = ctx.figures_context()
    for name in SHOWN:
        e = fcache.Entry(fctx, TOOL, f"call-{name}", _call_description(name))
        if not e.done or not _inputs_current(e.info()):
            fcache.recompute(fctx, TOOL, e.case, _call_description(name), _compute_call(name),
                             why="its inputs changed: the oracle run made again on 2026-11-09 "
                             "with OpenBLAS's Haswell kernels")  # fmt: skip
            continue
        info = e.info()
        have = {r["label"] for r in info["runs"]}
        args = call_args(name)
        named = {}
        for label, program, kw in RUNS:
            if program == "replica" or label not in have:
                r = _go(e, label, program, kw, args)
                if program == "replica":
                    named[label] = defects.notices_in(r.stderr)
        new = {r.label: r.__dict__ for r in e.runs}
        added = [x for x in new if x not in have]
        info["runs"] = [new.get(r["label"], r) for r in info["runs"]] + [new[x] for x in added]
        info["named"] = {**info.get("named", {}), **named}
        info["versions"] = _versions()
        info["description"] = _call_description(name)
        previous = info.get("larmorx_commit")
        info["larmorx_commit"] = git_commit(ctx.repo)
        info.setdefault("rerun", []).append({
            "computed": time.strftime("%Y-%m-%dT%H:%M:%S+00:00", time.gmtime()),
            "labels": sorted(new),
            "added": added,
            "previous_larmorx_commit": previous,
            "why": "the replica's runs made again with the replica of this commit (its one-thread "
            "run with OMP_NUM_THREADS=1, as the parity suite gives it)"
            + (f"; added: {', '.join(added)} (new runs of RUNS)" if added else ""),
        })  # fmt: skip
        e.info_path.write_text(json.dumps(info, indent=1) + "\n", encoding="utf-8")
    fcache.recompute(
        fctx,
        TOOL,
        "memory",
        MEMORY_DESCRIPTION,
        _compute_memory,
        why="the replica of this commit, at one thread (OMP_NUM_THREADS=1)",
    )


#: fMRIPrep's oracle runs made on another machine, which has no AVX-512 (their RUN_INFO.txt names
#: its 12 logical CPUs): there numpy computes float64 exp and log with glibc's functions.
NO_AVX512_RUNS = {
    "ds003592-sub-280-noreconall": "the project's earlier main machine, an Intel Core i7-8750H "
    "(AVX2, no AVX-512)",
}
#: fMRIPrep's curve-fit calls whose own files tedana 25.0.1 is run against here (every one).
T25_CALLS = (
    "ds000210-sub-02-cuedSGT-curvefit",
    "ds000210-sub-02-rest-curvefit",
    "ds000258-sub-21262-rest-curvefit",
    "ds003592-sub-280-rest-curvefit",
)


def _compute_t25(e: fcache.Entry) -> None:
    tt = _suite()
    counts: dict[str, dict] = {}
    for name in T25_CALLS:
        args = call_args(name)
        p = parse(args)
        node = node_dir(name)
        for i, f in enumerate(p["echoes"]):
            e.input(f"{name}: echo {i + 1} (fMRIPrep's)", f)
        e.input(f"{name}: mask", p["mask"])
        for f in tt.FMRIPREP_FILES:
            e.input(f"{name}: fMRIPrep's own {f}", node / f)
        counts[name] = {"host": _fmriprep_host(name)}
        for avx in (False, True):
            label = f"{name}-{'avx512' if avx else 'off'}"
            r = _go(e, label, "fmriprep-tedana", {"avx512": avx}, args)
            counts[name][label.rsplit("-", 1)[1]] = {
                f: list(v) for f, v in tt.value_differences(node, r.out, tt.FMRIPREP_FILES).items()
            }
            shutil.rmtree(r.out, ignore_errors=True)  # the counts are kept; the cache has them
    e.finish(
        versions={"fmriprep": f"tedana {tt.FMRIPREP_TEDANA} in fMRIPrep 25.2.6's image"},
        counts=counts,
        settings="tedana 25.0.1 in fMRIPrep 25.2.6's image (Apptainer, --cleanenv) through the "
        "oracle-output cache, on fMRIPrep's own call of each oracle run (its node's command.txt), "
        "OPENBLAS_CORETYPE=Haswell, one BLAS thread, numpy's AVX-512 kernels off and on; the "
        "values of fMRIPrep's T2* map and combined series that differ (the parity suite's "
        "value_differences); the outputs removed once counted",
    )


def t25_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "fmriprep-tedana-25",
        "fMRIPrep's own T2* maps and combined series against tedana 25.0.1 run here in "
        "fMRIPrep's image, numpy's AVX-512 kernels off and on, on fMRIPrep's four curve-fit calls",
        _compute_t25,
    )


# -------------------------------------------------------------------------- the outputs


def _stored(path: Path) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(str(path)).dataobj)


def _out(e: fcache.Entry, label: str) -> Path:
    return e.dir / label / "out"


def _maps(e: fcache.Entry, label: str) -> dict[str, np.ndarray]:
    d = _out(e, label)
    return {oid: _stored(d / f) for oid, f, _l, _u in MAPS if (d / f).is_file()}


def _diff_count(a: np.ndarray, b: np.ndarray) -> int:
    """How many values differ (NaN alike), or -1 when the shapes differ."""
    if a.shape != b.shape:
        return -1
    if a.dtype == b.dtype and a.tobytes() == b.tobytes():
        return 0
    x, y = np.asarray(a, np.float64), np.asarray(b, np.float64)
    return int(np.count_nonzero(~((x == y) | (np.isnan(x) & np.isnan(y)))))


def _named(e: fcache.Entry, label: str = "replica-fix") -> list[str]:
    return [n for n in e.info().get("named", {}).get(label, []) if n]


# ---------------------------------------------------------------------------- the links


def _links(repo: Path) -> dict[str, str]:
    """TED-xx, NP-03/04 and GLIBC-07 -> where they are described (the findings document's
    defects section; TED-05's own section; the register for GLIBC-07)."""
    text = (repo / FINDINGS).read_text(encoding="utf-8")
    out: dict[str, str] = {}
    for line in text.splitlines():
        if line.startswith("## "):
            head = line.lstrip("#").strip()
            if head.startswith("Defects"):
                sec = f"{FINDINGS}#{core.github_anchor(head)}"
                for i in range(1, 31):
                    out.setdefault(f"TED-{i:02d}", sec)
                out["NP-03"] = out["NP-04"] = sec
            elif head.startswith("TED-05"):
                out["TED-05"] = f"{FINDINGS}#{core.github_anchor(head)}"
    reg = core.register(repo)
    if "GLIBC-07" in reg:
        out["GLIBC-07"] = reg["GLIBC-07"]["link"]
    return out


def _link(links: dict[str, str], ids) -> list[dict]:
    return [{"label": i, "href": links[i]} for i in ids if i in links]


# ------------------------------------------------------------------ the clean room's runs

#: The clean room's runs on each shown call (the documentation revision, 2026-11-13: the
#: examples showed tedana and the replica only): LARMORX's clean room (crate larmorx-tedana)
#: through its binding, as its own example runs it (``tedana_t2smap_clean_room.RUNNER`` and its
#: settings, a black box), in `reproduce` at one thread and in `fix` at eight and at one. Each:
#: the run's id here, its D13 mode, its threads, its label in the clean room's own example.
CLEAN_RUNS = [
    ("clean-reproduce", "reproduce", 1, "original-reproduce"),
    ("clean", "fix", 8, "original"),
    ("clean-1", "fix", 1, "original-1"),
]
#: A shown call whose clean-room runs the clean room's own example made: the same call (its
#: echoes' and mask's SHA-256 those of the call's entry, checked when the example is made), so
#: no run is made twice.
CLEAN_ELSEWHERE = {"ds003592-sub-280-rest-curvefit": "fmriprep-ds003592-curvefit-original"}
_CLEAN_NOTICE = re.compile(r"^larmorx: upstream defect (\S+) \((\w+)\) (corrected|reproduced)")


def _clean_description(name: str) -> str:
    return (
        f"fMRIPrep's t2smap call {name}: LARMORX's clean room (crate larmorx-tedana) through "
        "its binding, reproduce at 1 thread, fix at 8 and at 1"
    )


def _compute_clean(name: str):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.evidence import tedana_t2smap_clean_room as cr

        args = call_args(name)
        p = parse(args)
        for i, f in enumerate(p["echoes"]):
            e.input(f"echo {i + 1} (fMRIPrep's)", f)
        e.input("mask (fMRIPrep's dilated BOLD mask)", p["mask"])
        unset = [k for k in os.environ if k.startswith(("NPY_", "OPENBLAS_", "OMP_"))]
        notices: dict[str, list[str]] = {}
        for label, mode, n, _theirs in CLEAN_RUNS:
            d = e.dir / label
            if d.is_dir():
                shutil.rmtree(d)
            d.mkdir()
            words = [*args, "--n-threads", str(n), "--out-dir", "out"]
            e.run(label, [sys.executable, "-c", cr.RUNNER, *words], env=cr._settings(mode),
                  unset=unset, cwd=d)  # fmt: skip
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            notices[label] = sorted(
                {m.group(1) for x in log.splitlines() if (m := _CLEAN_NOTICE.match(x))}
            )
        e.finish(
            versions={"larmorx": "the clean room (crate larmorx-tedana) through "
                      "larmorx._core.tedana_t2smap_cleanroom, as the clean room's own example "
                      "runs it"},
            notices=notices,
            threads={label: str(n) for label, _m, n, _t in CLEAN_RUNS},
            node=str(node_dir(name).relative_to(common.oracles() / "fmriprep-25.2.6")),
            settings="fMRIPrep 25.2.6's t2smap call of the run (its node's command.txt), with "
            "--n-threads and --out-dir out in each run's folder; the clean room's settings of "
            "its own example (tedana_t2smap_clean_room._settings: the D13 mode)",
        )  # fmt: skip

    return compute


def clean_entry(ctx: core.Context, name: str) -> tuple[fcache.Entry, dict[str, str]]:
    """The clean room's runs of the shown call ``name``: its entry and each run's folder name
    there (the clean room's own example's entry where it holds the same call)."""
    if name in CLEAN_ELSEWHERE:
        from larmorx_validation.evidence import tedana_t2smap_clean_room as cr

        e = cr.entry(ctx)
        if e.case != CLEAN_ELSEWHERE[name]:
            raise ValueError(f"the clean room's example is {e.case}, not {CLEAN_ELSEWHERE[name]}")
        return e, {label: theirs for label, _m, _n, theirs in CLEAN_RUNS}
    e = fcache.entry(
        ctx.figures_context(), TOOL, f"clean-room-{name}", _clean_description(name),
        _compute_clean(name),
    )  # fmt: skip
    return e, {label: label for label, *_ in CLEAN_RUNS}


def _same_inputs(call: dict, clean: dict) -> None:
    """The clean room's runs read the call's own echoes and mask (their SHA-256)."""

    def shas(info: dict) -> list[str]:
        return [
            v.rsplit("sha256 ", 1)[1].rstrip(")")
            for k, v in info["inputs"].items()
            if k.startswith(("echo", "mask", "fMRIPrep's dilated"))
        ]

    if shas(call) != shas(clean):
        raise ValueError(f"{call['case']}: the clean room's runs read other inputs")


def _clean_part(ctx: core.Context, name: str, e: fcache.Entry) -> dict:
    """The clean room's runs of a shown call: their folders, notices, entry and its info."""
    ce, folders = clean_entry(ctx, name)
    info = ce.info()
    _same_inputs(e.info(), info)
    notices = info.get("notices", {})
    return {
        "entry": ce,
        "info": info,
        "dirs": {label: ce.dir / folders[label] for label in folders},
        "notices": {label: [n for n in notices.get(folders[label], []) if n] for label in folders},
    }


def _clean_maps(part: dict, label: str) -> dict[str, np.ndarray]:
    d = part["dirs"][label] / "out"
    return {oid: _stored(d / f) for oid, f, _l, _u in MAPS if (d / f).is_file()}


#: The clean room's panels: id, label, second line (in the shapes the presenter reads).
CLEAN_PANELS = {
    "clean-reproduce": ("larmorx (clean room), 1 thread", "reproduce mode"),
    "clean": ("larmorx (clean room), 8 threads", "default mode (D13 corrections)"),
}


def _clean_diffs() -> list[dict]:
    d = common.d
    return [
        d("cr-ted", "clean-reproduce", "tedana", "main",
          "clean-room original (reproduce) − tedana 26.0.3", common.ROLE_LX),
        d("cr1-cr", "clean-1", "clean", "check", "clean-room original, 1 thread − 8 threads",
          common.ROLE_SELF),
        d("crfix-cr", "clean", "clean-reproduce", "check",
          "clean-room original: default − reproduce mode", common.ROLE_SELF),
        d("crfix-rfix", "clean", "replica-fix", "check",
          "clean-room original − replica, both default", common.ROLE_SELF),
        d("crfix-fmriprep", "clean", "fmriprep", "fmriprep",
          "clean-room original, default − fMRIPrep 25.2.6's own file", common.ROLE_FMRIPREP),
    ]  # fmt: skip


def _check_tedana(e: fcache.Entry, other: Path) -> tuple[bool, str, tuple[int, int] | None]:
    """check.py (§17.3, the clean room's suite) of a run's folder against tedana's run of the
    call's entry. That run's folder holds tedana's files as the oracle-output cache gave them
    back, without the empty ``figures`` folder tedana writes (the clean room's own example's
    tedana run has it, and both ports write it): the check reads tedana's files from a folder
    that links each of them and holds that empty folder, so it compares the files, not the
    replay's folder."""
    import tempfile

    from larmorx_validation.evidence import tedana_t2smap_clean_room as cr

    src = e.dir / "tedana" / "out"
    if (src / "figures").is_dir():
        return cr._check(e.dir / "tedana", other)
    with tempfile.TemporaryDirectory(prefix="t2smap-tedana-") as tmp:
        out = Path(tmp) / "tedana" / "out"
        out.mkdir(parents=True)
        for f in src.iterdir():
            (out / f.name).symlink_to(f)
        (out / "figures").mkdir()
        return cr._check(out.parent, other)


def _clean_verdicts(diffs: list[dict], part: dict, e: fcache.Entry, links: dict,
                    capped: int | None, series: bool = False) -> None:  # fmt: skip
    """The clean room's comparisons' verdicts and reasons: with tedana and between the two
    ports' defaults by specs/t2smap.md §17.3's check.py (the clean room's suite, as its own
    example judges them), the rest from their metrics and the runs' notices."""
    from larmorx_validation.evidence import tedana_t2smap_clean_room as cr

    same = common.identical(diffs)
    fixed = part["notices"]["clean"]

    def counted(c: tuple[int, int] | None) -> str:
        return f"{c[0]:,} of {c[1]:,} compared values identical" if c else "values compared"

    for x in diffs:
        i = x["id"]
        if i not in {"cr-ted", "cr1-cr", "crfix-cr", "crfix-rfix", "crfix-fmriprep"}:
            continue
        n, total = _count(x), _ncomp(x)
        x["links"] = []
        if i == "cr-ted":
            ok, det, cnt = _check_tedana(e, part["dirs"]["clean-reproduce"])
            x["verdict"] = "bit-identical" if same[i] else ("within-variability" if ok else "open")
            x["reason"] = common.clip(
                f"specs/t2smap.md §17.3 by its check.py, over the run's files: {counted(cnt)}; "
                f"the curve fit's {cr.BC} share {cr._share(det)} of the fit voxels (the clean "
                "room's own trust-region solver, its log-linear fit in closed form: the fitted "
                "numbers within the thresholds, not bit for bit)",
                420,
            )
            x["links"] = [{"label": "§17.3", "href": cr.SPEC}]
        elif i == "cr1-cr":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The same bits at 1 and 8 threads.") if same[i]
                else ("open", f"{n:,} values differ between 1 and 8 threads."))  # fmt: skip
        elif i == "crfix-cr":
            if same[i]:
                x["verdict"] = "bit-identical"
                x["reason"] = "No correction of the register changes this call's values."
            else:
                x["verdict"] = "d13"
                cap = f" TED-05 caps {capped:,} voxels of the full T2* map." if capped else ""
                x["reason"] = common.clip(
                    "The default's corrections, each named by its notice: "
                    + ("; ".join(f"{t} ({cr.WHAT.get(t, 'see the register')})" for t in fixed)
                       or "none")
                    + f": {n:,} of {total:,} values change.{cap}",
                    420,
                )  # fmt: skip
                x["links"] = _link(links, fixed)
        elif i == "crfix-rfix":
            ok, det, cnt = cr._check(e.dir / "replica-fix", part["dirs"]["clean"])
            x["verdict"] = "bit-identical" if same[i] else ("within-variability" if ok else "open")
            x["reason"] = common.clip(
                f"The two ports' defaults by §17.3 (check.py), over the run's files: "
                f"{counted(cnt)}; the {cr.BC} share {cr._share(det)}.",
                400,
            )
            x["links"] = [{"label": "§17.3", "href": cr.SPEC}]
        else:  # crfix-fmriprep
            x["verdict"] = "d13" if fixed else "within-variability"
            how = (
                "The combination weighs the echoes with the full T2* map, which the default "
                "caps as tedana 25.0.1 did (TED-05)."
                if series
                else "The full T2* map's cap (TED-05) is tedana 25.0.1's behaviour again."
            )
            x["reason"] = common.clip(
                "tedana's own step from 25.0.1 to 26.0.3, the default's corrections "
                f"({', '.join(fixed) or 'none'}) and the clean room's own solvers, within "
                f"§17.3's thresholds of tedana 26.0.3: {n:,} of {total:,} values differ. {how}",
                420,
            )
            x["links"] = _link(links, fixed)


def _clean_provenance(part: dict) -> str:
    """The provenance line of the clean room's runs (their entry, when, with which commit)."""
    info = part["info"]
    line = next(
        x for x in common.provenance(info, f"{TOOL}/{part['entry'].case}", []) if
        x.startswith("Runs: ")
    )  # fmt: skip
    return "The clean room's runs: " + line.removeprefix("Runs: ")


def _clean_fact(name: str, part: dict) -> dict:
    """The fact that names the clean room's runs of an example."""
    where = (
        "; these are the runs of its own example below, the same call"
        if name in CLEAN_ELSEWHERE
        else ""
    )
    return {
        "label": "Clean room",
        "text": "LARMORX's clean room (crate larmorx-tedana, experimental) through its binding "
        "`larmorx._core.tedana_t2smap_cleanroom`, as its own example runs it, at commit "
        f"`{part['info']['larmorx_commit'][:12]}`{where}",
    }


#: The metrics notes' sentences on the two records' thresholds: the replica's, then the clean
#: room's, by which its comparisons here are judged.
CLEAN_NOTE = (
    "The replica's record holds each call to tedana 26.0.3's files, messages and exit status "
    "bit for bit in `reproduce`. "
    "The clean room's comparisons with tedana 26.0.3 and with the replica's default are judged "
    "over the runs' files by specs/t2smap.md §17.3's check.py, its record's thresholds (tedana's "
    "files as stored for the call, with the empty `figures` folder tedana writes)."
)


# ------------------------------------------------------------------------- the examples


def _diffs() -> list[dict]:
    d = common.d
    out = [
        d("rep-ted", "replica", "tedana", "main", "larmorx (replica) − tedana 26.0.3",
          common.ROLE_LX),
        d("rep14-rep", "replica-14", "replica", "check", "larmorx: 14 threads − 1 thread",
          common.ROLE_SELF),
        d("fix-rep", "replica-fix", "replica-14", "check", "larmorx: default − reproduce mode",
          common.ROLE_SELF),
        d("rep-fmriprep", "replica", "fmriprep", "fmriprep",
          "larmorx (replica) − fMRIPrep 25.2.6's own file (tedana 25.0.1)", common.ROLE_FMRIPREP),
        d("fix-fmriprep", "replica-fix", "fmriprep", "fmriprep",
          "larmorx's default − fMRIPrep 25.2.6's own file", common.ROLE_FMRIPREP),
        d("avx-ted", "tedana-avx512", "tedana", "variability",
          "tedana: numpy's AVX-512 kernels on − off", common.ROLE_ORIG),
        d("shared-ted", "tedana-shared", "tedana", "variability",
          "tedana: numpy 2.3.5 / SciPy 1.15.3 − numpy 2.5.4 / SciPy 1.18.1", common.ROLE_ORIG),
        d("thr4-ted", "tedana-threads4", "tedana", "variability",
          "tedana: --n-threads 4 − 1", common.ROLE_ORIG),
        d("t25-ted", "tedana-25", "tedana", "variability",
          "tedana 25.0.1 (fMRIPrep's image, here) − tedana 26.0.3", "the original against itself: "
          "the version step"),
        d("t25-fmriprep", "tedana-25-avx512", "fmriprep", "variability",
          "tedana 25.0.1 here, numpy's AVX-512 kernels on (as in fMRIPrep's run) − fMRIPrep's "
          "own file", "the original against itself: the machine"),
        d("t25off-fmriprep", "tedana-25", "fmriprep", "variability",
          "tedana 25.0.1 here, numpy's AVX-512 kernels off − fMRIPrep's own file (tedana 25.0.1 "
          "in its run)", "the original against itself: the machine and numpy's kernels"),
    ]  # fmt: skip
    return out


def _count(x: dict) -> int:
    return sum(m["differ"] for m in x["metrics"].values())


def _ncomp(x: dict) -> int:
    return sum(m["voxels"] for m in x["metrics"].values())


def _verdicts(diffs: list[dict], named: list[str], links: dict, capped: int | None,
              what: str, host: str, series: bool = False) -> None:  # fmt: skip
    """Each comparison's verdict and reason, decided from its metrics."""
    by = {x["id"]: x for x in diffs}
    same = common.identical(diffs)
    what_shown = "the optimally combined series" if series else "the maps shown"
    for x in diffs:
        i = x["id"]
        x["links"] = []
        n, total = _count(x), _ncomp(x)
        if x["kind"] == "variability":
            x["verdict"] = None
            lead = "A reference row (the original against itself): "
            body = {
                "avx-ted": "numpy's float64 exp and log are Intel's SVML kernels on CPUs with "
                "AVX-512 (NP-04), glibc 2.35's otherwise; larmorx's reference is the path "
                "without them (decision D2)",
                "shared-ted": RELEASE_STEP.get(what, RELEASE_STEP["curvefit"]),
                "thr4-ted": "tedana's --n-threads 4 runs the curve fits in four joblib processes "
                "and lets OpenBLAS use four threads",
                "t25-ted": "tedana's own step from 25.0.1 (fMRIPrep 25.2.6's) to 26.0.3: "
                + VERSION_STEP,
                "t25-fmriprep": "tedana 25.0.1 in fMRIPrep's image, here with numpy's AVX-512 "
                "kernels on (this CPU's default: fMRIPrep's runs set only OpenBLAS's Haswell "
                "kernels), against fMRIPrep's own run of it, " + host,
                "t25off-fmriprep": "tedana 25.0.1 in fMRIPrep's image, here with numpy's "
                "AVX-512 kernels off (larmorx's reference setting: glibc 2.35's float64 exp and "
                "log, NP-04), against fMRIPrep's own run of it, " + host,
            }[i]
            x["reason"] = lead + (
                f"{body}: every one of the {total:,} values is the same."
                if same[i]
                else f"{body}: {n:,} of {total:,} values differ."
            )
            if i in ("avx-ted", "t25off-fmriprep"):
                x["links"] += _link(links, ["NP-04"])
            continue
        if i == "rep-ted":
            if same[i]:
                x["verdict"] = "bit-identical"
                x["reason"] = (
                    f"Every value of {what_shown} is tedana 26.0.3's ({total:,} values): the "
                    "adaptive mask, the log-linear fit (numpy's lstsq: LAPACK's dgelsd over each "
                    "group of voxels at once, with OpenBLAS's Haswell kernels), SciPy's bounded "
                    "trust-region curve fit voxel by voxel, the floors and the cap"
                    + (", the optimal combination" if series else "")
                    + ", written as nibabel and nilearn write them."
                )
            else:
                x["verdict"], x["reason"] = "open", "Not accounted for (see the record)."
        elif i == "rep14-rep":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = (
                "The same bytes at 1 and 14 threads: the replica divides the voxels between "
                "threads in fixed pieces, and the log-linear fit runs over each group at once as "
                "tedana's one lstsq call does."
                if same[i]
                else f"{n:,} values differ between the thread counts (see the record)."
            )
        elif i == "fix-rep":
            if same[i]:
                x["verdict"] = "bit-identical"
                x["reason"] = "No correction of the register changes this call's values."
            else:
                x["verdict"] = "d13"
                cap = (
                    f" TED-05: {capped:,} voxels of the full T2* map more than ten times the "
                    "cap, which tedana 26.0.x leaves uncapped (25.0.1 capped them), get the "
                    "limited map's cap."
                    if capped
                    else ""
                )
                x["reason"] = (
                    f"The default corrects {', '.join(named)} and names each where it changes a "
                    f"written value: {n:,} of {total:,} values change.{cap} NP-03: the "
                    "log-linear fit's float32 log correctly rounded (numpy's own kernel is not); "
                    "GLIBC-07: float64 exp, log and the curve fit's powers correctly rounded "
                    "(glibc 2.35's are not): the curve fit starts from other last bits and ends "
                    "within its tolerances elsewhere. `upstream_defects='reproduce'` gives "
                    "tedana's outputs."
                )
                x["links"] += _link(links, named)
        elif i == "rep-fmriprep":
            step = by.get("t25-ted")
            if same["rep-ted"]:
                x["verdict"] = "within-variability"
                x["reason"] = (
                    "The replica is tedana 26.0.3 bit for bit, so this is tedana's own step from "
                    f"25.0.1 (fMRIPrep's) to 26.0.3 ({n:,} of {total:,} values): "
                    + VERSION_STEP
                    + (
                        f". On this machine tedana 25.0.1 in fMRIPrep's image differs from "
                        f"tedana 26.0.3 in {_count(step):,} values."
                        if step and step.get("metrics")
                        else "."
                    )
                )
            else:
                x["verdict"], x["reason"] = "open", "The replica is not tedana 26.0.3 here."
        elif i == "fix-fmriprep":
            x["verdict"] = "d13" if named else "within-variability"
            x["reason"] = (
                f"tedana's step from 25.0.1 to 26.0.3 (the row above) and the default's "
                f"corrections ({', '.join(named) or 'none'}): {n:,} of {total:,} values differ. "
                "The full T2* map's cap (TED-05) is 25.0.1's behaviour again."
            )
            x["links"] += _link(links, named)


def _capped(rep: np.ndarray, fix: np.ndarray) -> int:
    """Voxels whose full T2* the default divides by more than ten: TED-05's cap (the maths
    corrections move a value by a few units in its last places only)."""
    a, b = rep.astype(np.float64), fix.astype(np.float64)
    with np.errstate(invalid="ignore", divide="ignore"):
        return int(np.count_nonzero((b > 0) & (a > 10 * b)))


def _show(mask: np.ndarray) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=1)


def _peak(ex, a: str, b: str, out: str, t: int | None = None) -> list[int]:
    """The default view's voxel: where A and B differ most over the shown region (in volume
    ``t`` of a series; the region's centre where they agree)."""
    va, vb = ex.values(ex.key(a, out)), ex.values(ex.key(b, out))
    if va.ndim == 4:
        va, vb = va[..., t or 0], vb[..., t or 0]
    with np.errstate(invalid="ignore"):
        d = np.where(ex.m, np.abs(va - vb), -1.0)
    d = np.nan_to_num(d, nan=-1.0, posinf=np.finfo(np.float64).max)
    if d.max() > 0:
        return [int(i) for i in np.unravel_index(int(np.argmax(d)), d.shape)]
    at, _ = common.centre_view(ex)
    return at


#: The T2* maps' long tail (floors, caps, a T2* of 500 ms where the decay is flat) puts their
#: 99.5th percentile far above tissue: their first window ends at the 95th.
TISSUE = ("t2s", "t2s-limited")


def _tissue_window(ctx: core.Context, ex, out: dict) -> None:
    """A first window for the T2* maps, 0 to tedana's 95th percentile over the shown region, and
    the default view's posters again in it (``common.finish`` drew them in the 0.5-99.5 %
    window, where tissue is near black)."""
    from larmorx_validation.evidence import example as exm

    win = {}
    for o in out["outputs"]:
        if o["id"] not in TISSUE:
            continue
        v = ex.values(ex.key("tedana", o["id"]))[ex.m]
        w = {"id": "tissue", "label": "brain, 0 to its 95th percentile (T2*'s long tail out)",
             "lo": 0.0, "hi": float(np.percentile(v, 95))}  # fmt: skip
        o["windows"].insert(0, w)
        win[o["id"]] = w
    d, post = out["default"], out["poster"]
    w = win.get(d["output"])
    if w is None:
        return
    plane, k = post["plane"], post["slice"]
    m2 = exm.plane_slice(ex.m, plane, k)
    for pid in post["panels"]:
        p = ex.panel(pid)
        if p.role == "input" or (p.overlay or {}).get(d["output"]):
            continue
        a = exm.plane_slice(ex.values(ex.key(pid, d["output"])), plane, k)
        post["panels"][pid] = exm.poster_gray(ctx, a, m2, w["lo"], w["hi"])


def _dtype(img) -> str:
    dt = img.get_data_dtype()
    return dt.name + (", big-endian" if dt.byteorder == ">" else "")


def _echo_facts(p: dict, img) -> str:
    tes = ", ".join(f"{t:g}" for t in p["tes"])
    z = img.header.get_zooms()
    return (
        f"{len(p['echoes'])} echoes (TE {tes} ms) of {' × '.join(map(str, img.shape[:3]))} "
        f"voxels of {' × '.join(f'{v:.3g}' for v in z[:3])} mm, {img.shape[3]} volumes (TR "
        f"{float(z[3]):g} s), {_dtype(img)}"
    )


def _example_maps(ctx: core.Context, name: str, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.evidence.fsl_bet import window_of

    e = call_entry(ctx, name)
    info = e.info()
    args = call_args(name)
    p = parse(args)
    ds = name.split("-")[0]
    mimg = nib.load(str(p["mask"]))
    mask = common.load(str(p["mask"])) > 0
    show = _show(mask)
    grid = core.Grid.around(mimg, show)
    ex = exm.Example(ctx, grid, show)
    eimg = nib.load(str(p["echoes"][0]))
    for k, f in enumerate(p["echoes"]):
        arr, sc, it = common.stored(f)
        ex.add(exm.Panel(f"echo{k + 1}", "input", f"{ds}: echo {k + 1} ({p['tes'][k]:g} ms)",
                         f"volume 1 of {arr.shape[3]}, as fMRIPrep gives it to t2smap (the "
                         "echo resampled onto the BOLD reference)",
                         {"all": np.ascontiguousarray(arr[..., 0])}, scale=sc, inter=it))  # fmt: skip
    outputs = [{"id": o, "label": label, "unit": u} for o, _f, label, u in MAPS]
    have = _maps(e, "tedana")
    outputs = [o for o in outputs if o["id"] in have]
    mk, ks, ki = common.stored(p["mask"])
    ex.add(exm.Panel("bmask", "input", f"{ds}: fMRIPrep's mask",
                     "the BOLD mask dilated by two voxels (t2smap's --mask), over echo 1",
                     {"all": mk}, scale=ks, inter=ki,
                     overlay={o["id"]: {"on": "echo1", "window": window_of(ex, "echo1")}
                              for o in outputs}))  # fmt: skip
    runs = {label: _maps(e, label) for label, *_ in RUNS}
    for pid in ("tedana", "replica", "replica-14", "replica-fix", "tedana-avx512", "tedana-shared"):
        role, label, sub = PANELS[pid]
        ex.add(exm.Panel(pid, role, label, sub, {o["id"]: runs[pid][o["id"]] for o in outputs}))
    part = _clean_part(ctx, name, e)
    clean = {label: _clean_maps(part, label) for label, *_ in CLEAN_RUNS}
    for pid, (label, sub) in CLEAN_PANELS.items():
        ex.add(
            exm.Panel(pid, "larmorx", label, sub, {o["id"]: clean[pid][o["id"]] for o in outputs})
        )
    node = node_dir(name)
    full = {
        "tedana-threads4": runs["tedana-threads4"],
        "tedana-25": runs["tedana-25"],
        "tedana-25-avx512": runs["tedana-25-avx512"],
        "fmriprep": {"t2s": _stored(node / FMRIPREP_FILES["t2s"])},
        "clean-1": clean["clean-1"],
    }
    diffs = _diffs() + _clean_diffs()
    common.compare_all(ex, diffs, outputs, full=full)
    named = _named(e)
    capped = _capped(runs["replica-14"]["t2s"], runs["replica-fix"]["t2s"])
    _verdicts(diffs, named, links, capped, p["fittype"], _fmriprep_host(name))
    _clean_verdicts(
        diffs, part, e, links, _capped(clean["clean-reproduce"]["t2s"], clean["clean"]["t2s"])
    )
    at = _peak(ex, "replica-fix", "replica-14", "t2s")
    default = {
        "output": "t2s",
        "plane": "axial",
        "voxel": at,
        "panels": ["echo1", "tedana", "replica", "replica-fix", "clean-reproduce"],
        "diffs": ["rep-ted", "fix-rep", "avx-ted"],
        "compare": {"a": "tedana", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="tedana",
        input_windows={"bmask": "full"},
        caption="the crosshair where the default's T2* map differs most from the reproduce "
        "mode's; choose the output above the images: the full T2* map (fMRIPrep's output) and S0, "
        "the limited maps, the adaptive mask (good echoes per voxel), the curve fit's failures.",
    )  # fmt: skip
    _tissue_window(ctx, ex, out)
    data = _dataset(name)
    n_vox = int(np.prod(mimg.shape[:3]))
    t2 = runs["tedana"]["t2s"].astype(np.float64)
    return {
        "id": f"maps-{ds}",
        "title": f"fMRIPrep's T2* and S0 maps of {ds}'s multi-echo run",
        "summary": f"fMRIPrep 25.2.6's t2smap call on OpenNeuro {ds} {data['subject']}'s "
        f"{_task(name)} run ({_echo_facts(p, eimg)}): tedana's adaptive mask (how many echoes "
        "have good signal in each voxel), the monoexponential fit of S0 and T2* in every voxel "
        "(log-linear first, then SciPy's bounded curve fit from it, `--fittype curvefit` as "
        "fMRIPrep calls it), the full and the limited maps and the fit's failures (the RMSE map "
        "and the confounds table are in the table of every file below). "
        "tedana 26.0.3 and the replica compute every map; fMRIPrep's own T2* map (tedana 25.0.1, "
        "the only map fMRIPrep keeps) and further runs of tedana are compared with them value by "
        "value. LARMORX's clean room computes every map too, in `reproduce` at one thread and in "
        "`fix`, its default, at eight threads and at one; its comparisons with tedana 26.0.3 and "
        "with the replica's default are judged by specs/t2smap.md §17.3's thresholds.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']}, task {_task(name)} "
                f"({data['licence']}): the echoes as fMRIPrep's node was given them; shown "
                "inside fMRIPrep's mask dilated by one voxel, so no face shows",
            },
            {
                "label": "Node",
                "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}` (`bold_t2smap_wf`, its "
                "`T2SMap` interface)",
            },
            {
                "label": "Command",
                "text": common.clip(common.command_text("t2smap", args, core.short_path), 600),
            },
            {
                "label": "Maps",
                "text": f"tedana's full T2* map ranges from {np.nanmin(t2):.4g} to "
                f"{np.nanmax(t2):.4g} s; its adaptive mask keeps "
                f"{int(np.count_nonzero(runs['tedana']['mask'])):,} voxels; the curve fit "
                f"failed in {int(np.count_nonzero(runs['tedana'].get('failures', 0))):,}",
            },
            {
                "label": "Programs",
                "text": "tedana 26.0.3 as its program (`t2smap`) in both kept environments, "
                "tedana 25.0.1 in fMRIPrep 25.2.6's image; larmorx's replica, the `larmorx-gpl` "
                f"program, at commit `{info['larmorx_commit'][:12]}`; numpy's OpenBLAS on its "
                "Haswell kernels, one thread",
            },
            _clean_fact(name, part),
            {"label": "Grid", "text": common.describe_grid(mimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against tedana 26.0.3"],
            ["fmriprep", "LARMORX against fMRIPrep's own file"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "tedana against itself: kernels, releases, threads, versions"],
        ],
        "metrics_note": f"Δ is A − B over all {n_vox:,} voxels of each output file, not only "
        "those shown; fMRIPrep keeps the full T2* map only, so its comparisons cover that map; "
        "tedana at four threads and tedana 25.0.1 are compared over their files; the RMSE map "
        "and the confounds table in the table of every file. " + CLEAN_NOTE,
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [data]),
            _clean_provenance(part),
        ],
    }


def _example_series(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    name = SERIES
    e = call_entry(ctx, name)
    info = e.info()
    args = call_args(name)
    p = parse(args)
    ds = name.split("-")[0]
    mimg = nib.load(str(p["mask"]))
    mask = common.load(str(p["mask"])) > 0
    show = _show(mask)
    grid = core.Grid.around(mimg, show)
    ex = exm.Example(ctx, grid, show)
    eimg = nib.load(str(p["echoes"][0]))
    n = int(eimg.shape[3])
    shown = [int(t) for t in np.linspace(0, n - 1, SERIES_SHOWN).round()]

    def vols(x: np.ndarray) -> np.ndarray:
        return np.ascontiguousarray(x[..., shown])

    for k, f in enumerate(p["echoes"]):
        arr, sc, it = common.stored(f)
        ex.add(exm.Panel(f"echo{k + 1}", "input", f"{ds}: echo {k + 1} ({p['tes'][k]:g} ms)",
                         "as fMRIPrep gives it to t2smap", {"all": vols(arr)}, scale=sc,
                         inter=it))  # fmt: skip
        del arr
    node = node_dir(name)
    files = {label: _out(e, label) / OPTCOM for label, *_ in RUNS}
    files["fmriprep"] = node / OPTCOM
    part = _clean_part(ctx, name, e)
    files.update({label: part["dirs"][label] / "out" / OPTCOM for label, *_ in CLEAN_RUNS})
    full = {k: _stored(v) for k, v in files.items()}
    for pid in ("tedana", "replica", "replica-14", "replica-fix", "fmriprep", "tedana-avx512"):
        role, label, sub = PANELS[pid]
        ex.add(exm.Panel(pid, role, label, sub, {"optcom": vols(full[pid])}))
    for pid, (label, sub) in CLEAN_PANELS.items():
        ex.add(exm.Panel(pid, "larmorx", label, sub, {"optcom": vols(full[pid])}))
    outputs = [{"id": "optcom", "label": "Optimally combined series", "unit": ""}]
    diffs = _diffs() + _clean_diffs()
    panels = {q.id for q in ex.panels}
    for x in diffs:
        x["metrics"] = {"optcom": core.compare(full[x["a"]], full[x["b"]])}
        if x["a"] in panels and x["b"] in panels:
            x["display"] = {"optcom": ex.compare(x["a"], x["b"], "optcom")["display"]}
    named = _named(e)
    _verdicts(diffs, named, links, None, p["fittype"], _fmriprep_host(name), series=True)
    _clean_verdicts(diffs, part, e, links, None, series=True)
    for x in diffs:
        if x["id"] == "fix-fmriprep":
            x["reason"] = x["reason"].replace(
                " The full T2* map's cap (TED-05) is 25.0.1's behaviour again.",
                " The combination weighs the echoes with the full T2* map, which the default "
                "caps as 25.0.1 did (TED-05).",
            )
    t0 = int(np.argmax([np.abs(full["replica-fix"][..., t].astype(np.float64)
                               - full["replica-14"][..., t]).max() for t in shown]))  # fmt: skip
    at = _peak(ex, "replica-fix", "replica", "optcom", t0)
    default = {
        "output": "optcom",
        "plane": "axial",
        "voxel": at[:3],
        "t": t0,
        "panels": ["echo1", "tedana", "replica", "fmriprep", "clean-reproduce"],
        "diffs": ["rep-ted", "fix-rep", "rep-fmriprep"],
        "compare": {"a": "tedana", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="tedana",
        caption=f"file volume {shown[t0]}; the crosshair where the default's combination "
        "differs most from the reproduce mode's.",
    )  # fmt: skip
    out["grid"]["t_index"] = shown
    data = _dataset(name)
    n_val = int(np.prod(full["tedana"].shape))
    return {
        "id": f"optcom-{ds}",
        "title": f"The optimally combined series of {ds}'s run",
        "summary": f"The series fMRIPrep carries on with: tedana's weighted average of the "
        f"{len(p['echoes'])} echoes in every voxel and volume, each echo weighted by TE · "
        "exp(−TE / T2*) with the voxel's full T2* map (Posse et al., 1999), over the echoes of "
        f"good signal of the adaptive mask. {len(shown)} of the {n} volumes are shown; the "
        "metrics cover every volume. fMRIPrep's own combined series (tedana 25.0.1) is a panel. "
        "LARMORX's clean room combines the echoes too, in `reproduce` at one thread and in "
        "`fix`, its default, at eight threads and at one.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']}, task {_task(name)} "
                f"({data['licence']}); shown inside fMRIPrep's mask dilated by one voxel",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Volumes", "text": "shown: file volumes " + ", ".join(map(str, shown))},
            {
                "label": "Programs",
                "text": "tedana 26.0.3 (both kept environments), tedana 25.0.1 (fMRIPrep's "
                f"image), larmorx-gpl at commit `{info['larmorx_commit'][:12]}`",
            },
            _clean_fact(name, part),
            {"label": "Grid", "text": common.describe_grid(eimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against tedana 26.0.3"],
            ["fmriprep", "LARMORX against fMRIPrep's own file"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "tedana against itself: kernels, releases, threads, versions"],
        ],
        "metrics_note": f"Δ is A − B over all {n_val:,} values of the "
        f"{' × '.join(map(str, full['tedana'].shape))} series (every volume); runs that are no "
        "panel are compared over their files. " + CLEAN_NOTE,
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [data]),
            _clean_provenance(part),
        ],
    }


# ----------------------------------------------------------------------------- the tables


def _calls_table(rec, bench) -> dict:
    tt = _suite()
    named = {r["call"]: r.get("fix_named", []) for r in (bench.data["rows"] if bench else [])}
    rows = []
    for r in rec.results:
        if r["category"] != "fmriprep":
            continue
        name = r["case"].removeprefix("fmriprep/")
        rep = r.get("reported", {})
        p = parse(call_args(name))
        ds, task, fit = name.split("-")[0], _task(name), name.rsplit("-", 1)[-1]

        def c(key: str, rep: dict = rep) -> dict:
            v = rep.get(key)
            return common.cell("–") if v is None else common.cell(int(v), "count")

        rows.append({
            "head": f"{ds} {task}, {fit}",
            "sub": f"{len(p['echoes'])} echoes, TE {', '.join(f'{t:g}' for t in p['tes'])} ms",
            "cells": [
                common.cell("identical" if r["status"] == "pass" else r["status"]),
                c("tedana's step: shared -> newest releases"),
                c("D13 fix pass"),
                common.cell(", ".join(named.get(name, [])) or "–"),
                c(f"fMRIPrep's files (tedana {tt.FMRIPREP_TEDANA}) -> tedana {tt.TEDANA}"),
                c(f"fMRIPrep's files (tedana {tt.FMRIPREP_TEDANA}) -> larmorx's default"),
            ],
            "verdict": "bit-identical" if r["status"] == "pass" else "open",
        })  # fmt: skip
    return common.table(
        "calls",
        "fMRIPrep's multi-echo calls on three oracle runs: values that differ",
        [
            "Call",
            "Replica vs tedana 26.0.3",
            {"label": "tedana: shared → newest", "num": True},
            {"label": "Default vs reproduce", "num": True},
            "Named by default",
            {"label": "fMRIPrep's files vs tedana 26.0.3", "num": True},
            {"label": "vs larmorx's default", "num": True},
        ],
        rows,
        note="From the record (docs/validation/tedana-t2smap-replica.md): each call's every file, "
        "message and exit status compared with tedana 26.0.3 in both kept environments (the "
        "replica in reproduce mode, with each environment's libraries); the other columns count "
        "the values of the images and the confounds table that differ: tedana's own step "
        "between the two environments' releases, the replica's default against its reproduce "
        "mode (the defects it names: the benchmark's runs), fMRIPrep's own T2* map and combined "
        "series (tedana 25.0.1 in fMRIPrep 25.2.6's image; fMRIPrep keeps no other output, and "
        "its log-linear call was not run by fMRIPrep) against tedana 26.0.3 and against the "
        f"default. Why fMRIPrep's files differ: {VERSION_STEP}; ds000210's and ds000258's runs "
        "were also made with numpy's AVX-512 kernels on (NP-04), ds003592's on a machine without "
        "them.",
    )


def _option_code(text: str) -> str:
    """A row head that is a command-line option (``--n-threads 4``) as a code span: the site
    renders row heads through markdown-it's typographer, which turns a leading ``--`` into an
    en dash (relay RS4's typographer check, 2026-11-12)."""
    return f"`{text}`" if text.startswith("--") else text


def _spread_table(rec) -> dict:
    cases = [r for r in rec.results if r["category"] == "spread"]
    labels = []
    for r in cases:
        for k in r.get("reported", {}):
            if k.startswith("tedana's spread: ") and k not in labels:
                labels.append(k)
    heads = {
        "spread/ds003592-sub-280-rest-curvefit": "fMRIPrep's ds003592 call",
        "spread/syn-curvefit": "synthetic, curvefit",
        "spread/syn-loglin": "synthetic, loglin",
    }
    rows = []
    for k in labels:
        rows.append({
            "head": _option_code(k.removeprefix("tedana's spread: ")),
            "cells": [common.cell(int(r["reported"][k]), "count") if k in r.get("reported", {})
                      else common.cell("–") for r in cases],
        })  # fmt: skip
    rows.append({
        "head": "larmorx (replica) at `--n-threads 4`",
        "cells": [common.cell(int(r.get("measured", {}).get("the replica at --n-threads 4", 0)),
                              "count") for r in cases],
    })  # fmt: skip
    return common.table(
        "spread",
        "tedana's own variability: values that differ from tedana 26.0.3's reference setup",
        ["Variant", *({"label": heads.get(r["case"], r["case"]), "num": True} for r in cases)],
        rows,
        note="The record's spread cases: tedana 26.0.3 in its reference setup (numpy 2.5.4, SciPy "
        "1.18.1, numpy's AVX-512 kernels off, one thread) against itself with one setting "
        "changed, and against tedana 25.0.1 in fMRIPrep's image on this machine; every value of "
        "every image and of the confounds table. The synthetic cases are 12 × 10 × 6 voxels, 24 "
        "volumes, four echoes in fMRIPrep's form. The replica gives the same bytes at any thread "
        "count.",
    )


def _t25_table(ctx: core.Context) -> dict:
    """fMRIPrep's own files against tedana 25.0.1 run here, numpy's AVX-512 kernels off and on;
    the verdict from the setting fMRIPrep's own run had (its machine's numpy kernels)."""
    info = t25_entry(ctx).info()
    tt = _suite()
    rows = []
    for name in T25_CALLS:
        c = info["counts"][name]
        ds, task = name.split("-")[0], _task(name)
        run = node_dir(name).relative_to(common.oracles() / "fmriprep-25.2.6").parts[0]
        other = NO_AVX512_RUNS.get(run)
        if other is None and "this machine" not in c["host"]:
            raise ValueError(f"{name}: fMRIPrep's run was made elsewhere ({c['host']})")
        same = "off" if other else "avx512"
        where = (
            f"made on {other}: numpy's kernels off there"
            if other
            else "made on this machine, numpy's AVX-512 kernels on"
        )
        for f in tt.FMRIPREP_FILES:
            off, on = c["off"].get(f), c["avx512"].get(f)
            if off is None or on is None:
                continue
            match = c[same][f][0]
            verdict = "bit-identical" if match == 0 else "open"
            rows.append({
                "head": f"{ds} {task}: `{f}`",
                "sub": f"{on[1]:,} values; fMRIPrep's run {where}",
                "cells": [common.cell(int(off[0]), "count"), common.cell(int(on[0]), "count"),
                          {"verdict": verdict,
                           "text": f"kernels {'off' if same == 'off' else 'on'}, as in "
                           "fMRIPrep's run" + ("" if match == 0 else f": {match:,} differ")}],
                "verdict": verdict,
            })  # fmt: skip
    return common.table(
        "fmriprep-tedana-25",
        "fMRIPrep's own files against tedana 25.0.1 run here: values that differ",
        ["fMRIPrep's call and file",
         {"label": "numpy's AVX-512 kernels off", "num": True},
         {"label": "on", "num": True},
         "Run as fMRIPrep's was"],
        rows,
        note="tedana 25.0.1 in fMRIPrep 25.2.6's image (Apptainer, through the oracle-output "
        "cache) on fMRIPrep's own call of each oracle run, against the T2* map and the combined "
        "series fMRIPrep kept, every value. The oracle runs of ds000210 and ds000258 were made "
        "again on 2026-11-09 on this machine with OpenBLAS's Haswell kernels; their settings name "
        "only those, so numpy used its AVX-512 kernels for float64 exp and log (NP-04). "
        "ds003592's run was made on a machine without AVX-512, where numpy uses glibc 2.35's "
        "functions, as it does here with those kernels off (larmorx's reference setting, "
        "decision D2). The verdict compares the run made as fMRIPrep's was: bit-identical in "
        "every call; the other column shows how far numpy's other kernels move the curve fit.",
    )  # fmt: skip


def _defect_rows(ctx: core.Context) -> list[tuple[str, str, str, str]]:
    """The findings' table of TED-xx: (id, what tedana does, rule, larmorx's default)."""
    text = (ctx.repo / FINDINGS).read_text(encoding="utf-8")
    out = []
    for m in re.finditer(r"^\| (TED-\d+) \| (.*?) \| ([^|]*?) \| (.*?) \|\s*$", text, re.M):
        tid, what, rule, default = (g.strip() for g in m.groups())
        # the findings' working notes (which step of the part measured what, with its script)
        what = re.sub(r" \(relay \w+, [^)]*\)", "", what).replace(" (below)", "")
        out.append((tid, what, rule, default))
    return out


def _record_cases(rec) -> dict[str, tuple[int, list[str]]]:
    """Each defect -> how many of the record's cases named it by default, and the first ones
    the record lists (its D13 list)."""
    out: dict[str, tuple[int, list[str]]] = {}
    pat = r"^\s*- ([A-Z]+-\d+): (\d+) case\(s\) named it.*?: ((?:`[^`]+`(?:, )?)+)"
    for m in re.finditer(pat, rec.text, re.M):
        out[m.group(1)] = (int(m.group(2)), re.findall(r"`([^`]+)`", m.group(3)))
    return out


def _named_in(named: dict[str, tuple[int, list[str]]], tid: str) -> dict:
    n, found = named.get(tid, (0, []))
    if not found:
        return common.cell("–")
    more = f" and {n - 1} more" if n > 1 else ""
    # the shortest, as text that may wrap at its hyphens: the column stays narrow
    return common.cell(f"{min(found, key=len)}{more}")


def _corrections_table(ctx: core.Context, links: dict, rec) -> dict:
    named = _record_cases(rec)
    for r in rec.results:  # a defect's own case that names nothing by default (TED-16: refused)
        k = re.match(r"(?:fail/\d+-|difftest/)ted-?(\d+)\b", r["case"])
        if k and f"TED-{k.group(1)}" not in named:
            named[f"TED-{k.group(1)}"] = (1, [r["case"]])
    rows = []
    for tid, what, rule, default in _defect_rows(ctx):
        rows.append({
            "head": f"[{tid}](repo:{links.get(tid, FINDINGS)})",
            "cells": [common.cell(what), common.cell(rule), common.cell(default),
                      _named_in(named, tid)],
        })  # fmt: skip
    for tid, what, rule, default in (
        ("NP-03", "numpy's float32 log is numpy's own kernel on x86-64 CPUs with AVX2/FMA3 or "
         "AVX-512 (at most 4 ulp from the correctly rounded value), the C library's elsewhere: "
         "tedana's log-linear fit depends on the CPU", "A",
         "the correctly rounded logf; named where a written value changes"),
        ("GLIBC-07", "numpy's float64 exp and log (glibc 2.35's where numpy uses no AVX-512 "
         "kernel) and SciPy's powers are not correctly rounded", "A",
         "correctly rounded functions; named where a written value changes"),
        ("NP-04", "numpy's float64 exp and log are Intel's SVML kernels on CPUs with AVX-512, "
         "glibc's elsewhere", "C", "the path without them is the reference (decision D2); "
         "measured in tedana's own variability"),
    ):  # fmt: skip
        rows.append({
            "head": f"[{tid}](repo:{links.get(tid, FINDINGS)})",
            "cells": [common.cell(what), common.cell(rule), common.cell(default),
                      _named_in(named, tid)],
        })  # fmt: skip
    return common.table(
        "corrections",
        "tedana's defects (TED-01 to TED-18 found by the replica, TED-31 and TED-46 by the "
        "clean-room part) and the maths entries",
        ["Entry", "What tedana does", "Rule", "larmorx's default", "The record's cases"],
        rows,
        note="Each decided by rules A to D (A: corrected by default; B: a clear error; C: kept "
        "and documented; D: measured first), each with `upstream_defects='reproduce'` "
        "(LARMORX_UPSTREAM_DEFECTS=reproduce) giving tedana's behaviour, its errors and messages "
        "included; the findings: docs/findings/tedana-t2smap.md. The record's cases: those whose "
        "default mode named the entry (the record's D13 list), else the entry's own case. "
        "fMRIPrep's calls meet TED-05 "
        "(every call), TED-08 (three of the four curve fits) and the maths entries; the others "
        "need options or inputs fMRIPrep does not give.",
    )


def _files_table(ctx: core.Context) -> dict:
    """Every file of the shown calls (images and the confounds table): the values that differ
    in each comparison, NaN alike (the parity suite's ``value_differences``)."""
    tt = _suite()
    rows = []
    for name in SHOWN:
        e = call_entry(ctx, name)
        diffs = {lab: tt.value_differences(_out(e, b), _out(e, a)) for lab, a, b in FILE_PAIRS}
        ds = name.split("-")[0]
        for f in FILES:
            cells = []
            total = None
            for lab, _a, _b in FILE_PAIRS:
                v = diffs[lab].get(f)
                if v is None:
                    cells.append(common.cell("–"))
                    continue
                total = v[1]
                cells.append(common.cell(int(v[0]), "count"))
            if total is None:
                continue
            same = diffs[FILE_PAIRS[0][0]].get(f)
            rows.append({
                "head": f"`{f}`",
                "sub": f"{ds}: {total:,} values",
                "cells": cells,
                "verdict": "bit-identical" if same and same[0] == 0 else "open",
            })  # fmt: skip
    return common.table(
        "files",
        "Every file of the shown calls: values that differ",
        ["File", *({"label": lab, "num": True} for lab, _a, _b in FILE_PAIRS)],
        rows,
        note="Each image's stored values and each cell of the confounds table, NaN counted as "
        "equal to NaN (the RMSE map holds NaN in the voxels with fewer than two good echoes); "
        "the verdict is the replica's against tedana 26.0.3. tedana 25.0.1 writes no adaptive "
        "mask and no failures map in this form ('–'). The record compares the JSON files too "
        "(dataset_description.json but for the host's and Python's fields, decision D6).",
    )


def _memory_table(ctx: core.Context) -> dict:
    peaks = memory_entry(ctx).info()["peaks_mib"]
    labels = {
        "ds003592-sub-280-rest-curvefit": "fMRIPrep's ds003592 call (curvefit)",
        "ds003592-sub-280-rest-loglin": "the same call with --fittype loglin",
    }
    rows = []
    for name, p in peaks.items():
        worst = max(p["larmorx-reproduce"], p["larmorx-fix"])
        rows.append({
            "head": labels.get(name, name),
            "cells": [common.cell(round(p["tedana"]), "count"),
                      common.cell(round(p["larmorx-reproduce"]), "count"),
                      common.cell(round(p["larmorx-fix"]), "count"),
                      common.cell(worst / p["tedana"], "sig")],
        })  # fmt: skip
    return common.table(
        "memory",
        "Peak memory (MiB): the memory rule's calls, one thread",
        [
            "Call",
            {"label": "tedana 26.0.3", "num": True},
            {"label": "larmorx, reproduce", "num": True},
            {"label": "larmorx, default", "num": True},
            {"label": "larmorx ÷ tedana", "num": True},
        ],
        rows,
        note="Each call in a process of its own, its peak resident memory from /usr/bin/time -v "
        "(tedana's program in the newest environment; the replica as the larmorx-gpl program), "
        "one run each; tests/parity/test_memory_peak.py checks the same calls through "
        "`python -m larmorx tedana t2smap`. The rule: at most 10 % above the original's peak "
        "plus 8 MiB. tedana holds the echoes in float64 and float32 copies at once; the replica "
        "holds the float32 data once, one pass's arrays at a time.",
    )


def trial(ctx: core.Context) -> tuple[list[dict], dict]:
    """Random testing's trial and its check (``summary.json``), and the triage: two tables."""
    import tomllib

    out_rows, prov = [], {}
    for name in TRIALS:
        path = common.oracles() / "difftest" / name / "summary.json"
        raw = path.read_bytes()
        s = json.loads(raw)
        t = s["tools"][TOOL]
        o, im = t["options"], t["implementations"]["replica"]
        kind = "the check of the final build" if name.endswith("-check") else "the trial"
        rows = [
            (f"{kind}: random cases, each compared with tedana 26.0.3 in both environments",
             t["cases"]),
            ("  the same (every file, message and exit status; the default by D13's rule)",
             im.get("agree", 0)),
            ("  differing (triaged below: fixed in larmorx, or TED-16 to TED-18 registered)",
             im.get("differ", 0)),
            ("  a notice the default gave where the rule expected none (triaged below)",
             im.get("notice", 0)),
            ("  echoes: 2, 3, 4, 5, 6", [o.get(f"{k} echoes", 0) for k in range(2, 7)]),
            ("  fits: loglin, curvefit, default; modes all, ts",
             [o.get("fittype loglin", 0), o.get("fittype curvefit", 0),
              o.get("fittype default", 0), o.get("fitmode all", 0), o.get("fitmode ts", 0)]),
            ("  combinations: t2s, paid", [o.get("combmode t2s", 0), o.get("combmode paid", 0)]),
            ("  files: .nii.gz, .nii, .hdr pairs, NIfTI-2",
             [o.get("file .nii.gz", 0), o.get("file .nii", 0), o.get("file .hdr", 0),
              o.get("file nifti2", 0)]),
            ("  without a mask", o.get("no mask", 0)),
            ("  distinct options and settings drawn", t["options_covered"]),
            ("  length (minutes; 12 jobs at one thread each)", round(s["elapsed_s"] / 60, 1)),
        ]  # fmt: skip
        for head, v in rows:
            if isinstance(v, list):
                cell = common.cell(" / ".join(f"{x:,}" for x in v))
            elif isinstance(v, float):
                cell = common.cell(v, "sig")
            else:
                cell = common.cell(int(v), "count")
            out_rows.append({"head": head, "cells": [cell]})
        prov[name] = {"summary": f"oracles/difftest/{name}/summary.json",
                      "sha256": hashlib.sha256(raw).hexdigest(), "seed": s["seed"],
                      "larmorx_commit": s["larmorx_commit"]}  # fmt: skip
    first = json.loads((common.oracles() / "difftest" / TRIALS[0] / "summary.json").read_bytes())
    table = common.table(
        "trial",
        f"Random testing: the trial of {first['date']} and the check of the final build",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note="Differential random testing (docs/validation/difftest.md): random multi-echo "
        "series that share one S0 and one T2* map (a head phantom, rising and dropout voxels, "
        "temporal noise, NaN, infinite, zero, negative and −0 samples), two to six echoes in "
        "seconds or ms, 1 to 12 volumes, drawn geometries, float32, float64, scaled int16, "
        "uint16 and uint8 data, little- or big-endian; masks of every kind or none; every "
        "option, abbreviations and `=` forms, runs twice with and without --overwrite. tedana "
        "runs as its program in both environments, the replica in reproduce mode compared file "
        f"by file, its default checked by D13's rule. Every case in a memory box of "
        f"{first['memory']['case']}. The trial (seed `{first['seed']}`, 30 minutes) found the "
        "differences triaged below; every one of its 70 differing or noticed cases agrees with "
        "the final build, and the 10-minute check of the final build found one notice (TED-15 "
        "in a run made twice), fixed with its permanent case.",
    )
    from larmorx_validation.difftest import permanent

    n_perm = len(permanent.load(TOOL))
    tri = tomllib.loads(
        (ctx.repo / "validation" / "difftest" / "triage.toml").read_text(encoding="utf-8")
    )
    trows = []
    for d in tri.get("difference", []):
        if d.get("tool") != TOOL:
            continue
        trows.append({
            "head": d.get("status", ""),
            "cells": [common.cell(common.clip(d.get("explanation", ""), 420)),
                      common.cell(d.get("defect", "–"))],
            "sub": d.get("link", ""),
        })  # fmt: skip
    triage = common.table(
        "triage",
        "Random testing: every difference of the trial, triaged",
        ["Status", "What it was", "Register"],
        trows,
        note="validation/difftest/triage.toml (block tedana-t2smap, 2026-11-08); each difference "
        f"has a permanent case (validation/difftest/cases/tedana-t2smap/, {n_perm} in all), run "
        "by the record.",
    )
    return [table, triage], prov


#: The clean-room original's benchmark (its relay's report, 2026-11-11): its series on the chart.
ORIGINAL_BENCH = "docs/benchmarks/tedana-t2smap-original.json"


def _speed(bench, ctx: core.Context | None = None) -> dict | None:
    if bench is None:
        return None
    series = [
        {"id": "tedana|1", "label": "tedana 26.0.3, 1 thread", "mark": "original"},
        {"id": "reproduce|1", "label": "larmorx (replica), reproduce, 1 thread", "mark": "replica"},
        {"id": "reproduce|14", "label": "larmorx (replica), reproduce, 14 threads",
         "mark": "replica-open"},
        {"id": "fix|1", "label": "larmorx (replica), default, 1 thread", "mark": "replica"},
        {"id": "fix|14", "label": "larmorx (replica), default, 14 threads", "mark": "replica-open"},
    ]  # fmt: skip
    orig = {}
    if ctx is not None and (ctx.repo / ORIGINAL_BENCH).is_file():
        ob = json.loads((ctx.repo / ORIGINAL_BENCH).read_text(encoding="utf-8"))
        orig = ob["calls"]
        series += [
            {"id": "original|1", "label": "larmorx (clean-room original), default, 1 thread",
             "mark": "larmorx"},
            {"id": "original|8", "label": "larmorx (clean-room original), default, 8 threads",
             "mark": "larmorx-open"},
        ]  # fmt: skip
    rows = []
    for r in bench.data["rows"]:
        sec = {
            "tedana|1": r["tedana_s"],
            "reproduce|1": r["reproduce_1_s"],
            "reproduce|14": r["reproduce_14_s"],
            "fix|1": r["fix_1_s"],
            "fix|14": r["fix_14_s"],
        }
        name = r["call"]
        o = orig.get(name, {})
        for sid, key in (("original|1", "original fix (1)"), ("original|8", "original fix (8)")):
            if o.get(key) is not None:
                sec[sid] = o[key]
        rows.append({
            "label": f"{name.split('-')[0]} {_task(name)}, {name.rsplit('-', 1)[-1]}",
            "input": name,
            "seconds": sec,
            "speedup": sec["tedana|1"] / sec["reproduce|14"],
        })  # fmt: skip
    axis, ticks = common.ticks_log([v for row in rows for v in row["seconds"].values()], " s")
    meta = bench.data.get("meta", {})
    return {
        "title": "Seconds per call (wall time), log scale; the factor is tedana's time ÷ the "
        "replica's at 14 threads",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": "From the benchmark report `docs/benchmarks/tedana-t2smap.json` "
        f"({meta.get('cpu_count', '?')} logical CPUs; the best of {meta.get('repeat', '?')} runs "
        "for the replica, tedana once; a machine shared with another agent's work). Both of the "
        "replica's modes run the stages from the mask to the combination again with the other "
        "mode's maths functions, writing nothing, to name NP-03 and GLIBC-07 exactly where they "
        "change a value (up to four passes on these calls): one pass of a curve-fit call takes "
        "about a quarter of these times."
        + (
            " The clean-room original, the fast implementation, from its own benchmark "
            f"(`{ORIGINAL_BENCH}`, 2026-11-11: the best of two runs at 1 and 8 threads, 8 the "
            "budget of its relay, on a machine shared with two other agents' work; tedana, timed "
            "again there, took "
            f"{min(v['tedana'] for v in orig.values()):g} to "
            f"{max(v['tedana'] for v in orig.values()):g} s on these calls)."
            if orig
            else " The clean-room original is the fast implementation."
        ),
    }


# ------------------------------------------------------------------------------ the cases


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.difftest import permanent

    tt = _suite()
    defs = {cid: (desc, payload) for cid, _tier, desc, payload in tt._all_cases()}
    perm = {f"difftest/{p['id']}": p for p in permanent.load(TOOL)}
    table = {tid: (rule, default) for tid, _what, rule, default in _defect_rows(ctx)}
    out = []
    for r in rec.results:
        cid = r["case"]
        v = common.outcome_verdict(r)
        p = perm.get(cid)
        if cid in defs:
            desc, payload = defs[cid]
            args = [a for a in payload[2] if a != "@twice"]
            command = common.command_text("t2smap", args, core.short_path)
            if "@twice" in payload[2]:
                command += "   (run twice in one folder)"
        else:
            desc = p["description"] if p else ""
            command = ""
        rep = r.get("reported") or {}
        defect = (payload[1] if cid in defs and payload[0] == "fail" else None) or (
            p.get("defect") if p else None
        )
        if v is not None and v[0] == "both-reject" and defect in table:
            rule, default = table[defect]
            v = (v[0], common.clip(f"{v[1]} The default ({defect}, rule {rule}): {default}.", 420))
        if v is None:
            fixed = rep.get("D13 fix pass")
            if r["category"] == "spread":
                spread = "; ".join(
                    f"{k.removeprefix('tedana' + chr(39) + 's spread: ')}: {int(n):,}"
                    for k, n in rep.items()
                    if k.startswith("tedana's spread")
                )
                v = ("bit-identical", common.clip(
                    "Bit-identical to tedana 26.0.3 in its reference setup (every file, "
                    f"message and exit status) and at 4 threads; tedana's own spread, values "
                    f"that differ: {spread}.", 400))  # fmt: skip
            else:
                tail = (
                    f" The default mode changes {int(fixed):,} values, each change named "
                    "(D13's rule)."
                    if fixed
                    else " The default mode gives the same values."
                )
                if p and p.get("defect"):
                    tail += f" {p['defect']}'s case (random testing)."
                elif p:
                    tail += " A mistake of larmorx's found by random testing, fixed; its case."
                v = ("bit-identical", common.clip(
                    "Bit-identical to tedana 26.0.3 in both kept environments: every file, "
                    f"message and exit status ({r['checks_passed']} checks).{tail}", 400))  # fmt: skip
        out.append({
            "id": cid,
            "category": r["category"],
            "description": desc,
            "command": command,
            "results": {
                "replica": {
                    "outcome": r["status"],
                    "verdict": v[0],
                    "checks": f"{r['checks_passed']}/{r['checks_total']}",
                    "reason": v[1],
                }
            },
        })  # fmt: skip
    return out


# -------------------------------------------------------------------------------- export


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [*(call_entry(ctx, n) for n in SHOWN), memory_entry(ctx), t25_entry(ctx),
            *(clean_entry(ctx, n)[0] for n in SHOWN if n not in CLEAN_ELSEWHERE)]  # fmt: skip


def _check_default(ex: dict) -> None:
    """The viewer draws the default's panels and comparisons: every panel must exist and every
    comparison must have images (``display``: both sides panels), or the viewer cannot draw."""
    panels = {p["id"] for p in ex["panels"]}
    drawn = {x["id"] for x in ex["diffs"] if x.get("display")}
    d = ex["default"]
    bad = [p for p in d["panels"] if p not in panels] + [x for x in d["diffs"] if x not in drawn]
    bad += [v for v in (d["compare"]["a"], d["compare"]["b"]) if v not in panels]
    if bad:
        raise ValueError(f"{ex['id']}: the default view names what the viewer cannot draw: {bad}")


def _check_finite(ex: dict) -> None:
    """Every window and metric a number: the viewer cannot draw a window of null."""
    for o in ex["outputs"]:
        for w in o["windows"]:
            if not (np.isfinite(w["lo"]) and np.isfinite(w["hi"])):
                raise ValueError(f"{ex['id']}: output {o['id']}'s window {w['id']} is not finite")


def _bench_source(ctx: core.Context, bench) -> list[dict]:
    if bench is None:
        return []
    return [
        {
            "path": bench.path.relative_to(ctx.repo).as_posix(),
            "sha256": hashlib.sha256(bench.path.read_bytes()).hexdigest(),
        }
    ]


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    bench = records.load_bench(ctx.repo, BENCH)
    links = _links(ctx.repo)
    ents = entries(ctx)
    cases = _cases(ctx, rec)
    examples = [_example_maps(ctx, SHOWN[0], links), _example_series(ctx, links)]
    examples += [_example_maps(ctx, n, links) for n in SHOWN[1:]]
    for ex in examples:
        _check_default(ex)
        _check_finite(ex)
    trial_tables, tprov = trial(ctx)
    tables = [
        _calls_table(rec, bench),
        _t25_table(ctx),
        _spread_table(rec),
        _corrections_table(ctx, links, rec),
        _files_table(ctx),
        _memory_table(ctx),
        *trial_tables,
    ]
    doc = {
        "schema": 1,
        "tool": TOOL,
        "title": "t2smap",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": _bench_source(ctx, bench),
            "cache": [fcache.provenance(x) for x in ents],
            "difftest": [{"night": k, **v} for k, v in tprov.items()],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases,
                "replica",
                "larmorx-gpl replica, against tedana 26.0.3 (standard tier)",
                "replica",
                f"docs/validation/{RECORD}.md",
                "experimental",
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "speed": _speed(bench, ctx),
        "tables": tables,
        "examples": examples,
    }
    # The clean-room original's part (its own module, written by its implementer: its example,
    # its record's cases as original/<case>, its implementation, its tables, its provenance).
    from larmorx_validation.evidence import tedana_t2smap_clean_room

    return present(tedana_t2smap_clean_room.extend(ctx, doc))


# ---------------------------------------------------------------------- the presentation


def _page():
    from larmorx_validation.evidence import present as present_

    return present_.Page(
        "tedana",
        "t2smap",
        "26.0.3",
        port="replica",
        packages={"replica": "larmorx-gpl, GPL-3.0", "clean-room": "larmorx, Apache-2.0"},
        texts=TEXTS,
    )


#: The page's own rewordings (applied before the guide's words): LARMORX's reference setup is
#: both ports'; "the row above" named by what that row compares; the clean room's texts that
#: call it "the original" (its module's words: there "the original" is the clean room, "tedana"
#: the original); D13, a decision's number, by what it is.
TEXTS = [
    (r"\blarmorx's reference\b", "LARMORX's reference"),
    (r"\(the row above\)", "(the replica's `reproduce` against fMRIPrep's output)"),
    (r"\bthe original holds tedana's\b", "the clean room holds tedana's"),
    (r"\(the original's own trust-region solver\b", "(the clean room's own trust-region solver"),
    (r"\bThe two implementations' defaults\b", "The two ports' defaults"),
    (r"\btedana and the original's reproduce stop\b", "tedana and the clean room's `reproduce` stop"),
    (r"\(LARMORX against itself\)", "(the clean room against itself)"),
    (r"\(the original's test, its log\)", "(the clean room's test, its log)"),
    (r"\bthe verdict: the original's files\b", "the verdict: the clean room's files"),
    (r"\(A\) the original's reproduce against tedana, \(B\) its default against the replica's, "
     r"both by the original's suite\b",
     "(A) the clean room's `reproduce` against tedana, (B) its `fix` against the replica's, both "
     "by the clean room's comparison suite"),
    (r"\bthe clean-room part's branch\b", "the branch the clean room was written on"),
    (r"\bby the clean-room part\)", "while the clean room was written)"),
    (r"\bfound by the replica, ", "found while the replica was written, "),
    (r"\(D13's rule\)", "(the rule of the corrections by default)"),
    (r"\bits default checked by D13's rule\b", "its `fix` checked by the rule of the corrections by "
     "default"),
    (r"\bthe default by D13's rule\b", "`fix` as the rule of the corrections expects"),
    (r"\(the record's D13 list\)", "(the record's list of what `fix` changed)"),
    (r"\bthe original's solver is its own\b", "the clean room's solver is its own"),
    (r"\bthose whose default mode named\b", "those whose `fix` named"),
    (r"\(relay 3c, (2026-11-11)", r"(\1"),
    (r"\bin both modes\b", "in `fix` and in `reproduce`"),
    (r", 8 the budget of its relay, on a machine shared with two other agents' work",
     ", 8 the threads it was given, on a machine shared with other work"),
    (r"\ba machine shared with another agent's work\b", "a machine shared with other work"),
    (r"\bThe clean-room original, the fast implementation,", "The clean room, the port built for "
     "speed,"),
    (r"\bTrial (2026-11-11-t2smap-original-2)\b", r"The run \1"),
    (r"^the trial: random cases\b", "the run: random cases"),
    (r"\bThe trial \(seed `t2smap-trial`", "The run (seed `t2smap-trial`"),
]  # fmt: skip


def _diff(ex: dict, i: str) -> dict:
    return next(d for d in ex["diffs"] if d["id"] == i)


def _want(ex: dict, **verdicts: str | None) -> None:
    """Raise unless each comparison (``rep_ted`` for ``rep-ted``) has its verdict."""
    for i, v in verdicts.items():
        got = _diff(ex, i.replace("_", "-")).get("verdict")
        if got != v:
            raise ValueError(f"{ex['id']}: {i.replace('_', '-')} is {got}, not {v}")


def _found(ex: dict, i: str, pat: str) -> re.Match[str]:
    m = re.search(pat, _diff(ex, i)["reason"])
    if not m:
        raise ValueError(f"{ex['id']}: {i}'s reason without {pat!r}")
    return m


def _ids(ex: dict, i: str) -> list[str]:
    """The register's entries a comparison's reason names, in its order."""
    return list(dict.fromkeys(re.findall(r"\b(?:TED|NP|GLIBC)-\d+\b", _diff(ex, i)["reason"])))


def _and(xs: list[str]) -> str:
    return xs[0] if len(xs) == 1 else ", ".join(xs[:-1]) + " and " + xs[-1]


#: check.py's counts in a reason: the values identical, then the (b)/(c) share.
_CHECKED = (
    r"([\d,]+) of ([\d,]+) compared values identical; the (?:curve fit's )?\(b\)/\\\(c\) share "
    r"([\d.]+%)"
)
_SPLIT = r"\(([\d,]+) of ([\d,]+) values\)"


def _machine(ex: dict) -> str:
    """Which of tedana 25.0.1's two runs here gives fMRIPrep's own file bit for bit: numpy's
    AVX-512 kernels on or off, as in fMRIPrep's run."""
    same = [i for i in ("t25-fmriprep", "t25off-fmriprep")
            if re.search(r"every one of the [\d,]+ values is the same", _diff(ex, i)["reason"])]  # fmt: skip
    if len(same) != 1:
        raise ValueError(f"{ex['id']}: tedana 25.0.1 here gives fMRIPrep's file in {same}")
    return "on" if same[0] == "t25-fmriprep" else "off"


def _both(ex: dict, series: bool) -> str:
    """The sentences on the two ports of a call shown by tedana and the replica."""
    _want(ex, rep_ted="bit-identical", rep14_rep="bit-identical", fix_rep="d13",
          rep_fmriprep="within-variability", cr_ted="within-variability", cr1_cr="bit-identical",
          crfix_cr="d13", crfix_rfix="within-variability", crfix_fmriprep="d13")  # fmt: skip
    a, b, s = _found(ex, "cr-ted", _CHECKED).groups()
    a2, b2, s2 = _found(ex, "crfix-rfix", _CHECKED).groups()
    rep, cr = _ids(ex, "fix-rep"), _ids(ex, "crfix-cr")
    if not cr or not set(cr) <= set(rep):
        raise ValueError(f"{ex['id']}: the clean room's corrections {cr}, the replica's {rep}")
    extra = [i for i in rep if i not in cr]
    if extra and not set(extra) <= {"NP-03", "GLIBC-07"}:
        raise ValueError(f"{ex['id']}: the replica's other corrections {extra}")
    what = "combined series" if series else "maps"
    out = (
        f"In `reproduce` the replica gives every value of tedana 26.0.3's {what}, at 1 and 14 "
        "threads alike, and the clean room holds them within the thresholds of specs/t2smap.md "
        f"§17.3: {a} of the {b} values of the run's files are identical, and {s} of the curve "
        "fit's voxels are judged by the fit's own objective (its files are the same at 1 and 8 "
        f"threads). In `fix`, their default, both correct {_and(cr)}"
        + (f" and the replica also the maths entries {_and(extra)}" if extra else "")
        + f"; the two defaults agree by the same thresholds ({a2} of {b2} values identical, "
        f"{s2})."
    )  # fmt: skip
    n, total = _found(ex, "rep-fmriprep", _SPLIT).groups()
    kernels = _machine(ex)
    out += (
        f" fMRIPrep's own {'combined series' if series else 'T2* map'}, made by tedana 25.0.1, "
        f"differs from the replica's `reproduce` in {n} of its {total} values, tedana's own step "
        "between its versions; run here in fMRIPrep's image with numpy's AVX-512 kernels "
        f"{kernels}, as in fMRIPrep's run, tedana 25.0.1 gives it bit for bit."
    )
    return out


def _shows_maps(ex: dict) -> tuple[str, list[str]]:
    return _both(ex, series=False), ["rep-ted", "cr-ted"]


def _shows_series(ex: dict) -> tuple[str, list[str]]:
    return _both(ex, series=True), ["rep-ted", "cr-ted"]


def _shows_clean(ex: dict) -> tuple[str, list[str]]:
    """The clean room's own example: both modes and both thread counts beside tedana and the
    replica's default."""
    _want(ex, orepro_tedana="within-variability", original_tedana="d13", replica_tedana="d13",
          original_replica="within-variability", original1_original="bit-identical")  # fmt: skip
    a, b, s = _found(ex, "orepro-tedana", _CHECKED).groups()
    a2, b2, s2 = _found(ex, "original-replica", _CHECKED).groups()
    cr, rep = _ids(ex, "original-tedana"), _ids(ex, "replica-tedana")
    if not cr or not set(cr) <= set(rep):
        raise ValueError(f"{ex['id']}: the clean room's corrections {cr}, the replica's {rep}")
    extra = [i for i in rep if i not in cr]
    return (
        "In `reproduce` the clean room holds tedana 26.0.3's maps within the thresholds of "
        f"specs/t2smap.md §17.3: {a} of the {b} values of the run's files are identical, and "
        f"{s} of the curve fit's voxels are judged by the fit's own objective. In `fix`, its "
        f"default, it corrects {_and(cr)}, as the replica's default does"
        + (f" (which also corrects the maths entry {_and(extra)})" if extra else "")
        + f", and the two defaults agree by the same thresholds ({a2} of {b2} values identical, "
        f"{s2}); its files are the same at 1 and 8 threads."
    ), ["replica-tedana", "orepro-tedana"]  # fmt: skip


SHOWS = {
    "maps-ds003592": _shows_maps,
    "optcom-ds003592": _shows_series,
    "maps-ds000258": _shows_maps,
    "fmriprep-ds003592-curvefit-original": _shows_clean,
}
#: Comparisons the shapes name otherwise: tedana's step between its versions (its old label
#: names fMRIPrep's image first).
DIFFS = {
    "t25-ted": ("tedana t2smap 25.0.1 (fMRIPrep's image, here) − 26.0.3",
                "the original against itself: versions 25.0.1 and 26.0.3"),
}  # fmt: skip
#: The examples' titles where the old one names the clean room in the old words.
TITLES = {
    "fmriprep-ds003592-curvefit-original": "fMRIPrep's multi-echo call: the clean room beside "
    "tedana and the replica",
}
#: Each example's sentence on the replica's record's threshold, before the clean room's.
REPLICA_THRESHOLD = (
    "The replica's record holds each call to tedana 26.0.3's files, messages and exit status "
    "bit for bit in `reproduce`."
)
#: The case categories of the two records, as the page names them.
CATEGORIES = {
    "probe": "the specification's probe",
    "syn": "synthetic inputs",
    "fail": "failures and refusals",
    "spread": "tedana's own variability",
    "threads": "thread counts",
}
#: The tables' titles by the guide's pattern.
TABLE_TITLES = {
    "calls": "fMRIPrep's calls, the replica against the original: tedana `t2smap` 26.0.3 "
    "(values that differ)",
    "fmriprep-tedana-25": "fMRIPrep's output against the original run here: tedana `t2smap` "
    "25.0.1 (values that differ)",
    "spread": "The original against itself: tedana `t2smap` 26.0.3 (values that differ from its "
    "reference setup)",
    "corrections": "Corrections by default, both ports: tedana `t2smap` 26.0.3's defects and the "
    "maths entries",
    "files": "Every file of the two calls shown, the replica against the original: tedana "
    "`t2smap` 26.0.3 (values that differ)",
    "memory": "Peak memory, the replica against the original: tedana `t2smap` 26.0.3 (MiB, one "
    "thread)",
    "trial": "Random testing, the replica against the original: tedana `t2smap` 26.0.3 (the run "
    "of 2026-11-08 and the check of the final build)",
    "triage": "The replica's differences from the original in the run of 2026-11-08, triaged",
    "fmriprep-calls-original": "fMRIPrep's calls, the clean room against the original: tedana "
    "`t2smap` 26.0.3 (A, in `reproduce`) and against the replica (B, both `fix`)",
    "difftest-original": "Random testing, the clean room against the original: tedana `t2smap` "
    "26.0.3 (A) and against the replica (B) ({cases:,} random calls, {date})",
    "speed-original": "Speed, both ports against the original: tedana `t2smap` 26.0.3 (wall time "
    "of fMRIPrep's calls in seconds, the best of two runs)",
    "memory-original": "Peak memory, the clean room against the original: tedana `t2smap` 26.0.3 "
    "(MiB, fMRIPrep's ds003592 curve-fit call)",
}
#: The tables' columns: each run as the panels name it.
COLUMNS = {
    "Replica vs tedana 26.0.3": "replica, reproduce, against tedana 26.0.3",
    "Default vs reproduce": "replica, fix against reproduce",
    "vs larmorx's default": "against the replica's fix",
    "larmorx's default": "both ports' fix",
    "replica − tedana 26.0.3": "replica − tedana 26.0.3",
    "14 − 1 thread": "replica: 14 − 1 thread",
    "default − reproduce": "replica: fix − reproduce",
    "larmorx, reproduce": "replica, reproduce",
    "larmorx, default": "replica, fix",
    "larmorx ÷ tedana": "replica ÷ tedana",
    "replica, 1 thread": "replica, fix, 1 thread",
    "replica, 8": "replica, fix, 8",
    "original reproduce, 1": "clean room, reproduce, 1",
    "original default, 1": "clean room, fix, 1",
    "original default, 8": "clean room, fix, 8",
    "the original": "the clean room",
    "Trial": "Run",
}
#: The rows that name the clean room "the original" (its memory table).
_ROWS = [(r"^the original, ", "the clean room, "), (r" thread\(s\)$", " thread(s)")]
#: The speed chart's series, as the page names them.
SPEED_LABELS = {
    "tedana 26.0.3, 1 thread": "tedana t2smap 26.0.3, 1 thread",
    "larmorx (replica), reproduce, 1 thread": "replica, reproduce, 1 thread",
    "larmorx (replica), reproduce, 14 threads": "replica, reproduce, 14 threads",
    "larmorx (replica), default, 1 thread": "replica, fix, 1 thread",
    "larmorx (replica), default, 14 threads": "replica, fix, 14 threads",
    "larmorx (clean-room original), default, 1 thread": "clean room, fix, 1 thread",
    "larmorx (clean-room original), default, 8 threads": "clean room, fix, 8 threads",
}
#: The records' implementations: their ids (the case explorer's) and ports.
PORTS = {"replica": "replica", "original": "clean-room"}


def present(doc: dict) -> dict:
    """t2smap's page in the writing guide's words and order: texts, ports and order only."""
    from dataclasses import replace

    from larmorx_validation.evidence import present as present_

    page = _page()
    clean = replace(page, port="clean-room")
    for ex in doc["examples"]:
        line, verdicts = SHOWS[ex["id"]](ex)
        own = clean if ex["id"] == CASE_CLEAN else page
        panels = {
            p["id"]: ("fMRIPrep's output",
                      f"{p['label']}: {p['sub'].replace('in the oracle run', 'in its run')}")
            for p in ex["panels"] if p["role"] == "fmriprep"
        }  # fmt: skip
        shown = {p["id"] for p in ex["panels"]}
        hidden = (
            any("fmriprep" in (d["a"], d["b"]) for d in ex["diffs"]) and "fmriprep" not in shown
        )
        present_.example(own, ex, shows=line, verdicts=verdicts, title=TITLES.get(ex["id"]),
                         runs={"fmriprep": "fmriprep"} if hidden else None,
                         panels=panels or None,
                         diffs={k: v for k, v in DIFFS.items() if any(
                             d["id"] == k for d in ex["diffs"])} or None)  # fmt: skip
        note = ex.get("metrics_note") or ""
        if REPLICA_THRESHOLD not in note:  # the clean room's example: its module's note
            ex["metrics_note"] = f"{note} {REPLICA_THRESHOLD}".strip()
    for im in doc["implementations"]:
        comparison = "standard comparison" if im["id"] == "replica" else "full comparison"
        present_.implementation(page, im, comparison, port=PORTS[im["id"]])
    note = (doc.get("speed") or {}).get("note")
    present_.speed(
        page,
        doc.get("speed"),
        "Seconds per call (wall time) on a log scale; the factor is tedana's time divided by "
        "the replica's at 14 threads.",
        marks={"larmorx": "clean-room", "larmorx-open": "clean-room-open"},
        labels=SPEED_LABELS,
    )
    if doc.get("speed"):  # about both ports
        doc["speed"]["note"] = present_.text(page, note, port="both")
    tables = {t: clean for t in ("fmriprep-calls-original", "difftest-original",
                                 "speed-original", "memory-original")}  # fmt: skip
    trial_rows = {}
    for t in doc.get("tables", []):
        if t["id"] == "difftest-original":
            m = re.match(r"([\d,]+) random cases \(30 minutes", t["rows"][0]["head"])
            if not m:
                raise ValueError("difftest-original: its first row's head changed")
            trial_rows[t["id"]] = int(m.group(1).replace(",", ""))
    present_.finish(page, doc, CATEGORIES, pages=tables, ports=PORTS)
    for t in doc.get("tables", []):
        for c in t["columns"]:
            c["label"] = COLUMNS.get(c["label"], c["label"])
        if t["id"] == "memory-original":
            for r in t["rows"]:
                for pat, rep in _ROWS:
                    r["head"] = re.sub(pat, rep, r["head"])
        if t["id"] in TABLE_TITLES:
            t["title"] = TABLE_TITLES[t["id"]].format(
                cases=trial_rows.get(t["id"], 0), date=_trial_date()
            )
    return doc


#: The clean room's own example (its module's id).
CASE_CLEAN = "fmriprep-ds003592-curvefit-original"


def _trial_date() -> str:
    from larmorx_validation.evidence import tedana_t2smap_clean_room as cr

    return cr.TRIAL[:10]


if __name__ == "__main__":  # compute the cache entries only: python -m ... [--refresh]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    if "--refresh" in sys.argv:
        refresh(c)
    for x in entries(c):
        print("done", x.dir, flush=True)
