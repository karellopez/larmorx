# SPDX-License-Identifier: Apache-2.0
"""niworkflows' participants of a BIDS dataset: ``collect_participants`` and its ``BIDSError``.

Ported from niworkflows 1.15.4 (Apache-2.0, the NiPreps developers; the same in 1.15.2,
fMRIPrep 25.2.6's image): ``utils/bids.py`` (``BIDSError``, ``BIDSWarning``,
``collect_participants``), on a :class:`larmorx.bids.BIDSLayout`. sdcflows' command line
(:mod:`larmorx.sdcflows.app`) lists its participants with it.
"""

from __future__ import annotations

import warnings
from collections.abc import Sequence
from typing import Any

__all__ = ["BIDSError", "BIDSWarning", "collect_participants"]


class BIDSError(ValueError):
    """niworkflows' ``BIDSError``: ``message`` framed by a header naming the dataset's root
    (``msg``) and the root itself (``bids_root``)."""

    def __init__(self, message: str, bids_root: Any):
        indent = 10
        header = '{sep} BIDS root folder: "{bids_root}" {sep}'.format(
            bids_root=bids_root, sep="".join(["-"] * indent)
        )
        self.msg = "\n{header}\n{indent}{message}\n{footer}".format(
            header=header,
            indent="".join([" "] * (indent + 1)),
            message=message,
            footer="".join(["-"] * len(header)),
        )
        super().__init__(self.msg)
        self.bids_root = bids_root


class BIDSWarning(RuntimeWarning):
    """niworkflows' ``BIDSWarning``: participants asked for that are not in the dataset."""


def collect_participants(
    bids_dir: Any,
    participant_label: str | Sequence[str] | None = None,
    strict: bool = False,
    bids_validate: bool = True,
) -> list[str]:
    """niworkflows' ``collect_participants``: the participants of ``bids_dir`` (a
    :class:`larmorx.bids.BIDSLayout`, or a dataset's folder, then indexed with
    ``validate=bids_validate``) to process, sorted.

    Without ``participant_label`` every participant; else those of its labels (``sub-``
    prefixes dropped, duplicates removed) found in the dataset, a :class:`BIDSWarning` naming the
    others (with ``strict``, a :class:`BIDSError`). A dataset without participants, or labels of
    which none is found: :class:`BIDSError`, with niworkflows' messages."""
    from larmorx.bids import BIDSLayout

    if isinstance(bids_dir, BIDSLayout):
        layout = bids_dir
    else:
        layout = BIDSLayout(str(bids_dir), validate=bids_validate)

    all_participants = set(layout.get_subjects())

    # Error: bids_dir does not contain subjects
    if not all_participants:
        raise BIDSError(
            "Could not find participants. Please make sure the BIDS data "
            "structure is present and correct. Datasets can be validated "
            "online using the BIDS Validator "
            "(https://bids-standard.github.io/bids-validator/).\n"
            "If you are using Docker for Mac or Docker for Windows, you "
            'may need to adjust your "File sharing" preferences.',
            bids_dir,
        )

    # No --participant-label was set, return all
    if not participant_label:
        return sorted(all_participants)

    if isinstance(participant_label, str):
        participant_label = [participant_label]

    # Drop sub- prefixes; remove duplicates; remove labels not found
    labels = sorted({sub.removeprefix("sub-") for sub in participant_label})
    found_label = sorted(set(labels) & all_participants)
    if not found_label:
        raise BIDSError("Could not find participants [{}]".format(", ".join(labels)), bids_dir)

    # Warn if some IDs were not found
    notfound_label = sorted(set(labels) - all_participants)
    if notfound_label:
        exc = BIDSError(
            "Some participants were not found: {}".format(", ".join(notfound_label)), bids_dir
        )
        if strict:
            raise exc
        warnings.warn(exc.msg, BIDSWarning, stacklevel=2)

    return found_label
