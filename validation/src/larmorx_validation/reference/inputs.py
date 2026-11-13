# SPDX-License-Identifier: Apache-2.0
"""The inputs the tests make for a reference page's examples (``<!-- inputs: name=kind -->``).

Each kind is small, synthetic and made from seeded integers and plain arithmetic, so the
examples run in a second or two and need no data download.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import numpy as np


@dataclass(frozen=True)
class Kind:
    description: str
    make: Callable[[Path], None]


def _head(shape: tuple[int, ...] = (40, 48, 40), voxel: float = 4.0, seed: int = 0):
    """A head-like image: a bright ellipsoid (the brain) inside a dimmer shell (skull and scalp),
    with seeded noise; float32, 4 mm voxels, centred on the origin."""
    rng = np.random.default_rng(seed)
    c = (np.array(shape[:3]) - 1) / 2
    i, j, k = np.indices(shape[:3], dtype=np.float64)
    r2 = (
        ((i - c[0]) / (shape[0] * 0.30)) ** 2
        + ((j - c[1]) / (shape[1] * 0.34)) ** 2
        + ((k - c[2]) / (shape[2] * 0.30)) ** 2
    )
    data = np.zeros(shape[:3], np.float32)
    data[r2 < 1.0] = 600.0
    data[(r2 >= 1.15**2) & (r2 < 1.30**2)] = 300.0
    affine = np.diag([voxel, voxel, voxel, 1.0])
    affine[:3, 3] = -c * voxel
    if len(shape) == 4:
        vols = [
            data + rng.integers(0, 40, size=data.shape).astype(np.float32) for _ in range(shape[3])
        ]
        return np.stack(vols, axis=-1), affine
    return data + rng.integers(0, 40, size=data.shape).astype(np.float32), affine


def _save(path: Path, data, affine) -> None:
    import larmorx as lx

    lx.save(lx.Image(data, affine), str(path))


def _make_head(path: Path) -> None:
    _save(path, *_head())


def _make_head4d(path: Path) -> None:
    _save(path, *_head((40, 48, 40, 4)))


def _make_head_mask(path: Path) -> None:
    """The synthetic head's brain: 1 inside its bright ellipsoid, 0 elsewhere (uint8)."""
    data, affine = _head()
    _save(path, (data >= 600).astype(np.uint8), affine)


def _make_bold(path: Path) -> None:
    _save(path, *_head((40, 48, 40, 8)))


def _make_echo(path: Path, factor: float) -> None:
    """One echo of a multi-echo series: the 8-volume series times ``factor``, the decay since the
    first echo (0.6 per echo spacing: a T2* of about 29 ms for echoes 15 ms apart)."""
    data, affine = _head((40, 48, 40, 8))
    _save(path, (data * np.float32(factor)).astype(np.float32), affine)


def _make_bids_pepolar(path: Path) -> None:
    """A BIDS dataset (a folder) of one participant: two PEPOLAR images (``dir-AP`` along ``j-``,
    ``dir-PA`` along ``j``, 2 volumes of 10 × 12 × 10 voxels of 4 mm) meant for a BOLD run of 3
    volumes, with their sidecars; enough for sdcflows' search of estimators."""
    import json

    import larmorx as lx

    fmap, func = path / "sub-01" / "fmap", path / "sub-01" / "func"
    fmap.mkdir(parents=True)
    func.mkdir(parents=True)
    (path / "dataset_description.json").write_text(
        json.dumps({"Name": "example", "BIDSVersion": "1.9.0"}), encoding="utf-8"
    )
    affine = np.diag([4.0, 4.0, 4.0, 1.0])
    bold = "func/sub-01_task-rest_bold.nii.gz"
    for d, pe in (("AP", "j-"), ("PA", "j")):
        data = np.full((10, 12, 10, 2), 100.0, np.float32)
        lx.save(lx.Image(data, affine), str(fmap / f"sub-01_dir-{d}_epi.nii.gz"))
        meta = {"PhaseEncodingDirection": pe, "TotalReadoutTime": 0.05, "IntendedFor": bold}
        (fmap / f"sub-01_dir-{d}_epi.json").write_text(json.dumps(meta), encoding="utf-8")
    lx.save(
        lx.Image(np.full((10, 12, 10, 3), 50.0, np.float32), affine), str(path / "sub-01" / bold)
    )
    meta = {"PhaseEncodingDirection": "j", "TotalReadoutTime": 0.05, "RepetitionTime": 2.0}
    (func / "sub-01_task-rest_bold.json").write_text(json.dumps(meta), encoding="utf-8")


