// Saved reads (bc_profile_read, #3820): one GET on a profile's own session, with no tab, apart
// from the profile so browser_core_unittests drives it with a test loader factory or a network
// context of its own. profile_reads.cc gives it each profile's.

#ifndef BROWSER_CORE_SAVED_READ_H_
#define BROWSER_CORE_SAVED_READ_H_

#include <cstddef>
#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/functional/callback.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "url/gurl.h"

namespace net {
struct RedirectInfo;
}

namespace network {
class SharedURLLoaderFactory;
class SimpleURLLoader;
namespace mojom {
class URLResponseHead;
}
}  // namespace network

namespace browser_core {

// What a read came to.
struct SavedReadAnswer {
  // The HTTP status, or 0 if none came.
  int status = 0;
  // The address that answered, after redirects on the asked address's own origin.
  std::string url;
  // A redirect to another origin, which the read told and didn't follow, or empty.
  std::string redirect;
  // The body's MIME type, or empty.
  std::string type;
  // The body, its first part with |cut|.
  std::string body;
  bool cut = false;
  // Why it wasn't sent or broke off, or empty.
  std::string error;
};

// The reads under way on one session, which end with it: destroyed, it ends each with no answer,
// as the session it read on has gone.
class SavedReads {
 public:
  using Done = base::OnceCallback<void(const SavedReadAnswer&)>;

  explicit SavedReads(scoped_refptr<network::SharedURLLoaderFactory> factory);
  SavedReads(const SavedReads&) = delete;
  SavedReads& operator=(const SavedReads&) = delete;
  ~SavedReads();

  // Reads |url| with one GET, as the site's own page would read it: with the session's cookies for
  // it, the site as the first party, nothing from the HTTP cache and nothing put in it. A redirect
  // on |url|'s origin is followed; one to another origin is told, not followed. |done| runs once,
  // never inside this call, with the first |max_bytes| of the body (cut past them), or after
  // |timeout_ms| with what came, each no more than the most (saved_read_rule.h). The caller sets
  // nothing of the request: no method, header or body.
  void Read(const GURL& url, size_t max_bytes, int64_t timeout_ms, Done done);

  // Ends each read under way with |why|, from a task of its own, and sends nothing more for them.
  void EndAll(const std::string& why);

  // How many reads are under way.
  size_t Reading() const { return reads_.size(); }

 private:
  // A read under way.
  struct Under {
    Under();
    Under(Under&&);
    Under& operator=(Under&&);
    ~Under();

    GURL asked;
    std::unique_ptr<network::SimpleURLLoader> loader;
    Done done;
  };

  void Redirected(uint64_t read,
                  const GURL& from,
                  const net::RedirectInfo& redirect,
                  const network::mojom::URLResponseHead& head,
                  std::vector<std::string>* removed_headers);
  void Answered(uint64_t read, std::optional<std::string> body);

  scoped_refptr<network::SharedURLLoaderFactory> factory_;
  std::map<uint64_t, Under> reads_;
  uint64_t last_ = 0;
  base::WeakPtrFactory<SavedReads> weak_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SAVED_READ_H_
