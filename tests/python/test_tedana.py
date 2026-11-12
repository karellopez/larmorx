# SPDX-License-Identifier: Apache-2.0
"""``lx.tedana.t2smap`` and ``larmorx tedana t2smap`` with the replica of ``larmorx-gpl``: a
stand-in program written by the test (a small Python script that records its arguments and
environment and writes t2smap's files), and, where it is built, the replica itself; without the
replica, ``implementation="auto"`` runs the clean-room original (since 2026-11-12; its own tests
are in ``test_tedana_cleanroom.py``)."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica

STUB = """\
import json, os, shutil, sys
from pathlib import Path

here = Path(__file__).parent
args = sys.argv[1:]
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps({"args": args, "cwd": os.getcwd(),
                        "threads": os.environ.get("OMP_NUM_THREADS"),
                        "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"),
                        "libraries": os.environ.get("LARMORX_TEDANA_LIBRARIES"),
                        "columns": os.environ.get("COLUMNS")}) + "\\n")
if args == ["--version"]:
    print("larmorx-gpl 9.9.9")
    sys.exit(0)
assert args[:2] == ["tedana", "t2smap"], args
a = args[2:]
first = a[a.index("-d") + 1]
out = Path(a[a.index("--out-dir") + 1])
prefix = a[a.index("--prefix") + 1] if "--prefix" in a else ""
if prefix and not prefix.endswith("_"):
    prefix += "_"
if (here / "fail.txt").exists():
    sys.stderr.write("INFO     t2smap:t2smap_workflow:365 Using output directory\\n")
    sys.stderr.write("ValueError: something tedana says\\n")
    sys.exit(1)
out.mkdir(exist_ok=True)
names = {"t2star img": "T2starmap", "s0 img": "S0map", "limited t2star img":
         "desc-limited_T2starmap", "limited s0 img": "desc-limited_S0map", "combined img":
         "desc-optcom_bold", "adaptive mask img": "desc-adaptiveGoodSignal_mask",
         "rmse img": "desc-rmse_statmap"}
if "curvefit" in a:
    names["fit failures img"] = "desc-decayFitFailures_mask"
registry = {}
for key, name in names.items():
    registry[key] = f"{prefix}{name}.nii.gz"
    shutil.copyfile(first, out / registry[key])
registry["confounds tsv"] = f"{prefix}desc-confounds_timeseries.tsv"
(out / registry["confounds tsv"]).write_text(
    "rmse_mean\\trmse_max\\n1.5\\t2.25\\nn/a\\t3.0\\n", encoding="utf-8")
registry["data description json"] = f"{prefix}dataset_description.json"
(out / registry["data description json"]).write_text(
    json.dumps({"Name": "t2smap Outputs"}), encoding="utf-8")
reg = "registry.json" if "orig" in a else "desc-tedana_registry.json"
(out / f"{prefix}{reg}").write_text(json.dumps(registry), encoding="utf-8")
sys.stderr.write("INFO     t2smap:t2smap_workflow:365 Using output directory: x\\n")
sys.stderr.write("larmorx-gpl: upstream defect TED-05 (tedana) corrected: 3 full-map values "
                 "capped [LARMORX_UPSTREAM_DEFECTS=reproduce gives tedana's result]\\n")
"""


def stub(directory: Path) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    script = directory / "stub.py"
    if os.name == "nt":
        program = directory / "larmorx-gpl.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))
    else:
        program = directory / "larmorx-gpl"
        program.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n', "utf-8")
        program.chmod(0o755)
    return program


def calls(program: Path) -> list[dict]:
    """The stand-in's runs of t2smap (not its ``--version`` calls)."""
    lines = (program.parent / "calls.jsonl").read_text(encoding="utf-8").splitlines()
    return [c for c in map(json.loads, lines) if c["args"][:1] == ["tedana"]]


AFFINE = np.diag([-3.0, 3.0, 3.5, 1.0])
TES = (0.012, 0.028, 0.044)


