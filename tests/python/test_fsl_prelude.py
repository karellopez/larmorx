# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.prelude``: FSL's PRELUDE, with its replica (``larmorx-nc``) and its clean-room
original (this package, since 2026-11-06). Without the replica's program, ``auto`` and
``original`` run the original in the process and ``replica`` raises
:class:`larmorx._replica.ReplicaNotFoundError`; the arguments are checked first. With a stand-in
program written by the test (a small Python script that records its arguments and writes
prelude's output files), the wrapper's command line, its settings and the outputs it reads back.
The outputs themselves are checked against FSL by the parity suites (``fsl-prelude-replica``,
``fsl-prelude``) and on every platform by the golden cases."""

from __future__ import annotations

import json
import os
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
if args == ["--version"]:
    print("larmorx-nc 9.9.9")
    sys.exit(0)
assert args[:2] == ["fsl", "prelude"], args
a = args[2:]
record = {"args": a, "threads": os.environ.get("OMP_NUM_THREADS"),
          "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"),
          "fsl": {k: v for k, v in os.environ.items() if k.upper().startswith("FSL")},
          "cwd": os.getcwd()}
if "-c" in a:
    with open(a[a.index("-c") + 1], "rb") as f:
        header = f.read(348)
    record["complex_datatype"] = int.from_bytes(header[70:72], "little")
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps(record) + "\\n")
names = [a[a.index("-o") + 1]]
for flag in ("-l", "-r"):
    if flag in a:
        names.append(a[a.index(flag) + 1])
names += [w.partition("=")[2] for w in a if w.startswith("--savemask=")]
for name in names:
    shutil.copyfile(here / "template.nii", name + ".nii")
if "-v" in a:
    print("Loading volumes")
if any(w.startswith("--savemask=") for w in a):
    sys.stderr.write("larmorx-nc: upstream defect FSL-262 (prelude) corrected: every volume's "
                     "mask or labels saved, not only the last "
                     "[LARMORX_UPSTREAM_DEFECTS=reproduce gives prelude's result]\\n")
"""

AFFINE = np.diag([-2.0, 2.0, 2.5, 1.0])


def _phase() -> np.ndarray:
    i, j, k = np.meshgrid(np.arange(6), np.arange(5), np.arange(4), indexing="ij")
    return ((i * 7 + j * 3 + k * 5) % 61 / 10.0 - 3.0).astype(np.float32)


def _magnitude() -> np.ndarray:
    return np.full((6, 5, 4), 100, np.int16)


def stub(directory: Path) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "stub.py").write_text(STUB, encoding="utf-8")
    lx.save((_phase(), AFFINE), directory / "template.nii")
    script = directory / "stub.py"
    if os.name == "nt":
        program = directory / "larmorx-nc.cmd"
        text = f'@echo off\r\n"{sys.executable}" "{script}" %*\r\nexit /b %ERRORLEVEL%\r\n'
        program.write_bytes(text.encode("utf-8"))
    else:
        program = directory / "larmorx-nc"
        program.write_text(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n', "utf-8")
        program.chmod(0o755)
    return program


def calls(program: Path) -> list[dict]:
    lines = (program.parent / "calls.jsonl").read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in lines]


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


def _inputs() -> tuple[lx.Image, lx.Image]:
    return lx.Image(_phase(), AFFINE), lx.Image(_magnitude(), AFFINE)


@pytest.mark.usefixtures("no_replica")
@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({}, "needs phase and magnitude together"),
        ({"phase": "p.nii"}, "needs phase and magnitude together"),
        ({"magnitude": "m.nii"}, "needs phase and magnitude together"),
        ({"phase": "p.nii", "magnitude": "m.nii", "complex_image": "c.nii"}, "not both"),
        ({"complex_image": "c.nii", "num_phase_split": 1}, "greater than 1"),
        ({"complex_image": "c.nii", "start": -1}, "start must be a volume index"),
        ({"complex_image": "c.nii", "end": -2}, "end must be a volume index"),
        ({"complex_image": "c.nii", "n_threads": -1}, "n_threads must be"),
    ],
)
def test_the_arguments_are_checked_first(kwargs, message):
    with pytest.raises(ValueError, match=message):
        lx.fsl.prelude(**kwargs)


@pytest.mark.usefixtures("no_replica")
def test_the_original_runs_without_the_replica():
    phase, magnitude = _inputs()
    for implementation in ("auto", "original"):
        r = lx.fsl.prelude(
            phase,
            magnitude,
            implementation=implementation,
            save_mask=True,
            save_labels=True,
            save_raw_phase=True,
            verbose=True,
        )
        assert r.unwrapped.implementation == _replica.ORIGINAL
        assert r.mask is not None and r.labels is not None and r.raw_phase is not None
        # a constant magnitude: every voxel in the mask, each unwrapped by whole turns
        assert np.all(np.asarray(r.mask.data) == 1)
        turns = (np.asarray(r.unwrapped.data, np.float64) - _phase()) / (2 * np.pi)
        assert np.allclose(turns, np.round(turns), atol=1e-5)
        assert np.asarray(r.labels.data).min() >= 1
        assert r.stdout.startswith("Loading volumes\nPhase loaded\nMagnitude loaded\n")
        assert "Did while loop" in r.stdout
    with pytest.raises(_replica.ReplicaNotFoundError, match="larmorx-nc"):
        lx.fsl.prelude(phase, magnitude, implementation="replica")


