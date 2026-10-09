"""larmorx.Image and NIfTI reading/writing (no external test data needed)."""

import gzip
import logging

import numpy as np
import pytest

import larmorx as lx
from larmorx.io import NiftiError, NiftiHeader

DTYPES = [
    "uint8",
    "int8",
    "uint16",
    "int16",
    "uint32",
    "int32",
    "uint64",
    "int64",
    "float32",
    "float64",
    "complex64",
    "complex128",
]
SUFFIXES = [".nii", ".nii.gz", ".hdr", ".img.gz"]


def oblique_affine() -> np.ndarray:
    c, s = np.cos(0.3), np.sin(0.3)
    a = np.array(
        [[2 * c, 0, -3 * s, -80.0], [0, 2.5, 0, 110.0], [2 * s, 0, 3 * c, -50.0], [0, 0, 0, 1]]
    )
    return a


def sample(dtype: str, shape=(4, 5, 6)) -> np.ndarray:
    rng = np.random.default_rng(0)
    values = rng.standard_normal(int(np.prod(shape))) * 100
    if np.dtype(dtype).kind == "c":
        values = values + 1j * values[::-1]
    return np.asarray(values.reshape(shape), dtype=dtype)


# --- Image --------------------------------------------------------------------------------------


def test_image_validates_and_freezes_its_affine():
    img = lx.Image(np.zeros((2, 3, 4), np.float32), np.eye(4))
    assert img.affine.dtype == np.float64
    assert not img.affine.flags.writeable
    assert img.shape == (2, 3, 4) and img.ndim == 3 and img.dtype == np.float32
    assert img.voxel_sizes == (1.0, 1.0, 1.0)
    assert img.header is None and img.tr is None
    assert "shape=(2, 3, 4)" in repr(img)
    with pytest.raises(ValueError, match="4x4"):
        lx.Image(np.zeros(3), np.eye(3))
    with pytest.raises(ValueError, match="dimensions"):
        lx.Image(np.zeros((1,) * 8), np.eye(4))
    with pytest.raises(AttributeError):
        img.data = np.zeros(1)  # frozen


def test_with_data_and_replace_keep_the_rest():
    img = lx.Image(np.zeros((2, 2, 2)), oblique_affine())
    doubled = img.with_data(np.ones((2, 2, 2)))
    assert np.array_equal(doubled.affine, img.affine) and doubled.data.sum() == 8
    moved = img.replace(affine=np.eye(4))
    assert np.array_equal(moved.affine, np.eye(4))


def test_as_image_accepts_paths_pairs_and_nibabel(tmp_path):
    data, aff = sample("float32"), oblique_affine()
    assert lx.as_image(lx.Image(data, aff)).shape == data.shape
    pair = lx.as_image((data, aff))
    assert np.array_equal(pair.data, data)
    path = tmp_path / "x.nii.gz"
    lx.save(pair, path)
    assert np.array_equal(lx.as_image(path).data, data)
    assert np.array_equal(lx.as_image(str(path)).data, data)
    nib = pytest.importorskip("nibabel")
    from_nib = lx.as_image(nib.load(path))
    assert np.array_equal(from_nib.data, data)
    assert from_nib.header is not None and from_nib.header.sform_code == 2
    with pytest.raises(TypeError):
        lx.as_image(42)


# --- reading and writing ----------------------------------------------------------------------------


@pytest.mark.parametrize("suffix", SUFFIXES)
@pytest.mark.parametrize("dtype", DTYPES)
def test_round_trip_every_type_and_layout(tmp_path, dtype, suffix):
    data = sample(dtype)
    path = tmp_path / f"img{suffix}"
    lx.save(lx.Image(data, oblique_affine()), path)
    back = lx.load(path)
    assert back.dtype == data.dtype
    np.testing.assert_array_equal(back.data, data)
    np.testing.assert_allclose(back.affine, oblique_affine(), atol=1e-4)  # float32 storage
    assert back.data.flags.f_contiguous


@pytest.mark.parametrize("version", [1, 2])
def test_versions_and_big_endian_templates(tmp_path, version):
    data = sample("int16", (3, 4, 5, 6))
    path = tmp_path / "v.nii"
    lx.save(lx.Image(data, np.eye(4)), path, version=version)
    img = lx.load(path)
    assert img.header.version == version
    big = img.replace(header=img.header.replace(byte_order=">"))
    lx.save(big, tmp_path / "be.nii.gz")
    back = lx.load(tmp_path / "be.nii.gz")
    assert back.header.byte_order == ">"
    assert back.data.dtype.isnative  # values come back in native byte order
    np.testing.assert_array_equal(back.data, data)