def _make_slice_timing(path: Path) -> None:
    """The times of 40 slices acquired interleaved (even slices first) within 2 s, 0.05 s apart,
    on one line separated by tabs, as fMRIPrep writes a ``-tpattern`` file."""
    order = list(range(0, 40, 2)) + list(range(1, 40, 2))
    times = [0.0] * 40
    for n, k in enumerate(order):
        times[k] = n * 5 / 100
    path.write_text("\t".join(str(t) for t in times) + "\n", encoding="utf-8")


def _make_matrix(path: Path) -> None:
    path.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")


def _make_coords(path: Path) -> None:
    path.write_text("10 20 30\n5 6 7\n", encoding="utf-8")


#: Eight points in millimetres, no four of them in one plane.
_POINTS = (
    (10, 20, 30),
    (-15, 25, 40),
    (30, -10, 20),
    (5, 5, -25),
    (-20, -30, 10),
    (25, 30, -5),
    (0, -20, -15),
    (-10, 10, 35),
)


def _make_points(path: Path, shift: tuple[int, int, int] = (0, 0, 0)) -> None:
    rows = [" ".join(str(v + s) for v, s in zip(p, shift, strict=True)) for p in _POINTS]
    path.write_text("\n".join(rows) + "\n", encoding="utf-8")


def _make_points_moved(path: Path) -> None:
    _make_points(path, (2, -3, 4))


def _make_phase(path: Path) -> None:
    """A phase difference in radians on the head's grid: a ramp of 0.5 rad per voxel along x,
    wrapped into [-pi, pi)."""
    _, affine = _head()
    ramp = np.indices((40, 48, 40), dtype=np.float64)[0] * 0.5
    wrapped = (ramp + np.pi) % (2 * np.pi) - np.pi
    _save(path, wrapped.astype(np.float32), affine)


def _make_acqp4(path: Path) -> None:
    path.write_text("0 1 0 0.05\n0 1 0 0.05\n0 -1 0 0.05\n0 -1 0 0.05\n", encoding="utf-8")


def _make_log(path: Path) -> None:
    path.write_text("start\nfirst value 1\nmiddle\nsecond value 2\nend\n", encoding="utf-8")


KINDS: dict[str, Kind] = {
    "head": Kind("a synthetic head of 40 × 48 × 40 voxels of 4 mm", _make_head),
    "head4d": Kind("a synthetic series of 4 such heads", _make_head4d),
    "head-mask": Kind(
        "the synthetic head's brain as a mask (1 inside, 0 outside)", _make_head_mask
    ),
    "bold": Kind("a synthetic series of 8 heads of 40 × 48 × 40 voxels of 4 mm", _make_bold),
    "echo-1": Kind(
        "the first echo of a synthetic multi-echo series: 8 heads of 40 × 48 × 40 voxels of 4 mm",
        lambda path: _make_echo(path, 1.0),
    ),
    "echo-2": Kind("the second echo: the first times 0.6", lambda path: _make_echo(path, 0.6)),
    "echo-3": Kind("the third echo: the first times 0.36", lambda path: _make_echo(path, 0.36)),
    "bids-pepolar": Kind(
        "a BIDS dataset of one participant with two PEPOLAR images and a BOLD run (a folder)",
        _make_bids_pepolar,
    ),
    "slice-timing": Kind(
        "the times of 40 slices acquired interleaved within 2 s, on one line, as fMRIPrep writes "
        "them",
        _make_slice_timing,
    ),
    "matrix": Kind("the identity as a 4 × 4 text matrix", _make_matrix),
    "coords": Kind("two coordinates, one `x y z` per line", _make_coords),
    "points": Kind("eight points in millimetres, one `x y z` per line", _make_points),
    "points-moved": Kind("the same eight points moved by 2, −3 and 4 mm", _make_points_moved),
    "phase": Kind("a phase ramp along x, wrapped into [−pi, pi), on the head's grid", _make_phase),
    "acqp4": Kind(
        "the phase encodings of four volumes, two along +y and two along −y, read out in 0.05 s",
        _make_acqp4,
    ),
    "log": Kind("a text of five lines, two of them with the word `value`", _make_log),
}


def make(folder: Path, wanted: dict[str, str]) -> None:
    """Makes each input ``name=kind`` in ``folder``."""
    for name, kind in wanted.items():
        KINDS[kind].make(folder / name)
