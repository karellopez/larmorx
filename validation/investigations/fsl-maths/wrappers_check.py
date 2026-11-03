# SPDX-License-Identifier: Apache-2.0
"""lx.fsl's fslmaths and fslstats wrappers against the replica's own command line, on small
seeded images: each typed wrapper's result (and the generic functions') equals what the command
line it stands for writes or prints, bit for bit; tensor_decomp's and roc's files are read back;
a failure carries fslmaths' message without its usage text.

Run boxed, with the replica built (``LARMORX_NC_BIN``: ``crates-nc/target/release/larmorx-nc``):
``scripts/orchestration/boxed.sh 4 python -P validation/investigations/fsl-maths/wrappers_check.py``.
"""

import os
import subprocess
import sys
import tempfile
import warnings
from pathlib import Path

import numpy as np

import larmorx as lx

NC = os.environ["LARMORX_NC_BIN"]
rng = np.random.default_rng(5)
aff = np.diag([2.0, 2.0, 2.5, 1.0])
a = rng.integers(0, 100, size=(9, 8, 7)).astype(np.float32)
b = (rng.integers(0, 2, size=(9, 8, 7))).astype(np.uint8)
ts = rng.normal(100, 10, size=(9, 8, 7, 6)).astype(np.float32)
lab = rng.integers(0, 4, size=(9, 8, 7)).astype(np.int16)
tensor = rng.normal(0, 1, size=(9, 8, 7, 6)).astype(np.float32)
tensor[..., 0] += 3
tensor[..., 3] += 3
tensor[..., 5] += 3
tmp = Path(tempfile.mkdtemp(prefix="wrap check "))  # a blank in the path, as on purpose
files = {}
for name, arr in (("a", a), ("b", b), ("ts", ts), ("lab", lab), ("tensor", tensor)):
    p = tmp / f"{name}.nii"
    lx.save((arr, aff), p)
    files[name] = str(p)
raw_a = a
a, b, ts, lab, tensor = ((x, aff) for x in (a, b, ts, lab, tensor))
env = {k: v for k, v in os.environ.items() if not k.upper().startswith("FSL")}
env["FSLOUTPUTTYPE"] = "NIFTI"


def cli(tool, args, out=None):
    p = subprocess.run([NC, "fsl", tool, *args], capture_output=True, text=True, env=env, cwd=tmp)
    assert p.returncode == 0, (tool, args, p.stdout, p.stderr)
    if out is None:
        return p.stdout
    return np.asarray(lx.load(str(tmp / f"{out}.nii")).data)


bad = 0
n = 0


def same(label, got, want):
    global bad, n
    n += 1
    g = np.asarray(got.data if hasattr(got, "data") else got)
    w = np.asarray(want)
    ok = g.shape == w.shape and g.dtype == w.dtype and np.array_equal(g, w, equal_nan=True)
    if not ok:
        bad += 1
        print("DIFF", label, g.shape, w.shape, g.dtype, w.dtype)


