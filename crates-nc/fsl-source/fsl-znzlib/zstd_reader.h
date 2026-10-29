/**
 * I/O support for zstandard compression.
 *
 * This module defines the zstdReader class, for reading/decompressing
 * zstandard-compressed files.
 */

#ifndef ZSTD_READER_H
#define ZSTD_READER_H

#include <stdio.h>
#include <vector>

extern "C" {
#include "zstd.h"
}

namespace znz {

  /**
   * The zstdReader class provides simple I/O functionality for reading
   * zstandard-compressed files. It is primarily intended for one-shot
   * sequential reads, but provides very primitive support for seeking.
   *
   * Every call to seek(pos, SEEK_SET) will cause the file to be re-opened
   * and read/decompressed from the beginning.
   */
  class zstdReader : public znzPlugin {
  public:

    zstdReader();
    ~zstdReader();

    // Re-initialised an existing zstdReader to read from the given path.
    virtual void open(std::string path, std::string mode) override;

    // Close the path and clear the zstd structures. This effectively clears
    // the state of the zstdReader.
    virtual void close() override;

    // Read nbytes uncompressed bytes from the file, and copy them into buf.
    // Returns the number of bytes that were read.
    virtual size_t read(void* buf, size_t nbytes) override;

    virtual size_t write(const void* buf, size_t nbytes) override {
      throw znzError("Not implemented");
    }

    // Seek to the given location in the uncompressed stream. Using SEEK_SET
    // causes the file to be re-opened and re-read from the beginning.
    virtual void seek(size_t pos, int whence) override;

    // Seek to the start of the stream
    virtual void rewind() override;

    // Return true if all of the input data has been read, false otherwise.
    virtual bool eof() override;

  private:

    // Read data from the file and copy it into the input buffer.
    void _read_from_file();

    // Remove bytes from the output buffer, and shift any remaining bytes to
    // the beginning.
    void _shift_output_buffer(size_t copied);

    // File path
    std::string _path;

    // File handle
    FILE* _file;

    // Zstd stream
    ZSTD_DStream* _stream;

    // Zstd compressed input buffer struct, populated from the file
    ZSTD_inBuffer _zin;

    // Zstd decompressed output buffer struct.
    ZSTD_outBuffer _zout;

    // Input/output buffers. These are created and persisted for the lifetime
    // of a zstdReader instance. They are typically only ~100kb each.
    std::vector<char> _in_buf;
    std::vector<char> _out_buf;
  };

} // namespace znz

#endif // ZSTD_READER_H