def echoes(tmp_path: Path, n: int = 4) -> list[Path]:
    """Three echoes of a small series (8 x 7 x 6, ``n`` volumes) decaying by plain ratios."""
    rng = np.random.default_rng(5)
    base = np.zeros((8, 7, 6), dtype=np.float32)
    base[1:7, 1:6, 1:5] = 1000.0 + rng.integers(0, 200, (6, 5, 4))
    paths = []
    for e, ratio in enumerate((1.0, 0.6, 0.36)):
        data = np.stack([base * ratio + t for t in range(n)], axis=-1).astype(np.float32)
        path = tmp_path / "with space" / f"echo-{e + 1}.nii"
        path.parent.mkdir(exist_ok=True)
        lx.save((data, AFFINE), path)
        paths.append(path)
    return paths


def mask(tmp_path: Path) -> Path:
    m = np.zeros((8, 7, 6), dtype=np.uint8)
    m[1:7, 1:6, 1:5] = 1
    path = tmp_path / "with space" / "mask.nii"
    path.parent.mkdir(exist_ok=True)
    lx.save((m, AFFINE), path)
    return path


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def test_the_registry_has_t2smap():
    r = _replica.registry()[("tedana", "t2smap")]
    assert (r.package, r.program, r.env, r.licence) == (
        "larmorx-gpl",
        "larmorx-gpl",
        "LARMORX_GPL_BIN",
        "GPL-3.0-only",
    )
    # its clean-room original since 2026-11-12 (crates/larmorx-tedana)
    assert r.original and not r.builtin and "larmorx-gpl" in r.install


def test_without_the_replica_auto_runs_the_clean_room_original(scripts, tmp_path):
    files = echoes(tmp_path)
    with pytest.raises(lx.ReplicaNotFoundError, match="pip install larmorx-gpl"):
        lx.tedana.t2smap(files, TES, implementation="replica")
    with pytest.raises(lx.ReplicaNotFoundError, match="LARMORX_GPL_BIN"):
        lx.tedana.t2smap(files, TES, implementation="replica")
    auto = lx.tedana.t2smap(files, TES, mask=mask(tmp_path))
    assert (
        auto.implementation
        == _replica.ORIGINAL
        == lx.Implementation("clean-room", "larmorx", lx.__version__, "Apache-2.0")
    )
    clean = lx.tedana.t2smap(files, TES, mask=mask(tmp_path), implementation="clean-room")
    assert np.array_equal(auto.t2star.data, clean.t2star.data)


@pytest.mark.parametrize(
    ("options", "message"),
    [
        ({"fittype": "nonlinear"}, "fittype must be one of"),
        ({"fitmode": "volume"}, "fitmode must be one of"),
        ({"combmode": "mean"}, "combmode must be one of"),
        ({"masktype": ["dropout", "nope"]}, "masktype must be one of"),
        ({"masktype": []}, "masktype needs at least one"),
        ({"convention": "bidsv1.5.0"}, "convention must be one of"),
        ({"libraries": "oldest"}, "libraries must be one of"),
        ({"upstream_defects": "keep"}, "upstream_defects must be"),
        ({"n_threads": -1}, "n_threads must be 0"),
        ({"implementation": "fast"}, "implementation must be"),
    ],
)
def test_arguments_are_checked_before_anything_runs(scripts, tmp_path, options, message):
    with pytest.raises(ValueError, match=message):
        lx.tedana.t2smap(echoes(tmp_path), TES, **options)


