# SPDX-License-Identifier: Apache-2.0
"""sdcflows 2.17.1's BIDS app (the ``sdcflows`` program) against ``larmorx sdcflows sdcflows``
(:mod:`larmorx.sdcflows.app`).

- ``parser``: sdcflows' ``_parser()`` and larmorx's :func:`build_parser` on a battery of
  command lines (help, version, every option, abbreviations, the errors): exit status, standard
  output and error, the parsed values; ``reproduce`` exactly, the default apart only where
  SDC-33 corrects (``--mem-gb`` units sdcflows fails on). The messages naming a converter by its
  ``functools.partial`` representation (a memory address, SDC-34) are compared with the name.
- ``toml``: larmorx's :func:`toml_dumps` against toml 0.10.2's ``dumps`` (this environment's)
  on the settings of every case and on edge values (quoting, escapes, numbers, nested tables,
  arrays of tables, ``None``).
- ``settings``: the app's checks and folders (``parse_args``) and its settings file, each run
  of sdcflows in its own process (``relay16/app_original.py``) against larmorx's: the errors
  (exit status, messages; a traceback by its first and last lines), the folders made, the
  ``[execution]``, ``[workflow]`` and ``[nipype]`` tables (the run's identifier apart; the
  database path, which larmorx never writes, apart), ``[environment]``'s keys.
- ``dryrun``: ``--dry-run``'s listing on sdcflows 2.17.1's own app runs' datasets, the SyN run's
  dataset (fieldmap-less estimators), sdcflows' test datasets and generated trees, with and
  without ``--no-fmapless``: ``reproduce`` against sdcflows 2.17.1, the default against sdcflows
  with 2.15.2's search (SDC-17) and with the sessions and filters passed (SDC-30, SDC-31).
- ``exits``: the run's exit statuses: no estimator (64 and its message), ``group`` (1),
  fieldmap-less estimators (SDC-29: sdcflows fails when it runs them; larmorx stops before any
  work, ``reproduce`` fails as sdcflows), a missing readout time.
- ``run``: the whole run on chosen datasets (``--datasets``; sdcflows with ANTs' N4 at one
  thread with correctly rounded maths, FSL's ``prelude`` and ``topup`` and AFNI's ``3dvolreg``
  through the oracle cache, nipype's ``Linear`` plugin; larmorx with the replicas,
  ``reproduce``): every derivative file (names, uncompressed and gzip bytes, sidecars), the
  standard output's lines (time stamps apart), the settings files.

Usage: ``LARMORX_WORKSPACE=<workspace> LARMORX_NC_BIN=<larmorx-nc> python -P check_app.py
--scratch <folder> [--sections ...] [--datasets ...] [--jobs N]``.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import time
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_estimators as ce  # noqa: E402
import check_workflows as cw  # noqa: E402

W = ce.W
SDC = ce.SDC_RUNS
DRIVER = HERE / "relay16" / "app_original.py"
EXPECTED: list[str] = []
PARTIAL = re.compile(
    r"functools\.partial\(<function _parser\.<locals>\.(\w+) at 0x[0-9a-f]+>, "
    r"parser=ArgumentParser\(.*?\)\)",
    re.S,
)


def env_common() -> dict[str, str]:
    env = dict(os.environ)
    env.update(
        {
            "COLUMNS": "100",
            "OMP_NUM_THREADS": "1",
            "TEMPLATEFLOW_HOME": str(W / "oracles" / "templateflow"),
        }
    )
    env.pop("LARMORX_UPSTREAM_DEFECTS", None)
    return env


# --- running both -------------------------------------------------------------------------


def run_original(args: list[str], cwd: Path, *opts: str, timeout: int = 900) -> dict[str, Any]:
    cmd = [sys.executable, "-P", str(DRIVER), *opts, "--", *args]
    try:
        p = subprocess.run(
            cmd, cwd=cwd, env=env_common(), capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return {"code": "timeout", "out": "", "err": ""}
    return {"code": p.returncode, "out": p.stdout, "err": p.stderr}


def run_mine(
    args: list[str], cwd: Path, mode: str, extra_env: dict[str, str] | None = None
) -> dict[str, Any]:
    from larmorx.sdcflows.app import run_app

    env = env_common()
    env.update(extra_env or {})
    out, err = io.StringIO(), io.StringIO()
    old = os.getcwd()
    os.chdir(cwd)
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = run_app(args, upstream_defects=mode, env=env)  # type: ignore[arg-type]
    finally:
        os.chdir(old)
    return {"code": code, "out": out.getvalue(), "err": err.getvalue()}


def tb_lines(err: str) -> str:
    """A Python traceback's first and last lines (argparse's messages as they are)."""
    lines = err.splitlines()
    if "Traceback (most recent call last):" in lines:
        i = lines.index("Traceback (most recent call last):")
        last = [
            ln
            for ln in lines
            if ln and not ln.startswith((" ", "Traceback", "During handling", "The above"))
        ]
        return lines[i] + "\n" + (last[-1] if last else "") + "\n"
    return err


def norm(s: str) -> str:
    return PARTIAL.sub(r"\1", s)


NOTICES: dict[str, int] = {}


def notices(err: str) -> str:
    """larmorx's standard error without its notices of corrected defects (D13: printed by
    Python's warnings, two lines each), which are counted by ID."""
    lines, out, skip = err.splitlines(keepends=True), [], False
    for ln in lines:
        if skip:
            skip = False
            if ln.startswith("  "):
                continue
        m = re.search(r": UpstreamDefectWarning: ([A-Z]+-\d+) ", ln)
        if m:
            NOTICES[m.group(1)] = NOTICES.get(m.group(1), 0) + 1
            skip = True
            continue
        out.append(ln)
    return "".join(out)


