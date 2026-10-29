# Benchmarks: antsAI and antsAffineInitializer

larmorx against ANTs 2.6.5's own binaries (with CORE-MATH's correctly rounded `exp`, `log`, `sin`, `cos`, ... preloaded, the build the parity records compare with), on `tests/parity/test_memory_peak.py`'s head and on the data panel's 7 T MP2RAGE as the template registration gets it. Median of 3 runs.

- **Generated:** 2026-10-28 on Linux x86_64 (AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs), larmorx `ec2910367e8d`, with `python -m larmorx_validation bench ants-ai --repeats 3 --threads 1 8`
- **What is measured:** each run is its own process under `/usr/bin/time -v`: the wall time, the CPU time (user + system) and the peak resident memory of the whole command (reading the images, the search, writing the transform). ANTs runs at one thread (`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1`: 99 % of one CPU and 3 voluntary context switches in a whole run, so it stays on one thread); larmorx through its command (`python -m larmorx ants <program>`, the interpreter included) at the thread counts shown, in `reproduce` (ANTs' behaviour) and in its default mode (`antsAffineInitializer` then runs the 10 iterations the call asks for, ANTs 20: ANTS-54).
- **Same result:** larmorx's transform in `reproduce` is compared with ANTs' byte for byte; larmorx's result does not depend on its thread count.
- **Machine load:** shared machine, informal timings: load average 4.06, 3.81, 5.17 before the run, 2.0 to 13.1 (1 minute) during it.

## antsAffineInitializer, the memory test's head

| Tool | Threads | Wall time | CPU time | Peak memory | vs ANTs (wall) | Output vs ANTs |
|---|---|---|---|---|---|---|
| ANTs 2.6.5 (CORE-MATH) | 1 | 33.5 s | 33.5 s | 89 MiB | 1.00x |  |
| larmorx, reproduce | 1 | 40.0 s | 40.0 s | 91 MiB | 0.84x | byte for byte |
| larmorx, default | 1 | 20.9 s | 21.0 s | 91 MiB | 1.60x | 10 iterations (ANTS-54) |
| larmorx, reproduce | 8 | 8.7 s | 56.6 s | 93 MiB | 3.87x | byte for byte |
| larmorx, default | 8 | 4.8 s | 30.1 s | 92 MiB | 6.96x | 10 iterations (ANTS-54) |

## antsAI, the memory test's head

| Tool | Threads | Wall time | CPU time | Peak memory | vs ANTs (wall) | Output vs ANTs |
|---|---|---|---|---|---|---|
| ANTs 2.6.5 (CORE-MATH) | 1 | 31.2 s | 31.2 s | 96 MiB | 1.00x |  |
| larmorx, reproduce | 1 | 17.5 s | 17.5 s | 95 MiB | 1.78x | byte for byte |
| larmorx, default | 1 | 17.8 s | 17.8 s | 96 MiB | 1.75x |  |
| larmorx, reproduce | 8 | 5.0 s | 25.7 s | 106 MiB | 6.25x | byte for byte |
| larmorx, default | 8 | 4.8 s | 25.6 s | 103 MiB | 6.48x |  |

## antsAffineInitializer, the 7 T MP2RAGE and the MNI template (niworkflows' call)

| Tool | Threads | Wall time | CPU time | Peak memory | vs ANTs (wall) | Output vs ANTs |
|---|---|---|---|---|---|---|
| ANTs 2.6.5 (CORE-MATH) | 1 | 252.2 s | 252.2 s | 688 MiB | 1.00x |  |
| larmorx, reproduce | 1 | 254.5 s | 254.6 s | 676 MiB | 0.99x | byte for byte |
| larmorx, default | 1 | 124.7 s | 124.7 s | 676 MiB | 2.02x | 10 iterations (ANTS-54) |
| larmorx, reproduce | 8 | 51.6 s | 345.6 s | 677 MiB | 4.89x | byte for byte |
| larmorx, default | 8 | 26.1 s | 172.3 s | 678 MiB | 9.66x | 10 iterations (ANTS-54) |

## antsAI, the 7 T MP2RAGE and the MNI template

| Tool | Threads | Wall time | CPU time | Peak memory | vs ANTs (wall) | Output vs ANTs |
|---|---|---|---|---|---|---|
| ANTs 2.6.5 (CORE-MATH) | 1 | 155.8 s | 155.8 s | 754 MiB | 1.00x |  |
| larmorx, reproduce | 1 | 79.0 s | 79.0 s | 717 MiB | 1.97x | byte for byte |
| larmorx, default | 1 | 79.9 s | 79.9 s | 718 MiB | 1.95x |  |
| larmorx, reproduce | 8 | 21.8 s | 114.8 s | 718 MiB | 7.15x | byte for byte |
| larmorx, default | 8 | 20.9 s | 113.4 s | 718 MiB | 7.44x |  |

## Notes

- **Where the time goes.** Both programs spend nearly all their time in the Mattes metric over the sampled points: on the head, antsAffineInitializer's search makes about 2,200 value-only and 300 value-and-derivative evaluations of 157,000 points (16 start points, 20 iterations of golden-section line searches). larmorx computes each point's fixed side once (ITK at every evaluation, to the same value) and the moving side in batches, operation for operation: at one thread a search takes it about 0.6 of ANTs' time (antsAI shows it directly).
- **antsAffineInitializer in `reproduce`.** To say where ITK-21 (the learning rate carried from one start point to the next) changes the result, larmorx runs the search a second time, the other way; at one thread the second search reuses the first one's evaluations where the two ask the same question (the first start point, each start point's first evaluation: 1/16 of it here), so the two together take ANTs' time on the 7 T and about 1.2 times it on the head; at more threads they run at once. In the default mode the call's own 10 iterations run (ANTs always 20, ANTS-54).
- **Memory.** The images are kept in float, as ANTs keeps them; the rule (at most 1.10 times ANTs' peak plus 8 MiB, at one thread) holds on every input. At eight threads each thread adds a few MiB of buffers.
- **The 7 T.** The moving image is 224 x 272 x 288 voxels (70 MB in float, its gradient 420 MB): both programs wait on memory; larmorx reads the corners of a batch of 64 points in one loop, many reads in flight, so its gain is larger there.
- **Details:** `docs/findings/ants-ai.md`, "Memory and speed (2026-10-28)"; the tools: `validation/investigations/ants-ai-speed/`.

