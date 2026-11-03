#include "browser_core/platform/app_library.h"

#include <dlfcn.h>

#include "base/files/file_path.h"
#include "base/logging.h"
#include "base/path_service.h"

namespace browser_core::platform {

const bc_host* LoadApp(const std::vector<std::string>& given,
                       std::vector<std::string>* switches) {
  base::FilePath dir;
  base::PathService::Get(base::DIR_EXE, &dir);
  const base::FilePath path = dir.Append(BC_APP_LIBRARY);
  // Resolved now, so a library built against another engine's C API fails here, by name, rather
  // than at its first call. It finds the API in chrome, which exports it (chrome.map).
  void* app = dlopen(path.value().c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!app) {
    // Read before LOG, whose timestamp calls localtime_r, which Chromium's libc interceptor
    // answers with a dlsym of localtime64_r that fails on x86-64 and replaces this error.
    const std::string error = dlerror();
    LOG(ERROR) << error;
    return nullptr;
  }
  auto entry = reinterpret_cast<bc_app_entry>(dlsym(app, BC_APP_ENTRY));
  if (!entry) {
    return nullptr;
  }
  std::vector<const char*> given_argv;
  for (const std::string& arg : given) {
    given_argv.push_back(arg.c_str());
  }
  int argc = static_cast<int>(given_argv.size());
  const char** argv = given_argv.data();
  const bc_host* host = entry(&argc, &argv);
  for (int i = 0; i < argc; ++i) {
    switches->push_back(argv[i]);
  }
  // The library stays loaded for as long as the process runs: Chromium calls into it until exit.
  return host;
}

}  // namespace browser_core::platform