def same(t: cw.Tally, name: str, label: str, a: dict[str, Any], b: dict[str, Any]) -> bool:
    b = {**b, "err": notices(b["err"])}
    ok = (a["code"], a["out"], tb_lines(norm(a["err"]))) == (
        b["code"],
        b["out"],
        tb_lines(b["err"]),
    )
    detail = ""
    if not ok:
        detail = f"{label}: {a['code']} vs {b['code']}\n--- sdcflows out:\n{a['out'][-800:]}\n--- larmorx out:\n{b['out'][-800:]}\n--- sdcflows err:\n{tb_lines(norm(a['err']))[-800:]}\n--- larmorx err:\n{b['err'][-800:]}"
    t.add(name, ok, detail)
    return ok


# --- parser -------------------------------------------------------------------------------


def _parse_both(argv: list[str], cwd: Path, mode: str) -> tuple[Any, Any]:
    from larmorx.sdcflows.app import build_parser

    def one(make: Any) -> Any:
        out, err = io.StringIO(), io.StringIO()
        old = os.getcwd()
        os.chdir(cwd)
        sys.argv[0] = "sdcflows"
        try:
            parser = make()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                try:
                    ns = parser.parse_args(argv)
                    res: Any = (
                        "ok",
                        {k: str(v) if isinstance(v, Path) else v for k, v in vars(ns).items()},
                    )
                except SystemExit as e:
                    res = ("exit", e.code)
                except Exception as e:  # sdcflows' converters that fail (SDC-33)
                    res = ("raise", type(e).__name__, str(e))
        finally:
            os.chdir(old)
        return res, out.getvalue(), norm(err.getvalue())

    from sdcflows.cli.parser import _parser

    return one(_parser), one(lambda: build_parser(upstream_defects=mode))


