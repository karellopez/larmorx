# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.fslmaths``, ``lx.fsl.fslstats`` and their typed wrappers: with a stand-in for the
``larmorx-nc`` program (written by the test, as in ``test_fsl_topup.py``) the generic functions
pass their operations as the command line has them (in-memory images written in their own data
type, numbers as their shortest text), in the caller's working directory, with FSL's settings and
D13's choice of this call; the typed wrappers build the command lines they stand for; fslstats'
printed numbers are read per volume and per label; a failure carries fslmaths' message without
its usage text; without the program fslmaths runs its clean-room original in this process, and
fslstats refuses clearly (no clean-room original yet). The
replicas themselves are checked by their golden tests (``crates-nc/larmorx-nc-cli/tests/
golden.rs``) and by the parity suites against FSL."""

from __future__ import annotations

import json
import os
import sys
import warnings
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
record = {"args": args, "defects": os.environ.get("LARMORX_UPSTREAM_DEFECTS"), "cwd": os.getcwd(),
          "fsl": {k: v for k, v in os.environ.items() if k.upper().startswith("FSL")},
          "types": {os.path.basename(a): int.from_bytes(Path(a).read_bytes()[70:72], "little")
                    for a in args if a.endswith(".nii") and os.path.isfile(a)}}
with open(here / "calls.jsonl", "a", encoding="utf-8") as f:
    f.write(json.dumps(record) + "\\n")
if args == ["--version"]:
    print("larmorx-nc 9.9.9")
    sys.exit(0)
sys.stderr.write((here / "stderr.txt").read_text(encoding="utf-8"))
sys.stdout.write((here / "stdout.txt").read_text(encoding="utf-8"))
code = int((here / "status.txt").read_text(encoding="utf-8"))
if code:
    sys.exit(code)
if args[1] == "fslmaths":
    words = args[2:]
    if words[-2] == "-odt":
        words = words[:-2]
    first = words[2] if words[0] == "-dt" else words[0]
    out = words[-1]
    shutil.copyfile(first, out + ".nii")
    if "-tensor_decomp" in words:
        for s in ("L1", "L2", "L3", "FA", "MD", "MO", "V1", "V2", "V3"):
            shutil.copyfile(first, out + "_" + s + ".nii")
    if "-roc" in words:
        k = words.index("-roc")
        Path(words[k + 2]).write_text("0 0 5\\n0.5 1 2\\n", encoding="utf-8")
"""


def stub(directory: Path, *, stdout: str = "", stderr: str = "", status: int = 0) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    script = directory / "stub.py"
    script.write_text(STUB, encoding="utf-8")
    (directory / "stdout.txt").write_text(stdout, encoding="utf-8")
    (directory / "stderr.txt").write_text(stderr, encoding="utf-8")
    (directory / "status.txt").write_text(str(status), encoding="utf-8")
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
    path = program.parent / "calls.jsonl"
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in lines if json.loads(line)["args"] != ["--version"]]


def last_args(program: Path) -> list[str]:
    """The last call's arguments after ``fsl <tool>``, the generated file names as their base
    names (``lx-in``, ``lx-in-0``, ``lx-out``, ...)."""
    out = []
    for a in calls(program)[-1]["args"][2:]:
        base = os.path.basename(a)
        out.append(base.removesuffix(".nii") if base.startswith("lx-") else a)
    return out


@pytest.fixture
def scripts(monkeypatch, no_replica, tmp_path) -> Path:
    directory = tmp_path / "scripts"
    directory.mkdir()
    monkeypatch.setattr(_replica, "search_dirs", lambda: [str(directory)])
    return directory


AFFINE = np.diag([2.0, 2.0, 2.5, 1.0])


def image(dtype=np.float32, shape=(5, 4, 3)) -> tuple[np.ndarray, np.ndarray]:
    data = np.arange(int(np.prod(shape))).reshape(shape).astype(dtype)
    return data, AFFINE


