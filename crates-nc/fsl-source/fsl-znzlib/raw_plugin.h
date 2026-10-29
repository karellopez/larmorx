/**
 * I/O support for raw (uncompressed) files.
 *
 * This module defines the rawPlugin class, which is just a wrapper around
 * a FILE* pointer and the standard file function.
 */

#ifndef RAW_PLUGIN_H
#define RAW_PLUGIN_H


#include <stdio.h>
#include <string>

#include "znzlib.h"


namespace znz {

  class rawPlugin : public znzPlugin {
  public:

    rawPlugin();
    ~rawPlugin();

    virtual void   open(std::string path, std::string mode) override;
    virtual void   close() override;
    virtual size_t read(void* buf, size_t nbytes) override;
    virtual size_t write(const void* buf, size_t nbytes) override;
    virtual void   seek(size_t pos, int whence) override;
    virtual void   rewind() override;
    virtual bool   eof() override;

  private:
    std::FILE* _file;
  };

} // namespace znz

#endif // RAW_PLUGIN_H
