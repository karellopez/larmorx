# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit: proof, from the session transcripts made at the time, that no agent that
wrote a clean-room original read the original's source or anything derived from it (the user,
2026-11-06: "the only thing most important to prove is that the agents did not look directly the
source code"; code similarity is extra evidence only).

- :mod:`.restrictions`: the restriction table (what is restricted, for which family, since when)
  and the classification of a path;
- :mod:`.shell`: Bash commands read into simple commands (quotes, here-documents, variables,
  ``cd``, loops, command substitutions);
- :mod:`.accesses`: what one tool call read, listed, searched, ran, wrote or committed;
- :mod:`.transcripts`: the transcripts found and read as a stream, every call with its result;
- :mod:`.verdicts`: who touched each clean-room original, in which role, after seeing what;
- :mod:`.similarity`: code similarity against the upstream source and the replicas, with its
  calibration (figures only, no threshold);
- :mod:`.cli`: ``python -m larmorx_validation cleanroom-audit scan|verdicts|similarity``.

The outputs hold absolute paths and the account name: they stay in ``<workspace>/archive/`` (or a
scratch directory), never in the repository.
"""
