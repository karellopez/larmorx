# SPDX-License-Identifier: Apache-2.0
"""lx.ants.atropos and ``larmorx ants Atropos``: every initialization, likelihood model and
option family on small phantoms; thread invariance; the command line equals the function;
errors."""

from __future__ import annotations

import contextlib
import io
import warnings

import numpy as np
import pytest

import larmorx as lx
from larmorx.cli import run as cli_run

AFFINE = np.diag([1.5, 1.5, 2.0, 1.0])
SHAPE = (18, 16, 12)


def image(data, affine=AFFINE):
    return lx.Image(np.asfortranarray(np.asarray(data, dtype=np.float32)), affine)


def phantom(seed=3, shape=SHAPE):
    """Three tissues in nested boxes (labels 1 dark, 2 middle, 3 bright) with seeded noise, a
    second contrast, and a mask that leaves a border out: (t1, t2, mask, truth)."""
    rng = np.random.default_rng(seed)
    truth = np.zeros(shape, dtype=np.uint32)
    truth[2:-2, 2:-2, 2:-2] = 1
    truth[4:-4, 4:-4, 3:-3] = 2
    truth[6:-6, 6:-6, 4:-4] = 3
    means1 = np.array([0.0, 40.0, 100.0, 160.0])
    means2 = np.array([0.0, 150.0, 90.0, 30.0])
    t1 = means1[truth] + rng.normal(0, 8, shape)
    t2 = means2[truth] + rng.normal(0, 8, shape)
    mask = (truth > 0).astype(np.float32)
    return image(t1), image(t2), image(mask), truth


