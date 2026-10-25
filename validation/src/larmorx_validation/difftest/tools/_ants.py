# SPDX-License-Identifier: Apache-2.0
"""What the descriptors of ANTs' image programs share: recipes become ``AntsCase``s of the
parity suites (``larmorx_validation.parity.ants_programs``), whose runners and comparators
decide them, and the inputs the programs read."""

from __future__ import annotations

from collections.abc import Sequence

from larmorx_validation.difftest import gen, inputs
from larmorx_validation.difftest.core import arguments, substitute
from larmorx_validation.parity.harness import Case


def prefix_of(recipe: dict, case_prefix: str | None) -> str:
    """The key prefix the recipe's inputs are found under: a permanent case's, or a fresh
    registration for a random case."""
    return case_prefix if case_prefix is not None else inputs.register(recipe["inputs"])


def gen_args(recipe: dict, case_prefix: str | None = None) -> tuple[str, ...]:
    """The recipe's arguments with ``{in:NAME}`` as the ANTs suites' ``{gen:dt:<key>}``."""
    prefix = prefix_of(recipe, case_prefix)
    return tuple(
        substitute(arguments(recipe), lambda name: "{gen:dt:" + inputs.key(prefix, name) + "}")
    )


def case_id(recipe: dict) -> str:
    return f"difftest/{recipe['tool']}/{recipe['seed']}/{recipe['n']}"


def describe(recipe: dict) -> str:
    return "random case: " + " ".join(arguments(recipe))[:300]


def ants_case(
    recipe: dict,
    program: str,
    *,
    outputs: Sequence[str] = ("{out}",),
    stdout: bool = False,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
) -> Case:
    from larmorx_validation.parity.ants_programs import AntsCase

    c = AntsCase(
        cid or case_id(recipe),
        "difftest",
        description or describe(recipe),
        program,
        gen_args(recipe, case_prefix),
        outputs=tuple(outputs),
        stdout=stdout,
        **kw,
    )
    return Case(c.id, c.category, c.description, c)


def ants_unavailable() -> str:
    from larmorx_validation.parity.ants_programs import ants_bin

    return "" if ants_bin() is not None else "the ANTs 2.6.5 oracle was not found"


# ------------------------------------------------------------------------------------------------
# Inputs


def image(
    d: gen.Draw,
    ndim: int,
    *,
    contents: Sequence[str] = ("head", "blobs", "noise", "ramp", "checker", "constant", "zeros", "sparse", "labels", "crop"),
    weights: Sequence[float] | None = (5, 2, 2, 1, 1, 0.5, 0.3, 0.5, 1, 1),
    **kw,
) -> dict:
    """An image spec for an ANTs program (2D images are one slice thick in NIfTI)."""
    if ndim == 2:
        kw.setdefault("shape", [d.integer("nx", 2, 32), d.integer("ny", 2, 32)])
        return gen.draw_image(d, 2, contents=contents, content_weights=weights, **kw)
    return gen.draw_image(d, ndim, contents=contents, content_weights=weights, **kw)


def same_grid(spec: dict, d: gen.Draw, **overrides) -> dict:
    """Another image on ``spec``'s grid (shape and geometry), its own content and values."""
    other = gen.draw_image(
        d,
        len(spec["shape"]),
        shape=spec["shape"],
        geometry=spec["geometry"],
        contents=overrides.pop("contents", ("head", "blobs", "noise", "ramp", "constant", "zeros", "sparse")),
        **overrides,
    )
    return other


def mask_of(spec: dict, d: gen.Draw, *, dtype: str | None = None) -> dict:
    """A binary mask on ``spec``'s grid: an ellipsoid, the labels' support, or sparse points."""
    m = gen.draw_image(
        d,
        len(spec["shape"]),
        shape=spec["shape"][:3] if len(spec["shape"]) == 4 else spec["shape"],
        geometry=spec["geometry"],
        contents=("labels", "sparse", "constant", "zeros"),
        content_weights=(6, 1, 1, 0.3),
        dtypes=(dtype,) if dtype else ("uint8", "uint8", "int16", "float32"),
        specials=0.0,
        extreme=0.0,
        scaling=0.0,
    )
    m["binary"] = True
    m["values"] = {"scale": 1.0, "offset": 0.0, "binarize": True}
    return m


def scalar(d: gen.Draw, name: str, values: Sequence[str]) -> str:
    return d.choice(name, tuple(values))


__all__ = ["ants_case", "ants_unavailable", "case_id", "describe", "gen_args", "image", "mask_of", "prefix_of", "same_grid", "scalar"]
