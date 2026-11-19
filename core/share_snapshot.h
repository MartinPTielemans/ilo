// A shared link's snapshot (#680, public/browser_core.h, bc_share_make): what it holds, how it's
// sealed with the link's key and opened again, the caps on what the core takes from a server, and
// the page the core shows in the server's page's place. Apart so browser_core_unittests tests it
// alone; share.cc does the sharing, the asking and the adding.
//
// The format, which ilo-server's share page reads too:
//   link = <server>/s/<id>#<key>, the id 16 random bytes and the key 32, in unpadded base64url
//   blob = 0x01 || iv(12) || AES-256-GCM(key, iv, aad = "ilo share v1 " + id, deflate-raw(JSON))
// AES-GCM as the page has it in WebCrypto, which has no ChaCha; the id in the additional data, so a
// blob served under another id doesn't open. The key reaches the server only inside the link's
// entry in the account's list, sealed under a key derived from the account's secret (share.cc), so
// another of the account's devices can copy the link; the server reads neither blob nor key.

#ifndef BROWSER_CORE_SHARE_SNAPSHOT_H_
#define BROWSER_CORE_SHARE_SNAPSHOT_H_

#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/containers/span.h"
#include "url/gurl.h"

namespace browser_core {

// What the core takes from a server, as the header says, and what it puts in a link, so a link ilo
// makes opens in ilo: the blob it asks for, the snapshot inflated, its tabs and folders together
// and how deep its folders go, a title's bytes, cut at a character, an address's, past which its
// tab is left out, a favicon's, past which it is, and a Space's icon's, an emoji's few characters.
inline constexpr size_t kShareBlobCap = 1 << 20;
inline constexpr size_t kShareInflatedCap = 4 << 20;
inline constexpr size_t kShareItemCap = 5000;
inline constexpr int kShareDepthCap = 8;
inline constexpr size_t kShareTitleCap = 1024;
inline constexpr size_t kShareAddressCap = 8 * 1024;
inline constexpr size_t kShareFaviconCap = 4 * 1024;
inline constexpr size_t kShareIconCap = 32;

// A tab, or a folder with |items|, in a snapshot.
struct ShareItem {
  ShareItem();
  ShareItem(const ShareItem&);
  ShareItem(ShareItem&&);
  ShareItem& operator=(const ShareItem&);
  ShareItem& operator=(ShareItem&&);
  ~ShareItem();

  std::string title;
  // A tab's, http(s); empty for a folder.
  GURL url;
  // A tab's favicon, a PNG's bytes, or none.
  std::string favicon;
  bool folder = false;
  std::vector<ShareItem> items;
};

struct ShareSnapshot {
  ShareSnapshot();
  ShareSnapshot(const ShareSnapshot&);
  ShareSnapshot(ShareSnapshot&&);
  ShareSnapshot& operator=(const ShareSnapshot&);
  ShareSnapshot& operator=(ShareSnapshot&&);
  ~ShareSnapshot();

  // A BC_SHARE_ kind.
  int kind = 0;
  std::string title;
  // A Space's colour, 0xRRGGBB, and its icon, an emoji, or "".
  std::optional<uint32_t> color;
  std::string icon;
  // When it was shared, in seconds since the Unix epoch.
  int64_t made = 0;
  std::vector<ShareItem> items;
};

// A link's id and key.
struct ShareLink {
  std::string id;
  std::array<uint8_t, 32> key{};
};

// A new link's id and key, random.
ShareLink NewShareLink();

// The id and key of |url| if it has a link's shape: a path /s/ and a 22-character id, and a
// 43-character key in its fragment, both base64url. Its scheme and host are the server's own.
std::optional<ShareLink> ShareLinkOf(const GURL& url);

// The link on |server| for |link|, as the user copies it.
GURL ShareLinkUrl(const GURL& server, const ShareLink& link);

// How many tabs and folders |items| holds, all the way down.
struct ShareCounts {
  size_t tabs = 0;
  size_t folders = 0;
};
ShareCounts CountShare(const std::vector<ShareItem>& items);

// Whether a tab at |url| goes in a snapshot: http(s), at most kShareAddressCap long. Its user and
// password come out first (ShareAddress).
bool ShareTakes(const GURL& url);
GURL ShareAddress(const GURL& url);

// A sealed snapshot, and whether its favicons are in it.
struct SealedShare {
  SealedShare();
  SealedShare(const SealedShare&);
  SealedShare(SealedShare&&);
  SealedShare& operator=(const SealedShare&);
  SealedShare& operator=(SealedShare&&);
  ~SealedShare();

  std::vector<uint8_t> blob;
  bool favicons = false;
};

// |snapshot| sealed for |link|, at most |cap| bytes: with every favicon if |favicons| and it fits,
// else with none, else nothing. Nothing too when it's past what OpenShare takes: over
// kShareItemCap tabs and folders, folders deeper than kShareDepthCap, or over kShareInflatedCap
// inflated. Seconds for thousands of tabs, so off the UI thread.
std::optional<SealedShare> SealShare(const ShareSnapshot& snapshot,
                                     const ShareLink& link,
                                     size_t cap,
                                     bool favicons = true);

// Why a blob doesn't open, for the page that says so.
enum class ShareRefusal {
  // Over a cap: the blob, the snapshot inflated, its tabs and folders, or its depth.
  kTooBig,
  // A version, or a shape, the core doesn't read.
  kNewer,
  // The key doesn't open it, or what it opens isn't a snapshot.
  kKey,
};

// Opens |blob|, a server's answer for |link|, under the caps: titles are cut, and tabs at an
// address the snapshot can't hold, and favicons that aren't a small PNG, are left out. Off the UI
// thread, as SealShare.
struct ShareOpened {
  std::optional<ShareSnapshot> snapshot;
  ShareRefusal refusal = ShareRefusal::kKey;
};
ShareOpened OpenShare(base::span<const uint8_t> blob, const ShareLink& link);

// The page the core commits in a link's server's page's place: |snapshot|'s list, its links only
// http(s), everything it shows escaped, and a policy that runs no script. |host| is the server's.
std::string SharePage(const ShareSnapshot& snapshot, std::string_view host);

// What the page says when a link doesn't open: it was unshared (|gone|), or |refusal|.
std::string ShareRefusedPage(bool gone, std::optional<ShareRefusal> refusal);

}  // namespace browser_core

#endif  // BROWSER_CORE_SHARE_SNAPSHOT_H_