def run(argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = cli_run(["larmorx", "ants", "Atropos", *argv])
    return code, out.getvalue(), err.getvalue()


def quiet(**kwargs):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return lx.ants.atropos(**kwargs)


def same(a, b):
    return np.array_equal(np.asarray(a.data), np.asarray(b.data))


def test_kmeans_segments_the_phantom_and_posteriors_sum_to_one():
    t1, _, mask, truth = phantom()
    r = quiet(images=t1, mask=mask, use_random_seed=False)
    labels = np.asarray(r.labels.data)
    assert labels.dtype == np.uint32 and labels.shape == SHAPE
    assert np.all(labels[truth == 0] == 0)
    assert np.mean(labels[truth > 0] == truth[truth > 0]) > 0.97
    assert len(r.posteriors) == 3
    total = sum(np.asarray(p.data, dtype=np.float64) for p in r.posteriors)
    np.testing.assert_allclose(total[truth > 0], 1.0, atol=1e-5)
    assert np.all(total[truth == 0] == 0)
    np.testing.assert_array_equal(r.labels.affine, AFFINE)
    assert r.iterations and r.iterations[0].iteration == 0
    assert "Iteration 0 (of 5)" in r.log


def test_threads_do_not_change_the_result_in_either_mode():
    t1, t2, mask, _ = phantom()
    for mode in ("fix", "reproduce"):
        for extra in ({}, {"icm": False}, {"mrf_radius": (2, 1, 1)}):
            a = quiet(images=[t1, t2], mask=mask, n_threads=1, upstream_defects=mode, **extra)
            b = quiet(images=[t1, t2], mask=mask, n_threads=5, upstream_defects=mode, **extra)
            assert same(a.labels, b.labels)
            assert all(same(x, y) for x, y in zip(a.posteriors, b.posteriors, strict=True))


def test_the_command_line_equals_the_function(tmp_path):
    t1, _, mask, _ = phantom()
    lx.save(t1, tmp_path / "t1.nii.gz")
    lx.save(mask, tmp_path / "mask.nii.gz")
    seg = tmp_path / "seg.nii.gz"
    post = tmp_path / "post%02d.nii.gz"
    argv = ["-d", "3", "-a", str(tmp_path / "t1.nii.gz"), "-x", str(tmp_path / "mask.nii.gz")]
    argv += ["-i", "KMeans[3]", "-m", "[0.1,1x1x1]", "-c", "[3,0]", "-r", "0"]
    code, _, _ = run([*argv, "-o", f"[{seg},{post}]"])
    assert code == 0
    r = quiet(
        images=str(tmp_path / "t1.nii.gz"),
        mask=str(tmp_path / "mask.nii.gz"),
        mrf_smoothing=0.1,
        iterations=3,
        convergence_threshold=0,
        use_random_seed=False,
    )
    np.testing.assert_array_equal(lx.load(seg).data, r.labels.data)
    for i, p in enumerate(r.posteriors, 1):
        np.testing.assert_array_equal(lx.load(tmp_path / f"post{i:02d}.nii.gz").data, p.data)


@pytest.mark.parametrize(
    "kwargs",
    [
        {"initialization": "otsu"},
        {"initialization": "random"},
        {"initialization": "kmeans", "kmeans_centers": [30, 90, 170]},
        {"posterior_formulation": "plato", "annealing": (2.0, 0.9, 0.5)},
        {"use_mixture_model_proportions": False, "icm": False},
        {"outliers": lx.ants.BoxPlot()},
        {"outliers": lx.ants.GrubbsRosner(0.05, 0.1)},
        {"likelihood": lx.ants.HistogramParzenWindows(1.0, 16)},
        {"likelihood": lx.ants.ManifoldParzenWindows(1.0, 10)},
        {"likelihood": lx.ants.ManifoldParzenWindows(1.0, 10, 5, 1.0)},
        {"mrf_smoothing": 0.0},
        {"minimize_memory": True},
    ],
)
def test_initializations_models_and_options_segment_the_phantom(kwargs):
    t1, _, mask, truth = phantom()
    r = quiet(images=t1, mask=mask, use_random_seed=False, **kwargs)
    labels = np.asarray(r.labels.data)
    assert set(np.unique(labels[truth > 0])) <= {1, 2, 3}
    assert np.all(labels[truth == 0] == 0)
    # Each class is one tissue (in some order, for the random and Otsu initializations).
    agreement = max(
        np.mean(labels[truth > 0] == np.array(perm)[truth[truth > 0] - 1])
        for perm in ((1, 2, 3), (1, 3, 2), (2, 1, 3), (2, 3, 1), (3, 1, 2), (3, 2, 1))
    )
    assert agreement > 0.9


def test_prior_probability_images_from_memory_and_files(tmp_path):
    t1, _, mask, truth = phantom()
    rng = np.random.default_rng(9)
    priors = []
    for c in (1, 2, 3):
        p = np.where(truth == c, 0.8, 0.1) + rng.uniform(0, 0.05, SHAPE)
        priors.append(image(p))
    paths = []
    for i, p in enumerate(priors, 1):
        paths.append(tmp_path / f"prior{i:02d}.nii.gz")
        lx.save(p, paths[-1])
    kwargs = dict(images=t1, mask=mask, initialization="prior-probabilities", use_random_seed=False)
    for weight, formulation in ((0.0, "socrates"), (0.3, "aristotle"), (0.5, "sigmoid")):
        a = quiet(**kwargs, priors=priors, prior_weight=weight, posterior_formulation=formulation)
        b = quiet(**kwargs, priors=paths, prior_weight=weight, posterior_formulation=formulation)
        assert same(a.labels, b.labels)
        assert np.mean(np.asarray(a.labels.data)[truth > 0] == truth[truth > 0]) > 0.95
    r = quiet(
        **kwargs,
        priors=priors,
        prior_weight=0.5,
        prior_threshold=0.2,
        euclidean_distance=True,
        label_propagation={2: (1.0, 0.75)},
        distance_images=True,
        smoothed_images=True,
        adaptive_smoothing=0.5,
        bspline=(2, 2, 3),
    )
    assert len(r.distances) == 3 and len(r.smoothed) == 3 and r.likelihoods == ()


def test_prior_label_image_and_icm_codes():
    t1, _, mask, truth = phantom()
    r = quiet(
        images=t1,
        mask=mask,
        initialization="prior-label",
        prior_label=image(truth),
        prior_weight=0.5,
        label_propagation=(1.0, 0.5),
        icm_code_image=True,
        use_random_seed=False,
    )
    assert np.mean(np.asarray(r.labels.data)[truth > 0] == truth[truth > 0]) > 0.95
    codes = np.asarray(r.icm_codes.data)
    assert codes.max() >= 2 and np.all(codes[truth == 0] == 0)


def test_tensor_likelihoods_take_six_images():
    _, _, mask, truth = phantom()
    rng = np.random.default_rng(4)
    # Diffusion-like tensors (eigenvalues per tissue), upper triangles xx xy xz yy yz zz.
    evals = {0: (1.0, 1.0, 1.0), 1: (2.0, 1.0, 0.5), 2: (1.5, 1.5, 0.7), 3: (3.0, 0.6, 0.4)}
    comps = [np.zeros(SHAPE) for _ in range(6)]
    for label, (a, b, c) in evals.items():
        for q, v in enumerate((a, 0.0, 0.0, b, 0.0, c)):
            comps[q][truth == label] = v
    comps = [image(c + rng.normal(0, 0.05, SHAPE)) for c in comps]
    for model in (
        lx.ants.LogEuclideanGaussian(),
        lx.ants.JointShapeAndOrientationProbability(2.0, 16, 1.0, 8),
    ):
        r = quiet(images=comps, mask=mask, likelihood=model, use_random_seed=False, iterations=2)
        assert np.all(np.asarray(r.labels.data)[truth == 0] == 0)
    with pytest.raises(lx.ants.AtroposError, match="Incorrect number of intensity images"):
        quiet(images=comps[:3], mask=mask, likelihood=lx.ants.LogEuclideanGaussian())


def test_two_and_four_dimensions():
    t1, _, mask, truth = phantom()
    a2 = lx.Image(np.asarray(t1.data)[:, :, 6].copy(), AFFINE)
    m2 = lx.Image(np.asarray(mask.data)[:, :, 6].copy(), AFFINE)
    r = quiet(images=a2, mask=m2, use_random_seed=False)
    assert np.asarray(r.labels.data).shape == SHAPE[:2]
    assert (
        np.mean(np.asarray(r.labels.data)[truth[:, :, 6] > 0] == truth[:, :, 6][truth[:, :, 6] > 0])
        > 0.9
    )
    a4 = lx.Image(np.stack([np.asarray(t1.data)] * 2, axis=-1), AFFINE)
    m4 = lx.Image(np.stack([np.asarray(mask.data)] * 2, axis=-1), AFFINE)
    r = quiet(images=a4, mask=m4, use_random_seed=False, mrf_radius=(1, 1, 1, 0))
    assert np.asarray(r.labels.data).shape == (*SHAPE, 2)


def test_fix_and_reproduce_warn_about_the_defects_they_meet():
    t1, _, mask, truth = phantom()
    data = np.asarray(t1.data).copy()
    data[truth == 2] = np.where(
        np.arange(np.count_nonzero(truth == 2)) % 17 == 0, 900.0, data[truth == 2]
    )
    t1 = image(data)
    for mode, word in (("fix", "corrected"), ("reproduce", "reproduced")):
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            lx.ants.atropos(
                t1,
                mask,
                initialization="prior-label",
                prior_label=image(truth),
                outliers=lx.ants.BoxPlot(),
                upstream_defects=mode,
            )
        messages = [str(w.message) for w in caught]
        assert any(m.startswith("ANTS-94") and word in m for m in messages)
    with pytest.raises(ValueError):
        lx.ants.atropos(t1, mask, upstream_defects="sometimes")


def pv_phantom(n=20, up=4, seed=5):
    """Nested spheres (60, 130, 200 over 0) box-averaged from a grid ``up`` times finer, so
    the boundary voxels are mixtures of two tissues; noise sd 6; the mask: voxels less than
    half background."""
    c = (n - 1) / 2.0
    g = (np.indices((n * up,) * 3) + 0.5) / up - 0.5
    r = np.sqrt(((g - c) ** 2).sum(0))
    tissue = np.where(r < 4.2, 3, np.where(r < 6.9, 2, np.where(r < 9.0, 1, 0)))
    frac = np.stack([(tissue == k).reshape(n, up, n, up, n, up).mean((1, 3, 5)) for k in range(4)])
    t1 = np.tensordot(np.array([0.0, 60.0, 130.0, 200.0]), frac, 1)
    t1 = t1 + np.random.default_rng(seed).normal(scale=6.0, size=t1.shape)
    return image(t1, np.eye(4)), image(frac[0] < 0.5, np.eye(4))


def test_partial_volume_classes_with_the_mrf():
    """ANTS-106: in ANTs' MRF a partial-volume label is +1 against every neighbour, its own
    tissues included, so with an MRF the partial-volume classes hardly win a voxel and Atropos
    stops when one wins none. Corrected (the user's decision of 2026-10-22), a partial-volume
    label is a member of its own set: -1 with its tissues, -2 with itself, +1 with others."""
    t1, mask = pv_phantom()
    kw = {
        "images": t1,
        "mask": mask,
        "classes": 3,
        "partial_volume": [(1, 2), (2, 3)],
        "partial_volume_likelihoods": True,
        "convergence_threshold": 0.0,
        "use_random_seed": False,
    }
    for beta in (0.1, 0.3):
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            res = lx.ants.atropos(**kw, mrf_smoothing=beta)
        assert any(str(w.message).startswith("ANTS-106") for w in caught)
        labels = np.asarray(res.labels.data)
        # Every class wins voxels, the two partial-volume classes (4 = 1x2, 5 = 2x3) included,
        # at the boundaries between their tissues.
        assert all((labels == k).any() for k in range(1, 6))
        assert len(res.posteriors) == 3  # one posterior per tissue class, as in Atropos
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            with pytest.raises(lx.ants.AtroposError, match="non-zero label sets"):
                lx.ants.atropos(**kw, mrf_smoothing=beta, upstream_defects="reproduce")
        assert any(
            str(w.message).startswith("ANTS-106") and "reproduced" in str(w.message)
            for w in caught
        )
    # Without the MRF the correction does not apply.
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        res = lx.ants.atropos(**kw, mrf_smoothing=0.0)
    assert not any(str(w.message).startswith("ANTS-106") for w in caught)
    assert (np.asarray(res.labels.data) == 5).any()


def test_seeds():
    t1, _, mask, _ = phantom()
    a = quiet(images=t1, mask=mask, initialization="random", use_random_seed=True)
    b = quiet(images=t1, mask=mask, initialization="random", use_random_seed=True)
    c = quiet(images=t1, mask=mask, initialization="random", use_random_seed=False)
    # larmorx never seeds from the clock: the same call gives the same result.
    assert same(a.labels, b.labels)
    assert a.posteriors and c.posteriors


def test_errors(tmp_path):
    t1, _, mask, _ = phantom()
    missing = str(tmp_path / "nope.nii.gz")
    with pytest.raises(lx.ants.AtroposError, match="does not exist"):
        quiet(images=t1, mask=mask, initialization="prior-probabilities", priors=[missing] * 3)
    with pytest.raises(ValueError, match="one prior image per class"):
        quiet(images=t1, mask=mask, initialization="prior-probabilities", priors=[t1])
    with pytest.raises(ValueError, match="mrf_radius"):
        quiet(images=t1, mask=mask, mrf_radius=(1, 1))
    with pytest.raises(lx.ants.AtroposError):
        quiet(images=t1, mask=mask, kmeans_centers=[10.0, 20.0])
    code, stdout, _ = run([])
    assert code == 1 and "Atropos" in stdout
    code, stdout, _ = run(["--help"])
    assert code == 0 and "--initialization" in stdout
