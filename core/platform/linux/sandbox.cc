#include "browser_core/platform/sandbox.h"

#include "base/command_line.h"
#include "content/public/browser/zygote_host/zygote_host_linux.h"
#include "sandbox/policy/linux/sandbox_linux.h"
#include "sandbox/policy/switches.h"

namespace browser_core::platform {

// On Linux the sandbox has two layers, and either can be missing while the browser runs: the
// namespaces (the setuid helper's or user namespaces, which a Flatpak's zypak provides) and
// seccomp-bpf. Both, as chrome://sandbox reads the zygote's status.
bool Sandboxed() {
  using sandbox::policy::SandboxLinux;
  if (base::CommandLine::ForCurrentProcess()->HasSwitch(sandbox::policy::switches::kNoSandbox)) {
    return false;
  }
  const int status = content::ZygoteHost::GetInstance()->GetRendererSandboxStatus();
  if (status & SandboxLinux::kInvalid) {
    return false;
  }
  const bool namespaces =
      (status & SandboxLinux::kSUID) || (status & SandboxLinux::kUserNS) ||
      ((status & SandboxLinux::kPIDNS) && (status & SandboxLinux::kNetNS));
  return namespaces && (status & SandboxLinux::kSeccompBPF);
}

}  // namespace browser_core::platform
