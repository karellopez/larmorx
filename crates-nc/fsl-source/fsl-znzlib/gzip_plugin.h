/**
 * I/O support for gzip-compressed files.
 *
 * This module defines the gzipPlugin class, which is just a wrapper around
 * a gzFile* pointer and the standard gzfile functions.
 */
#ifndef GZIP_PLUGIN_H
#define GZIP_PLUGIN_H

#include "zlib_wrapper.h"
#include "znzlib.h"

namespace znz {

  class gzipPlugin : public znzPlugin {
  public:

    gzipPlugin();
    ~gzipPlugin();

    virtual void open(std::string path, std::string mode) override;
    virtual void close() override;
    virtual size_t read(void* buf, size_t nbytes) override;
    virtual size_t write(const void* buf, size_t nbytes) override;
    virtual void seek(size_t pos, int whence) override;
    virtual void rewind() override;
    virtual bool eof() override;

  private:
    zlib::gzFile_s* _file;
  };
} // namespace znz

#endif // GZIP_PLUGIN_H