def sec_parser(t: cw.Tally, scratch: Path) -> None:
    base = scratch / "parser"
    base.mkdir(parents=True, exist_ok=True)
    bids = base / "bids"
    bids.mkdir(exist_ok=True)
    out = str(base / "out")
    plugin = base / "plugin.yml"
    plugin.write_text("plugin: Linear\n", encoding="utf-8")
    B = str(bids)
    P = [B, out, "participant"]
    cases: list[list[str]] = [
        [], ["-h"], ["--help"], ["--version"], [B], [B, out], P, [B, out, "group"],
        [B, out, "participant", "group", "participant"], [B, out, "other"],
        ["/nonexistent/x", out, "participant"], [*P, "--nprocs", "0"], [*P, "--nprocs", "-3"],
        [*P, "--nprocs", "abc"], [*P, "--omp-nthreads", "x1"], [*P, "--omp-nthreads", "4", "--nprocs", "8"],
        [*P, "--participant-label", "sub-01", "02", "01"], [*P, "--participant-label"],
        [*P, "--participant-label=sub-03"], [*P, "--session-label"], [*P, "--session-label", "a", "b"],
        [*P, "-vvv"], [*P, "-v", "-v"], [*P, "--verbose"], [*P, "-w", "rel/work"], [*P, "--work-dir=/abs/w"],
        [*P, "-n"], [*P, "--dry-run"], [*P, "--no-fmapless"], [*P, "--use-plugin", str(plugin)],
        [*P, "--bids-filter-file", "f.json"], [*P, "--bids-database-dir", B],
        [*P, "--bids-database-dir", "/nonexistent/db"], [*P, "--bids-database-wipe", "--notrack", "--debug", "--pdb", "--sloppy"],
        [*P, "--unknown"], [*P, "--part", "01"], [*P, "--omp", "2"], [*P, "--no-f"], [*P, "--n", "2"],
        [*P, "-h", "--bogus"], ["--version", "x"], [*P, "-x"],
    ]  # fmt: skip
    for mem in ("16", "16G", "16GB", "2T", "512M", "100K", "5B", "0", "abc", "", "T"):
        cases.append([*P, "--mem-gb", mem])
    sdc33 = ("16g", "1.5G", "16 GB", "G16", "3m")
    for mem in sdc33:
        cases.append([*P, "--mem-gb", mem])
    for mode in ("reproduce", "fix"):
        for argv in cases:
            a, b = _parse_both(argv, base, mode)
            label = f"{mode} {argv[3:] if len(argv) > 3 else argv}"
            if mode == "fix" and "--mem-gb" in argv and argv[-1] in sdc33:
                ok = a[0][0] == "raise" and b[0] == ("exit", 2) and "invalid _to_gb value" in b[2]
                t.add("parser: SDC-33 (--mem-gb units) corrected", ok, f"{label}: {a} | {b}")
                if ok:
                    EXPECTED.append(
                        f"SDC-33 {argv[-1]!r}: sdcflows {a[0][1]}, larmorx: {b[2].splitlines()[-1]}"
                    )
                continue
            t.add(f"parser: {mode}", a == b, f"{label}:\n{a}\n{b}")


# --- toml ---------------------------------------------------------------------------------


def sec_toml(t: cw.Tally, scratch: Path) -> None:
    import toml

    from larmorx.sdcflows.app import toml_dumps

    samples: list[dict[str, Any]] = [
        {"a": {"s": "plain", "q": 'say "hi"', "sq": "it's", "bs": "a\\b", "u": "é ü", "ctl": "x\ty\n\x01",
               "i": 3, "neg": -7, "f": 0.1, "big": 1e300, "small": 1e-5, "e": 1.5e-7, "fi": 2.0, "nan": float("nan"),
               "inf": float("inf"), "b": True, "c": False, "l": [1, "x", True, 2.5], "e0": [], "nested": [[1, 2], []],
               "none": None, "ln": [None]}},
        {"a": {"t": {"x": 1, "y": {"z": "deep"}}, "after": 1}, "b": {}},
        {"a": {"aot": [{"k": 1}, {"k": 2, "sub": {"q": 1}}], "v": 1}},
        {"sec": {"key with space": 1, "dotted.key": 2, "ok_key-1": 3}},
        {"execution": {"bids_filters": {"fmap": {"acquisition": "A", "session": ["1", "2"], "x": None}, "bold": {"task": "rest"}}}},
    ]  # fmt: skip
    for i, s in enumerate(samples):
        ok = toml.dumps(s) == toml_dumps(s)
        t.add("toml: dumps", ok, f"sample {i}:\n{toml.dumps(s)}\n---\n{toml_dumps(s)}")


# --- settings -----------------------------------------------------------------------------


def _tree(
    base: Path, name: str, subjects: tuple[str, ...] = ("01",), description: bool = True
) -> Path:
    import nibabel as nb
    import numpy as np

    root = base / name
    if root.exists():
        shutil.rmtree(root)
    for sub in subjects:
        for kind, fname, meta in (
            ("fmap", f"sub-{sub}_dir-AP_epi.nii.gz", {"PhaseEncodingDirection": "j-", "TotalReadoutTime": 0.05, "IntendedFor": f"func/sub-{sub}_task-rest_bold.nii.gz"}),
            ("fmap", f"sub-{sub}_dir-PA_epi.nii.gz", {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "IntendedFor": f"func/sub-{sub}_task-rest_bold.nii.gz"}),
            ("func", f"sub-{sub}_task-rest_bold.nii.gz", {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}),
            ("anat", f"sub-{sub}_T1w.nii.gz", {}),
        ):  # fmt: skip
            d = root / f"sub-{sub}" / kind
            d.mkdir(parents=True, exist_ok=True)
            nb.Nifti1Image(np.zeros((4, 4, 4), np.int16), np.eye(4)).to_filename(d / fname)
            (d / fname.replace(".nii.gz", ".json")).write_text(json.dumps(meta), encoding="utf-8")
    if description:
        (root / "dataset_description.json").write_text(
            '{"Name": "t", "BIDSVersion": "1.8.0"}', encoding="utf-8"
        )
    return root


