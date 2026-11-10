# SPDX-License-Identifier: Apache-2.0
"""A BIDS dataset index: ``lx.bids``.

pybids 0.22.0's ``BIDSLayout`` ported without its database (D3 of the field maps' part B): the
same files, entities (``bids.json``, ``derivatives.json``, niworkflows' ``nipreps.json``),
metadata by pybids' inheritance and query results, validated against pybids 0.22.0 and 0.20.0
(fMRIPrep 25.2.6's) on the oracle datasets. sdcflows' estimator search
(:func:`larmorx.sdcflows.find_estimators`) and fMRIPrep's field-map selection
(:mod:`larmorx.pipelines.larmorprepx.estimators`) query it.

- :class:`BIDSLayout`, :class:`BIDSLayoutIndexer`: the index and which files it holds.
- :class:`BIDSFile` (and :class:`BIDSImageFile`, :class:`BIDSDataFile`,
  :class:`BIDSJSONFile`): a file, its entities and metadata.
- :class:`Query`, :func:`parse_file_entities`, :class:`Config`, :class:`Entity`,
  :class:`PaddedInt`, :class:`BIDSMetadata`, :func:`listify`, :func:`natural_sort`; pybids'
  errors.
- :func:`build_path`: pybids' ``build_path``, a file's path from entities and path patterns.
"""

from larmorx.bids.layout import (
    ALWAYS_IGNORE,
    BIDS_CONFIG,
    DEFAULT_LOCATIONS_TO_IGNORE,
    DERIVATIVES_CONFIG,
    NIPREPS_CONFIG,
    PYBIDS_VERSIONS,
    BIDSConflictingValuesError,
    BIDSDataFile,
    BIDSDerivativesValidationError,
    BIDSEntityError,
    BIDSFile,
    BIDSImageFile,
    BIDSJSONFile,
    BIDSLayout,
    BIDSLayoutIndexer,
    BIDSMetadata,
    BIDSValidationError,
    Config,
    Entity,
    PaddedInt,
    PyBIDSError,
    Query,
    TargetError,
    listify,
    natural_sort,
    parse_file_entities,
)
from larmorx.bids.writing import build_path

__all__ = [
    "ALWAYS_IGNORE",
    "BIDS_CONFIG",
    "DEFAULT_LOCATIONS_TO_IGNORE",
    "DERIVATIVES_CONFIG",
    "NIPREPS_CONFIG",
    "PYBIDS_VERSIONS",
    "BIDSConflictingValuesError",
    "BIDSDataFile",
    "BIDSDerivativesValidationError",
    "BIDSEntityError",
    "BIDSFile",
    "BIDSImageFile",
    "BIDSJSONFile",
    "BIDSLayout",
    "BIDSLayoutIndexer",
    "BIDSMetadata",
    "BIDSValidationError",
    "Config",
    "Entity",
    "PaddedInt",
    "PyBIDSError",
    "Query",
    "TargetError",
    "build_path",
    "listify",
    "natural_sort",
    "parse_file_entities",
]