def test_without_the_replica(scripts, tmp_path):
    # fslmaths' clean-room original runs ("auto" without larmorx-nc, and "original")
    data, affine = image()
    for impl in ("auto", "original"):
        r = lx.fsl.fslmaths((data - 30, affine), "-abs", "-thr", 5, implementation=impl)
        want = np.abs(data - 30)
        assert np.array_equal(np.asarray(r.data), np.where(want >= 5, want, 0))
        assert np.asarray(r.data).dtype == np.float32
    mask = ((data > 20).astype(np.uint8), affine)
    m = lx.fsl.fslmaths((data, affine), "-mul", mask, n_threads=3)
    assert np.array_equal(np.asarray(m.data), data * (data > 20))
    with pytest.raises(RuntimeError, match="unknown option"):
        lx.fsl.fslmaths(image(), "-nosuchop", implementation="original")
    with pytest.raises(lx.ReplicaNotFoundError, match="no clean-room original of fsl fslstats"):
        lx.fsl.image_mean(image())


def test_the_generic_fslmaths(scripts, monkeypatch, tmp_path):
    program = stub(scripts)
    monkeypatch.setenv("FSLOUTPUTTYPE", "NIFTI_PAIR")  # must not reach the replica
    monkeypatch.setenv("FSL_LOAD_NIFTI_EXTENSIONS", "1")
    work = tmp_path / "with space"
    work.mkdir()
    monkeypatch.chdir(work)
    mask = image(np.uint8)
    lx.save(mask, work / "mask.nii")
    r = lx.fsl.fslmaths(image(np.int16), "-mul", mask, "-add", 2.5, "-mas", "mask.nii", "-thr", 3)
    (call,) = calls(program)
    assert call["args"][:2] == ["fsl", "fslmaths"]
    assert last_args(program) == [
        "lx-in",
        "-mul",
        "lx-in-1",
        "-add",
        "2.5",
        "-mas",
        "mask.nii",
        "-thr",
        "3",
        "lx-out",
    ]
    # relative names are the caller's, as in a shell; the generated files are absolute
    assert Path(call["cwd"]).resolve() == work.resolve()
    assert os.path.isabs(call["args"][2]) and os.path.isabs(call["args"][-1])
    assert call["fsl"] == {"FSLOUTPUTTYPE": "NIFTI"} and call["defects"] == "fix"
    # the in-memory images in their own data types (NIfTI's codes: int16 4, uint8 2)
    assert call["types"] == {"lx-in.nii": 4, "lx-in-1.nii": 2, "mask.nii": 2}
    np.testing.assert_array_equal(r.data, image(np.int16)[0])
    assert r.implementation.kind == "replica" and r.implementation.version == "9.9.9"
    lx.fsl.fslmaths(image(), "-sqrt", dt="double", odt="char", upstream_defects="reproduce")
    assert last_args(program) == ["-dt", "double", "lx-in", "-sqrt", "lx-out", "-odt", "char"]
    assert calls(program)[-1]["defects"] == "reproduce"
    # booleans are uint8, other types float32 (16); int8 and uint16 as they are (256, 512)
    for dtype, code in ((np.bool_, 2), (np.float16, 16), (np.int8, 256), (np.uint16, 512)):
        lx.fsl.fslmaths(image(dtype), "-bin")
        assert calls(program)[-1]["types"] == {"lx-in.nii": code}
    # paths as given
    lx.fsl.fslmaths(work / "mask.nii", "-bin")
    assert calls(program)[-1]["args"][2] == str(work / "mask.nii")