def test_save_converts_what_nifti_cannot_store(tmp_path):
    path = tmp_path / "x.nii"
    lx.save((np.array([[[True, False]]]), np.eye(4)), path)
    assert lx.load(path).dtype == np.uint8
    big_endian = np.arange(8, dtype=">f4").reshape(2, 2, 2)
    lx.save((big_endian, np.eye(4)), path)
    np.testing.assert_array_equal(lx.load(path).data, big_endian)
    with pytest.raises(TypeError, match="float16"):
        lx.save((np.zeros((2, 2, 2), np.float16), np.eye(4)), path)
    lx.save((np.arange(8.0).reshape(2, 2, 2), np.eye(4)), path, dtype=np.int16)
    assert lx.load(path).dtype == np.int16


def test_non_contiguous_arrays_are_written_correctly(tmp_path):
    base = sample("float64", (6, 7, 8))
    views = [base[::2, :, 1:], base.transpose(2, 0, 1), np.ascontiguousarray(base)]
    for i, view in enumerate(views):
        path = tmp_path / f"v{i}.nii"
        lx.save((view, np.eye(4)), path)
        np.testing.assert_array_equal(lx.load(path).data, view)


def test_compressed_output_does_not_depend_on_threads(tmp_path):
    data = sample("float32", (64, 64, 40))
    outputs = []
    for n in (1, 3, 0):
        path = tmp_path / f"t{n}.nii.gz"
        lx.save((data, np.eye(4)), path, n_threads=n)
        outputs.append(path.read_bytes())
    assert outputs[0] == outputs[1] == outputs[2]
    assert gzip.decompress(outputs[0])[352:] == np.asfortranarray(data).tobytes(order="F")


def test_compression_level(tmp_path):
    data = np.zeros((32, 32, 32), np.int16)
    sizes = {}
    for level in (0, 1, 9):
        path = tmp_path / f"l{level}.nii.gz"
        lx.save((data, np.eye(4)), path, compression_level=level)
        sizes[level] = path.stat().st_size
    assert sizes[0] > sizes[1] >= sizes[9]


# --- scaling and read modes -------------------------------------------------------------------


def scaled_file(tmp_path):
    path = tmp_path / "s.nii"
    stored = np.arange(-6, 6, dtype=np.int16).reshape(2, 3, 2)
    img = lx.Image(stored, np.eye(4))
    lx.save(img, path)
    header = lx.load(path).header.replace(scl_slope=0.5, scl_inter=10.0)
    lx.io.nifti._core.nifti_write(str(path), stored, header.to_dict())
    return path, stored


def test_read_modes(tmp_path):
    path, stored = scaled_file(tmp_path)
    expected = stored * 0.5 + 10.0
    assert lx.load(path).dtype == np.float64
    np.testing.assert_array_equal(lx.load(path).data, expected)
    np.testing.assert_array_equal(lx.load(path, dtype=np.float32).data, expected.astype(np.float32))
    raw = lx.load(path, scaled=False)
    assert raw.dtype == np.int16
    np.testing.assert_array_equal(raw.data, stored)
    with pytest.raises(ValueError):
        lx.load(path, dtype=np.float32, scaled=False)
    with pytest.raises(ValueError):
        lx.load(path, dtype=np.int16)


def test_saving_scaled_data_stores_values_unscaled(tmp_path):
    path, _ = scaled_file(tmp_path)
    img = lx.load(path)
    out = tmp_path / "out.nii"
    lx.save(img, out)
    again = lx.load(out)
    assert again.header.slope_inter == (1.0, 0.0)
    np.testing.assert_array_equal(again.data, img.data)


# --- headers ----------------------------------------------------------------------------------------


def test_header_properties(tmp_path):
    path = tmp_path / "bold.nii.gz"
    lx.save(lx.Image(np.zeros((4, 4, 3, 5), np.int16), np.diag([2, 2, 3, 1.0])), path)
    header = lx.load(path).header.replace(
        xyzt_units=2 | 16, pixdim=(1, 2, 2, 3, 800.0, 1, 1, 1), descrip=b"hello"
    )
    assert header.spatial_unit == "mm" and header.time_unit == "msec"
    assert header.tr == pytest.approx(0.8)
    assert header.description == "hello"
    assert header.shape == (4, 4, 3, 5)
    assert header.zooms == (2.0, 2.0, 3.0, 800.0)
    assert header.data_dtype == np.dtype("int16")
    assert header.is_single_file
    np.testing.assert_array_equal(header.affine, np.diag([2, 2, 3, 1.0]))
    assert NiftiHeader.from_dict(header.to_dict()).to_bytes() == header.to_bytes()


