# SPDX-License-Identifier: Apache-2.0
"""Which runs of an original program the cache may keep: only what the original determines.

A run whose result the original itself varies is never cached and always runs: that variation
is what the records measure. The rules, in the order they are applied (``decide``):

1. **The case or the runner says so** (``Policy.never``, with the reason): a case marked
   ``nondeterministic`` (the original reads the clock, memory it never wrote, or writes a random
   identifier), the runners' own rules (a clock seed: ``antsRegistration`` and ``antsAI``
   without a seed, ``Atropos`` without ``--use-random-seed 0``; a hang), a case that runs the
   same command several times to measure the original's own spread.
2. **Above one thread, programs known to vary from run to run** (:data:`VARIES_ABOVE_ONE_THREAD`,
   each with its finding) are never cached at that thread count.
3. **Any other run above one thread** (or with an unknown thread count) is cached, but served only
   once :data:`CONFIRMATIONS` runs of it gave the same result (the porting guide's rule: a race
   shows only in repeated runs, at least three at every thread count above one). Until then it
   runs, and each run is compared with the stored one; one difference and the entry is never
   served (a finding, kept in ``disputes/``).
4. **One thread, a fixed seed:** cached and served from its first run.

A command a case runs a second time runs again and is compared with the first (a case that runs
the same command twice may be measuring the original's own spread): it is served to such a case
only once :data:`CONFIRMATIONS` runs of it agreed, and one difference makes it a finding.

And, whatever the policy says, a run that ends by a signal (a crash: memory the source does not
define) or by its timeout is never stored; a run whose outputs or messages name its temporary
directories in a form the cache cannot map back is never stored either.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field, replace

#: Identical runs needed before a run above one thread is served from the cache.
CONFIRMATIONS = 3

#: Programs whose result changes from run to run above one thread (never cached there).
VARIES_ABOVE_ONE_THREAD: dict[str, str] = {
    "antsRegistration": (
        "above one thread the metrics' per-work-unit sums change the result from run to run "
        "(docs/findings/ants-registration.md); ITK-34's point-set race"
    ),
    "antsAI": "above one thread its Mattes metric moves by about 1e-13 from run to run "
    "(docs/findings/ants-ai.md); ANTS-60's per-thread heaps",
    "antsAffineInitializer": "the optimiser and metric of antsAI (docs/findings/ants-ai.md)",
    "DenoiseImage": "ANTS-42: its slabs race above one thread (docs/findings/ants-denoise.md)",
}


#: Defects of the originals whose result changes from run to run (the register's class (b), and
#: the class (a) ones whose result depends on memory the source never defines): a case whose
#: description, reason or ``nondeterministic`` text names one of them never keeps the original's
#: runs in the cache (``never_for``).
VARYING_DEFECTS: dict[str, str] = {
    "ANTS-28": "uninitialised values reach ANTs' output",
    "ANTS-42": "DenoiseImage races above one thread",
    "ANTS-58": "antsAI draws from the clock",
    "ANTS-63": "the VTK reader's labels are uninitialised memory",
    "ANTS-64": "the point sets' sampling draws from the clock",
    "ANTS-66": "IGDM reads before its arrays",
    "ANTS-72": "a linear stage after a displacement field draws its voxels from the clock",
    "ANTS-77": "a B-spline mesh read past the end of its list (garbage indices)",
    "ANTS-82": "the velocity-field stages' metric sampling is seeded from the clock",
    "ANTS-103": "an unset coordinate of the joint shape and orientation histograms",
    "ANTS-107": "Atropos weighs samples with uninitialised memory (the heap)",
    "ITK-34": "the point-set metric's derivatives race above one thread",
    "AFNI-20": "AFNI's 1D text file holds a random identifier",
    "AFNI-45": "3dUnifize reads memory it never wrote",
    "AFNI-47": "3dUnifize reads past the end of its scale image",
    "ANTS-160": "ResampleImage reads missing Gaussian sigmas past the end of their list (the heap)",
    "ANTS-163": "ImageMath reads a mask of another size outside its buffer (the heap)",
    "ITK-98": "B-spline interpolation reads the coefficients outside their buffer on a short axis",
}
_DEFECT_ID = re.compile(r"\b(ANTS|ITK|AFNI|FSL|NIB)-\d+\b")


def never_for(*texts: str) -> str:
    """Why a case never keeps the original's runs, from its texts: the first defect of
    :data:`VARYING_DEFECTS` they name (``""``: none)."""
    for text in texts:
        for m in _DEFECT_ID.finditer(text or ""):
            if m.group(0) in VARYING_DEFECTS:
                return f"{m.group(0)}: {VARYING_DEFECTS[m.group(0)]}"
    return ""


@dataclass(frozen=True)
class Policy:
    """How the cache treats one run of an original program.

    ``threads``: the original's thread count (``None``: not set, the machine's). ``never``: why
    this run is never cached (empty: it may be). ``volatile``: regular expressions of what the
    run prints that changes from run to run without changing the result (masked when a run is
    compared with its stored entry; never in what is served). ``nifti_extensions``: why the
    NIfTI extensions the program writes are not part of the result (AFNI's: a random
    identifier, the date and the output's own path, which no suite reads); then they are left
    out when runs are compared, and may hold the run's paths. ``family``: which variables of
    the environment are in the key (``"itk"``, ``"afni"``, ``"fsl"``:
    :data:`larmorx_validation.oracle_cache.key.ENV_FAMILIES`; ``None``: every family's)."""

    threads: int | None = 1
    never: str = ""
    volatile: tuple[str, ...] = field(default=())
    nifti_extensions: str = ""
    family: str | None = None

    def but_never(self, reason: str) -> Policy:
        """This policy, never cached for ``reason`` (the first reason given is kept)."""
        if self.never or not reason:
            return self
        return replace(self, never=reason)


@dataclass(frozen=True)
class Decision:
    cache: bool
    required: int = 1
    reason: str = ""


def decide(policy: Policy, program: str) -> Decision:
    """Whether a run may be cached, and how many identical runs it needs before it is served."""
    if policy.never:
        return Decision(False, 0, policy.never)
    if policy.threads == 1:
        return Decision(True, 1)
    if program in VARIES_ABOVE_ONE_THREAD:
        return Decision(False, 0, f"{program} above one thread: {VARIES_ABOVE_ONE_THREAD[program]}")
    return Decision(
        True, CONFIRMATIONS, f"above one thread: served after {CONFIRMATIONS} identical runs"
    )


def threads_from_env(env: dict[str, str], *names: str) -> int | None:
    """The thread count the first of ``names`` sets in ``env`` (``None``: none is set). ITK reads
    ``ITK_NUMBER_OF_THREADS_ENV_LIST``'s variables, then ``NSLOTS``, then
    ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS``: the last one set wins (docs/findings/ants-cli.md)."""
    for name in names:
        value = env.get(name)
        if value is not None:
            try:
                return int(value)
            except ValueError:
                return None
    return None


def itk_threads(env: dict[str, str]) -> int | None:
    """The thread count an ITK program reads from ``env`` (``None``: the machine's)."""
    order = [*env.get("ITK_NUMBER_OF_THREADS_ENV_LIST", "").split(":"), "NSLOTS"]
    order.append("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS")
    found = None
    for name in order:
        if name and name in env:
            found = threads_from_env(env, name)
    return found


def ants_seed_never(program: str, args: list[str], env: dict[str, str]) -> str:
    """The clock rules of ANTs' programs: why a run reads the clock, or ``""``.

    - ``antsRegistration``: no ``ANTS_RANDOM_SEED`` and no ``--random-seed`` seed its samplers
      from the clock (docs/findings/ants-registration.md).
    - ``antsAI``: seeds with 1234 unless told otherwise, but ``--random-seed 0`` and ``Random``
      sampling draw from the clock (ANTS-58).
    - ``Atropos``: ``--use-random-seed 1`` (its default) seeds from the clock.
    """
    seed = None
    for i, a in enumerate(args[:-1]):
        if a == "--random-seed":
            seed = args[i + 1]
    if program == "antsRegistration":
        if seed is None and "ANTS_RANDOM_SEED" not in env:
            return "no seed: antsRegistration seeds its samplers from the clock"
        return ""
    if program == "antsAI":
        if seed is not None and seed.strip() == "0":
            return "--random-seed 0: antsAI seeds Regular sampling from the clock (ANTS-58)"
        if any(",Random" in a or "[Random" in a for a in args):
            return "Random sampling: antsAI draws the voxels from the clock (ANTS-58)"
        return ""
    if program == "Atropos":
        value = "1"
        for i, a in enumerate(args[:-1]):
            if a in ("-r", "--use-random-seed"):
                value = args[i + 1]
        if value.strip() not in ("0", "false"):
            return "--use-random-seed 1 (Atropos' default) seeds from the clock"
    return ""


__all__ = [
    "CONFIRMATIONS",
    "VARIES_ABOVE_ONE_THREAD",
    "VARYING_DEFECTS",
    "Decision",
    "Policy",
    "ants_seed_never",
    "decide",
    "itk_threads",
    "never_for",
    "threads_from_env",
]