TYPED = [
    (lambda i: lx.fsl.arithmetic(i, "multiply", i), ["-mul", "lx-in-1"]),
    (lambda i: lx.fsl.arithmetic(i, "-div", 0.1), ["-div", "0.1"]),
    (lambda i: lx.fsl.arithmetic(i, "remainder", 3), ["-rem", "3"]),
    (lambda i: lx.fsl.apply_mask(i, i), ["-mas", "lx-in-1"]),
    (lambda i: lx.fsl.threshold(i, 2, 7), ["-thr", "2.0", "-uthr", "7.0"]),
    (lambda i: lx.fsl.threshold(i, upper=90, of="robust_range"), ["-uthrp", "90.0"]),
    (lambda i: lx.fsl.threshold(i, 5, of="positive_robust_range"), ["-thrP", "5.0"]),
    (lambda i: lx.fsl.unary(i, "-nanm"), ["-nanm"]),
    (lambda i: lx.fsl.unary(i, "fillh26"), ["-fillh26"]),
    (lambda i: lx.fsl.binarise(i), ["-bin"]),
    (lambda i: lx.fsl.binarise(i, invert=True), ["-binv"]),
    (lambda i: lx.fsl.add_noise(i), ["-rand"]),
    (lambda i: lx.fsl.add_noise(i, "gaussian", seed=4), ["-seed", "4", "-randn"]),
    (lambda i: lx.fsl.grid(i, 9, 2), ["-grid", "9.0", "2"]),
    (lambda i: lx.fsl.dog_edge(i, 1, 1.6), ["-dog_edge", "1.0", "1.6"]),
    (lambda i: lx.fsl.normalise_intensity(i, 1000), ["-inm", "1000.0"]),
    (lambda i: lx.fsl.normalise_intensity(i, 10, per_volume=False), ["-ing", "10.0"]),
    (lambda i: lx.fsl.spatial_filter(i, "dilD"), ["-dilD"]),
    (lambda i: lx.fsl.spatial_filter(i, "ero", kernel="2D"), ["-kernel", "2D", "-ero"]),
    (
        lambda i: lx.fsl.spatial_filter(i, "-fmean", kernel=("gauss", 2)),
        ["-kernel", "gauss", "2.0", "-fmean"],
    ),
    (
        lambda i: lx.fsl.spatial_filter(i, "dilM", kernel=("boxv3", 3, 3, 1)),
        ["-kernel", "boxv3", "3", "3", "1", "-dilM"],
    ),
    (
        lambda i: lx.fsl.spatial_filter(i, "dilF", kernel=("file", i)),
        ["-kernel", "file", "lx-in-2", "-dilF"],
    ),
    (lambda i: lx.fsl.smooth(i, 5), ["-s", "5.0"]),
    (lambda i: lx.fsl.subsample2(i), ["-subsamp2"]),
    (lambda i: lx.fsl.subsample2(i, centred=False), ["-subsamp2offc"]),
    (lambda i: lx.fsl.collapse(i, "std"), ["-Tstd"]),
    (lambda i: lx.fsl.collapse(i, "perc", axis="X", percentile=25), ["-Xperc", "25.0"]),
    (lambda i: lx.fsl.statistical(i, "ranknorm"), ["-ranknorm"]),
    (
        lambda i: lx.fsl.roi(i, (1, 2), z=(0, 1)),
        ["-roi", "1", "2", "0", "-1", "0", "1", "0", "-1"],
    ),
    (lambda i: lx.fsl.bandpass_temporal_filter(i, 25, -1), ["-bptf", "25.0", "-1.0"]),
    (lambda i: lx.fsl.tfce(i, connectivity=26), ["-tfce", "2.0", "0.5", "26"]),
    (
        lambda i: lx.fsl.tfce(i, min_t=1, delta=0.5),
        ["-tfce_minT", "1.0", "-tfce_delta", "0.5", "-tfce", "2.0", "0.5", "6"],
    ),
    (
        lambda i: lx.fsl.tfce_support(i, (1, 2, 0), 3.5),
        ["-tfceS", "2.0", "0.5", "6", "1", "2", "0", "3.5"],
    ),
]


@pytest.mark.parametrize(("call", "expected"), TYPED, ids=[str(k) for k in range(len(TYPED))])
def test_typed_fslmaths_wrappers(scripts, call, expected):
    """Each typed wrapper's command line (the generated images named ``lx-in-<k>`` by their
    place among the arguments)."""
    program = stub(scripts)
    r = call(image())
    assert last_args(program) == ["lx-in", *expected, "lx-out"]
    assert r.data.shape == (5, 4, 3)


def test_tensor_decomp_and_roc_read_their_files(scripts, tmp_path):
    program = stub(scripts, stdout="0.25 0.125 0.5\n")
    t = lx.fsl.tensor_decomp(image(shape=(3, 2, 2, 6)), odt="float")
    assert last_args(program) == ["lx-in", "-tensor_decomp", "lx-out", "-odt", "float"]
    for name in ("l1", "l2", "l3", "fa", "md", "mo", "v1", "v2", "v3", "image"):
        assert getattr(t, name).data.shape == (3, 2, 2, 6)
    r = lx.fsl.roc(image(), image())
    assert last_args(program) == ["lx-in", "-roc", "-0.05", "lx-roc.txt", "lx-in-3", "lx-out"]
    assert r.aroc == 0.25 and r.aroc_quartiles == (0.125, 0.5)
    np.testing.assert_array_equal(r.curve, [[0, 0, 5], [0.5, 1, 2]])
    lx.fsl.roc(image(), image(), 0.1, noise=image())
    assert last_args(program)[1:5] == ["-roc", "0.1", "lx-roc.txt", "lx-in-3"]


