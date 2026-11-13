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


def _make_matrix(path: Path) -> None:
    path.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")


def _make_coords(path: Path) -> None:
    path.write_text("10 20 30\n5 6 7\n", encoding="utf-8")


KINDS: dict[str, Kind] = {
    "head": Kind("a synthetic head of 40 × 48 × 40 voxels of 4 mm", _make_head),
    "head4d": Kind("a synthetic series of 4 such heads", _make_head4d),
    "matrix": Kind("the identity as a 4 × 4 text matrix", _make_matrix),
    "coords": Kind("two coordinates, one `x y z` per line", _make_coords),
}


def make(folder: Path, wanted: dict[str, str]) -> None:
    """Makes each input ``name=kind`` in ``folder``."""
    for name, kind in wanted.items():
        KINDS[kind].make(folder / name)