def test_save_keeps_header_metadata_and_codes(tmp_path):
    path = tmp_path / "a.nii"
    lx.save(lx.Image(np.zeros((3, 3, 3, 2), np.float32), oblique_affine()), path)
    img = lx.load(path)
    tagged = img.replace(
        header=img.header.replace(sform_code=4, qform_code=1, xyzt_units=10, descrip=b"keep me")
    )
    lx.save(tagged, tmp_path / "b.nii")
    back = lx.load(tmp_path / "b.nii").header
    assert (back.sform_code, back.qform_code, back.xyzt_units, back.description) == (
        4,
        1,
        10,
        "keep me",
    )
    # Changing the affine resets the codes to nibabel's defaults (sform aligned, qform unknown).
    lx.save(tagged.replace(affine=np.eye(4)), tmp_path / "c.nii")
    moved = lx.load(tmp_path / "c.nii").header
    assert (moved.sform_code, moved.qform_code, moved.description) == (2, 0, "keep me")


def test_extensions_survive(tmp_path):
    path = tmp_path / "e.nii"
    lx.save(lx.Image(np.zeros((2, 2, 2), np.uint8), np.eye(4)), path)
    img = lx.load(path)
    ext = lx.io.Extension(6, b"comment")
    lx.save(img.replace(header=img.header.replace(extensions=(ext,))), path)
    back = lx.load(path).header.extensions
    assert len(back) == 1 and back[0].code == 6 and back[0].trimmed_content == b"comment"


def test_read_header_is_raw_but_load_fixes(tmp_path, caplog):
    path = tmp_path / "f.nii"
    lx.save(lx.Image(np.zeros((2, 2, 2), np.float32), np.eye(4)), path)
    raw = bytearray(path.read_bytes())
    raw[76:80] = np.float32(0).tobytes()  # qfac = 0
    path.write_bytes(bytes(raw))
    assert lx.io.read_header(path).pixdim[0] == 0.0
    with caplog.at_level(logging.WARNING, logger="larmorx.io"):
        assert lx.load(path).header.pixdim[0] == 1.0
    assert "qfac" in caplog.text


# --- errors ----------------------------------------------------------------------------------------


def test_errors(tmp_path):
    with pytest.raises(FileNotFoundError):
        lx.load(tmp_path / "missing.nii")
    with pytest.raises(ValueError, match="NIfTI file name"):
        lx.load(tmp_path / "x.mgz")
    bad = tmp_path / "bad.nii"
    bad.write_bytes(b"not an image" * 40)
    with pytest.raises(NiftiError):
        lx.load(bad)
    assert issubclass(NiftiError, ValueError)


# --- nibabel interoperability -----------------------------------------------------------------------


def test_nibabel_reads_what_larmorx_writes_and_back(tmp_path):
    nib = pytest.importorskip("nibabel")
    data = sample("float32", (5, 6, 7, 3))
    path = tmp_path / "x.nii.gz"
    lx.save(lx.Image(data, oblique_affine()), path)
    n = nib.load(path)
    np.testing.assert_array_equal(np.asanyarray(n.dataobj), data)
    np.testing.assert_allclose(n.affine, oblique_affine(), atol=1e-4)
    converted = lx.load(path).to_nibabel()
    np.testing.assert_array_equal(np.asanyarray(converted.dataobj), data)
    assert int(converted.header["sform_code"]) == 2


# --- memory mapping -------------------------------------------------------------------------------


@pytest.mark.parametrize("suffix", [".nii", ".hdr"])
def test_mmap_maps_eligible_files(tmp_path, suffix):
    data = sample("float32", (5, 6, 7, 2))
    path = tmp_path / f"m{suffix}"
    lx.save(lx.Image(data, oblique_affine()), path)
    img = lx.load(path, mmap=True)
    assert isinstance(img.data.base, np.memmap) or isinstance(img.data, np.memmap)
    np.testing.assert_array_equal(img.data, data)
    np.testing.assert_array_equal(img.affine, lx.load(path).affine)
    img.data[0, 0, 0, 0] = 42  # copy-on-write: the file is unchanged
    np.testing.assert_array_equal(lx.load(path).data, data)


def test_mmap_falls_back_for_compressed_and_scaled_files(tmp_path):
    data = sample("int16")
    gz = tmp_path / "c.nii.gz"
    lx.save(lx.Image(data, np.eye(4)), gz)
    assert not isinstance(lx.load(gz, mmap=True).data.base, np.memmap)
    path, stored = scaled_file(tmp_path)
    scaled = lx.load(path, mmap=True)
    assert scaled.dtype == np.float64  # scaled: read normally
    raw = lx.load(path, mmap=True, scaled=False)
    np.testing.assert_array_equal(raw.data, stored)