def _settings_file(work: Path) -> dict[str, Any] | None:
    files = sorted(work.glob(".sdcflows.*.toml"))
    if len(files) != 1:
        return None
    return tomllib.loads(files[0].read_text(encoding="utf-8"))


def _compare_settings(
    t: cw.Tally,
    label: str,
    a: dict[str, Any] | None,
    b: dict[str, Any] | None,
    expect: dict[str, Any] | None = None,
) -> None:
    t.add("settings: file written", (a is None) == (b is None), f"{label}: {a is None} {b is None}")
    if a is None or b is None:
        return
    for sec in ("execution", "workflow", "nipype"):
        x, y = dict(a.get(sec, {})), dict(b.get(sec, {}))
        for d in (x, y):
            if "analysis_level" in d:  # SDC-34: sdcflows writes a set's order
                d["analysis_level"] = sorted(d["analysis_level"])
            d.pop("run_uuid", None)
            d.pop("bids_database_dir", None)  # larmorx writes no database
        for k, v in (expect or {}).get(sec, {}).items():
            x[k] = v
        t.add(
            f"settings: [{sec}]",
            x == y,
            f"{label}: {sorted(set(x.items()) ^ set(y.items()), key=str) if all(isinstance(v, (str, int, float, bool)) for v in [*x.values(), *y.values()]) else (x, y)}",
        )
    keys_a = [
        k for k in a.get("environment", {}) if k not in ("nipype_version", "templateflow_version")
    ]
    keys_b = [k for k in b.get("environment", {}) if k != "larmorx_version"]
    t.add("settings: [environment] keys", keys_a == keys_b, f"{label}: {keys_a} {keys_b}")
    for k in (
        "cpu_count",
        "exec_env",
        "overcommit_policy",
        "overcommit_limit",
        "total_memory",
        "version",
    ):
        t.add(
            "settings: [environment] values",
            a["environment"].get(k) == b["environment"].get(k),
            f"{label}: {k}",
        )


def _rebased(settings: Any, base: str) -> Any:
    """The settings with the run's own folder written ``<base>``."""
    if isinstance(settings, dict):
        return {k: _rebased(v, base) for k, v in settings.items()}
    if isinstance(settings, list):
        return [_rebased(v, base) for v in settings]
    if isinstance(settings, str):
        return settings.replace(base, "<base>")
    return settings