F = files
maths = [
    ("fslmaths generic", lambda: lx.fsl.fslmaths(a, "-mul", b), [F["a"], "-mul", F["b"]]),
    (
        "arithmetic multiply",
        lambda: lx.fsl.arithmetic(F["a"], "multiply", F["b"]),
        [F["a"], "-mul", F["b"]],
    ),
    ("arithmetic div number", lambda: lx.fsl.arithmetic(a, "divide", 0.1), [F["a"], "-div", "0.1"]),
    ("apply_mask", lambda: lx.fsl.apply_mask(a, b), [F["a"], "-mas", F["b"]]),
    ("threshold", lambda: lx.fsl.threshold(a, 20, 80), [F["a"], "-thr", "20.0", "-uthr", "80.0"]),
    (
        "threshold robust",
        lambda: lx.fsl.threshold(a, 10, of="robust_range"),
        [F["a"], "-thrp", "10.0"],
    ),
    ("unary sqrt", lambda: lx.fsl.unary(a, "sqrt"), [F["a"], "-sqrt"]),
    ("binarise invert", lambda: lx.fsl.binarise(a, invert=True), [F["a"], "-binv"]),
    (
        "add_noise",
        lambda: lx.fsl.add_noise(a, "gaussian", seed=7),
        [F["a"], "-seed", "7", "-randn"],
    ),
    ("grid", lambda: lx.fsl.grid(a, 5, 3), [F["a"], "-grid", "5.0", "3"]),
    ("dog_edge", lambda: lx.fsl.dog_edge(a, 1.0, 1.6), [F["a"], "-dog_edge", "1.0", "1.6"]),
    (
        "normalise",
        lambda: lx.fsl.normalise_intensity(ts, 1000, per_volume=False),
        [F["ts"], "-ing", "1000.0"],
    ),
    (
        "spatial_filter",
        lambda: lx.fsl.spatial_filter(a, "dilD", kernel=("boxv3", 3, 3, 1)),
        [F["a"], "-kernel", "boxv3", "3", "3", "1", "-dilD"],
    ),
    ("spatial_filter default", lambda: lx.fsl.spatial_filter(a, "-fmedian"), [F["a"], "-fmedian"]),
    ("smooth", lambda: lx.fsl.smooth(a, 3), [F["a"], "-s", "3.0"]),
    ("subsample2", lambda: lx.fsl.subsample2(a, centred=False), [F["a"], "-subsamp2offc"]),
    ("collapse std", lambda: lx.fsl.collapse(ts, "std"), [F["ts"], "-Tstd"]),
    (
        "collapse Zperc",
        lambda: lx.fsl.collapse(ts, "perc", axis="Z", percentile=30),
        [F["ts"], "-Zperc", "30.0"],
    ),
    ("statistical ztop", lambda: lx.fsl.statistical(ts, "ztop"), [F["ts"], "-ztop"]),
    (
        "roi",
        lambda: lx.fsl.roi(ts, (1, 4), (0, -1), (2, 3), (1, 2)),
        [F["ts"], "-roi", "1", "4", "0", "-1", "2", "3", "1", "2"],
    ),
    ("bptf", lambda: lx.fsl.bandpass_temporal_filter(ts, 2, -1), [F["ts"], "-bptf", "2.0", "-1.0"]),
    ("tfce", lambda: lx.fsl.tfce(a, 2, 0.5, 26), [F["a"], "-tfce", "2.0", "0.5", "26"]),
    (
        "tfce_support",
        lambda: lx.fsl.tfce_support(a, (4, 4, 3), 10.0),
        [F["a"], "-tfceS", "2.0", "0.5", "6", "4", "4", "3", "10.0"],
    ),
    (
        "odt char",
        lambda: lx.fsl.fslmaths(a, "-div", 3, odt="char"),
        [F["a"], "-div", "3", "-odt", "char"],
    ),
    (
        "dt input",
        lambda: lx.fsl.fslmaths(lab, "-mul", 3, dt="input", odt="input"),
        ["-dt", "input", F["lab"], "-mul", "3", "-odt", "input"],
    ),
]
for label, fn, args in maths:
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        got = fn()
    if "-odt" in args:
        want = cli("fslmaths", [*args[:-2], "o", *args[-2:]], "o")
    else:
        want = cli("fslmaths", [*args, "o"], "o")
    same(label, got, want)
# tensor_decomp's files
t = lx.fsl.tensor_decomp(tensor)
cli("fslmaths", [F["tensor"], "-tensor_decomp", "td"])
for s in ("L1", "L2", "L3", "FA", "MD", "MO", "V1", "V2", "V3"):
    same(f"tensor {s}", getattr(t, s.lower()), np.asarray(lx.load(str(tmp / f"td_{s}.nii")).data))
same("tensor image", t.image, np.asarray(lx.load(str(tmp / "td.nii")).data))
# roc
truth = ((raw_a > 50).astype(np.float32), aff)
lx.save(truth, tmp / "truth.nii")
r = lx.fsl.roc(a, truth, 0.05)
printed = cli(
    "fslmaths", [F["a"], "-roc", "-0.05", str(tmp / "curve.txt"), str(tmp / "truth.nii"), "o"]
)
n += 1
if [float(v) for v in printed.split()[-3:]] != [r.aroc, *r.aroc_quartiles]:
    bad += 1
    print("DIFF roc", printed, r)