def test_failures_and_notices(scripts):
    usage = "\nUsage: fslmaths [-dt <datatype>] <first_input> [operations and inputs] <output>\n"
    stub(scripts, stdout='\n Error in command line: unknown option "-bogus"\n\n' + usage, status=1)
    with pytest.raises(lx.ReplicaError) as e:
        lx.fsl.fslmaths(image(), "-bogus")
    assert 'unknown option "-bogus"' in str(e.value) and "Usage" not in str(e.value)
    assert e.value.returncode == 1 and "Usage" in e.value.stdout
    notice = (
        "larmorx-nc: upstream defect FSL-160 (fslmaths) corrected: a Gaussian kernel of zero "
        "width\n"
    )
    stub(scripts, stderr=notice)
    with pytest.warns(lx.UpstreamDefectWarning, match="FSL-160"):
        lx.fsl.smooth(image(), 0)
    stub(scripts, stdout="Warning: tfce has detected a large number of integral steps.\n")
    with pytest.warns(UserWarning, match="integral steps"):
        lx.fsl.tfce(image())


def test_arguments_are_checked_before_anything_runs(scripts):
    program = stub(scripts)
    i = image()
    for call, match in (
        (lambda: lx.fsl.fslmaths(i, "-abs", dt="uint8"), "dt"),
        (lambda: lx.fsl.fslmaths(i, "-abs", odt="half"), "odt"),
        (lambda: lx.fsl.arithmetic(i, "power", 2), "unknown operation"),
        (lambda: lx.fsl.threshold(i), "lower, upper"),
        (lambda: lx.fsl.threshold(i, float("nan")), "finite"),
        (lambda: lx.fsl.threshold(i, 1, of="percent"), "of"),
        (lambda: lx.fsl.unary(i, "sinh"), "operation"),
        (lambda: lx.fsl.add_noise(i, "poisson"), "distribution"),
        (lambda: lx.fsl.add_noise(i, seed=1.5), "whole"),
        (lambda: lx.fsl.spatial_filter(i, "s"), "operation"),
        (lambda: lx.fsl.spatial_filter(i, "dilM", kernel=("gauss",)), "argument"),
        (lambda: lx.fsl.spatial_filter(i, "dilM", kernel="cross"), "kernel"),
        (lambda: lx.fsl.collapse(i, "sum"), "statistic"),
        (lambda: lx.fsl.collapse(i, "mean", axis="W"), "axis"),
        (lambda: lx.fsl.collapse(i, "perc"), "percentile"),
        (lambda: lx.fsl.collapse(i, "mean", percentile=3), "percentile"),
        (lambda: lx.fsl.statistical(i, "zscore"), "operation"),
        (lambda: lx.fsl.roi(i, (1,)), "first voxel"),
        (lambda: lx.fsl.tfce_support(i, (1, 2), 3), "voxel"),
        (lambda: lx.fsl.roc(i, i, 0), "positive"),
        (lambda: lx.fsl.fslmaths(np.zeros((2, 2)), "-abs"), "Image"),
        (lambda: lx.fsl.image_percentile(i, 101), "between 0 and 100"),
        (lambda: lx.fsl.image_histogram(i, 0), "bins"),
        (lambda: lx.fsl.image_histogram(i, 4, limits=(3, 3)), "limits"),
        (lambda: lx.fsl.image_mean(i, mask=i, lower=1), "mask or thresholds"),
    ):
        with pytest.raises((ValueError, TypeError), match=match):
            call()
    assert calls(program) == []