def sec_settings(t: cw.Tally, scratch: Path, jobs: int) -> None:
    base = scratch / "settings"
    if base.exists():
        shutil.rmtree(base)
    base.mkdir(parents=True)
    two = _tree(base, "two", ("01", "02"))
    nodesc = _tree(base, "nodesc", description=False)
    filt = base / "filters.json"
    filt.write_text(
        json.dumps({"fmap": {"direction": "AP"}, "bold": {"task": "rest"}}), encoding="utf-8"
    )
    bad = base / "bad.json"
    bad.write_text("{not json", encoding="utf-8")
    plugin = base / "plugin.yml"
    plugin.write_text("plugin: Linear\nplugin_args: {nprocs: 3}\n", encoding="utf-8")
    ds = Path(cd.run_info(SDC / "ds005040-sub-002")["root"])
    cases: list[tuple[str, list[str], dict[str, Any] | None]] = []

    def case(
        name: str,
        root: Path,
        extra: list[str],
        out: str = "out",
        work: str = "work",
        expect: dict[str, Any] | None = None,
    ) -> None:
        cases.append(
            (
                name,
                [
                    str(root),
                    f"{{base}}/{out}",
                    "participant",
                    "-w",
                    f"{{base}}/{work}",
                    "--dry-run",
                    *extra,
                ],
                expect,
            )
        )

    case(
        "d8-ds005040",
        ds,
        [
            "--participant-label",
            "002",
            "--nprocs",
            "4",
            "--omp-nthreads",
            "4",
            "--no-fmapless",
            "--notrack",
            "-v",
        ],
    )
    case("all-subjects", two, [])
    case("labels", two, ["--participant-label", "02", "sub-01"])
    case("missing-one", two, ["--participant-label", "01", "07"])
    case("missing-two", two, ["--participant-label", "08", "07", "01"])
    case(
        "sessions",
        two,
        ["--session-label", "1", "2", "-vvv", "--mem-gb", "512M", "--sloppy", "--debug"],
    )
    case("filters", two, ["--bids-filter-file", str(filt), "--bids-database-wipe"])
    case("filters-missing", two, ["--bids-filter-file", str(base / "none.json")])
    case("filters-bad", two, ["--bids-filter-file", str(bad)])
    case("plugin", two, ["--use-plugin", str(plugin)])
    case("plugin-missing", two, ["--use-plugin", str(base / "none.yml")])
    case("pdb", two, ["--pdb"])
    case("no-description", nodesc, [])
    case("group", two, [], expect=None)
    cases[-1][1][2:3] = ["participant", "group"]
    for name, src in (("same-as-bids", "two-copy"), ("work-in-bids", "two-copy2")):
        root = _tree(base, src, ("01",))
        if name == "same-as-bids":
            cases.append(
                (
                    name,
                    [str(root), str(root), "participant", "-w", "{base}/work", "--dry-run"],
                    None,
                )
            )
        else:
            cases.append(
                (
                    name,
                    [str(root), "{base}/out", "participant", "-w", str(root / "work"), "--dry-run"],
                    None,
                )
            )

    def prepare(
        item: tuple[str, list[str], dict[str, Any] | None], mode: str, side: str
    ) -> tuple[Path, list[str]]:
        name, args, _ = item
        b = base / f"{name}-{mode}-{side}"
        if b.exists():
            shutil.rmtree(b)
        b.mkdir(parents=True)
        return b, [a.replace("{base}", str(b)) for a in args]

    def finish(b: Path, r: dict[str, Any]) -> dict[str, Any]:
        # pybids' database folders, and an output folder made only to hold one, are sdcflows'
        # (larmorx writes no database)
        def kept(p: Path) -> bool:
            if ".bids_db" in p.as_posix():
                return False
            return not (p == b / "out" and all(x.name.startswith(".bids_db") for x in p.iterdir()))

        r["dirs"] = sorted(
            p.relative_to(b).as_posix() for p in b.rglob("*") if p.is_dir() and kept(p)
        )
        r["settings"] = _settings_file(b / "work") if (b / "work").exists() else None
        r["base"] = str(b)
        return r

    def original(item: Any, mode: str) -> dict[str, Any]:
        b, argv = prepare(item, mode, "sdcflows")
        return finish(b, run_original(argv, b))

    def compare(item: Any, mode: str, a: dict[str, Any]) -> None:
        name, _, expect = item
        b, argv = prepare(item, mode, "larmorx")
        m = finish(b, run_mine(argv, b, mode))
        if mode == "fix" and name == "plugin":  # SDC-35: sdcflows always fails on the file
            ok = a["code"] == 1 and "Loader" in a["err"] and m["code"] == 0 and m["settings"]
            t.add("settings: SDC-35 corrected", bool(ok), f"{name}: {a['code']} {m['code']}")
            if ok:
                EXPECTED.append(
                    f"SDC-35: sdcflows {tb_lines(a['err']).splitlines()[-1]}; larmorx runs"
                )
            return
        fixed = mode == "fix" and name in ("filters-missing", "filters-bad", "plugin-missing")
        if fixed:  # SDC-33: argparse's error instead of the traceback
            ok = a["code"] == 1 and m["code"] == 2 and "error:" in m["err"]
            t.add(
                "settings: SDC-33 corrected",
                ok,
                f"{name}: {a['code']} {m['code']} {m['err'][-200:]}",
            )
            if ok:
                EXPECTED.append(
                    f"SDC-33 {name}: sdcflows {tb_lines(a['err']).splitlines()[-1]}; "
                    f"larmorx {m['err'].splitlines()[-1]}"
                )
            return
        if name == "missing-two":  # SDC-34: a set's order in sdcflows' message
            a = dict(a)
            msg = re.search(r"directory: (.*)\.$", a["err"], re.M)
            if msg:
                a["err"] = a["err"].replace(
                    msg.group(1), ", ".join(sorted(msg.group(1).split(", ")))
                )
        same(t, f"settings: run ({mode})", f"{name}", a, m)
        t.add(
            "settings: folders", a["dirs"] == m["dirs"], f"{name} {mode}: {a['dirs']} {m['dirs']}"
        )
        _compare_settings(
            t,
            f"{name} {mode}",
            _rebased(a["settings"], a["base"]),
            _rebased(m["settings"], m["base"]),
            expect,
        )

    work = [(c, "reproduce") for c in cases] + [
        (c, "fix")
        for c in cases
        if c[0]
        in (
            "filters-missing",
            "filters-bad",
            "plugin-missing",
            "plugin",
            "d8-ds005040",
            "labels",
            "sessions",
            "filters",
        )
    ]
    with ThreadPoolExecutor(max(1, jobs)) as pool:
        originals = list(pool.map(lambda w: original(*w), work))
    for (c, mode), a in zip(work, originals, strict=True):  # larmorx in this process, one at a time
        compare(c, mode, a)


