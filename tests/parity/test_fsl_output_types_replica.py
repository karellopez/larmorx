# SPDX-License-Identifier: Apache-2.0
"""FSL-11 for every FSL replica (``larmorx-nc``), ``FSLOUTPUTTYPE`` unset, against FSL 6.0.7.23
itself (this machine only: the ``larmorx-nc`` program built in ``crates-nc/`` or named by
``LARMORX_NC_BIN``, and FSL's installation).

For each FSL tool's image calls (one or more per tool, small generated inputs):

- by default the replica writes ``NIFTI_GZ``: its exit status, standard output and files are
  those of the run with ``FSLOUTPUTTYPE=NIFTI_GZ``, byte for byte, and its standard error the
  same plus one last line naming FSL-11, in the words of the clean-room original's line for the
  same call (``larmorx --implementation clean-room``: the same text after the program's name);
- with ``--upstream-defects reproduce`` the replica gives FSL's own run: its exit status, the
  files FSL writes before it stops (their names and bytes), its standard output and its standard
  error (``larmorx-nc``'s own notices aside; FSL's ``bet`` names itself by its full path).

The calls that write no image run as with the variable set in both modes, naming nothing, as
FSL's run. FSL's runs go through the oracle-output cache."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

pytest.importorskip("larmorx_validation.parity")
from larmorx_validation.parity import fsl_mcflirt_replica, fsl_versions

import larmorx as lx

NC = fsl_mcflirt_replica.replica_binary()
FSLDIR = fsl_mcflirt_replica.fsldir()
CLEAN_ROOM = [sys.executable, "-m", "larmorx", "--implementation", "clean-room"]

pytestmark = pytest.mark.skipif(
    NC is None or os.name == "nt" or not (FSLDIR / "bin" / "fslmaths").is_file(),
    reason="the larmorx-nc program or FSL is not here",
)

#: FSL's message when ``FSLOUTPUTTYPE`` is not set, its first line.
UNSET = b"ERROR:: Environment variable FSLOUTPUTTYPE is not set!"

TOPUP = ["--imain=pair.nii", "--datain=acq.txt", "--subsamp=1", "--miter=1", "--warpres=20", "--fwhm=0"]  # fmt: skip

#: ``(tool FSL-11's line names, the call)``: every FSL tool's calls that write an image.
IMAGE_CALLS: list[tuple[str, list[str]]] = [
    ("fslmaths", ["fslmaths", "in.nii", "-add", "1", "m"]),
    ("fslroi", ["fslroi", "in.nii", "r", "0", "6", "0", "6", "0", "5"]),
    (
        "flirt",
        ["flirt", "-in", "in.nii", "-ref", "in.nii", "-applyxfm", "-init", "eye.mat", "-out", "f"],
    ),
    # the matrix is written before the image (FSL writes it, then stops)
    (
        "flirt",
        [
            "flirt",
            "-in",
            "in.nii",
            "-ref",
            "in.nii",
            "-out",
            "f",
            "-omat",
            "f.mat",
            "-dof",
            "6",
            "-nosearch",
        ],
    ),
    ("applyxfm4D", ["applyxfm4D", "series.nii", "in.nii", "a", "eye.mat", "-singlematrix"]),
    ("bet2", ["bet2", "in.nii", "b2", "-m"]),
    # the script's three bet2 runs: one line
    ("bet", ["bet", "in.nii", "bR", "-R"]),
    # the inner skull's mesh is written before the first image
    ("betsurf", ["betsurf", "--t1only", "-m", "in.nii", "bm_mesh.vtk", "eye.mat", "bs"]),
    ("fast", ["fast", "-o", "seg", "in.nii"]),
    ("prelude", ["prelude", "-a", "in.nii", "-p", "phase.nii", "-o", "pu"]),
    ("topup", ["topup", *TOPUP, "--out=tu"]),
    (
        "applytopup",
        [
            "applytopup",
            "--imain=in.nii",
            "--inindex=1",
            "--datain=acq.txt",
            "--topup=tu0",
            "--out=at",
            "--method=jac",
        ],
    ),
    # the script goes on after its flirt stops, as FSL's does (exit 0, no output)
    ("standard_space_roi", ["standard_space_roi", "in.nii", "ss", "-ssref", "in.nii", "-maskNONE"]),
    # the matrices and the .par file before the series
    ("mcflirt", ["mcflirt", "-in", "series.nii", "-out", "mc", "-mats", "-plots"]),
    # the mean image first: nothing written
    ("mcflirt", ["mcflirt", "-in", "series.nii", "-out", "mm", "-meanvol", "-mats"]),
]

#: Calls that write no image.
NO_IMAGE_CALLS: list[list[str]] = [
    ["flirt", "-in", "in.nii", "-ref", "in.nii", "-omat", "o.mat", "-dof", "6", "-nosearch"],
    ["fslstats", "in.nii", "-M"],
    ["bet2", "in.nii", "bn", "-e", "-n"],
    ["convert_xfm", "-omat", "inv.mat", "-inverse", "eye.mat"],
    ["avscale", "eye.mat"],
    ["pairreg", "in.nii", "in.nii", "in.nii", "in.nii", "p.mat"],
]


def line(tool: str, program: str = "larmorx-nc") -> bytes:
    """FSL-11's line for a run of ``tool`` that wrote its images as ``NIFTI_GZ``."""
    return (
        f"{program}: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images "
        f"are written as NIFTI_GZ (.nii.gz); FSL's {tool} stops with an error at its first image "
        "(exit 1) [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]"
    ).encode()


