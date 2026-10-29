/**
 * I/O support for zstandard compression.
 *
 * This module defines the zstdWriter class, for writing/compressing
 * zstandard-compressed files.
 *
 * Useful references:
 *  - https://facebook.github.io/zstd/doc/api_manual_latest.html#Chapter8
 *  - https://github.com/facebook/zstd/blob/dev/examples/streaming_compression.c
 */

#include <stdio.h>
#include <string>
#include <vector>

extern "C" {
#include "zstd.h"
}

#include "znzlib.h"
#include "zstd_writer.h"


namespace znz {


  zstdWriter::zstdWriter()
    : _file(nullptr),
      _stream(nullptr)
  {}

  zstdWriter::~zstdWriter() {close();}


  // Re-initialise the zstdWriter to write to the given path.
  void zstdWriter::open(std::string path, std::string mode) {

    close();

    if (mode != "wb") {
      throw znzError("The zstdWriter only supports open mode 'wb'");
    }

    // TODO compression settings

    _file   = std::fopen(path.c_str(), mode.c_str());
    _stream = ZSTD_createCStream();

    if (_file == nullptr || _stream == nullptr) {
      close();
      throw znzError("Unable to open " + path);
    }

  }

  // Close the file handle and clear the zstd structs.
  void zstdWriter::close() {
    if (_stream != nullptr) ZSTD_freeCStream(_stream);
    if (_file   != nullptr) fclose(_file);
    _file   = nullptr;
    _stream = nullptr;
  }

  // Compress and write <nbytes> bytes from buf to file.
  size_t zstdWriter::write(const void* buf, size_t nbytes) {

    char*  cbuf     = (char*)buf;
    size_t to_write = nbytes;

    std::vector<char> in_buf( ZSTD_CStreamInSize());
    std::vector<char> out_buf(ZSTD_CStreamOutSize());

    // compress up to <in_buf.size()> chunks of bytes at a time
    while (to_write > 0) {

      // figure out how many bytes to copy
      // in this iteration, and copy them
      // into the zstd input buffer
      size_t chunk_size = std::min(to_write, in_buf.size());
      std::copy(cbuf, cbuf + chunk_size, in_buf.begin());

      // keep going until the input buffer is empty
      ZSTD_inBuffer zin      = {&in_buf[0], chunk_size, 0};
      bool          finished = false;

      while (!finished) {

        // We need to tell zstd whether to expect
        // more input data (e_continue), or whether
        // this is the last chunk of bytes to write.
        bool              last_chunk = chunk_size == to_write;
        ZSTD_EndDirective mode       = last_chunk ? ZSTD_e_end : ZSTD_e_continue;

        ZSTD_outBuffer zout = {&out_buf[0], out_buf.size(), 0};

        size_t remaining = ZSTD_compressStream2(_stream, &zout, &zin, mode);

        if (ZSTD_isError(remaining)) {
          throw znzError("Error compressing data");
        }

        if (std::fwrite(zout.dst, 1, zout.pos, _file) != zout.pos) {
          throw znzError("Error writing to file");
        }

        to_write -= chunk_size;
        cbuf     += chunk_size;

        // If this is the last chunk, we're finished when
        // compressStream returns 0. Otherwise we're
        // finished with this chunk when the input buffer
        // is empty.
        if (last_chunk) finished = remaining == 0;
        else            finished = zin.pos == zin.size;
      }
    }
    return nbytes - to_write;
  }

}; // namespace znz
