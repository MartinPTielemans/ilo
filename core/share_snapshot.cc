#include "browser_core/share_snapshot.h"

#include <algorithm>
#include <utility>

#include "base/base64.h"
#include "base/base64url.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/strings/escape.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/stringprintf.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "crypto/aead.h"
#include "crypto/random.h"
#include "third_party/zlib/zlib.h"
#include "url/url_constants.h"

namespace browser_core {

ShareItem::ShareItem() = default;
ShareItem::ShareItem(const ShareItem&) = default;
ShareItem::ShareItem(ShareItem&&) = default;
ShareItem& ShareItem::operator=(const ShareItem&) = default;
ShareItem& ShareItem::operator=(ShareItem&&) = default;
ShareItem::~ShareItem() = default;

ShareSnapshot::ShareSnapshot() = default;
ShareSnapshot::ShareSnapshot(const ShareSnapshot&) = default;
ShareSnapshot::ShareSnapshot(ShareSnapshot&&) = default;
ShareSnapshot& ShareSnapshot::operator=(const ShareSnapshot&) = default;
ShareSnapshot& ShareSnapshot::operator=(ShareSnapshot&&) = default;
ShareSnapshot::~ShareSnapshot() = default;

SealedShare::SealedShare() = default;
SealedShare::SealedShare(const SealedShare&) = default;
SealedShare::SealedShare(SealedShare&&) = default;
SealedShare& SealedShare::operator=(const SealedShare&) = default;
SealedShare& SealedShare::operator=(SealedShare&&) = default;
SealedShare::~SealedShare() = default;

namespace {

constexpr uint8_t kVersion = 1;
constexpr size_t kIvSize = 12;
constexpr size_t kTagSize = 16;
constexpr size_t kIdBytes = 16;
constexpr size_t kIdLength = 22;
constexpr size_t kKeyLength = 43;
constexpr std::string_view kPng = "\x89PNG\r\n\x1a\n";
constexpr std::string_view kPngData = "data:image/png;base64,";

std::string Base64Url(base::span<const uint8_t> bytes) {
  std::string out;
  base::Base64UrlEncode(bytes, base::Base64UrlEncodePolicy::OMIT_PADDING, &out);
  return out;
}

bool IsBase64Url(std::string_view text, size_t length) {
  if (text.size() != length) {
    return false;
  }
  for (char c : text) {
    if (!base::IsAsciiAlphaNumeric(c) && c != '-' && c != '_') {
      return false;
    }
  }
  return true;
}

std::string AdditionalData(const ShareLink& link) {
  return base::StrCat({"ilo share v1 ", link.id});
}

const char* KindName(int kind) {
  switch (kind) {
    case BC_SHARE_SPACE:
      return "space";
    case BC_SHARE_TABS:
      return "tabs";
    default:
      return "folder";
  }
}

std::optional<int> KindOf(const std::string* name) {
  if (!name) {
    return std::nullopt;
  }
  if (*name == "folder") {
    return BC_SHARE_FOLDER;
  }
  if (*name == "space") {
    return BC_SHARE_SPACE;
  }
  if (*name == "tabs") {
    return BC_SHARE_TABS;
  }
  return std::nullopt;
}

std::string Title(std::string_view title) {
  std::string cut;
  base::TruncateUTF8ToByteSize(std::string(title), kShareTitleCap, &cut);
  return cut;
}

// A favicon it keeps: a PNG, by its signature, of at most kShareFaviconCap bytes.
bool KeepsFavicon(std::string_view png) {
  return !png.empty() && png.size() <= kShareFaviconCap && png.starts_with(kPng);
}

base::ListValue ItemsValue(const std::vector<ShareItem>& items, bool favicons) {
  base::ListValue list;
  for (const ShareItem& item : items) {
    base::DictValue dict;
    dict.Set("title", Title(item.title));
    if (item.folder) {
      dict.Set("items", ItemsValue(item.items, favicons));
    } else {
      dict.Set("url", ShareAddress(item.url).spec());
      if (favicons && KeepsFavicon(item.favicon)) {
        dict.Set("icon", base::StrCat({kPngData, base::Base64Encode(item.favicon)}));
      }
    }
    list.Append(std::move(dict));
  }
  return list;
}

std::string Json(const ShareSnapshot& snapshot, bool favicons) {
  base::DictValue dict;
  dict.Set("v", static_cast<int>(kVersion));
  dict.Set("kind", KindName(snapshot.kind));
  dict.Set("title", Title(snapshot.title));
  if (snapshot.color) {
    dict.Set("color", base::StringPrintf("#%06x", *snapshot.color & 0xffffff));
  }
  // OpenShare drops a longer one.
  if (!snapshot.icon.empty() && snapshot.icon.size() <= kShareIconCap) {
    dict.Set("icon", snapshot.icon);
  }
  dict.Set("made", static_cast<double>(snapshot.made));
  dict.Set("items", ItemsValue(snapshot.items, favicons));
  return base::WriteJson(dict).value_or(std::string());
}

std::optional<std::string> Deflate(std::string_view in) {
  z_stream stream = {};
  if (deflateInit2(&stream, 9, Z_DEFLATED, -MAX_WBITS, 8, Z_DEFAULT_STRATEGY) != Z_OK) {
    return std::nullopt;
  }
  std::string out(deflateBound(&stream, in.size()), '\0');
  stream.next_in = reinterpret_cast<Bytef*>(const_cast<char*>(in.data()));
  stream.avail_in = in.size();
  stream.next_out = reinterpret_cast<Bytef*>(out.data());
  stream.avail_out = out.size();
  const int result = deflate(&stream, Z_FINISH);
  out.resize(stream.total_out);
  deflateEnd(&stream);
  if (result != Z_STREAM_END) {
    return std::nullopt;
  }
  return out;
}

// |in| inflated, or nothing once it would pass kShareInflatedCap: it stops there, so a small blob
// that inflates to a gigabyte costs the cap and no more.
std::optional<std::string> Inflate(base::span<const uint8_t> in, bool* too_big) {
  z_stream stream = {};
  if (inflateInit2(&stream, -MAX_WBITS) != Z_OK) {
    return std::nullopt;
  }
  stream.next_in = const_cast<Bytef*>(in.data());
  stream.avail_in = in.size();
  std::string out;
  char chunk[16 * 1024];
  int result = Z_OK;
  while (result == Z_OK) {
    stream.next_out = reinterpret_cast<Bytef*>(chunk);
    stream.avail_out = sizeof(chunk);
    result = inflate(&stream, Z_NO_FLUSH);
    if (result != Z_OK && result != Z_STREAM_END) {
      break;
    }
    const size_t made = sizeof(chunk) - stream.avail_out;
    if (out.size() + made > kShareInflatedCap) {
      *too_big = true;
      inflateEnd(&stream);
      return std::nullopt;
    }
    out.append(chunk, made);
    if (result == Z_OK && made == 0 && stream.avail_in == 0) {
      break;
    }
  }
  inflateEnd(&stream);
  if (result != Z_STREAM_END) {
    return std::nullopt;
  }
  return out;
}

// What reading the items found past the caps.
struct Reading {
  // Tabs and folders together, so a snapshot of nothing but empty folders is capped as one of tabs.
  size_t items = 0;
  bool too_big = false;
};

// Whether any tab in |items| has a favicon a snapshot keeps.
bool AnyFavicon(const std::vector<ShareItem>& items) {
  return std::ranges::any_of(items, [](const ShareItem& item) {
    return item.folder ? AnyFavicon(item.items) : KeepsFavicon(item.favicon);
  });
}

// How deep |items|' folders go: 0 for none, 1 for folders of tabs alone.
int DepthOf(const std::vector<ShareItem>& items) {
  int depth = 0;
  for (const ShareItem& item : items) {
    if (item.folder) {
      depth = std::max(depth, 1 + DepthOf(item.items));
    }
  }
  return depth;
}

std::vector<ShareItem> ReadItems(const base::ListValue* list, int depth, Reading& reading) {
  std::vector<ShareItem> items;
  if (!list) {
    return items;
  }
  for (const base::Value& value : *list) {
    if (reading.too_big) {
      break;
    }
    const base::DictValue* dict = value.GetIfDict();
    if (!dict) {
      continue;
    }
    ShareItem item;
    if (const std::string* title = dict->FindString("title")) {
      item.title = Title(*title);
    }
    if (const base::ListValue* inner = dict->FindList("items")) {
      if (depth >= kShareDepthCap || ++reading.items > kShareItemCap) {
        reading.too_big = true;
        break;
      }
      item.folder = true;
      item.items = ReadItems(inner, depth + 1, reading);
      items.push_back(std::move(item));
      continue;
    }
    const std::string* url = dict->FindString("url");
    if (!url || url->size() > kShareAddressCap) {
      continue;
    }
    item.url = GURL(*url);
    if (!ShareTakes(item.url)) {
      continue;
    }
    item.url = ShareAddress(item.url);
    if (++reading.items > kShareItemCap) {
      reading.too_big = true;
      break;
    }
    if (const std::string* icon = dict->FindString("icon");
        icon && icon->starts_with(kPngData) &&
        icon->size() - kPngData.size() <= (kShareFaviconCap + 2) / 3 * 4) {
      std::string png;
      if (base::Base64Decode(std::string_view(*icon).substr(kPngData.size()), &png) &&
          KeepsFavicon(png)) {
        item.favicon = std::move(png);
      }
    }
    items.push_back(std::move(item));
  }
  return items;
}

ShareOpened Refused(ShareRefusal refusal) {
  ShareOpened opened;
  opened.refusal = refusal;
  return opened;
}

constexpr std::string_view kPageHead = R"html(<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'">
<meta name="referrer" content="no-referrer">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
/* ilo's share page (ilo-server's share.css), in error_page.cc's palette: Paper on the dark pane,
   Ink in Light. The accent is the Space's colour, or ilo green. */
:root { --ink: 243 240 232; --bg: #131313; --group: rgb(255 255 255 / 0.04); }
@media (prefers-color-scheme: light) {
  :root { --ink: 22 20 15; --bg: #fbf9f4; --group: rgb(22 20 15 / 0.035); }
}
html { background: var(--bg); color: rgb(var(--ink)); }
body { margin: 0; font: 14px/1.4 system-ui, sans-serif; }
main { max-width: 632px; margin: 0 auto; padding: 32px max(16px, env(safe-area-inset-right)) 48px
  max(16px, env(safe-area-inset-left)); }
.from { font-size: 11px; font-weight: 600; color: rgb(var(--ink) / 0.55); }
header { display: flex; gap: 14px; align-items: center; margin: 22px 0 18px; }
.mark { width: 40px; height: 40px; border-radius: 10px; background: var(--accent); flex: none;
  display: grid; place-items: center; font-size: 22px; }
h1 { margin: 0; font-size: 22px; font-weight: 600; overflow-wrap: anywhere; }
.meta { margin: 2px 0 0; font-size: 13px; color: rgb(var(--ink) / 0.66); }
.list { background: var(--group); border-radius: 12px; padding: 4px; }
.row { display: flex; align-items: center; gap: 10px; min-height: 40px; padding: 0 12px;
  border-radius: 8px; color: inherit; text-decoration: none; }
a.row:hover { background: rgb(var(--ink) / 0.06); }
.icon { width: 16px; height: 16px; border-radius: 4px; flex: none; background: rgb(var(--ink) / 0.4);
  color: var(--bg); font-size: 10px; font-weight: 600; display: grid; place-items: center; }
img.icon { background: none; }
.title { flex: 1; min-width: 0; font-weight: 500; overflow: hidden; text-overflow: ellipsis;
  white-space: nowrap; }
.host { font-size: 11px; color: rgb(var(--ink) / 0.55); }
.folder > .row { font-weight: 500; }
.folder-items { padding-left: 16px; }
.count { font-size: 11px; color: rgb(var(--ink) / 0.55); }
p.note { margin: 18px 0 0; font-size: 13px; color: rgb(var(--ink) / 0.66); }
@media (max-width: 480px) { .host { display: none; } }
</style>
)html";

void AppendRows(const std::vector<ShareItem>& items, std::string& out) {
  for (const ShareItem& item : items) {
    if (item.folder) {
      base::StrAppend(
          &out, {"<div class=\"folder\"><div class=\"row\"><span class=\"title\">",
                 base::EscapeForHTML(item.title.empty() ? "Folder" : item.title),
                 "</span><span class=\"count\">", base::NumberToString(CountShare(item.items).tabs),
                 "</span></div><div class=\"folder-items\">"});
      AppendRows(item.items, out);
      out += "</div></div>";
      continue;
    }
    // OpenShare took only http(s) addresses, and this checks again.
    if (!ShareTakes(item.url)) {
      continue;
    }
    const std::string host(item.url.host());
    std::string icon;
    if (KeepsFavicon(item.favicon)) {
      icon = base::StrCat({"<img class=\"icon\" alt=\"\" src=\"", kPngData,
                           base::Base64Encode(item.favicon), "\">"});
    } else {
      const std::string letter = host.empty() ? "?" : std::string(1, base::ToUpperASCII(host[0]));
      icon = base::StrCat({"<span class=\"icon\">", base::EscapeForHTML(letter), "</span>"});
    }
    base::StrAppend(&out,
                    {"<a class=\"row\" target=\"_blank\" rel=\"noopener noreferrer\" href=\"",
                     base::EscapeForHTML(item.url.spec()), "\">", icon, "<span class=\"title\">",
                     base::EscapeForHTML(item.title.empty() ? item.url.spec() : item.title),
                     "</span><span class=\"host\">", base::EscapeForHTML(host), "</span></a>"});
  }
}

}  // namespace

ShareLink NewShareLink() {
  ShareLink link;
  std::array<uint8_t, kIdBytes> id;
  crypto::RandBytes(id);
  link.id = Base64Url(id);
  crypto::RandBytes(link.key);
  return link;
}

std::optional<ShareLink> ShareLinkOf(const GURL& url) {
  if (!url.is_valid() || !url.SchemeIsHTTPOrHTTPS() || !url.has_ref()) {
    return std::nullopt;
  }
  const std::string_view path = url.path();
  const std::string_view ref = url.ref();
  if (!path.starts_with("/s/") || !IsBase64Url(path.substr(3), kIdLength) ||
      !IsBase64Url(ref, kKeyLength)) {
    return std::nullopt;
  }
  std::string key;
  if (!base::Base64UrlDecode(ref, base::Base64UrlDecodePolicy::DISALLOW_PADDING, &key) ||
      key.size() != 32) {
    return std::nullopt;
  }
  ShareLink link;
  link.id = std::string(path.substr(3));
  std::ranges::copy(key, link.key.begin());
  return link;
}

GURL ShareLinkUrl(const GURL& server, const ShareLink& link) {
  return server.Resolve(base::StrCat({"/s/", link.id, "#", Base64Url(link.key)}));
}

ShareCounts CountShare(const std::vector<ShareItem>& items) {
  ShareCounts counts;
  for (const ShareItem& item : items) {
    if (item.folder) {
      const ShareCounts inner = CountShare(item.items);
      counts.folders += 1 + inner.folders;
      counts.tabs += inner.tabs;
    } else {
      ++counts.tabs;
    }
  }
  return counts;
}

bool ShareTakes(const GURL& url) {
  return url.is_valid() && url.SchemeIsHTTPOrHTTPS() &&
         ShareAddress(url).spec().size() <= kShareAddressCap;
}

GURL ShareAddress(const GURL& url) {
  GURL::Replacements replacements;
  replacements.ClearUsername();
  replacements.ClearPassword();
  return url.ReplaceComponents(replacements);
}

std::optional<SealedShare> SealShare(const ShareSnapshot& snapshot,
                                     const ShareLink& link,
                                     size_t cap,
                                     bool favicons) {
  const ShareCounts counts = CountShare(snapshot.items);
  if (counts.tabs + counts.folders > kShareItemCap || DepthOf(snapshot.items) > kShareDepthCap) {
    return std::nullopt;
  }
  for (bool with : {true, false}) {
    if (with && (!favicons || !AnyFavicon(snapshot.items))) {
      continue;
    }
    const std::string json = Json(snapshot, with);
    // Addresses alike deflate to little, and OpenShare stops inflating at its cap.
    if (json.size() > kShareInflatedCap) {
      continue;
    }
    const std::optional<std::string> packed = Deflate(json);
    if (!packed) {
      return std::nullopt;
    }
    if (1 + kIvSize + packed->size() + kTagSize > cap) {
      continue;
    }
    std::array<uint8_t, kIvSize> iv;
    crypto::RandBytes(iv);
    const std::vector<uint8_t> sealed =
        crypto::aead::Seal(crypto::aead::AES_256_GCM, link.key, base::as_byte_span(*packed), iv,
                           base::as_byte_span(AdditionalData(link)));
    SealedShare made;
    made.favicons = with;
    made.blob.reserve(1 + kIvSize + sealed.size());
    made.blob.push_back(kVersion);
    made.blob.insert(made.blob.end(), iv.begin(), iv.end());
    made.blob.insert(made.blob.end(), sealed.begin(), sealed.end());
    return made;
  }
  return std::nullopt;
}

ShareOpened OpenShare(base::span<const uint8_t> blob, const ShareLink& link) {
  if (blob.size() > kShareBlobCap) {
    return Refused(ShareRefusal::kTooBig);
  }
  if (blob.size() < 1 + kIvSize + kTagSize) {
    return Refused(ShareRefusal::kKey);
  }
  if (blob[0] != kVersion) {
    return Refused(ShareRefusal::kNewer);
  }
  const std::optional<std::vector<uint8_t>> packed = crypto::aead::Open(
      crypto::aead::AES_256_GCM, link.key, blob.subspan(1 + kIvSize),
      blob.subspan(size_t{1}, kIvSize), base::as_byte_span(AdditionalData(link)));
  if (!packed) {
    return Refused(ShareRefusal::kKey);
  }
  bool too_big = false;
  const std::optional<std::string> json = Inflate(*packed, &too_big);
  if (!json) {
    return Refused(too_big ? ShareRefusal::kTooBig : ShareRefusal::kKey);
  }
  // Each folder is two levels of JSON, its object and its list, and the reader takes one level
  // fewer than it's given. One folder past the cap still reads, so ReadItems says it's too deep.
  const std::optional<base::DictValue> read =
      base::JSONReader::ReadDict(*json, base::JSON_PARSE_RFC, 2 * (kShareDepthCap + 1) + 4);
  if (!read) {
    // The key opened it, so it's a shape this version doesn't read.
    return Refused(ShareRefusal::kNewer);
  }
  const base::DictValue* dict = &*read;
  if (dict->FindInt("v") != static_cast<int>(kVersion)) {
    return Refused(ShareRefusal::kNewer);
  }
  const std::optional<int> kind = KindOf(dict->FindString("kind"));
  if (!kind) {
    return Refused(ShareRefusal::kNewer);
  }
  ShareSnapshot snapshot;
  snapshot.kind = *kind;
  if (const std::string* title = dict->FindString("title")) {
    snapshot.title = Title(*title);
  }
  if (const std::string* color = dict->FindString("color");
      color && color->size() == 7 && (*color)[0] == '#') {
    uint32_t rgb = 0;
    if (base::HexStringToUInt(std::string_view(*color).substr(1), &rgb)) {
      snapshot.color = rgb;
    }
  }
  // An emoji: a few characters at most.
  if (const std::string* icon = dict->FindString("icon"); icon && icon->size() <= kShareIconCap) {
    snapshot.icon = *icon;
  }
  snapshot.made = static_cast<int64_t>(dict->FindDouble("made").value_or(0));
  Reading reading;
  snapshot.items = ReadItems(dict->FindList("items"), 0, reading);
  if (reading.too_big) {
    return Refused(ShareRefusal::kTooBig);
  }
  ShareOpened opened;
  opened.snapshot = std::move(snapshot);
  return opened;
}

std::string SharePage(const ShareSnapshot& snapshot, std::string_view host) {
  const std::string title = snapshot.title.empty() ? "Shared tabs" : snapshot.title;
  const std::string accent =
      snapshot.color ? base::StringPrintf("#%06x", *snapshot.color & 0xffffff) : "#289054";
  const ShareCounts counts = CountShare(snapshot.items);
  std::string meta = snapshot.kind == BC_SHARE_SPACE    ? "Space"
                     : snapshot.kind == BC_SHARE_FOLDER ? "Folder"
                                                        : "Tabs";
  base::StrAppend(&meta,
                  {" · ", base::NumberToString(counts.tabs), counts.tabs == 1 ? " tab" : " tabs"});
  if (counts.folders) {
    base::StrAppend(&meta, {" · ", base::NumberToString(counts.folders),
                            counts.folders == 1 ? " folder" : " folders"});
  }
  std::string out(kPageHead);
  // The accent is #rrggbb from a number, so nothing of the snapshot's text reaches the style.
  base::StrAppend(&out, {"<style>:root { --accent: ", accent, "; }</style>\n<title>",
                         base::EscapeForHTML(title), "</title>\n</head>\n<body>\n<main>\n",
                         "<div class=\"from\">Shared from ilo · ", base::EscapeForHTML(host),
                         "</div>\n<header><div class=\"mark\">", base::EscapeForHTML(snapshot.icon),
                         "</div><div><h1>", base::EscapeForHTML(title), "</h1><p class=\"meta\">",
                         base::EscapeForHTML(meta), "</p></div></header>\n<div class=\"list\">"});
  AppendRows(snapshot.items, out);
  out += "</div>\n</main>\n</body>\n</html>\n";
  return out;
}

std::string ShareRefusedPage(bool gone, std::optional<ShareRefusal> refusal) {
  std::string_view why = "This link can't be opened right now. Try again in a while.";
  if (gone) {
    why = "This link was unshared.";
  } else if (refusal == ShareRefusal::kKey) {
    why =
        "This link can't be opened. Its key doesn't match what's shared, so it may have been "
        "copied wrong.";
  } else if (refusal == ShareRefusal::kNewer) {
    why = "This link needs a newer version of ilo.";
  } else if (refusal == ShareRefusal::kTooBig) {
    why = "This link holds more than ilo opens.";
  }
  return base::StrCat({kPageHead,
                       "<style>:root { --accent: #289054; }</style>\n<title>Shared from ilo"
                       "</title>\n</head>\n<body>\n<main>\n<div class=\"from\">Shared from ilo"
                       "</div>\n<p class=\"note\">",
                       why, "</p>\n</main>\n</body>\n</html>\n"});
}

}  // namespace browser_core