def image(path: Path, shape: tuple[int, ...], f=lambda v: v) -> None:
    """A float32 image with a bright block in its middle (plain arithmetic)."""
    i = np.arange(int(np.prod(shape)))
    v = ((i * 7919) % 97).astype(np.float32).reshape(shape, order="F")
    inner = tuple(slice(s // 4, s - s // 4) for s in shape[:3])
    v[inner] += 1000.0
    if len(shape) > 3:
        v += 3.0 * np.arange(shape[3], dtype=np.float32)
    lx.save((np.asfortranarray(f(v).astype(np.float32)), np.eye(4)), path)


def environment(**changes: str | None) -> dict[str, str]:
    """FSL's clean environment (as the FSL suites': no FSL on the ``PATH``, ``FSLDIR``, OpenBLAS's
    Haswell core, one thread) without ``FSLOUTPUTTYPE``, with ``changes``."""
    env = {
        "HOME": os.environ.get("HOME", str(Path.home())),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(FSLDIR),
        "OMP_NUM_THREADS": "1",
        **fsl_versions.openblas_env(),
    }
    for k, v in changes.items():
        if v is None:
            env.pop(k, None)
        else:
            env[k] = v
    return env


@pytest.fixture(scope="module")
def inputs(tmp_path_factory) -> Path:
    """The inputs of every call; topup's results (for applytopup) and bet2's mesh (for betsurf),
    made by the replica with ``FSLOUTPUTTYPE=NIFTI_GZ``."""
    d = tmp_path_factory.mktemp("fsl11-inputs")
    image(d / "in.nii", (12, 12, 10))
    image(d / "series.nii", (12, 12, 10, 3))
    image(d / "pair.nii", (12, 12, 10, 2))
    image(d / "phase.nii", (12, 12, 10), lambda v: v / 1100.0 * 3.0 - 1.5)
    (d / "eye.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", "utf-8")
    (d / "acq.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n", "utf-8")
    for args in (["topup", *TOPUP, "--out=tu0"], ["bet2", "in.nii", "bm", "-e", "-n"]):
        p = subprocess.run([str(NC), "fsl", *args], cwd=d, env=environment(FSLOUTPUTTYPE="NIFTI_GZ"),
                           capture_output=True, check=False)  # fmt: skip
        assert p.returncode == 0, p.stderr.decode()[-500:]
    return d


def fresh(inputs: Path, d: Path) -> Path:
    shutil.copytree(inputs, d)
    return d


def written(d: Path, inputs: Path) -> dict[str, bytes]:
    """Every file under ``d`` that is not an input, by its relative name."""
    return {
        p.relative_to(d).as_posix(): p.read_bytes()
        for p in sorted(d.rglob("*"))
        if p.is_file() and not (inputs / p.relative_to(d)).is_file()
    }


def replica(d: Path, inputs: Path, args: list[str], *, reproduce: bool = False, **env):
    """``larmorx-nc [--upstream-defects reproduce] fsl args`` in ``d``: its exit status, streams
    and files."""
    mode = ["--upstream-defects", "reproduce"] if reproduce else []
    p = subprocess.run([str(NC), *mode, "fsl", *args], cwd=d, env=environment(**env),
                       capture_output=True, check=False)  # fmt: skip
    return p.returncode, p.stdout, p.stderr, written(d, inputs)


def fsl(d: Path, inputs: Path, args: list[str]):
    """FSL's own program, ``FSLOUTPUTTYPE`` unset, through the oracle-output cache."""
    from larmorx_validation import oracle_cache

    p = oracle_cache.run(
        [str(FSLDIR / "bin" / args[0]), *args[1:]],
        env=environment(),
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="fsl-output-types-replica",
        inputs=[q for q in sorted(d.iterdir()) if q.is_file()],
        timeout=600,
    )
    return p.returncode, p.stdout_bytes, p.stderr_bytes, written(d, inputs)


def own(stream: bytes) -> bytes:
    """A stream as FSL's run would print it: ``larmorx-nc``'s own lines out, FSL's program
    paths shortened to their names."""
    keep = [x for x in stream.splitlines(keepends=True) if not x.startswith(b"larmorx-nc: ")]
    return b"".join(keep).replace(f"{FSLDIR}/bin/".encode(), b"")


@pytest.mark.parametrize(("tool", "args"), IMAGE_CALLS, ids=lambda x: "-".join(x) if isinstance(x, list) else x)  # fmt: skip
def test_the_replica_writes_nifti_gz_and_reproduces_fsl(tool, args, inputs, tmp_path):
    given = replica(fresh(inputs, tmp_path / "given"), inputs, args, FSLOUTPUTTYPE="NIFTI_GZ")
    unset = replica(fresh(inputs, tmp_path / "unset"), inputs, args)
    assert given[0] == 0, given[2].decode()[-800:]
    assert b"FSL-11" not in given[2]
    # the default: the variable's run, with one last line naming FSL-11
    assert unset[0] == 0, unset[2].decode()[-800:]
    assert unset[1] == given[1]
    lines = unset[2].splitlines()
    assert lines[-1] == line(tool), unset[2].decode()[-800:]
    assert lines[:-1] == given[2].splitlines()
    assert unset[3] == given[3], sorted(unset[3])
    assert any(name.endswith(".nii.gz") for name in unset[3]), sorted(unset[3])
    # the clean-room original's line for the same call: the same words
    d = fresh(inputs, tmp_path / "clean-room")
    here = os.pathsep.join(x for x in sys.path if x)  # this process's larmorx
    p = subprocess.run([*CLEAN_ROOM, "fsl", *args], cwd=d, env=environment(PYTHONPATH=here),
                       capture_output=True, check=False)  # fmt: skip
    assert p.returncode == 0, p.stderr.decode()[-800:]
    named = [x for x in p.stderr.splitlines() if b"FSL-11" in x]
    assert named == [line(tool, "larmorx")], p.stderr.decode()[-800:]
    # reproduced: FSL's run
    r = replica(fresh(inputs, tmp_path / "reproduce"), inputs, args, reproduce=True)
    f = fsl(fresh(inputs, tmp_path / "fsl"), inputs, args)
    assert UNSET in f[2] or tool == "standard_space_roi", f[2].decode()[-800:]
    assert r[0] == f[0], (r[2].decode()[-800:], f[2].decode()[-800:])
    assert sorted(r[3]) == sorted(f[3])
    assert r[3] == f[3], "the files FSL writes before it stops differ"
    assert own(r[1]) == own(f[1])
    assert own(r[2]) == own(f[2])
    assert b"FSL-11" not in r[2]


@pytest.mark.parametrize("args", NO_IMAGE_CALLS, ids=lambda x: "-".join(x))
def test_calls_that_write_no_image_run_as_with_the_variable(args, inputs, tmp_path):
    f = fsl(fresh(inputs, tmp_path / "fsl"), inputs, args)
    assert f[0] == 0, f[2].decode()[-800:]
    for reproduce in (False, True):
        mode = "reproduce" if reproduce else "fix"
        given = replica(fresh(inputs, tmp_path / f"given-{mode}"), inputs, args,
                        reproduce=reproduce, FSLOUTPUTTYPE="NIFTI_GZ")  # fmt: skip
        unset = replica(
            fresh(inputs, tmp_path / f"unset-{mode}"), inputs, args, reproduce=reproduce
        )
        assert unset == given, (mode, unset[2].decode()[-800:])
        assert b"FSL-11" not in unset[2]
        assert sorted(unset[3]) == sorted(f[3]), mode