def test_the_replica_receives_t2smaps_command_line(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("COLUMNS", "40")
    monkeypatch.setenv("LARMORX_TEDANA_LIBRARIES", "shared")
    files = echoes(tmp_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="TED-05"):
        res = lx.tedana.t2smap(
            files,
            [12.0, 28.0, 44.0],
            mask=mask(tmp_path),
            masktype=["dropout", "decay"],
            fittype="curvefit",
            fitmode="ts",
            combmode="paid",
            dummy_scans=1,
            exclude=[0, 5, 6],
            n_independent_echos=2,
            convention="orig",
            prefix="sub-01",
            verbose=True,
            debug=True,
            n_threads=3,
        )
    call = calls(program)[-1]
    a = call["args"]
    assert a[:2] == ["tedana", "t2smap"]
    assert a[a.index("-d") + 1 : a.index("-e")] == [str(f) for f in files]
    assert a[a.index("-e") + 1 : a.index("-e") + 4] == ["12.0", "28.0", "44.0"]
    for opt, value in [
        ("--mask", str(tmp_path / "with space" / "mask.nii")),
        ("--fittype", "curvefit"),
        ("--fitmode", "ts"),
        ("--combmode", "paid"),
        ("--dummy-scans", "1"),
        ("--exclude", "0,5,6"),
        ("--n-independent-echos", "2"),
        ("--convention", "orig"),
        ("--prefix", "sub-01"),
        ("--n-threads", "3"),
    ]:
        assert a[a.index(opt) + 1] == value, opt
    assert a[a.index("--masktype") + 1 : a.index("--masktype") + 3] == ["dropout", "decay"]
    for flag in ("--verbose", "--debug"):
        assert flag in a
    assert "--overwrite" not in a and "--quiet" not in a
    # the out-dir is a fresh directory of the call's temporary directory
    assert Path(a[a.index("--out-dir") + 1]).name == "out"
    # the wrapper's settings, not this process's
    assert (call["threads"], call["defects"], call["libraries"], call["columns"]) == (
        "3",
        "fix",
        "newest",
        None,
    )
    assert res.implementation == lx.Implementation(
        "replica", "larmorx-gpl", "9.9.9", "GPL-3.0-only", str(program)
    )
    # t2smap's files, read back (the stand-in copies the first echo into each image)
    first = lx.load(files[0])
    for name in ("t2star", "s0", "t2star_limited", "s0_limited", "optcom", "rmse"):
        img = getattr(res, name)
        assert np.array_equal(img.data, first.data), name
        assert img.implementation == res.implementation
    assert res.fit_failures is not None and res.t2star_variance is None
    assert res.files["t2star img"] == "sub-01_T2starmap.nii.gz"
    assert res.out_dir is None
    assert res.dataset_description == {"Name": "t2smap Outputs"}
    assert res.confounds["rmse_mean"][0] == 1.5 and np.isnan(res.confounds["rmse_mean"][1])
    assert res.confounds["rmse_max"].tolist() == [2.25, 3.0]
    # the log keeps tedana's lines, not larmorx's notices
    assert "Using output directory" in res.log and "upstream defect" not in res.log


def test_defaults_and_a_kept_output_directory(scripts, tmp_path):
    program = stub(scripts)
    files = echoes(tmp_path)
    out = tmp_path / "derivatives" / "t2smap"
    out.parent.mkdir()
    with pytest.warns(lx.UpstreamDefectWarning):
        res = lx.tedana.t2smap(
            str(files[0]), TES, out_dir=out, overwrite=True, upstream_defects="reproduce"
        )
    a = calls(program)[-1]["args"]
    # a single file is tedana's z-concatenated input; the defaults are not passed
    assert a[a.index("-d") + 1 : a.index("-e")] == [str(files[0])]
    assert a[a.index("-e") + 1 : a.index("-e") + 4] == ["0.012", "0.028", "0.044"]
    for opt in ("--mask", "--fittype", "--fitmode", "--combmode", "--masktype", "--prefix"):
        assert opt not in a, opt
    assert a[a.index("--out-dir") + 1] == str(out)
    assert a[a.index("--n-threads") + 1] == "1" and "--overwrite" in a
    assert calls(program)[-1]["defects"] == "reproduce"
    assert res.out_dir == str(out) and (out / "T2starmap.nii.gz").is_file()
    assert res.fit_failures is None


def test_in_memory_echoes_and_all_threads(scripts, tmp_path):
    program = stub(scripts)
    data = [lx.load(f) for f in echoes(tmp_path)]
    with pytest.warns(lx.UpstreamDefectWarning):
        res = lx.tedana.t2smap(data, TES, mask=lx.load(mask(tmp_path)), n_threads=0)
    call = calls(program)[-1]
    a = call["args"]
    written = a[a.index("-d") + 1 : a.index("-e")]
    assert [Path(p).name for p in written] == ["echo-1.nii", "echo-2.nii", "echo-3.nii"]
    assert a[a.index("--n-threads") + 1] == "-1" and call["threads"] is None
    assert np.array_equal(res.t2star.data, data[0].data)


def test_a_failing_run_raises_with_tedanas_messages(scripts, tmp_path):
    program = stub(scripts)
    (program.parent / "fail.txt").write_text("", encoding="utf-8")
    with pytest.raises(lx.ReplicaError, match="something tedana says") as e:
        lx.tedana.t2smap(echoes(tmp_path), TES)
    assert e.value.returncode == 1


def test_the_command_line_runs_the_replica(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("LARMORX_GPL_BIN", str(program))
    files = echoes(tmp_path)
    run = tmp_path / "run"
    run.mkdir()
    command = [sys.executable, "-m", "larmorx", "tedana", "t2smap", "-d", *map(str, files)]
    command += ["-e", *map(str, TES), "--out-dir", "out"]
    p = subprocess.run(command, cwd=run, capture_output=True, text=True, check=False)
    assert p.returncode == 0, p.stderr
    assert (run / "out" / "T2starmap.nii.gz").is_file()
    assert calls(program)[-1]["args"][:2] == ["tedana", "t2smap"]


#: ``LARMORX_GPL_BIN`` as the tests start (the ``no_replica`` fixture removes it).
_GPL_BIN = os.environ.get("LARMORX_GPL_BIN")


def _real_program() -> str | None:
    """The replica's program when this machine has it: ``LARMORX_GPL_BIN``, or one built in
    the repository (``crates-gpl/target/release``)."""
    if _GPL_BIN:
        return _GPL_BIN
    name = "larmorx-gpl.exe" if os.name == "nt" else "larmorx-gpl"
    built = Path(__file__).resolve().parents[2] / "crates-gpl" / "target" / "release" / name
    return str(built) if built.is_file() else shutil.which("larmorx-gpl")


def test_the_replica_itself(monkeypatch, no_replica, tmp_path):
    program = _real_program()
    if program is None:
        pytest.skip("larmorx-gpl is not built here (cargo build --release in crates-gpl)")
    monkeypatch.setenv("LARMORX_GPL_BIN", program)
    files = echoes(tmp_path)
    res = lx.tedana.t2smap(files, TES, mask=mask(tmp_path), n_threads=2)
    assert res.implementation.package == "larmorx-gpl"
    # the echoes decay by 0.6 per 16 ms: T2* = 16 ms / ln(1 / 0.6) = 31.3 ms inside the mask
    inside = res.adaptive_mask.data > 0
    assert inside.sum() == 6 * 5 * 4 and res.adaptive_mask.data.dtype == np.int32
    t2s = res.t2star.data[inside]
    assert np.all(np.abs(t2s - 0.016 / np.log(1 / 0.6)) < 0.002), t2s.min()
    assert res.optcom.data.shape == (8, 7, 6, 4)
    assert res.confounds["rmse_mean"].shape == (4,)
    assert res.dataset_description["GeneratedBy"][0]["Version"] == "26.0.3"
    # the wrapper's files are the program's own, run with the same arguments
    out = tmp_path / "direct"
    args = ["tedana", "t2smap", "-d", *map(str, files), "-e", *map(repr, TES)]
    args += ["--mask", str(tmp_path / "with space" / "mask.nii"), "--out-dir", str(out)]
    env = {k: v for k, v in os.environ.items() if k not in ("LARMORX_UPSTREAM_DEFECTS",)}
    p = subprocess.run([program, *args], capture_output=True, text=True, env=env, check=False)
    assert p.returncode == 0, p.stderr
    direct = lx.load(out / "T2starmap.nii.gz")
    assert np.array_equal(direct.data, res.t2star.data)
    assert np.array_equal(lx.load(out / "desc-optcom_bold.nii.gz").data, res.optcom.data)
