# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23 beside 6.0.7.17 in the FSL tools' evidence (the version update of 2026-10-29).

The figures cache keys an entry by its tool and case only, and the FSL tools' entries were
computed with FSL 6.0.7.17: they stay, and still give the 6.0.7.17 panels. FSL 6.0.7.23's runs,
with the replica's runs that are compared with them (the replica follows 6.0.7.23 since
2026-10-29), are entries of their own whose case carries the version (:func:`case`:
``<case>-fsl-6.0.7.23``). Each example then shows FSL 6.0.7.23's output as a panel of its own,
the replica against it, and FSL 6.0.7.23 against 6.0.7.17 (the version step: a reference row,
FSL against FSL), so a reader sees which version each panel ran and what changed. (AFNI's step,
:mod:`afni_step`, added its new runs to the old entries instead.)
"""

from __future__ import annotations

from larmorx_validation.evidence import common
from larmorx_validation.parity import fsl_versions

NEW = fsl_versions.NEWEST
OLD = fsl_versions.PREVIOUS
NEW_LABEL = f"FSL {NEW}"
OLD_LABEL = f"FSL {OLD}"
ROLE_STEP = "the original against itself: the version step"
#: What FSL 6.0.7.23 is to the replica.
REFERENCE = "the replica's reference since 2026-10-29"


def case(name: str) -> str:
    """The case of FSL 6.0.7.23's entry beside the entry ``name`` (computed with 6.0.7.17)."""
    return f"{name}-fsl-{NEW}"


def replica_diff(a: str, b: str, what: str = "") -> dict:
    """The replica (following FSL 6.0.7.23) against FSL 6.0.7.23."""
    return common.d(f"{a}-{b}", a, b, "main", f"replica − {NEW_LABEL}{what}", common.ROLE_LX)


def step_diff(a: str, b: str, what: str = "") -> dict:
    """FSL 6.0.7.23 against 6.0.7.17: a reference row (FSL against FSL), no verdict."""
    return common.d(f"{a}-{b}", a, b, "variability", f"{NEW_LABEL} − {OLD_LABEL}{what}", ROLE_STEP)


def step_reason(same: bool, what: str, headers_same: bool | None = None) -> str:
    """The version step's reason on ``what`` (a call): its values the same or not, and its
    headers (``headers_same``: None when not compared)."""
    if same:
        head = (
            ""
            if headers_same is None
            else (
                " The headers too, byte for byte."
                if headers_same
                else " The images' headers differ in FSL's build string only (`descrip`: newimage "
                "2601.0's `2601.0 2025-12-08T15:20:57Z`)."
            )
        )
        return (
            f"{NEW_LABEL} gives {OLD_LABEL}'s values on {what}: the version step changes "
            f"nothing computed here.{head}"
        )
    return (
        f"{NEW_LABEL} gives other values than {OLD_LABEL} on {what}; the replica follows "
        f"{NEW} (the row above)."
    )


def replica_reason(same: bool, note: str = "") -> str:
    if same:
        return f"The same values as {NEW_LABEL}, every one.{note}"
    return f"The replica should give {NEW_LABEL}'s values here; it does not (see the record)."


def fact(same: bool, what: str, records: str) -> dict:
    """The example's fact on the versions: what each panel ran, and the step on ``what``."""
    step = (
        f"on {what} it gives {OLD}'s values (the headers' build string aside)"
        if same
        else f"on {what} its values differ from {OLD}'s"
    )
    return {
        "label": "FSL versions",
        "text": f"{OLD_LABEL}'s panels as first computed; {NEW_LABEL} ({REFERENCE}) on the same "
        f"arguments, with the replica's run compared with it: {step}. {records}",
    }
