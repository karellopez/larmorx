# SPDX-License-Identifier: Apache-2.0
"""Shrinking a difference: the smallest input and fewest options that still show it.

Greedy and deterministic: from the case that differs, try each move below in order, keep the
first that still shows the difference (:func:`~larmorx_validation.difftest.core.same_difference`:
it differs, and one of the original's failed checks still fails), and start again from it, until
no move keeps the difference or the time budget is spent. The moves, the tool's own first:

1. drop an optional argument group (and the inputs only it named);
2. for every input (and the inputs on the same grid with it): halve the longest axis, keep fewer
   volumes, take the first half of an axis;
3. simplify: a plumb orientation, unit spacing, the origin at 0, an sform equal to the qform,
   qform and sform codes 1, millimetres; float32 storage, no scale factor, plain values (no
   NaN, infinities or outliers, no tiny or huge range), no slice timing; a simpler content
   (a constant, a ramp, polynomial blobs).

Every candidate is a complete recipe; what was dropped or changed is listed in the finding.
"""

from __future__ import annotations

import time
from collections.abc import Callable, Iterator

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Judgement, deep, same_difference

Move = tuple[str, dict]


def _referenced(recipe: dict) -> set[str]:
    words = " ".join(a for g in recipe["groups"] for a in g["args"])
    refs = {n for n in recipe["inputs"] if "{in:" + n + "}" in words}
    # Inputs named by other inputs (a reference image's grid, a series' companions) are kept.
    for spec in recipe["inputs"].values():
        refs |= set(spec.get("needs", ()))
    return refs


def _drop_groups(recipe: dict) -> Iterator[Move]:
    for i in reversed(range(len(recipe["groups"]))):
        g = recipe["groups"][i]
        if not g.get("optional"):
            continue
        r = deep(recipe)
        del r["groups"][i]
        keep = _referenced(r)
        r["inputs"] = {
            k: v
            for k, v in r["inputs"].items()
            if k in keep or k in recipe.get("extra", {}).get("keep_inputs", ())
        }
        yield f"dropped {g['name']} ({' '.join(g['args'])})", r


def _grid_key(spec: dict) -> str:
    return gen.digest([spec.get("shape"), spec.get("geometry")])


def _on_grid(recipe: dict, name: str) -> list[str]:
    """``name`` and the inputs that share its grid (so a move keeps them on one grid)."""
    key = _grid_key(recipe["inputs"][name])
    return [
        n
        for n, s in recipe["inputs"].items()
        if s.get("kind", "nifti") == "nifti" and _grid_key(s) == key
    ]