# --- dry runs -----------------------------------------------------------------------------


def _datasets(scratch: Path) -> list[tuple[str, Path, list[str]]]:
    out = []
    for run in sorted(SDC.iterdir()):
        info = cd.run_info(run)
        out.append((run.name, Path(info["root"]), ["--participant-label", info["sub"]]))
    out.append(
        (
            "ds000228-syn",
            W / "oracles" / "bids-views" / "ds000228",
            ["--participant-label", "pixar008"],
        )
    )
    data = ce.TAGS / "sdcflows-2.17.1" / "sdcflows" / "tests" / "data"
    for name in ("dsA", "dsB", "dsC"):
        out.append((name, data / name, []))
    for name, root, _subs in ce.make_trees(scratch / "dryrun-trees"):
        out.append((f"tree-{name}", root, []))
    return out


def sec_dryrun(t: cw.Tally, scratch: Path, jobs: int) -> None:
    base = scratch / "dryrun"
    if base.exists():
        shutil.rmtree(base)
    base.mkdir(parents=True)
    items = []
    for name, root, extra in _datasets(scratch):
        for fm in ([], ["--no-fmapless"]):
            items.append((f"{name}{'-nofmapless' if fm else ''}", root, [*extra, *fm]))
    sessions = [
        ("sessions", ["--session-label", "01"]),
        ("sessions-none", ["--session-label", "zz"]),
    ]

    def folder(
        item: tuple[str, Path, list[str]], mode: str, opts: tuple[str, ...], side: str
    ) -> tuple[Path, list[str]]:
        name, root, extra = item
        b = base / f"{name}-{mode}-{'-'.join(o.strip('-') for o in opts) or 'plain'}-{side}"
        b.mkdir(parents=True, exist_ok=True)
        return b, [
            str(root),
            str(b / "out"),
            "participant",
            "-w",
            str(b / "work"),
            "--dry-run",
            *extra,
        ]

    def run_all(work: list[tuple[Any, str, tuple[str, ...]]]) -> None:
        def original(w: Any) -> dict[str, Any]:
            b, argv = folder(*w, "sdcflows")
            return run_original(argv, b, *w[2])

        with ThreadPoolExecutor(max(1, jobs)) as pool:
            originals = list(pool.map(original, work))
        for w, a in zip(work, originals, strict=True):
            b, argv = folder(*w, "larmorx")
            m = run_mine(argv, b, w[1])
            if w[1] == "fix" and w[0][0].startswith("tree-sdc21"):
                # SDC-21: sdcflows 2.15.2 takes sub-010's files into sub-01's search and fails;
                # corrected, the two participants share the identifier 'pe' (SDC-36)
                ok = a["code"] == 1 and "is not in the subpath" in a["err"] and m["code"] == 0
                t.add("dryrun: SDC-21 + SDC-36 corrected", ok, f"{w[0][0]}: {a['code']} {m}")
                if ok:
                    EXPECTED.append(
                        f"SDC-21/SDC-36 {w[0][0]}: sdcflows 2.15.2 fails, larmorx lists"
                    )
                continue
            same(t, f"dryrun: {w[1]}{' ' + ' '.join(w[2]) if w[2] else ''}", w[0][0], a, m)

    # reproduce: sdcflows 2.17.1; the default: sdcflows 2.15.2's search (SDC-17)
    run_all(
        [(it, "reproduce", ()) for it in items]
        + [(it, "fix", ("--wrangler", "2.15.2")) for it in items]
    )
    # SDC-30, SDC-31: sessions and the filter file's fmap entry passed to the search
    filt = base / "filters.json"
    filt.write_text(
        json.dumps({"fmap": {"acquisition": "A"}, "bold": {"task": "x"}}), encoding="utf-8"
    )
    trees = {n: (n, r, e) for n, r, e in items}
    extra_items = []
    for name, opt in sessions:
        root = trees["tree-sessionless_t1w"][1]
        extra_items.append((f"tree-sessionless_t1w-{name}", root, opt))
    extra_items.append(
        ("tree-filters-filterfile", trees["tree-filters"][1], ["--bids-filter-file", str(filt)])
    )
    run_all(
        [(it, "fix", ("--wrangler", "2.15.2", "--pass-filters")) for it in extra_items]
        + [(it, "reproduce", ()) for it in extra_items]
    )


