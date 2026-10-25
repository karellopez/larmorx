# SPDX-License-Identifier: Apache-2.0
"""Differential random testing: random valid inputs and option combinations through each
original program and larmorx, every night (``python -m larmorx_validation difftest``;
``docs/validation/difftest.md``).

- :mod:`.gen`: seeded inputs from plain arithmetic (the golden tests' rule), every choice kept;
- :mod:`.core`: a tool's descriptor (:class:`~.core.Tool`), recipes, verdicts;
- :mod:`.tools`: the descriptors, one short module per tool;
- :mod:`.inputs`: the inputs' files, built from recipes on demand (the parity suites' hook);
- :mod:`.shrink`: a difference shrunk to the smallest input and fewest options;
- :mod:`.runner`: one case in a worker, a night across tools; the oracle cache's policy;
- :mod:`.permanent`: shrunk differences kept as parity cases;
- :mod:`.report`: the morning report and its numbers.
"""
