"""Timing harness: warm-up, repeated runs, robust statistics."""

from __future__ import annotations

import gc
import statistics
import time
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from typing import Any


@dataclass
class Measurement:
    """Repeated timings of one operation by one tool on one input."""

    input: str
    operation: str
    tool: str
    seconds: list[float]
    data_bytes: int  # uncompressed voxel bytes processed
    extra: dict[str, Any] = field(default_factory=dict)

    @property
    def median(self) -> float:
        return statistics.median(self.seconds)

    @property
    def best(self) -> float:
        return min(self.seconds)

    @property
    def throughput_mb_s(self) -> float:
        return self.data_bytes / 1e6 / self.median if self.median > 0 else float("inf")

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        d.update(median=self.median, best=self.best, throughput_mb_s=self.throughput_mb_s)
        return d


def measure(fn: Callable[[], Any], *, repeats: int, warmup: int = 1) -> list[float]:
    """Wall-clock seconds of ``repeats`` calls of ``fn`` after ``warmup`` calls.

    The garbage collector is run before, and disabled during, each timed call.
    """
    for _ in range(warmup):
        fn()
    times = []
    for _ in range(repeats):
        gc.collect()
        gc.disable()
        try:
            start = time.perf_counter()
            fn()
            times.append(time.perf_counter() - start)
        finally:
            gc.enable()
    return times
