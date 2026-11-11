# SPDX-License-Identifier: Apache-2.0
"""FSL-11 and FSL-09's program names for every FSL tool's clean-room original, against FSL
6.0.7.23 itself (this machine only: FSL's installation). The full set of calls;
``crates/larmorx-cli/tests/fsl_output_types.rs`` keeps one call per code path on CI (rule 10's
budget), as ``test_fsl_output_types_replica.py`` does for the replicas.

Every call runs through the console script (``python -m larmorx --implementation clean-room``,
the dispatch the standalone program shares), on small generated inputs. For each FSL tool's
calls that write an image:

- by default, ``FSLOUTPUTTYPE`` unset, the original writes ``NIFTI_GZ``: its exit status, standard
  output and files are those of its run with ``FSLOUTPUTTYPE=NIFTI_GZ``, byte for byte, and its
  standard error the same plus one line naming FSL-11, which says what FSL's program does
  instead (its first image stops it, exit 1; ``standard_space_roi``'s script goes on, exit 0);
- with ``--upstream-defects reproduce`` it stops where FSL's program does: FSL's run (through the
  oracle-output cache) has the same exit status, the same standard output (FSL's program paths
  shortened to their names) and writes the same files by name; FSL's message is printed and no
  image is written.

The calls that write no image run as with the variable set in both modes, naming nothing, and
write the files FSL's run writes. FSL-09's notice names the program that wrote an integer output
(flirt, applyxfm4D, applytopup), in both modes."""

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

FSLDIR = fsl_mcflirt_replica.fsldir()
CLEAN_ROOM = [sys.executable, "-m", "larmorx", "--implementation", "clean-room"]

pytestmark = pytest.mark.skipif(
    os.name == "nt" or not (FSLDIR / "bin" / "fslmaths").is_file(),
    reason="FSL is not here",
)

#: FSL's message when ``FSLOUTPUTTYPE`` is not set, its first line.
UNSET = b"ERROR:: Environment variable FSLOUTPUTTYPE is not set!"

TOPUP = ["--imain=pair.nii", "--datain=acq.txt", "--subsamp=1", "--miter=1", "--warpres=20", "--fwhm=0"]  # fmt: skip

