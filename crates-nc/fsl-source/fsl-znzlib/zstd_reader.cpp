/**
 * I/O support for zstandard compression.
 *
 * This module implements the zstdReader class, for reading/decompressing
 * zstandard-compressed files.
 *
 * Useful references:
 *  - https://facebook.github.io/zstd/doc/api_manual_latest.html#Chapter9
 *  - https://github.com/facebook/zstd/blob/dev/examples/streaming_decompression.c
 */
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <vector>

extern "C" {
#include "zstd.h"
}

#include "znzlib.h"
#include "zstd_reader.h"

namespace znz {

  // Create a zstdReader. The input/output buffers are initialised to a size
  // recommended by zstd.
  zstdReader::zstdReader()
    : _path(""),
      _file(nullptr),
      _stream(nullptr),
      _in_buf(ZSTD_DStreamInSize()),
      _out_buf(ZSTD_DStreamOutSize())
  {
    _zin  = {nullptr, 0, 0};
    _zout = {nullptr, 0, 0};
  }

  // Clear the state of this zstdReader.
  zstdReader::~zstdReader() {close();}

  // Open the path for reading. Any previous state is cleared, then the file
  // is opened and zstd structs are initialised.
  void zstdReader::open(std::string path, std::string mode) {

    close();

    if (mode != "rb") {
      throw znzError("The zstdReader only supports open mode 'rb'");
    }

    _path     = path;
    _file     = fopen(path.c_str(), "rb");
    _stream   = ZSTD_createDStream();
    _zin      = {&_in_buf[ 0], 0, 0};
    _zout     = {&_out_buf[0], _out_buf.size(), 0};

    if (_file == nullptr || _stream == nullptr) {
      close();
      throw znzError("Could not open file for reading: " + path);
    }

    if (ZSTD_isError(ZSTD_initDStream(_stream))) {
      close();
      throw znzError("Error initialising zstd decompressor");
    }
  }

  // Close the file handle and clear the zstd structs.
  void zstdReader::close() {
    if (_stream != nullptr) ZSTD_freeDStream(_stream);
    if (_file   != nullptr) fclose(_file);
    _file   = nullptr;
    _stream = nullptr;
    _zin    = {nullptr, 0, 0};
    _zout   = {nullptr, 0, 0};
  }

  // Read nbytes decompressed bytes from the file. Returns the number of bytes
  // that were read. The buf pointer may be nullptr, which causes the function to
  // effectively seek to the specified location in the uncompressed stream.
  size_t zstdReader::read(void* buf, size_t nbytes) {

    char*  cbuf    = (char*)buf;
    size_t to_read = nbytes;

    if (to_read == 0) {
      return 0;
    }

    // Consume any data remaining in the
    // output buffer from a previous read.
    if (_zout.pos > 0) {

      size_t to_copy = std::min(to_read, _zout.pos);
      to_read -= to_copy;

      if (cbuf != nullptr) {
        std::copy(_out_buf.begin(),
                  _out_buf.begin() + to_copy,
                  cbuf);
        cbuf += to_copy;
      }

      // Clear what we just copied, and shift
      // anything that still remains in the
      // output buffer to its beginning.
      _shift_output_buffer(to_copy);
    }

    // keep going until we've read enough
    while (to_read > 0) {

      // fill the input buffer if needed
      _read_from_file();

      // eof
      if (_zin.size == 0) {
        break;
      }

      // decompress while we have input data
      while (_zin.pos < _zin.size) {

        if (ZSTD_isError(ZSTD_decompressStream(_stream, &_zout, &_zin))) {
          throw znzError("Error decompressing data");
        }

        // Copy from output buffer into caller's buffer.
        // We may leave bytes in the output buffer, as
        // zstd may have decompressed more bytes than we
        // need.
        size_t to_copy = std::min(to_read, _zout.pos);

        if (cbuf != nullptr) {
          std::copy(_out_buf.begin(),
                    _out_buf.begin() + to_copy,
                    cbuf);
          cbuf += to_copy;
        }

        to_read -= to_copy;

        // Clear what we just copied from the output buffer.
        _shift_output_buffer(to_copy);

        // We've copied enough bytes.
        // Should never be less than 0.
        if (to_read <= 0)
          break;
      }
    }

    // Return the number of bytes that were read
    size_t num_read = nbytes - to_read;
    return num_read;
  }

  // Read compressed data from the file, and copy it into the input buffer.
  void zstdReader::_read_from_file() {

    size_t num_read;

    // There's already data in the input buffer - don't bother hitting the
    // file until the input buffer is empty.
    if (_zin.pos < _zin.size)
      return;

    // Fill the input buffer
    num_read = fread(&_in_buf[0], 1, _in_buf.size(), _file);

    if (num_read < 0) {
      throw znzError("Error reading from input file");
    }

    // update zstd state
    _zin.size = num_read;
    _zin.pos  = 0;
  }

  // Called by the read function after <copied> bytes have been consumed from
  // the output buffer.
  void zstdReader::_shift_output_buffer(size_t copied) {

    // The entire output buffer has been consumed -
    // just reset the pos offset and we're done.
    if (copied == _zout.size) {
      _zout.pos = 0;
      return;
    }

    // Any bytes that still remain are moved
    // to the front of the output buffer.
    size_t to_shift = _zout.pos - copied;
    std::copy(_out_buf.begin() + copied,
              _out_buf.begin() + _zout.pos,
              _out_buf.begin());
    _zout.pos = to_shift;
  }

  // "Seek" to the specified location in the uncompresesd stream. This causes
  // the file to be re-opened and decompressed from the beginning.
  void zstdReader::seek(size_t pos, int whence) {

    if (whence != SEEK_SET && whence != SEEK_CUR) {
      throw znzError("Not implemented");
    }

    if (whence == SEEK_SET) {
      open(_path, "rb");
    }

    read(nullptr, pos);
  }

  void zstdReader::rewind() {
    seek(0, SEEK_SET);
  }

  // Return true if the entire uncompressed stream has been read, false
  // otherwise.
  bool zstdReader::eof() {
    // We're at EOF if all of the following are true:
    //   - We've read everything from the file.
    //   - The input buffer is empty (entire file
    //     contents have been passed to zstd).
    //   - The output buffer is empty (entire
    //     decompressed stream has been passed to
    //     the read() caller).
    return ((_zin.size - _zin.pos) <= 0) &&
            (_zout.pos == 0)             &&
             std::feof(_file);
  }

}; // namespace znz