# --- exit statuses ------------------------------------------------------------------------


def sec_exits(t: cw.Tally, scratch: Path) -> None:
    base = scratch / "exits"
    if base.exists():
        shutil.rmtree(base)
    base.mkdir(parents=True)
    ds228 = W / "oracles" / "bids-views" / "ds000228"
    noreadout = _tree(base, "noreadout")
    for f in (noreadout / "sub-01" / "fmap").glob("*.json"):
        meta = json.loads(f.read_text(encoding="utf-8"))
        meta.pop("TotalReadoutTime")
        f.write_text(json.dumps(meta), encoding="utf-8")
    cases = [
        (
            "no-estimators",
            ds228,
            ["--participant-label", "pixar008", "--no-fmapless"],
            ("reproduce", "fix"),
        ),
        ("group", ds228, ["--participant-label", "pixar008", "--no-fmapless"], ("reproduce",)),
        ("fieldmapless", ds228, ["--participant-label", "pixar008"], ("reproduce", "fix")),
        ("no-readout", noreadout, ["--no-fmapless"], ("reproduce",)),
    ]
    for name, root, extra, modes in cases:
        for mode in modes:
            res = []
            for side in ("sdcflows", "larmorx"):
                b = base / f"{name}-{mode}-{side}"
                b.mkdir(parents=True, exist_ok=True)
                level = ["participant", "group"] if name == "group" else ["participant"]
                argv = [
                    str(root),
                    str(b / "out"),
                    *level,
                    "-w",
                    str(b / "work"),
                    "--notrack",
                    "--nprocs",
                    "1",
                    "--omp-nthreads",
                    "1",
                    *extra,
                ]
                if side == "sdcflows":
                    res.append(
                        run_original(argv, b, "--shims", str(base / "shims"), timeout=1800)
                        if mode == "reproduce"
                        else None
                    )
                else:
                    res.append(run_mine(argv, b, mode, {"LARMORX_IMPLEMENTATION": "replica"}))
                    res[-1]["files"] = sorted(
                        p.relative_to(b / "out").as_posix()
                        for p in (b / "out").rglob("*")
                        if p.is_file()
                    )
            a, m = res
            if mode == "fix":
                ok = (
                    (m["code"] == 1 and "SDC-29" in m["err"] and not m["files"])
                    if name == "fieldmapless"
                    else m["code"] == 64
                )
                t.add(f"exits: {name} (default)", ok, f"{m}")
                if name == "fieldmapless" and ok:
                    EXPECTED.append(
                        f"SDC-29: larmorx stops before any work: {m['err'].strip()[:160]}..."
                    )
                continue
            a = dict(a)
            a["out"] = _no_times(a["out"])
            m = dict(m)
            m["out"] = _no_times(m["out"])
            if name in ("fieldmapless", "no-readout"):
                # sdcflows: nipype's node messages and crash report; compare the exit status and say what each wrote
                ok = a["code"] == m["code"] == 1
                t.add(
                    f"exits: {name} (reproduce: both fail)",
                    ok,
                    f"{a['code']} {m['code']}\n{a['err'][-600:]}\n{m['err'][-600:]}",
                )
                print(
                    f"INFO {name}: sdcflows' last line: {tb_lines(a['err']).splitlines()[-1] if a['err'] else ''}"
                )
                print(
                    f"INFO {name}: larmorx's: {m['err'].strip().splitlines()[-1] if m['err'] else ''}"
                )
                continue
            same(t, f"exits: {name}", f"{name} {mode}", a, m)


def _no_times(text: str) -> str:
    return re.sub(r"^\d{6}-\d\d:\d\d:\d\d,\d+ ", "<time> ", text, flags=re.M)


# --- the whole run ------------------------------------------------------------------------


