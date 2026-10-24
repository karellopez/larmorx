#include <vector>
#include <cstdint>
#include <cstdlib>
#include <dlfcn.h>
#include <string>
#include "threading.h"


namespace Utilities {

  /*
   * Sub-function of fsl_init_blas_threading. Loads a shared
   * library, and calls a function to set the number of threads.
   */
  void _set_threads(
    std::vector<std::string>& libpaths,
    std::string               funcname) {

    /*
     * All errors (e.g. cannot find libblas.so) are
     * silently ignored.
     */

    /* Load the shared lib  */
    void *lib = NULL;
    for (auto path : libpaths) {
      lib = dlopen(path.c_str(), RTLD_LAZY);

      /*
       * clear any error messages, but ignore them - dlerror
       * may return an error message even if the library has
       * been successfully loaded.
       */
      dlerror();
      if (lib != NULL) {
        break;
      }
    }
    if (lib == NULL) {
      return;
    }

    /* get ref to the set_num_threads function */
    typedef void (*func)(int num_threads);
    func set_threads = (func)dlsym(lib, funcname.c_str());

    /* clear but ignore error messages */
    dlerror();
    if (set_threads == NULL) {
      return;
    }

    /* disable openblas threading */
    set_threads(1);

    /* close blas, and clear error in case dlclose fails */
    dlclose(lib);
    dlerror();
  }

  void fsl_init_blas_threading(uint16_t flags) {

    /*
     * Disable OpenBLAS and OpenMP threading.
     * We load the BLAS / omp libs dynamically,
     * and fail silently, to avoid errors if a
     * different BLAS implementation is in use,
     * or if OpenMP is not in use.
     */

    static std::vector<std::string> blaspaths = {
      "libblas.so",
      "libblas.dylib",
      "libcblas.so",
      "libcblas.dylib",
      "libopenblas.so",
      "libopenblas.dylib"
    };

    static std::vector<std::string> omppaths = {
      "libomp.dylib"
    };

    _set_threads(blaspaths, "openblas_set_num_threads");
    _set_threads(omppaths,  "omp_set_num_threads");
  }
} // namespace Utilities