def _shape_moves(recipe: dict, name: str) -> Iterator[Move]:
    spec = recipe["inputs"][name]
    shape = list(spec["shape"])
    spatial = min(3, len(shape))
    minimum = recipe.get("extra", {}).get("min_size", 1)
    order = sorted(range(spatial), key=lambda k: -shape[k])
    for k in order:
        if shape[k] > max(1, minimum):
            new = list(shape)
            new[k] = max(minimum, shape[k] // 2)
            yield f"{name}: axis {k} {shape[k]} -> {new[k]} voxels", _with_shape(recipe, name, new)
    if len(shape) == 4 and shape[3] > recipe.get("extra", {}).get("min_volumes", 2):
        new = list(shape)
        new[3] = max(recipe.get("extra", {}).get("min_volumes", 2), shape[3] // 2)
        yield f"{name}: {shape[3]} -> {new[3]} volumes", _with_shape(recipe, name, new)


def _with_shape(recipe: dict, name: str, shape: list[int]) -> dict:
    r = deep(recipe)
    for n in _on_grid(recipe, name):
        s = r["inputs"][n]
        old = s["shape"]
        s["shape"] = shape[: len(old)] if len(old) <= len(shape) else [*shape, *old[len(shape) :]]
        if len(old) == 4 and len(shape) == 3:
            s["shape"] = [*shape, old[3]]
        if s.get("timing", {}).get("slice_end") is not None and len(s["shape"]) > 2:
            s["timing"]["slice_end"] = s["shape"][2] - 1
    return r


def _simplify_moves(recipe: dict, name: str) -> Iterator[Move]:
    spec = recipe["inputs"][name]
    g = spec.get("geometry", {})
    geometry_moves = [
        ("plumb orientation", {"quaternion": [1, 0, 0, 0], "qfac": 1}),
        ("unit spacing", {"spacing": [1.0, 1.0, 1.0]}),
        ("origin at 0", {"origin": [0.0, 0.0, 0.0]}),
        ("sform = qform", {"sform": "same"}),
        ("qform and sform codes 1", {"qform_code": 1, "sform_code": 1}),
        ("millimetres", {"units": "mm"}),
    ]
    for label, change in geometry_moves:
        if all(g.get(k) == v for k, v in change.items()):
            continue
        r = deep(recipe)
        for n in _on_grid(recipe, name):
            r["inputs"][n]["geometry"].update(change)
            if change.get("sform") == "same":
                r["inputs"][n]["geometry"].pop("shear", None)
        yield f"{name} (and its grid): {label}", r
    if spec.get("dtype", "float32") != "float32" and not spec.get("keep_dtype"):
        r = deep(recipe)
        r["inputs"][name]["dtype"] = "float32"
        r["inputs"][name]["scaling"] = None
        yield f"{name}: float32 storage", r
    if spec.get("scaling"):
        r = deep(recipe)
        r["inputs"][name]["scaling"] = None
        yield f"{name}: no scale factor", r
    values = spec.get("values", {})
    if values.get("specials"):
        r = deep(recipe)
        r["inputs"][name]["values"].pop("specials")
        yield f"{name}: no special values", r
    if (
        any(values.get(k) for k in ("quantize",))
        or values.get("scale", 1.0) != 1.0
        or values.get("offset", 0.0) != 0.0
    ) and not values.get("binarize"):
        r = deep(recipe)
        r["inputs"][name]["values"] = {
            "scale": 1.0,
            "offset": 0.0,
            **({"specials": values["specials"]} if values.get("specials") else {}),
        }
        yield f"{name}: plain value range", r
    timing = spec.get("timing")
    if timing and timing.get("slice_code"):
        r = deep(recipe)
        r["inputs"][name]["timing"]["slice_code"] = 0
        yield f"{name}: no slice timing", r
    content = spec.get("content", {})
    if not spec.get("keep_content"):
        for simpler in ("constant", "ramp", "blobs"):
            if content.get("type") in (simpler, "zeros"):
                break
            r = deep(recipe)
            c = {"type": simpler, "seed": content.get("seed", 1)}
            if simpler == "ramp":
                c["direction"] = [1, 0, 0]
            if simpler == "blobs":
                c.update(centre=[0.0, 0.0, 0.0], size=12.0, axes=[1.0, 1.0, 1.0], n=1)
            r["inputs"][name]["content"] = c
            yield f"{name}: content {content.get('type')} -> {simpler}", r


def moves(recipe: dict, extra: Callable[[dict], Iterator[dict]] | None = None) -> Iterator[Move]:
    if extra is not None:
        for i, r in enumerate(extra(recipe)):
            yield f"tool move {i}", r
    yield from _drop_groups(recipe)
    for name in list(recipe["inputs"]):
        if recipe["inputs"][name].get("kind", "nifti") == "nifti":
            yield from _shape_moves(recipe, name)
    for name in list(recipe["inputs"]):
        if recipe["inputs"][name].get("kind", "nifti") == "nifti":
            yield from _simplify_moves(recipe, name)


def shrink(
    recipe: dict,
    signature: tuple[str, ...],
    run: Callable[[dict], Judgement],
    *,
    budget_s: float = 600.0,
    max_runs: int = 120,
    extra: Callable[[dict], Iterator[dict]] | None = None,
) -> tuple[dict, list[str], Judgement | None, int]:
    """The smallest recipe found that still shows ``signature``: (recipe, the moves kept, its
    judgement, the number of candidate runs)."""
    from larmorx_validation.difftest.core import recipe_id

    deadline = time.monotonic() + budget_s
    current, kept, last = recipe, [], None
    seen = {recipe_id(recipe)}
    runs = 0
    progress = True
    while progress and time.monotonic() < deadline and runs < max_runs:
        progress = False
        for label, cand in moves(current, extra):
            if time.monotonic() >= deadline or runs >= max_runs:
                break
            rid = recipe_id(cand)
            if rid in seen:
                continue
            seen.add(rid)
            runs += 1
            j = run(cand)
            if same_difference(signature, j):
                current, last = cand, j
                kept.append(label)
                progress = True
                break
    return current, kept, last, runs


__all__ = ["moves", "shrink"]