def test_the_generic_fslstats(scripts, tmp_path):
    program = stub(scripts, stdout="12.500000 \n")
    text = lx.fsl.fslstats(image(), "-M", "-l", 10, "-M")
    assert text == "12.500000 \n"
    assert last_args(program) == ["lx-in", "-M", "-l", "10", "-M"]
    lx.fsl.fslstats(image(), "-m", timeseries=True, index_mask=image(np.int16), json=True)
    assert last_args(program) == ["-t", "-K", "lx-labels", "-json", "lx-in", "-m"]


STATS = [
    (lambda i: lx.fsl.image_mean(i, nonzero=True), ["-M"], "1.500000 \n", 1.5),
    (
        lambda i: lx.fsl.image_std(i, mask=i, difference=i, absolute=True, nan_as_zero=True),
        ["-d", "lx-in-1", "-a", "-n", "-k", "lx-in-5", "-s"],
        "2.000000 \n",
        2.0,
    ),
    (
        lambda i: lx.fsl.image_percentile(i, 50, nonzero=True, lower=1, upper=9),
        ["-l", "1.0", "-u", "9.0", "-P", "50.0"],
        "nan \n",
        float("nan"),
    ),
    (lambda i: lx.fsl.image_range(i, robust=True), ["-r"], "0.000000 7.000000 \n", [0, 7]),
    (lambda i: lx.fsl.image_entropy(i), ["-e"], "0.300000 \n", 0.3),
    (lambda i: lx.fsl.image_volume(i, nonzero=True), ["-V"], "4 40.000000 \n", [4, 40]),
    (
        lambda i: lx.fsl.centre_of_gravity(i, lower=0.001, voxels=True),
        ["-l", "0.001", "-C"],
        "1.000000 2.000000 3.000000 \n",
        [1, 2, 3],
    ),
    (lambda i: lx.fsl.extremum_position(i, minimum=True), ["-X"], "0 1 2 \n", [0, 1, 2]),
    (
        lambda i: lx.fsl.bounding_roi(i),
        ["-w"],
        "0 5 0 4 0 3 0 1 \n",
        [0, 5, 0, 4, 0, 3, 0, 1],
    ),
    (
        lambda i: lx.fsl.image_histogram(i, 3, limits=(0, 9)),
        ["-H", "3", "0.0", "9.0"],
        "1.000000 \n2.000000 \n3.000000 \n \n",
        [1, 2, 3],
    ),
]


@pytest.mark.parametrize(
    ("call", "expected", "printed", "value"), STATS, ids=[str(k) for k in range(len(STATS))]
)
def test_typed_fslstats_wrappers(scripts, call, expected, printed, value):
    program = stub(scripts, stdout=printed)
    got = call(image())
    assert last_args(program) == ["lx-in", *expected]
    np.testing.assert_array_equal(got, value)
    if np.ndim(value) == 0:
        assert isinstance(got, float)


def test_fslstats_per_volume_and_label(scripts):
    # -t: one line per volume; -K: one per label, a missing label's line instead of its numbers
    stub(scripts, stdout="1.000000 \n2.000000 \n")
    np.testing.assert_array_equal(lx.fsl.image_mean(image(), per_volume=True), [1.0, 2.0])
    program = stub(scripts, stdout="1.000000 2.000000 \nmissing label: 2 in mask\n5.0 6.0 \n")
    got = lx.fsl.image_range(image(), labels=image(np.int16))
    assert last_args(program) == ["-K", "lx-labels", "lx-in", "-R"]
    np.testing.assert_array_equal(got, [[1, 2], [np.nan, np.nan], [5, 6]])
    # both: volumes by labels (the volumes from the input's header)
    stub(scripts, stdout="1.0 \n2.0 \n3.0 \n4.0 \n5.0 \n6.0 \n")
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        got = lx.fsl.image_mean(image(shape=(5, 4, 3, 2)), per_volume=True, labels=image(np.int16))
    np.testing.assert_array_equal(got, [[1, 2, 3], [4, 5, 6]])
    # a result fslstats did not print as expected is an error, not a guess
    stub(scripts, stdout="1.0 2.0 \n")
    with pytest.raises(lx.ReplicaError, match="one was expected"):
        lx.fsl.image_mean(image())
    stub(scripts, stdout="Index mask must be 3D\n", status=3)
    with pytest.raises(lx.ReplicaError, match="Index mask must be 3D"):
        lx.fsl.image_mean(image(), labels=image(shape=(5, 4, 3, 2)))