@pytest.mark.usefixtures("no_replica")
def test_the_originals_failure_raises_with_preludes_message():
    _, magnitude = _inputs()
    wide = lx.Image(_phase() * 2.5, AFFINE)
    with pytest.raises(RuntimeError, match="exceeds allowable phase range"):
        lx.fsl.prelude(wide, magnitude, implementation="original")
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-262"):
        series = lx.Image(np.stack([_phase()] * 2, -1), AFFINE)
        lx.fsl.prelude(
            series,
            lx.Image(np.stack([_magnitude()] * 2, -1), AFFINE),
            save_mask=True,
            implementation="original",
        )


def test_the_registry_lists_it_with_its_original():
    r = _replica.registry()[("fsl", "prelude")]
    assert r.original
    assert (r.package, r.program, r.env) == ("larmorx-nc", "larmorx-nc", "LARMORX_NC_BIN")


def test_the_replica_receives_preludes_command_line(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR_GZ")
    monkeypatch.setenv("FSL_LOAD_NIFTI_EXTENSIONS", "1")
    phase, magnitude = _inputs()
    mask_path = tmp_path / "with space" / "mask.nii"
    mask_path.parent.mkdir()
    lx.save((np.ones((6, 5, 4), np.uint8), AFFINE), mask_path)
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-262"):
        res = lx.fsl.prelude(
            phase,
            magnitude,
            mask=mask_path,
            threshold=0.25,
            num_phase_split=6,
            slices=True,
            label_slices=True,
            force_3d=True,
            start=0,
            end=0,
            remove_ramps=True,
            save_mask=True,
            save_labels=True,
            save_raw_phase=True,
            verbose=True,
            upstream_defects="reproduce",
        )
    (call,) = calls(program)
    a = call["args"]
    assert a[0] == "-p" and a[2] == "-a"
    assert Path(a[1]).name == "phase.nii" and Path(a[3]).name == "magnitude.nii"
    assert a[4:] == [
        "-o",
        "unwrapped",
        "-m",
        str(mask_path.resolve()),
        "-t",
        "0.25",
        "-n",
        "6",
        "-s",
        "--labelslices",
        "-f",
        "--start=0",
        "--end=0",
        "--removeramps",
        "--savemask=savedmask",
        "-l",
        "labels",
        "-r",
        "rawphase",
        "-v",
    ]
    # FSL's settings are the call's, never the caller's environment (CLAUDE.md rule 6)
    assert call["fsl"] == {"FSLOUTPUTTYPE": "NIFTI"}
    assert (call["threads"], call["defects"]) == ("1", "reproduce")
    assert res.stdout == "Loading volumes\n"
    for image in (res.unwrapped, res.mask, res.labels, res.raw_phase):
        assert image is not None
        np.testing.assert_array_equal(np.asarray(image.data), _phase())
        assert image.implementation.kind == "replica"
        assert image.implementation.version == "9.9.9"


def test_the_defaults_add_no_option(scripts, tmp_path):
    program = stub(scripts)
    src = tmp_path / "in"
    src.mkdir()
    lx.save((_phase(), AFFINE), src / "ph.nii")
    lx.save((_magnitude(), AFFINE), src / "mag.nii")
    res = lx.fsl.prelude(src / "ph.nii", src / "mag.nii")
    (call,) = calls(program)
    assert call["args"] == [
        "-p",
        str((src / "ph.nii").resolve()),
        "-a",
        str((src / "mag.nii").resolve()),
        "-o",
        "unwrapped",
    ]
    assert call["defects"] == "fix"
    assert (res.mask, res.labels, res.raw_phase) == (None, None, None)


def test_a_complex_image_is_written_as_complex64(scripts):
    program = stub(scripts)
    data = (np.cos(_phase()) + 1j * np.sin(_phase())).astype(np.complex128) * 100
    lx.fsl.prelude(complex_image=(data, AFFINE))
    (call,) = calls(program)
    assert call["args"][0] == "-c" and Path(call["args"][1]).name == "complex.nii"
    assert call["complex_datatype"] == 32  # NIfTI's complex64
    with pytest.raises(ValueError, match="must hold complex values"):
        lx.fsl.prelude(complex_image=(_phase(), AFFINE))


def test_a_missing_output_is_an_error(scripts, monkeypatch):
    stub(scripts)
    # a stand-in that writes nothing: prelude's exit 0 without an output (FSL-270's reproduction)
    script = scripts / "stub.py"
    script.write_text(
        "import sys\nif sys.argv[1:] == ['--version']:\n    print('larmorx-nc 9.9.9')\n",
        encoding="utf-8",
    )
    phase, magnitude = _inputs()
    with pytest.raises(RuntimeError, match=r"wrote no unwrapped\.nii"):
        lx.fsl.prelude(phase, magnitude)
