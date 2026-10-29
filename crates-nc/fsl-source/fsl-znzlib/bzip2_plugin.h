/**
 * I/O support for bzip2-compressed files.
 *
 * This module defines the bzip2Plugin class, which is just a wrapper around
 * a BZFILE* pointer and the standard gzfile functions.
 */
#ifndef BZIP2_PLUGIN_H
#define BZIP2_PLUGIN_H

#include <stdio.h>

extern "C" {
#include "bzlib.h"
}

#include "znzlib.h"

namespace znz {

  /**
   * The bzip2Plugin provides simple I/O functionality for bzip2-compressed
   * files.
   *
   * The seek, rewind, and eof functions are only supported on readable files -
   * if called on writeable files they will throw a znzError.
   *
   * For readable files, the seek/rewind functions will causethe file to be
   * re-opened and re-read from the beginning, so is only intended for one-shot
   * forward-seeking (e.g. skipping over file headers).
   */
  class bzip2Plugin : public znzPlugin {
  public:

    bzip2Plugin();
    ~bzip2Plugin();

    virtual void open(std::string path, std::string mode) override;
    virtual void close() override;
    virtual size_t read(void* buf, size_t nbytes) override;
    virtual size_t write(const void* buf, size_t nbytes) override;
    virtual void seek(size_t pos, int whence) override;
    virtual void rewind() override;
    virtual bool eof() override;

  private:
    std::string _path;
    std::string _mode;
    std::FILE*  _file;
    BZFILE*     _bz_file;

    // Used to keep track of
    // EOF status in read mode.
    bool        _at_eof;

  };
} // namespace znz

#endif // BZIP2_PLUGIN_H
