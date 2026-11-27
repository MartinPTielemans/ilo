#ifndef BROWSER_CORE_UTILITY_CLIENT_H_
#define BROWSER_CORE_UTILITY_CLIENT_H_

#include "chrome/utility/chrome_content_utility_client.h"  // nogncheck

namespace browser_core {

// Chrome's utility client, which the core's MainDelegate gives every utility process, with the
// core's own services added to Chrome's: so they run sandboxed apart from the browser's process
// with no engine patch.
class UtilityClient : public ChromeContentUtilityClient {
 public:
  UtilityClient();
  UtilityClient(const UtilityClient&) = delete;
  UtilityClient& operator=(const UtilityClient&) = delete;
  ~UtilityClient() override;

  // content::ContentUtilityClient:
  void RegisterMainThreadServices(mojo::ServiceFactory& services) override;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_UTILITY_CLIENT_H_
