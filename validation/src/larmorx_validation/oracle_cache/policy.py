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
from collections.abc import Sequence
from dataclasses import dataclass, field, replace
from pathlib import Path

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
    "ANTS-07": "a smaller second image is read past its end (the heap)",
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
    "ITK-99": "a sampled metric writes a derivative past the end of its array (the heap)",
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


#: antsRegistration's stages that never seed their metric's sampling (ANTS-82): with ``Regular``
#: or ``Random`` sampling they draw from the clock whatever the seed.
CLOCK_SAMPLED_STAGES = ("Exponential", "BSplineExponential", "TimeVaryingVelocityField")
_STAGE = re.compile(r"^([A-Za-z]+)\[")


def _clock_sampled(args: list[str]) -> str:
    """The first stage of ``CLOCK_SAMPLED_STAGES`` whose metric samples ``Regular`` or
    ``Random`` points (``""``: none). A stage is a ``-t``/``--transform`` and the ``-m``/
    ``--metric`` options after it."""
    stage = ""
    for i, a in enumerate(args[:-1]):
        value = args[i + 1]
        if a in ("-t", "--transform"):
            m = _STAGE.match(value)
            stage = m.group(1) if m else value
        elif a in ("-m", "--metric") and stage in CLOCK_SAMPLED_STAGES:
            fields = [f.strip(" []") for f in value.split(",")]
            if any(f in ("Regular", "Random") for f in fields):
                return stage
    return ""


def ants_seed_never(program: str, args: list[str], env: dict[str, str]) -> str:
    """The clock rules of ANTs' programs: why a run reads the clock, or ``""``.

    - ``antsRegistration``: no ``ANTS_RANDOM_SEED`` and no ``--random-seed`` seed its samplers
      from the clock (docs/findings/ants-registration.md); and whatever the seed, the
      ``Exponential``, ``BSplineExponential`` and ``TimeVaryingVelocityField`` stages sample
      their metric (``Regular`` or ``Random``) from the clock (ANTS-82; random testing's runs of
      2026-10-25 varied on their own machine, machine-move-proof.md).
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
        if stage := _clock_sampled(args):
            return f"ANTS-82: {stage} samples its metric from the clock, whatever the seed"
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


#: ImageMath's voxel-wise operations that read their second image at the first image's indices
#: (``ImageMath_Templates.hxx:5583-5762``; ANTS-07).
IMAGE_MATH_VOXELWISE = frozenset(
    ("m", "+", "-", "/", "^", "exp", "max", "abs", "addtozero", "overadd", "Decision", "total",
     "mean", "vtotal")
)  # fmt: skip


def _grid(path: str, dim: int) -> tuple[int, ...] | None:
    """A NIfTI image's voxel grid as an ITK program of ``dim`` dimensions reads it (its first
    ``dim`` axes, missing ones 1), from its header; ``None`` if it is not one."""
    if not path.endswith((".nii", ".nii.gz", ".hdr", ".img")):
        return None
    try:
        import nibabel as nib

        shape = tuple(int(n) for n in nib.load(path).shape)
    except Exception:
        return None
    return (shape + (1,) * dim)[:dim]


def read_past_end(first: tuple[int, ...], second: tuple[int, ...]) -> bool:
    """Whether reading ``second`` at each voxel index of ``first`` with ``second``'s own strides
    (``image2->GetPixel(index)``, ANTS-07) reads past the end of its buffer."""
    stride, last, size = 1, 0, 1
    for f, s in zip(first, second, strict=True):
        last += (f - 1) * stride
        stride *= s
        size *= s
    return last >= size


def ants_memory_never(program: str, args: list[str]) -> str:
    """Why a run of an ANTs program reads memory it never wrote, decided from its arguments and
    its images' headers (``""``: it does not):

    - ImageMath's voxel-wise operations and MultiplyImages with a second image read past its end
      at the first image's indices (ANTS-07);
    - ResampleImageBySpacing smoothing an axis of fewer than 4 voxels: ITK throws, ANTs catches
      it and resamples the filter's never-written buffer (ANTS-28).

    Random testing's runs of 2026-10-25 met them without naming them, and varied on their own
    machine (machine-move-proof.md); the suites' own cases name them in their texts."""
    try:
        dim = int(args[0])
    except (IndexError, ValueError):
        return ""
    if not 1 <= dim <= 6:
        return ""
    pair = None
    if program == "ImageMath" and len(args) > 4 and args[2] in IMAGE_MATH_VOXELWISE:
        pair = (args[3], args[4])
    elif program == "MultiplyImages" and len(args) > 2:
        pair = (args[1], args[2])
    if pair is not None:
        a, b = _grid(pair[0], dim), _grid(pair[1], dim)
        if a is not None and b is not None and a != b and read_past_end(a, b):
            return f"ANTS-07: {VARYING_DEFECTS['ANTS-07']} ({b} read at the indices of {a})"
        return ""
    if program == "ResampleImageBySpacing" and len(args) > 1:
        smooth = args[2 + dim + 1] if len(args) > 3 + dim else "1"
        grid = _grid(args[1], dim)
        if grid is not None and smooth.strip() != "0" and min(grid[: min(dim, 3)]) < 4:
            return (
                f"ANTS-28: smoothing an axis of fewer than 4 voxels ({grid}) resamples a buffer "
                "ITK never wrote"
            )
    return ""


def ants_never(program: str, args: list[str], env: dict[str, str]) -> str:
    """The program-level rules of ANTs' programs (the clock, memory never written): why a run of
    ``program`` is never cached, or ``""``. The suites' ANTs runners apply them to every run;
    ``oracle-cache verify`` skips an entry they now reach."""
    return ants_seed_never(program, args, env) or ants_memory_never(program, args)


#: Whitespace in a path: every program may read it as text (a shell script expanding its
#: arguments unquoted, FSL's ``bet``: ``bet2 $IN $OUT``; AFNI without ``AFNI_PATH_SPACES_OK``).
WHITESPACE = (" ", "\t", "\n")


def path_text(program: str, args: Sequence[str]) -> tuple[str, ...]:
    """The text a run of ``program`` may read in its arguments' paths, beyond the files they
    name: the cache key keeps whichever of these a path holds (``prepare(..., text=...)``).
    Whitespace for every program; ``-h`` and ``--help`` for bet2 with one argument, which then
    prints its usage when either is anywhere in it (``bet2.cpp:811-818``, docs/findings/
    fsl-bet.md; the cache served one run for both, and ``bet2/one-argument`` failed at random,
    machine-move-proof.md)."""
    texts = list(WHITESPACE)
    if Path(program).name == "bet2" and len(args) < 2:
        texts += ["-h", "--help"]
    return tuple(texts)


__all__ = [
    "CLOCK_SAMPLED_STAGES",
    "CONFIRMATIONS",
    "IMAGE_MATH_VOXELWISE",
    "VARIES_ABOVE_ONE_THREAD",
    "VARYING_DEFECTS",
    "WHITESPACE",
    "Decision",
    "Policy",
    "ants_memory_never",
    "ants_never",
    "ants_seed_never",
    "decide",
    "itk_threads",
    "never_for",
    "path_text",
    "read_past_end",
    "threads_from_env",
]