#: ``(tool FSL-11's line names, the family and its arguments)``: every FSL tool's calls that
#: write an image.
IMAGE_CALLS: list[tuple[str, list[str]]] = [
    ("fslmaths", ["fsl", "fslmaths", "in.nii", "-add", "1", "m"]),
    ("fslroi", ["fsl", "fslroi", "in.nii", "r", "0", "6", "0", "6", "0", "5"]),
    (
        "flirt",
        [
            "fsl",
            "flirt",
            "-in",
            "in.nii",
            "-ref",
            "in.nii",
            "-applyxfm",
            "-init",
            "eye.mat",
            "-out",
            "f",
        ],
    ),
    # the matrix is written before the image (FSL writes it, then stops)
    (
        "flirt",
        [
            "fsl",
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
    ("applyxfm4D", ["fsl", "applyxfm4D", "series.nii", "in.nii", "a", "eye.mat", "-singlematrix"]),
    ("bet2", ["fsl", "bet2", "in.nii", "b2", "-m"]),
    # the script's three bet2 runs: one line
    ("bet", ["fsl", "bet", "in.nii", "bR", "-R"]),
    # the inner skull's mesh is written before the first image
    ("betsurf", ["fsl", "betsurf", "--t1only", "-m", "in.nii", "bm_mesh.vtk", "eye.mat", "bs"]),
    ("fast", ["fsl", "fast", "-o", "seg", "in.nii"]),
    ("prelude", ["fsl", "prelude", "-a", "in.nii", "-p", "phase.nii", "-o", "pu"]),
    ("topup", ["fsl", "topup", *TOPUP, "--out=tu"]),
    (
        "applytopup",
        [
            "fsl",
            "applytopup",
            "--imain=in.nii",
            "--inindex=1",
            "--datain=acq.txt",
            "--topup=tu0",
            "--out=at",
            "--method=jac",
        ],
    ),
    # the script goes on after its steps stop, as FSL's does (exit 0, no image)
    (
        "standard_space_roi",
        ["fsl", "standard_space_roi", "in.nii", "ss", "-ssref", "in.nii", "-maskNONE"],
    ),
    # the matrices and the .par file before the series
    ("mcflirt", ["fsl", "mcflirt", "-in", "series.nii", "-out", "mc", "-mats", "-plots"]),
    # the mean image first: nothing written
    ("mcflirt", ["fsl", "mcflirt", "-in", "series.nii", "-out", "mm", "-meanvol", "-mats"]),
    # `mri hmc` is mcflirt's clean-room original under its own name: the same rule
    ("mcflirt", ["mri", "hmc", "-in", "series.nii", "-out", "mh", "-mats", "-plots"]),
]

#: Calls that write no image.
NO_IMAGE_CALLS: list[list[str]] = [
    ["fsl", "flirt", "-in", "in.nii", "-ref", "in.nii", "-omat", "o.mat", "-dof", "6", "-nosearch"],
    ["fsl", "fslstats", "in.nii", "-M"],
    ["fsl", "bet2", "in.nii", "bn", "-e", "-n"],
    ["fsl", "convert_xfm", "-omat", "inv.mat", "-inverse", "eye.mat"],
    ["fsl", "avscale", "eye.mat"],
    ["fsl", "pairreg", "in.nii", "in.nii", "in.nii", "in.nii", "p.mat"],
]

#: ``(tool, the call)``: integer outputs whose values a half-voxel shift or a fractional input
#: makes fractional, with ``FSLOUTPUTTYPE=NIFTI_GZ`` (FSL-09).
FSL09_CALLS: list[tuple[str, list[str]]] = [
    (
        "flirt",
        [
            "fsl",
            "flirt",
            "-in",
            "phase.nii",
            "-ref",
            "in.nii",
            "-applyxfm",
            "-init",
            "eye.mat",
            "-out",
            "x",
            "-datatype",
            "short",
        ],
    ),
    ("applyxfm4D", ["fsl", "applyxfm4D", "s16.nii", "in.nii", "a", "shift.mat", "-singlematrix"]),
    (
        "applytopup",
        [
            "fsl",
            "applytopup",
            "--imain=phase.nii",
            "--inindex=1",
            "--datain=acq.txt",
            "--topup=tu0",
            "--out=at",
            "--method=jac",
            "--datatype=short",
        ],
    ),
]


def line(tool: str) -> bytes:
    """FSL-11's line for a run of ``tool`` that wrote its images as ``NIFTI_GZ``: what FSL's
    program does instead (``standard_space_roi``'s script goes on and exits 0 without an image;
    FSL's other programs stop at their first image, exit 1)."""
    if tool == "standard_space_roi":
        fsl = (
            "FSL's standard_space_roi writes no image (its steps stop with an error at their "
            "first image) and exits 0"
        )
    else:
        fsl = f"FSL's {tool} stops with an error at its first image (exit 1)"
    return (
        "larmorx: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images "
        f"are written as NIFTI_GZ (.nii.gz); {fsl} [LARMORX_UPSTREAM_DEFECTS=reproduce gives "
        "FSL's result]"
    ).encode()


def image(path: Path, shape: tuple[int, ...], f=lambda v: v, dtype=np.float32) -> None:
    """An image with a bright block in its middle (plain arithmetic)."""
    i = np.arange(int(np.prod(shape)))
    v = ((i * 7919) % 97).astype(np.float32).reshape(shape, order="F")
    inner = tuple(slice(s // 4, s - s // 4) for s in shape[:3])
    v[inner] += 1000.0
    if len(shape) > 3:
        v += 3.0 * np.arange(shape[3], dtype=np.float32)
    lx.save((np.asfortranarray(f(v).astype(dtype)), np.eye(4)), path)


def environment(**changes: str | None) -> dict[str, str]:
    """FSL's clean environment (as the FSL suites': no FSL on the ``PATH``, ``FSLDIR``, OpenBLAS's
    Haswell core, one thread) without ``FSLOUTPUTTYPE``, this process's larmorx on
    ``PYTHONPATH``, with ``changes``."""
    env = {
        "HOME": os.environ.get("HOME", str(Path.home())),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(FSLDIR),
        "OMP_NUM_THREADS": "1",
        "PYTHONPATH": os.pathsep.join(x for x in sys.path if x),
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
    made by the clean-room original with ``FSLOUTPUTTYPE=NIFTI_GZ``."""
    d = tmp_path_factory.mktemp("fsl11-original-inputs")
    image(d / "in.nii", (12, 12, 10))
    image(d / "series.nii", (12, 12, 10, 3))
    image(d / "pair.nii", (12, 12, 10, 2))
    image(d / "phase.nii", (12, 12, 10), lambda v: v / 1100.0 * 3.0 - 1.5)
    image(d / "s16.nii", (12, 12, 10, 2), dtype=np.int16)
    (d / "eye.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", "utf-8")
    (d / "shift.mat").write_text("1 0 0 0.5\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", "utf-8")
    (d / "acq.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n", "utf-8")
    for args in (["fsl", "topup", *TOPUP, "--out=tu0"], ["fsl", "bet2", "in.nii", "bm", "-e", "-n"]):  # fmt: skip
        p = subprocess.run([*CLEAN_ROOM, *args], cwd=d, env=environment(FSLOUTPUTTYPE="NIFTI_GZ"),
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


def original(d: Path, inputs: Path, args: list[str], *, reproduce: bool = False, **env):
    """``larmorx --implementation clean-room [--upstream-defects reproduce] <args>`` in ``d``:
    its exit status, streams and files."""
    mode = ["--upstream-defects", "reproduce"] if reproduce else []
    p = subprocess.run([*CLEAN_ROOM, *mode, *args], cwd=d, env=environment(**env),
                       capture_output=True, check=False)  # fmt: skip
    return p.returncode, p.stdout, p.stderr, written(d, inputs)


def fsl(d: Path, inputs: Path, args: list[str]):
    """FSL's own program (``args`` after the family; `mri hmc` is mcflirt), ``FSLOUTPUTTYPE``
    unset, through the oracle-output cache."""
    from larmorx_validation import oracle_cache

    program = "mcflirt" if args[:2] == ["mri", "hmc"] else args[1]
    p = oracle_cache.run(
        [str(FSLDIR / "bin" / program), *args[2:]],
        env=environment(PYTHONPATH=None),
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="fsl-output-types-original",
        inputs=[q for q in sorted(d.iterdir()) if q.is_file()],
        timeout=600,
    )
    return p.returncode, p.stdout_bytes, p.stderr_bytes, written(d, inputs)


def own(stream: bytes) -> bytes:
    """A stream as FSL's run would print it: larmorx's own lines out, FSL's program paths
    shortened to their names."""
    keep = [x for x in stream.splitlines(keepends=True) if not x.startswith(b"larmorx: ")]
    return b"".join(keep).replace(f"{FSLDIR}/bin/".encode(), b"")


def _id(x):
    return "-".join(x) if isinstance(x, list) else x


@pytest.mark.parametrize(("tool", "args"), IMAGE_CALLS, ids=_id)
def test_the_original_writes_nifti_gz_and_stops_where_fsl_does(tool, args, inputs, tmp_path):
    given = original(fresh(inputs, tmp_path / "given"), inputs, args, FSLOUTPUTTYPE="NIFTI_GZ")
    unset = original(fresh(inputs, tmp_path / "unset"), inputs, args)
    assert given[0] == 0, given[2].decode()[-800:]
    assert b"FSL-11" not in given[2]
    # the default: the variable's run, with one line naming FSL-11
    assert unset[0] == 0, unset[2].decode()[-800:]
    assert unset[1] == given[1]
    named = [x for x in unset[2].splitlines() if b"FSL-11" in x]
    assert named == [line(tool)], unset[2].decode()[-800:]
    assert [x for x in unset[2].splitlines() if b"FSL-11" not in x] == given[2].splitlines()
    assert sorted(unset[3]) == sorted(given[3])
    assert unset[3] == given[3], "the bytes differ"
    assert any(name.endswith(".nii.gz") for name in unset[3]), sorted(unset[3])
    # reproduced: where FSL's run stops
    r = original(fresh(inputs, tmp_path / "reproduce"), inputs, args, reproduce=True)
    f = fsl(fresh(inputs, tmp_path / "fsl"), inputs, args)
    assert UNSET in f[2], f[2].decode()[-800:]
    assert UNSET in r[2], r[2].decode()[-800:]
    assert b"FSL-11" not in r[2]
    assert r[0] == f[0], (r[2].decode()[-800:], f[2].decode()[-800:])
    assert sorted(r[3]) == sorted(f[3]), "the files FSL writes before it stops"
    assert not any(n.endswith((".nii.gz", ".nii")) for n in r[3]), sorted(r[3])
    assert own(r[1]) == own(f[1])


@pytest.mark.parametrize("args", NO_IMAGE_CALLS, ids=_id)
def test_calls_that_write_no_image_run_as_with_the_variable(args, inputs, tmp_path):
    f = fsl(fresh(inputs, tmp_path / "fsl"), inputs, args)
    assert f[0] == 0, f[2].decode()[-800:]
    for reproduce in (False, True):
        mode = "reproduce" if reproduce else "fix"
        given = original(fresh(inputs, tmp_path / f"given-{mode}"), inputs, args,
                         reproduce=reproduce, FSLOUTPUTTYPE="NIFTI_GZ")  # fmt: skip
        unset = original(
            fresh(inputs, tmp_path / f"unset-{mode}"), inputs, args, reproduce=reproduce
        )
        assert given[0] == 0, (mode, given[2].decode()[-800:])
        assert unset == given, (mode, unset[2].decode()[-800:])
        assert b"FSL-11" not in unset[2]
        assert sorted(unset[3]) == sorted(f[3]), mode


@pytest.mark.parametrize(("tool", "args"), FSL09_CALLS, ids=_id)
def test_fsl09_names_the_program_that_ran(tool, args, inputs, tmp_path):
    for reproduce in (False, True):
        mode = "reproduce" if reproduce else "fix"
        code, _, err, _ = original(fresh(inputs, tmp_path / mode), inputs, args,
                                   reproduce=reproduce, FSLOUTPUTTYPE="NIFTI_GZ")  # fmt: skip
        assert code == 0, (mode, err.decode()[-800:])
        lines = [x.decode() for x in err.splitlines() if b"FSL-09" in x]
        assert len(lines) == 1, (mode, err.decode()[-800:])
        if reproduce:
            verb = "reproduced"
            says = f"integer output is truncated and wraps around beyond its range, as {tool} does on x86-64"  # fmt: skip
            hint = "[LARMORX_UPSTREAM_DEFECTS=fix corrects it]"
        else:
            verb = "corrected"
            says = f"integer output is rounded to the nearest and clamped; {tool} truncates and wraps around"  # fmt: skip
            hint = f"[LARMORX_UPSTREAM_DEFECTS=reproduce gives {tool}'s result]"
        x = lines[0]
        assert x.startswith(f"larmorx: upstream defect FSL-09 ({tool}) {verb}: "), x
        assert says in x and x.endswith(hint) and "mcflirt" not in x, x