same("roc curve", r.curve, np.loadtxt(tmp / "curve.txt", ndmin=2))

# fslstats
stats = [
    ("mean nonzero", lambda: lx.fsl.image_mean(a, nonzero=True), [F["a"], "-M"]),
    ("std mask", lambda: lx.fsl.image_std(a, mask=b), [F["a"], "-k", F["b"], "-s"]),
    (
        "percentile",
        lambda: lx.fsl.image_percentile(a, 30, lower=10),
        [F["a"], "-l", "10.0", "-p", "30.0"],
    ),
    ("range robust", lambda: lx.fsl.image_range(a, robust=True), [F["a"], "-r"]),
    ("entropy", lambda: lx.fsl.image_entropy(a, nonzero=True, absolute=True), [F["a"], "-a", "-E"]),
    ("volume", lambda: lx.fsl.image_volume(a, upper=50), [F["a"], "-u", "50.0", "-v"]),
    (
        "cog",
        lambda: lx.fsl.centre_of_gravity(a, lower=10, voxels=True),
        [F["a"], "-l", "10.0", "-C"],
    ),
    ("extremum", lambda: lx.fsl.extremum_position(a, minimum=True), [F["a"], "-X"]),
    ("roi", lambda: lx.fsl.bounding_roi(b), [F["b"], "-w"]),
    (
        "hist",
        lambda: lx.fsl.image_histogram(a, 5, limits=(0, 100)),
        [F["a"], "-H", "5", "0.0", "100.0"],
    ),
    ("hist auto", lambda: lx.fsl.image_histogram(a, 4), [F["a"], "-h", "4"]),
    ("mean per volume", lambda: lx.fsl.image_mean(ts, per_volume=True), ["-t", F["ts"], "-m"]),
    ("mean labels", lambda: lx.fsl.image_mean(a, labels=lab), ["-K", F["lab"], F["a"], "-m"]),
    (
        "range vol x labels",
        lambda: lx.fsl.image_range(ts, per_volume=True, labels=lab),
        ["-t", "-K", F["lab"], F["ts"], "-R"],
    ),
    (
        "difference nan",
        lambda: lx.fsl.image_mean(a, difference=b, nan_as_zero=True),
        [F["a"], "-d", F["b"], "-n", "-m"],
    ),
]
for label, fn, args in stats:
    got = fn()
    text = cli("fslstats", args)
    n += 1
    gv = np.atleast_1d(np.asarray(got, dtype=np.float64)).ravel()
    wv = np.array([float(w) for w in text.split()])
    if gv.shape != wv.shape or not np.array_equal(gv, wv, equal_nan=True):
        bad += 1
        print("DIFF", label, got, text)
# generic fslstats returns the text
n += 1
if lx.fsl.fslstats(a, "-M", "-l", 10, "-M") != cli("fslstats", [F["a"], "-M", "-l", "10", "-M"]):
    bad += 1
    print("DIFF generic fslstats")
n += 1
j = lx.fsl.fslstats(ts, "-m", timeseries=True, json=True)
if j != cli("fslstats", ["-t", "-json", F["ts"], "-m"]):
    bad += 1
    print("DIFF json")
# a failure carries fslmaths' message from its standard output
n += 1
try:
    lx.fsl.fslmaths(a, "-tensor_decomp")
    bad += 1
    print("no error for -tensor_decomp of a 3D image")
except lx.ReplicaError as e:
    if "not a tensor" not in str(e) or "Usage" in str(e):
        bad += 1
        print("ERROR TEXT", e)
n += 1
try:
    lx.fsl.fslmaths(a, "-bogus")
    bad += 1
except lx.ReplicaError as e:
    if 'unknown option "-bogus"' not in str(e) or "Usage" in str(e):
        bad += 1
        print("ERROR TEXT", e)
print(f"{n} checks, {bad} differ")
sys.exit(1 if bad else 0)
