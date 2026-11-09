# SPDX-License-Identifier: Apache-2.0 AND MIT
"""pybids' path builder: a file name from entities and path patterns (``build_path``).

Ported from pybids 0.22.0 (MIT, Copyright (c) 2016-2026 The PyBIDS Developers; tag
``pybids-0.22.0``; the same in 0.20.0, fMRIPrep 25.2.6's): ``layout/writing.py``
(``build_path``, ``_expand_options``, ``_expand_entities``). niworkflows'
``DerivativesDataSink`` names every derivative with it (:func:`larmorx.niworkflows.derivatives_data_sink`).
pybids' ``write_to_file`` is not ported (nothing on these paths calls it).
"""

from __future__ import annotations

import re
import warnings
from collections.abc import Mapping, Sequence
from itertools import product
from string import Formatter
from typing import Any

from larmorx.bids.layout import listify

__all__ = ["build_path"]

_PATTERN_FIND = re.compile(r"({([\w\d]*?)(?:<([^>]+)>)?(?:\|((?:\.?[\w])+))?\})")


def _expand_options(value: str) -> list[str]:
    """The valid values a pattern's ``[ab]`` alternatives give (``'[Jj]son'`` -> ``Json``,
    ``json``)."""
    expand_patterns = re.findall(r"\[(.*?)\]", value)
    if not expand_patterns:
        return [value]
    value = re.sub(r"\[(.*?)\]", "%s", value)
    return [value % _r for _r in product(*expand_patterns)]


def _expand_entities(entities: Mapping[str, Sequence[Any]]) -> list[dict[str, Any]]:
    """Every combination of the entities' values, the first entity's values varying slowest."""
    keys = list(entities.keys())
    values = list(product(*[entities[k] for k in keys]))
    return [dict(zip(keys, combs, strict=True)) for combs in values]


def build_path(
    entities: Mapping[str, Any], path_patterns: str | Sequence[str], strict: bool = False
) -> str | list[str] | None:
    """The path of a file with ``entities`` by the first of ``path_patterns`` they match
    (pybids' ``build_path``), or ``None``.

    Entities with ``None`` or an empty value are dropped (zeros are kept); list values give one
    path per combination (a list of paths). A pattern names an entity as ``{name}``, with
    ``{name<a|b>}`` for its valid values and ``{name|default}`` for a default; ``[...]`` parts
    are optional, kept when they hold a given entity. ``strict`` requires every entity in the
    pattern."""
    path_patterns = listify(path_patterns)

    # Drop None and empty strings, keep zeros, and listify
    ents = {k: listify(v) for k, v in entities.items() if v or v == 0}

    for pattern in path_patterns:
        entities_matched = list(_PATTERN_FIND.findall(pattern))
        defined = [e[1] for e in entities_matched]

        if strict and set(ents.keys()) - set(defined):
            continue

        new_path = pattern
        tmp_entities = ents.copy()

        # Accept extensions with and without a leading dot
        if "extension" in tmp_entities:
            exts = [e.lstrip(".") for e in tmp_entities["extension"]]
            if re.search(r"\.\{extension", pattern):
                tmp_entities["extension"] = exts
            else:
                tmp_entities["extension"] = ["." + e for e in exts]

        for fmt, name, valid, defval in entities_matched:
            valid_expanded = [v for val in valid.split("|") if val for v in _expand_options(val)]
            if valid_expanded and defval and defval not in valid_expanded:
                warnings.warn(
                    f"Pattern {fmt!r} is inconsistent as it defines an invalid default value.",
                    stacklevel=2,
                )
            if (
                valid_expanded
                and name in tmp_entities
                and set(tmp_entities[name]) - set(valid_expanded)
            ):
                continue
            if defval and name not in tmp_entities:
                tmp_entities[name] = [defval]
            # valid and default values checked and set: simplify the pattern
            new_path = new_path.replace(fmt, "{" + name + "}")

        optional_patterns = re.findall(r"(\[.*?\])", new_path)
        # Optional parts with a given entity become mandatory; the others are removed
        given = {k for k, v in ents.items() if v is not None}
        for op in optional_patterns:
            for ent_name in given:
                if "{" + ent_name + "}" in op:
                    new_path = new_path.replace(op, op[1:-1])
            new_path = new_path.replace(op, "")

        fields = {pat[1] for pat in Formatter().parse(new_path) if pat[1] and not pat[1].isdigit()}
        if fields - set(tmp_entities.keys()):
            continue

        tmp_entities = {k: v for k, v in tmp_entities.items() if k in fields}
        paths = [new_path.format(**e) for e in _expand_entities(tmp_entities)]
        if paths:
            return paths[0] if len(paths) == 1 else paths

    return None
