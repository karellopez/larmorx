# Benchmarks: NIfTI reading and writing

larmorx (`larmorx.load` / `larmorx.save`, crate `larmorx-io`) against nibabel and SimpleITK.

- **Generated:** 2026-10-09 on Linux x86_64 (Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs), with `python -m larmorx_validation bench nifti-io --tier full --repeats 3`
- **Method:** median of 3 runs after one warm-up, files in the OS page cache (parsing, decompression and conversion, not disk speed). Reads materialise the voxel array (nibabel with `mmap=False`; memory-mapping uncompressed files is a nibabel feature larmorx does not have yet). *Speed-up* is the nibabel median divided by the tool's median for the same operation.
- **Test data:** larmorx-testdata `db846de5f34a`, tier `full`
- **Pitfall avoided:** ITK (and so SimpleITK and ANTs) reads NIfTI through nifti_clib, which prefers `x.nii` over `x.nii.gz` when both exist; compressed and uncompressed inputs are kept in separate directories.
- **Defaults differ:** nibabel writes gzip level 1, larmorx level 2 (zlib-rs; the same file size as zlib's level 1), SimpleITK zlib's default (6). The *level 6* rows compare larmorx and nibabel at the same level.

## sub-f1031ax_ses-wave1bas_task-Cuedts_acq-mb4AP_run-1_sbref (90×90×60 uint16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 3.9 ms | 3.7 ms | 252 | 6.0× |  |
| read | larmorx (threads=4) | 3.9 ms | 3.8 ms | 251 | 6.0× |  |
| read | larmorx (threads=all) | 4.5 ms | 4.0 ms | 217 | 5.2× |  |
| read | nibabel | 23.3 ms | 22.7 ms | 42 | 1.0× |  |
| read | SimpleITK | 13.5 ms | 13.2 ms | 72 | 1.7× |  |
| read float32 | larmorx | 4.2 ms | 4.2 ms | 232 | 5.1× |  |
| read float32 | nibabel | 21.5 ms | 21.1 ms | 45 | 1.0× |  |
| write | larmorx (threads=1) | 20.6 ms | 20.4 ms | 47 | 1.3× | 0.8 MB |
| write | larmorx (threads=4) | 15.4 ms | 10.2 ms | 63 | 1.8× | 0.8 MB |
| write | larmorx (threads=all) | 11.3 ms | 11.1 ms | 86 | 2.4× | 0.8 MB |
| write | nibabel | 27.3 ms | 27.2 ms | 36 | 1.0× | 0.8 MB |
| write | SimpleITK | 25.7 ms | 24.7 ms | 38 | 1.1× | 0.7 MB |
| write (gzip level 6) | larmorx (threads=all) | 13.2 ms | 12.2 ms | 74 | 5.1× | 0.7 MB |
| write (gzip level 6) | nibabel | 67.5 ms | 65.5 ms | 14 | 1.0× | 0.8 MB |

## sub-f1031ax_ses-wave1bas_task-Cuedts_acq-mb4AP_run-1_sbref (90×90×60 uint16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 0.5 ms | 0.5 ms | 1,847 | 1.5× |  |
| read | larmorx (threads=4) | 0.5 ms | 0.4 ms | 2,045 | 1.7× |  |
| read | larmorx (threads=all) | 0.6 ms | 0.6 ms | 1,528 | 1.3× |  |
| read | nibabel | 0.8 ms | 0.8 ms | 1,205 | 1.0× |  |
| read | SimpleITK | 1.4 ms | 1.4 ms | 699 | 0.6× |  |
| read float32 | larmorx | 0.9 ms | 0.8 ms | 1,114 | 1.1× |  |
| read float32 | nibabel | 0.9 ms | 0.9 ms | 1,043 | 1.0× |  |
| write | larmorx (threads=1) | 1.3 ms | 1.1 ms | 724 | 1.0× | 1.0 MB |
| write | larmorx (threads=4) | 1.1 ms | 1.1 ms | 896 | 1.2× | 1.0 MB |
| write | larmorx (threads=all) | 1.1 ms | 1.1 ms | 863 | 1.2× | 1.0 MB |
| write | nibabel | 1.3 ms | 1.3 ms | 722 | 1.0× | 1.0 MB |
| write | SimpleITK | 1.4 ms | 1.4 ms | 677 | 0.9× | 1.0 MB |

## tpl-MNI152NLin2009cAsym_res-01_T1w (193×229×193 int16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 64.4 ms | 64.1 ms | 265 | 2.4× |  |
| read | larmorx (threads=4) | 64.1 ms | 62.8 ms | 266 | 2.5× |  |
| read | larmorx (threads=all) | 63.5 ms | 62.8 ms | 269 | 2.5× |  |
| read | nibabel | 157.4 ms | 156.4 ms | 108 | 1.0× |  |
| read | SimpleITK | 86.6 ms | 86.2 ms | 197 | 1.8× |  |
| read float32 | larmorx | 82.0 ms | 72.2 ms | 208 | 2.0× |  |
| read float32 | nibabel | 160.9 ms | 160.0 ms | 106 | 1.0× |  |
| write | larmorx (threads=1) | 423.5 ms | 413.1 ms | 40 | 1.2× | 13.9 MB |
| write | larmorx (threads=4) | 123.9 ms | 121.7 ms | 138 | 4.0× | 13.9 MB |
| write | larmorx (threads=all) | 86.9 ms | 83.2 ms | 196 | 5.7× | 13.9 MB |
| write | nibabel | 498.0 ms | 495.8 ms | 34 | 1.0× | 13.7 MB |
| write | SimpleITK | 460.6 ms | 450.3 ms | 37 | 1.1× | 13.8 MB |
| write (gzip level 6) | larmorx (threads=all) | 100.8 ms | 100.4 ms | 169 | 8.0× | 13.8 MB |
| write (gzip level 6) | nibabel | 810.5 ms | 786.5 ms | 21 | 1.0× | 13.7 MB |

## tpl-MNI152NLin2009cAsym_res-01_T1w (193×229×193 int16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 3.4 ms | 3.3 ms | 5,012 | 1.1× |  |
| read | larmorx (threads=4) | 3.6 ms | 3.4 ms | 4,680 | 1.1× |  |
| read | larmorx (threads=all) | 3.6 ms | 3.4 ms | 4,753 | 1.1× |  |
| read | nibabel | 3.8 ms | 3.7 ms | 4,453 | 1.0× |  |
| read | SimpleITK | 15.2 ms | 13.2 ms | 1,125 | 0.3× |  |
| read float32 | larmorx | 11.5 ms | 10.8 ms | 1,480 | 0.8× |  |
| read float32 | nibabel | 9.5 ms | 9.3 ms | 1,790 | 1.0× |  |
| write | larmorx (threads=1) | 12.9 ms | 12.8 ms | 1,317 | 1.1× | 17.1 MB |
| write | larmorx (threads=4) | 13.6 ms | 13.4 ms | 1,252 | 1.1× | 17.1 MB |
| write | larmorx (threads=all) | 13.5 ms | 13.2 ms | 1,263 | 1.1× | 17.1 MB |
| write | nibabel | 14.6 ms | 14.2 ms | 1,165 | 1.0× | 17.1 MB |
| write | SimpleITK | 14.1 ms | 13.7 ms | 1,213 | 1.0× | 17.1 MB |

## sub-f1031ax_ses-wave1bas_T1w (208×300×320 float64, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 272.4 ms | 270.0 ms | 586 | 1.1× |  |
| read | larmorx (threads=4) | 198.0 ms | 197.8 ms | 807 | 1.5× |  |
| read | larmorx (threads=all) | 184.1 ms | 183.3 ms | 868 | 1.6× |  |
| read | nibabel | 289.7 ms | 280.8 ms | 551 | 1.0× |  |
| read | SimpleITK | 255.5 ms | 255.4 ms | 625 | 1.1× |  |
| read float32 | larmorx | 176.3 ms | 173.7 ms | 906 | 1.8× |  |
| read float32 | nibabel | 309.5 ms | 302.5 ms | 516 | 1.0× |  |
| write | larmorx (threads=1) | 921.4 ms | 919.4 ms | 173 | 1.5× | 35.0 MB |
| write | larmorx (threads=4) | 372.6 ms | 360.3 ms | 429 | 3.7× | 35.0 MB |
| write | larmorx (threads=all) | 276.8 ms | 271.6 ms | 577 | 5.0× | 35.0 MB |
| write | nibabel | 1.39 s | 1.38 s | 115 | 1.0× | 36.7 MB |
| write | SimpleITK | 2.81 s | 2.78 s | 57 | 0.5× | 31.8 MB |
| write (gzip level 6) | larmorx (threads=all) | 615.6 ms | 612.6 ms | 259 | 8.0× | 31.7 MB |
| write (gzip level 6) | nibabel | 4.93 s | 4.91 s | 32 | 1.0× | 32.5 MB |

## sub-f1031ax_ses-wave1bas_T1w (208×300×320 float64, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 95.4 ms | 83.4 ms | 1,674 | 1.1× |  |
| read | larmorx (threads=4) | 85.1 ms | 82.1 ms | 1,878 | 1.3× |  |
| read | larmorx (threads=all) | 84.9 ms | 84.4 ms | 1,883 | 1.3× |  |
| read | nibabel | 108.5 ms | 106.7 ms | 1,473 | 1.0× |  |
| read | SimpleITK | 213.3 ms | 206.0 ms | 749 | 0.5× |  |
| read float32 | larmorx | 96.4 ms | 94.8 ms | 1,658 | 1.3× |  |
| read float32 | nibabel | 124.0 ms | 121.6 ms | 1,288 | 1.0× |  |
| write | larmorx (threads=1) | 149.2 ms | 146.4 ms | 1,071 | 1.0× | 159.7 MB |
| write | larmorx (threads=4) | 148.6 ms | 148.5 ms | 1,075 | 1.0× | 159.7 MB |
| write | larmorx (threads=all) | 150.0 ms | 145.6 ms | 1,065 | 1.0× | 159.7 MB |
| write | nibabel | 150.1 ms | 146.4 ms | 1,065 | 1.0× | 159.7 MB |
| write | SimpleITK | 144.2 ms | 143.2 ms | 1,108 | 1.0× | 159.7 MB |

## sub-16_acq-mp2rage_T1w (224×272×288 float64, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 190.8 ms | 179.8 ms | 736 | 1.0× |  |
| read | larmorx (threads=4) | 135.5 ms | 129.9 ms | 1,036 | 1.5× |  |
| read | larmorx (threads=all) | 122.2 ms | 122.1 ms | 1,149 | 1.6× |  |
| read | nibabel | 198.3 ms | 193.8 ms | 708 | 1.0× |  |
| read | SimpleITK | 189.7 ms | 188.9 ms | 740 | 1.0× |  |
| read float32 | larmorx | 117.6 ms | 117.1 ms | 1,193 | 1.8× |  |
| read float32 | nibabel | 211.7 ms | 211.5 ms | 663 | 1.0× |  |
| write | larmorx (threads=1) | 1.11 s | 1.09 s | 127 | 1.8× | 44.6 MB |
| write | larmorx (threads=4) | 394.9 ms | 387.0 ms | 356 | 4.9× | 44.6 MB |
| write | larmorx (threads=all) | 306.4 ms | 297.6 ms | 458 | 6.3× | 44.6 MB |
| write | nibabel | 1.94 s | 1.91 s | 72 | 1.0× | 46.9 MB |
| write | SimpleITK | 2.50 s | 2.48 s | 56 | 0.8× | 40.7 MB |
| write (gzip level 6) | larmorx (threads=all) | 616.3 ms | 568.5 ms | 228 | 8.3× | 40.7 MB |
| write (gzip level 6) | nibabel | 5.09 s | 5.01 s | 28 | 1.0× | 42.7 MB |

## sub-16_acq-mp2rage_T1w (224×272×288 float64, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 71.0 ms | 70.5 ms | 1,977 | 1.3× |  |
| read | larmorx (threads=4) | 74.2 ms | 72.5 ms | 1,891 | 1.2× |  |
| read | larmorx (threads=all) | 72.1 ms | 71.4 ms | 1,948 | 1.3× |  |
| read | nibabel | 91.4 ms | 91.1 ms | 1,536 | 1.0× |  |
| read | SimpleITK | 184.6 ms | 180.3 ms | 760 | 0.5× |  |
| read float32 | larmorx | 91.7 ms | 86.0 ms | 1,530 | 1.2× |  |
| read float32 | nibabel | 108.4 ms | 107.9 ms | 1,295 | 1.0× |  |
| write | larmorx (threads=1) | 137.8 ms | 124.1 ms | 1,019 | 1.0× | 140.4 MB |
| write | larmorx (threads=4) | 133.8 ms | 132.4 ms | 1,049 | 1.1× | 140.4 MB |
| write | larmorx (threads=all) | 136.8 ms | 133.5 ms | 1,026 | 1.0× | 140.4 MB |
| write | nibabel | 141.9 ms | 129.9 ms | 989 | 1.0× | 140.4 MB |
| write | SimpleITK | 129.7 ms | 117.7 ms | 1,083 | 1.1× | 140.4 MB |

## tpl-OASIS30ANTs_res-01_T1w (216×291×256 float32, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 151.4 ms | 149.7 ms | 425 | 1.4× |  |
| read | larmorx (threads=4) | 149.4 ms | 148.1 ms | 431 | 1.4× |  |
| read | larmorx (threads=all) | 166.5 ms | 149.4 ms | 386 | 1.3× |  |
| read | nibabel | 212.2 ms | 212.1 ms | 303 | 1.0× |  |
| read | SimpleITK | 229.9 ms | 229.1 ms | 280 | 0.9× |  |
| read float32 | larmorx | 152.0 ms | 151.2 ms | 423 | 1.4× |  |
| read float32 | nibabel | 215.8 ms | 212.6 ms | 298 | 1.0× |  |
| write | larmorx (threads=1) | 805.7 ms | 799.3 ms | 80 | 1.7× | 32.2 MB |
| write | larmorx (threads=4) | 257.1 ms | 256.6 ms | 250 | 5.3× | 32.2 MB |
| write | larmorx (threads=all) | 193.5 ms | 191.1 ms | 333 | 7.1× | 32.2 MB |
| write | nibabel | 1.37 s | 1.36 s | 47 | 1.0× | 32.4 MB |
| write | SimpleITK | 977.1 ms | 975.9 ms | 66 | 1.4× | 32.0 MB |
| write (gzip level 6) | larmorx (threads=all) | 247.2 ms | 244.2 ms | 260 | 6.5× | 32.0 MB |
| write (gzip level 6) | nibabel | 1.61 s | 1.60 s | 40 | 1.0× | 32.1 MB |

## tpl-OASIS30ANTs_res-01_T1w (216×291×256 float32, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 33.1 ms | 32.8 ms | 1,947 | 1.3× |  |
| read | larmorx (threads=4) | 33.1 ms | 32.9 ms | 1,947 | 1.3× |  |
| read | larmorx (threads=all) | 33.4 ms | 33.0 ms | 1,924 | 1.3× |  |
| read | nibabel | 43.6 ms | 42.1 ms | 1,476 | 1.0× |  |
| read | SimpleITK | 101.5 ms | 96.9 ms | 634 | 0.4× |  |
| read float32 | larmorx | 34.5 ms | 34.2 ms | 1,865 | 1.2× |  |
| read float32 | nibabel | 42.8 ms | 42.3 ms | 1,505 | 1.0× |  |
| write | larmorx (threads=1) | 52.5 ms | 51.3 ms | 1,227 | 1.3× | 64.4 MB |
| write | larmorx (threads=4) | 54.1 ms | 51.6 ms | 1,189 | 1.3× | 64.4 MB |
| write | larmorx (threads=all) | 55.6 ms | 49.5 ms | 1,158 | 1.2× | 64.4 MB |
| write | nibabel | 69.1 ms | 67.8 ms | 932 | 1.0× | 64.4 MB |
| write | SimpleITK | 56.6 ms | 55.0 ms | 1,137 | 1.2× | 64.4 MB |

## sub-01_task-mixedgamblestask_run-03_bold (64×64×34×240 int16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 260.2 ms | 257.2 ms | 257 | 1.5× |  |
| read | larmorx (threads=4) | 246.2 ms | 244.0 ms | 271 | 1.6× |  |
| read | larmorx (threads=all) | 252.7 ms | 252.2 ms | 265 | 1.6× |  |
| read | nibabel | 401.0 ms | 396.5 ms | 167 | 1.0× |  |
| read | SimpleITK | 294.8 ms | 293.8 ms | 227 | 1.4× |  |
| read float32 | larmorx | 305.3 ms | 276.2 ms | 219 | 1.4× |  |
| read float32 | nibabel | 436.9 ms | 424.4 ms | 153 | 1.0× |  |
| write | larmorx (threads=1) | 1.09 s | 1.05 s | 62 | 1.2× | 39.2 MB |
| write | larmorx (threads=4) | 380.4 ms | 371.8 ms | 176 | 3.4× | 39.2 MB |
| write | larmorx (threads=all) | 264.9 ms | 264.2 ms | 252 | 4.9× | 39.2 MB |
| write | nibabel | 1.31 s | 1.30 s | 51 | 1.0× | 39.4 MB |
| write | SimpleITK | 1.68 s | 1.65 s | 40 | 0.8× | 38.3 MB |
| write (gzip level 6) | larmorx (threads=all) | 382.8 ms | 380.1 ms | 175 | 13.6× | 38.3 MB |
| write (gzip level 6) | nibabel | 5.19 s | 5.13 s | 13 | 1.0× | 38.9 MB |

## sub-01_task-mixedgamblestask_run-03_bold (64×64×34×240 int16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 35.0 ms | 34.8 ms | 1,912 | 1.3× |  |
| read | larmorx (threads=4) | 34.5 ms | 34.4 ms | 1,936 | 1.3× |  |
| read | larmorx (threads=all) | 34.2 ms | 34.2 ms | 1,952 | 1.3× |  |
| read | nibabel | 44.8 ms | 43.5 ms | 1,491 | 1.0× |  |
| read | SimpleITK | 70.1 ms | 69.9 ms | 954 | 0.6× |  |
| read float32 | larmorx | 70.7 ms | 64.1 ms | 946 | 0.9× |  |
| read float32 | nibabel | 62.8 ms | 62.6 ms | 1,065 | 1.0× |  |
| write | larmorx (threads=1) | 57.5 ms | 56.8 ms | 1,163 | 1.0× | 66.8 MB |
| write | larmorx (threads=4) | 53.8 ms | 52.6 ms | 1,243 | 1.0× | 66.8 MB |
| write | larmorx (threads=all) | 58.3 ms | 54.8 ms | 1,147 | 1.0× | 66.8 MB |
| write | nibabel | 55.7 ms | 54.7 ms | 1,201 | 1.0× | 66.8 MB |
| write | SimpleITK | 59.5 ms | 51.4 ms | 1,123 | 0.9× | 66.8 MB |

## sub-206_task-category_run-01_bold (96×96×69×113 uint16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 513.7 ms | 493.1 ms | 280 | 1.4× |  |
| read | larmorx (threads=4) | 507.5 ms | 494.2 ms | 283 | 1.4× |  |
| read | larmorx (threads=all) | 503.1 ms | 500.5 ms | 286 | 1.4× |  |
| read | nibabel | 727.2 ms | 723.8 ms | 198 | 1.0× |  |
| read | SimpleITK | 640.5 ms | 597.5 ms | 224 | 1.1× |  |
| read float32 | larmorx | 570.5 ms | 556.5 ms | 252 | 1.4× |  |
| read float32 | nibabel | 782.2 ms | 775.8 ms | 184 | 1.0× |  |
| write | larmorx (threads=1) | 3.18 s | 3.15 s | 45 | 1.3× | 112.4 MB |
| write | larmorx (threads=4) | 1.11 s | 1.10 s | 129 | 3.7× | 112.4 MB |
| write | larmorx (threads=all) | 726.3 ms | 720.7 ms | 198 | 5.7× | 112.4 MB |
| write | nibabel | 4.14 s | 4.14 s | 35 | 1.0× | 115.5 MB |
| write | SimpleITK | 3.66 s | 3.60 s | 39 | 1.1× | 112.1 MB |
| write (gzip level 6) | larmorx (threads=all) | 949.3 ms | 910.6 ms | 151 | 9.1× | 112.1 MB |
| write (gzip level 6) | nibabel | 8.63 s | 8.44 s | 17 | 1.0× | 113.8 MB |

## sub-206_task-category_run-01_bold (96×96×69×113 uint16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 74.2 ms | 73.4 ms | 1,937 | 1.3× |  |
| read | larmorx (threads=4) | 74.5 ms | 74.3 ms | 1,928 | 1.3× |  |
| read | larmorx (threads=all) | 73.5 ms | 73.2 ms | 1,955 | 1.3× |  |
| read | nibabel | 94.2 ms | 94.2 ms | 1,525 | 1.0× |  |
| read | SimpleITK | 145.8 ms | 145.1 ms | 986 | 0.6× |  |
| read float32 | larmorx | 138.5 ms | 124.8 ms | 1,038 | 1.0× |  |
| read float32 | nibabel | 136.5 ms | 133.5 ms | 1,053 | 1.0× |  |
| write | larmorx (threads=1) | 137.2 ms | 134.9 ms | 1,048 | 1.1× | 143.7 MB |
| write | larmorx (threads=4) | 137.2 ms | 121.8 ms | 1,047 | 1.1× | 143.7 MB |
| write | larmorx (threads=all) | 135.7 ms | 134.4 ms | 1,059 | 1.1× | 143.7 MB |
| write | nibabel | 144.6 ms | 138.9 ms | 994 | 1.0× | 143.7 MB |
| write | SimpleITK | 137.1 ms | 132.1 ms | 1,048 | 1.1× | 143.7 MB |

## sub-f1031ax_ses-wave1bas_task-Cuedts_acq-mb4AP_run-1_bold (90×90×60×650 int16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 2.63 s | 2.62 s | 240 | 1.2× |  |
| read | larmorx (threads=4) | 2.64 s | 2.64 s | 239 | 1.2× |  |
| read | larmorx (threads=all) | 2.62 s | 2.59 s | 242 | 1.2× |  |
| read | nibabel | 3.25 s | 3.19 s | 194 | 1.0× |  |
| read | SimpleITK | 3.04 s | 3.01 s | 208 | 1.1× |  |
| read float32 | larmorx | 2.95 s | 2.94 s | 214 | 1.2× |  |
| read float32 | nibabel | 3.43 s | 3.39 s | 184 | 1.0× |  |
| write | larmorx (threads=1) | 14.07 s | 13.14 s | 45 | 1.2× | 465.4 MB |
| write | larmorx (threads=4) | 5.19 s | 5.02 s | 122 | 3.4× | 465.4 MB |
| write | larmorx (threads=all) | 3.67 s | 3.66 s | 172 | 4.8× | 465.4 MB |
| write | nibabel | 17.56 s | 17.20 s | 36 | 1.0× | 472.0 MB |
| write | SimpleITK | 15.41 s | 15.37 s | 41 | 1.1× | 462.5 MB |
| write (gzip level 6) | larmorx (threads=all) | 3.86 s | 3.81 s | 164 | 10.8× | 462.6 MB |
| write (gzip level 6) | nibabel | 41.55 s | 41.50 s | 15 | 1.0× | 465.7 MB |

## sub-f1031ax_ses-wave1bas_task-Cuedts_acq-mb4AP_run-1_bold (90×90×60×650 int16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 298.0 ms | 296.8 ms | 2,120 | 1.4× |  |
| read | larmorx (threads=4) | 295.6 ms | 293.9 ms | 2,138 | 1.4× |  |
| read | larmorx (threads=all) | 296.0 ms | 294.3 ms | 2,134 | 1.4× |  |
| read | nibabel | 407.1 ms | 399.9 ms | 1,552 | 1.0× |  |
| read | SimpleITK | 655.8 ms | 642.3 ms | 963 | 0.6× |  |
| read float32 | larmorx | 600.3 ms | 582.5 ms | 1,053 | 1.0× |  |
| read float32 | nibabel | 578.6 ms | 569.7 ms | 1,092 | 1.0× |  |
| write | larmorx (threads=1) | 585.3 ms | 575.5 ms | 1,079 | 1.1× | 631.8 MB |
| write | larmorx (threads=4) | 583.6 ms | 566.6 ms | 1,083 | 1.1× | 631.8 MB |
| write | larmorx (threads=all) | 581.2 ms | 571.9 ms | 1,087 | 1.1× | 631.8 MB |
| write | nibabel | 629.6 ms | 620.0 ms | 1,004 | 1.0× | 631.8 MB |
| write | SimpleITK | 587.9 ms | 574.3 ms | 1,075 | 1.1× | 631.8 MB |

## sub-16_task-rest_bold (160×160×96×280 int16, .nii.gz)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 3.91 s | 3.88 s | 352 | 1.5× |  |
| read | larmorx (threads=4) | 3.91 s | 3.85 s | 352 | 1.5× |  |
| read | larmorx (threads=all) | 3.96 s | 3.91 s | 348 | 1.5× |  |
| read | nibabel | 5.74 s | 5.72 s | 240 | 1.0× |  |
| read | SimpleITK | 4.71 s | 4.64 s | 292 | 1.2× |  |
| read float32 | larmorx | 4.57 s | 4.56 s | 301 | 1.3× |  |
| read float32 | nibabel | 6.10 s | 6.07 s | 226 | 1.0× |  |
| write | larmorx (threads=1) | 18.67 s | 18.59 s | 74 | 1.3× | 645.2 MB |
| write | larmorx (threads=4) | 6.99 s | 6.07 s | 197 | 3.5× | 645.2 MB |
| write | larmorx (threads=all) | 4.67 s | 4.67 s | 295 | 5.2× | 645.2 MB |
| write | nibabel | 24.40 s | 23.50 s | 56 | 1.0× | 645.9 MB |
| write | SimpleITK | 42.10 s | 41.53 s | 33 | 0.6× | 598.0 MB |
| write (gzip level 6) | larmorx (threads=all) | 8.72 s | 8.71 s | 158 | 11.7× | 597.9 MB |
| write (gzip level 6) | nibabel | 102.38 s | 100.92 s | 13 | 1.0× | 605.6 MB |

## sub-16_task-rest_bold (160×160×96×280 int16, .nii)

| Operation | Tool | Median | Best | MB/s | Speed-up vs nibabel | Output |
|---|---|---|---|---|---|---|
| read | larmorx (threads=1) | 628.0 ms | 623.5 ms | 2,192 | 1.4× |  |
| read | larmorx (threads=4) | 685.2 ms | 640.2 ms | 2,009 | 1.3× |  |
| read | larmorx (threads=all) | 613.0 ms | 608.0 ms | 2,245 | 1.4× |  |
| read | nibabel | 881.6 ms | 879.6 ms | 1,561 | 1.0× |  |
| read | SimpleITK | 1.38 s | 1.38 s | 994 | 0.6× |  |
| read float32 | larmorx | 1.34 s | 1.29 s | 1,029 | 0.9× |  |
| read float32 | nibabel | 1.26 s | 1.26 s | 1,088 | 1.0× |  |
| write | larmorx (threads=1) | 3.42 s | 3.35 s | 402 | 1.0× | 1376.3 MB |
| write | larmorx (threads=4) | 3.27 s | 3.19 s | 420 | 1.0× | 1376.3 MB |
| write | larmorx (threads=all) | 3.28 s | 2.96 s | 420 | 1.0× | 1376.3 MB |
| write | nibabel | 3.39 s | 3.34 s | 406 | 1.0× | 1376.3 MB |
| write | SimpleITK | 1.27 s | 1.27 s | 1,081 | 2.7× | 1376.3 MB |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (4fe591cf4057) |
| larmorx-testdata | db846de5f34a |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
| SimpleITK | 2.5.6 |