def sec_run(t: cw.Tally, scratch: Path, datasets: list[str], threads: int) -> None:
    base = scratch / "run"
    base.mkdir(parents=True, exist_ok=True)
    for name in datasets:
        info = cd.run_info(SDC / name)
        res, roots = [], []
        for side in ("sdcflows", "larmorx"):
            b = base / f"{name}-{side}"
            if b.exists():
                shutil.rmtree(b)
            b.mkdir(parents=True)
            roots.append(b)
            t0 = time.time()
            if side == "sdcflows":
                argv = [
                    info["root"],
                    str(b / "out"),
                    "participant",
                    "--participant-label",
                    info["sub"],
                    "-w",
                    str(b / "work"),
                    "--nprocs",
                    "1",
                    "--omp-nthreads",
                    "1",
                    "--no-fmapless",
                    "--notrack",
                    "--pdb",  # nipype's Linear plugin (sdcflows' --use-plugin always fails: SDC-35)
                ]
                r = run_original(argv, b, "--shims", str(base / "shims"), timeout=3600)
            else:
                argv = [
                    info["root"],
                    str(b / "out"),
                    "participant",
                    "--participant-label",
                    info["sub"],
                    "-w",
                    str(b / "work"),
                    "--nprocs",
                    "1",
                    "--omp-nthreads",
                    "1",
                    "--no-fmapless",
                    "--notrack",
                    "--pdb",  # nipype's Linear plugin (sdcflows' --use-plugin always fails: SDC-35)
                ]
                r = run_mine(
                    argv,
                    b,
                    "reproduce",
                    {
                        "LARMORX_IMPLEMENTATION": "replica",
                        "FSLDIR": __import__("check_pepolar").fsldir(),
                    },
                )
            print(f"INFO {name} {side}: {time.time() - t0:.0f} s, exit {r['code']}")
            res.append(r)
        a, m = res
        t.add(
            "run: exit status 0",
            a["code"] == m["code"] == 0,
            f"{name}: {a['code']} {m['code']}\n{a['err'][-1500:]}\n{m['err'][-1500:]}",
        )
        cli_a = _cli_lines(a["out"])
        cli_m = _cli_lines(m["out"])
        t.add("run: the cli logger's lines", cli_a == cli_m, f"{name}:\n{cli_a}\n{cli_m}")
        ref_out, mine_out = roots[0] / "out", roots[1] / "out"
        want = sorted(
            p.relative_to(ref_out)
            for p in ref_out.rglob("*")
            if p.is_file() and "figures" not in p.parts
        )
        got = sorted(p.relative_to(mine_out) for p in mine_out.rglob("*") if p.is_file())
        t.add("run: the derivative files", want == got, f"{name}: {set(want) ^ set(got)}")
        for rel in want:
            cd.compare_file(t, "run", name, ref_out / rel, mine_out / rel)
        t.add(
            "run: logs folder", (ref_out / "logs").is_dir() and (mine_out / "logs").is_dir(), name
        )
        _compare_settings(
            t,
            f"run {name}",
            _rebased(_settings_file(roots[0] / "work"), str(roots[0])),
            _rebased(_settings_file(roots[1] / "work"), str(roots[1])),
            None,
        )


def _cli_lines(text: str) -> list[str]:
    """The ``cli`` logger's records (header and message), time stamps apart."""
    out, keep = [], False
    for ln in text.splitlines():
        m = re.match(r"^\d{6}-\d\d:\d\d:\d\d,\d+ (\S+) (\S+):$", ln)
        if m:
            keep = m.group(1) == "cli"
            if keep:
                out.append(f"{m.group(1)} {m.group(2)}:")
            continue
        if keep:
            out.append(ln)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--sections", nargs="*", default=["parser", "toml", "settings", "dryrun", "exits"]
    )
    ap.add_argument("--datasets", nargs="*", default=["ds005752-sub-ON01016-direct"])
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--threads", type=int, default=1)
    args = ap.parse_args()
    scratch = Path(args.scratch)
    scratch.mkdir(parents=True, exist_ok=True)
    os.environ["COLUMNS"] = "100"  # argparse's width, in this process too
    t = cw.Tally()
    if "parser" in args.sections:
        sec_parser(t, scratch)
    if "toml" in args.sections:
        sec_toml(t, scratch)
    if "settings" in args.sections:
        sec_settings(t, scratch, args.jobs)
    if "dryrun" in args.sections:
        sec_dryrun(t, scratch, args.jobs)
    if "exits" in args.sections:
        sec_exits(t, scratch)
    if "run" in args.sections:
        sec_run(t, scratch, args.datasets, args.threads)
    for e in sorted(set(EXPECTED)):
        print(f"EXPECTED {e}")
    for k, n in sorted(NOTICES.items()):
        print(f"NOTICES {k}: {n} (larmorx's notices of corrected defects on standard error)")
    bad = t.report()
    print("ALL OK" if not bad else f"DIFFERENCES: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
