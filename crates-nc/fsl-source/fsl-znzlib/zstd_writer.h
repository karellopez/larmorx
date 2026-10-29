/**
 * I/O support for zstandard compression.
 *
 * This module defines the zstdWriter class, for reading/decompressing
 * zstandard-compressed files.
 */

#ifndef ZSTD_WRITER_H
#define ZSTD_WRITER_H

#include <vector>

extern "C" {
#include "zstd.h"
}

namespace znz {

  /**
   * The zstdWriter class provides simple I/O functionality for writing
   * zstandard-compressed files.
   *
   * Only the open, write, and close functions are implemented. The other
   * functions will throw a znzError if called.
   */
  class zstdWriter : public znzPlugin {
  public:

    zstdWriter();
    ~zstdWriter();

    virtual void open(std::string path, std::string mode) override;

    virtual void close() override;

    virtual size_t read(void* buf, size_t nbytes) override {
      throw znzError("Not implemented");
    }

    virtual size_t write(const void* buf, size_t nbytes) override;

    virtual void seek(size_t pos, int whence) override {
        throw znzError("Not implemented");
    }

    virtual void rewind() override {
      throw znzError("Not implemented");
    }

    virtual bool eof() override {
      throw znzError("Not implemented");
    };

  private:
    // File handle
    FILE* _file;

    // ZSTD compression stream
    ZSTD_CStream* _stream;
  };

} // namespace znz

#endif // ZSTD_WRITER_H
