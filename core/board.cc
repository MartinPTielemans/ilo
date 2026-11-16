#include "browser_core/board.h"

#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/base_paths.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/ref_counted_memory.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/path_service.h"
#include "base/strings/string_util.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/task_runner.h"
#include "base/task/thread_pool.h"
#include "base/threading/sequence_bound.h"
#include "base/values.h"
#include "browser_core/board_store.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ui/webui/favicon_source.h"
#include "components/favicon_base/favicon_url_parser.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/url_data_source.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_ui.h"
#include "content/public/browser/web_ui_controller.h"
#include "content/public/browser/web_ui_message_handler.h"
#include "content/public/browser/webui_config.h"
#include "content/public/browser/webui_config_map.h"
#include "services/network/public/mojom/content_security_policy.mojom.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

constexpr char kHost[] = "board";

// Where every Board's files are read and written, in order, off the UI thread. A write in progress
// at a quit finishes before the browser goes.
scoped_refptr<base::SequencedTaskRunner> Sequence() {
  static base::NoDestructor<scoped_refptr<base::SequencedTaskRunner>> sequence(
      base::ThreadPool::CreateSequencedTaskRunner({base::MayBlock(),
                                                   base::TaskPriority::USER_VISIBLE,
                                                   base::TaskShutdownBehavior::BLOCK_SHUTDOWN}));
  return *sequence;
}

// The profile's folder, where its Boards are, or empty where it keeps none: a private window's or a
// guest's profile, whose pages show "This Board isn't here."
base::FilePath ProfileDir(Profile* profile) {
  return profile->IsOffTheRecord() || profile->IsGuestSession() ? base::FilePath()
                                                                : profile->GetPath();
}

// The page's own files, from the app's board/ folder beside uBlock Origin's: a name, never a path.
bool IsPageFile(std::string_view name) {
  if (name.empty() || name.front() == '.' || name.find("..") != std::string_view::npos) {
    return false;
  }
  for (char c : name) {
    if (!base::IsAsciiLower(c) && !base::IsAsciiDigit(c) && c != '-' && c != '_' && c != '.') {
      return false;
    }
  }
  for (std::string_view extension : {".html", ".js", ".css", ".svg", ".woff2"}) {
    if (name.ends_with(extension)) {
      return true;
    }
  }
  return false;
}

// What a request for |path| reads, and as what.
struct Asked {
  base::FilePath file;
  uint64_t max = 0;
  std::string mime;
};

Asked Ask(const base::FilePath& profile_dir, std::string_view path) {
  base::FilePath assets;
  base::PathService::Get(base::DIR_ASSETS, &assets);
  assets = assets.Append(FILE_PATH_LITERAL("board"));
  // The page itself: at chrome://board/<id>, and at an address with no Board, which says so.
  if (path.empty() || board::IsBoardId(path)) {
    return {assets.Append(FILE_PATH_LITERAL("index.html")), 1 << 20, "text/html"};
  }
  if (IsPageFile(path)) {
    const std::string_view mime = path.ends_with(".html")  ? "text/html"
                                  : path.ends_with(".js")  ? "text/javascript"
                                  : path.ends_with(".css") ? "text/css"
                                  : path.ends_with(".svg") ? "image/svg+xml"
                                                           : "font/woff2";
    return {assets.AppendASCII(path), 8 << 20, std::string(mime)};
  }
  // A Board's own: <id>/board.json, <id>/thumb.png, <id>/files/<hash>.<picture>.
  const size_t slash = path.find('/');
  base::FilePath board = slash == std::string_view::npos
                             ? base::FilePath()
                             : board::BoardDir(profile_dir, path.substr(0, slash));
  if (profile_dir.empty() || board.empty()) {
    return {};
  }
  const std::string_view rest = path.substr(slash + 1);
  if (rest == "board.json") {
    return {board.Append(FILE_PATH_LITERAL("board.json")), board::kMaxBoardJson,
            "application/json"};
  }
  if (rest == "thumb.png") {
    return {board.Append(FILE_PATH_LITERAL("thumb.png")), board::kMaxThumb, "image/png"};
  }
  if (rest.starts_with("files/")) {
    const std::string_view name = rest.substr(6);
    if (board::Image image = board::FileImage(name); image != board::Image::kNone) {
      return {board.Append(FILE_PATH_LITERAL("files")).AppendASCII(name), board::kMaxFile,
              board::MimeType(image)};
    }
  }
  return {};
}

scoped_refptr<base::RefCountedMemory> Read(const base::FilePath& file, uint64_t max) {
  std::string bytes;
  if (file.empty() || !base::ReadFileToStringWithMaxSize(file, &bytes, max)) {
    return nullptr;
  }
  return base::MakeRefCounted<base::RefCountedString>(std::move(bytes));
}

// chrome://board's files. Each read is posted to the Boards' sequence, and its bytes come back as
// a pointer, which WebUI's loader copies into the page's pipe on the thread pool.
class BoardSource : public content::URLDataSource {
 public:
  explicit BoardSource(Profile* profile) : profile_dir_(ProfileDir(profile)) {}

  std::string GetSource() override { return kHost; }

  void StartDataRequest(const GURL& url,
                        const content::WebContents::Getter& wc_getter,
                        GotDataCallback callback) override {
    Asked asked = Ask(profile_dir_, URLToRequestPath(url));
    Sequence()->PostTaskAndReplyWithResult(
        FROM_HERE, base::BindOnce(&Read, std::move(asked.file), asked.max), std::move(callback));
  }

  std::string GetMimeType(const GURL& url) override {
    const std::string mime = Ask(profile_dir_, URLToRequestPath(url)).mime;
    return mime.empty() ? "text/plain" : mime;
  }

  // A Board changes as it's used: every read is the file as it is.
  bool AllowCaching() override { return false; }

  // A stored file navigated to is the picture its type says, which a navigation doesn't sniff
  // past, so it's never a document in chrome://board's origin.
  bool ShouldServeMimeTypeAsContentTypeHeader() override { return true; }

  std::string GetContentSecurityPolicy(network::mojom::CSPDirectiveName directive) override {
    using Directive = network::mojom::CSPDirectiveName;
    switch (directive) {
      // The page's own scripts, no inline script and no eval, and markup only through trusted
      // types, of which the page makes none: card text is set as text.
      case Directive::ScriptSrc:
        return "script-src 'self';";
      case Directive::DefaultSrc:
        return "default-src 'none';";
      // Its reads, from the host alone, which can't send anywhere.
      case Directive::ConnectSrc:
        return "connect-src 'self';";
      // A link card's icon is the one the profile holds, which chrome://favicon2 reads from the
      // profile's store and never fetches for this page.
      case Directive::ImgSrc:
        return "img-src 'self' chrome://favicon2;";
      case Directive::StyleSrc:
        return "style-src 'self';";
      case Directive::FontSrc:
        return "font-src 'self';";
      case Directive::BaseURI:
        return "base-uri 'none';";
      case Directive::FormAction:
        return "form-action 'none';";
      default:
        return URLDataSource::GetContentSecurityPolicy(directive);
    }
  }

 private:
  const base::FilePath profile_dir_;
};

// The page's writes, on the Boards' sequence, one at a time: the page sends a write's next slice
// once the last is answered. Gone with the page's handler, and with it any write it left
// unfinished, whose temporary file goes too.
class Writes {
 public:
  enum class Kind { kNone, kBoard, kThumb, kFile };

  explicit Writes(base::FilePath board) : board_(std::move(board)) {}

  bool Here() const { return base::DirectoryExists(board_); }

  // The most a write of |kind| may be.
  static uint64_t Cap(Kind kind) {
    return kind == Kind::kBoard   ? board::kMaxBoardJson
           : kind == Kind::kThumb ? board::kMaxThumb
           : kind == Kind::kFile  ? board::kMaxFile
                                  : 0;
  }

  bool Start(Kind kind, uint64_t size) {
    Drop();
    if (size == 0 || size > Cap(kind) || !Here()) {
      return false;
    }
    if (kind == Kind::kFile && !(add_ = board::FileAdd::Start(board_, size))) {
      return false;
    }
    kind_ = kind;
    size_ = size;
    return true;
  }

  bool Append(std::vector<uint8_t> slice) {
    if (kind_ == Kind::kFile) {
      return add_->Append(slice) || Drop();
    }
    if (kind_ == Kind::kNone || slice.size() > size_ - bytes_.size() ||
        (kind_ == Kind::kThumb && bytes_.empty() && board::Sniff(slice) != board::Image::kPng)) {
      return Drop();
    }
    bytes_.append(slice.begin(), slice.end());
    return true;
  }

  // The stored file's name for a picture, "" for the Board's file or its picture, or nothing if
  // the write failed.
  std::optional<std::string> Finish() {
    std::optional<std::string> done;
    if (kind_ == Kind::kFile) {
      done = add_->Finish();
    } else if (bytes_.size() == size_ &&
               (kind_ == Kind::kBoard   ? board::SaveBoardJson(board_, bytes_)
                : kind_ == Kind::kThumb ? board::SaveThumb(board_, base::as_byte_span(bytes_))
                                        : false)) {
      done = "";
    }
    Drop();
    return done;
  }

  // Drops the write on its way; false, so a refusal can end with it.
  bool Drop() {
    kind_ = Kind::kNone;
    size_ = 0;
    bytes_.clear();
    add_.reset();
    return false;
  }

 private:
  const base::FilePath board_;
  Kind kind_ = Kind::kNone;
  uint64_t size_ = 0;
  std::string bytes_;
  std::unique_ptr<board::FileAdd> add_;
};

class BoardHandler;

// The page each Board is open in, by its folder: a Board has one writer.
std::map<base::FilePath, BoardHandler*>& Open() {
  static base::NoDestructor<std::map<base::FilePath, BoardHandler*>> open;
  return *open;
}

// Pages that found their Board open in another tab, told when it closes.
std::set<BoardHandler*>& Waiting() {
  static base::NoDestructor<std::set<BoardHandler*>> waiting;
  return *waiting;
}

// The Board's id in |url|, chrome://board/<id>, or empty.
std::string_view IdIn(const GURL& url) {
  std::string_view path = url.path();
  if (path.starts_with('/')) {
    path.remove_prefix(1);
  }
  return board::IsBoardId(path) ? path : std::string_view();
}

// The page's messages. None names a Board: the handler's is the one its frame's committed address
// names, which the browser reads, at each hello since Chrome keeps a WebUI across navigations of
// its own type. A write whose frame's address no longer names that Board is refused, so a page
// that changes its own address still writes only the Board it was opened as.
//   hello(id) => "here", "missing" or "elsewhere"
//   write(id, "board" | "thumb" | "file", size) => whether it can start
//   slice(id, ArrayBuffer) => whether it was taken; the page sends the next once answered
//   done(id) => a picture's stored name, "" for the others, or false
class BoardHandler : public content::WebUIMessageHandler {
 public:
  explicit BoardHandler(base::FilePath profile_dir) : profile_dir_(std::move(profile_dir)) {}

  ~BoardHandler() override {
    Waiting().erase(this);
    Release();
  }

  void RegisterMessages() override {
    auto on = [this](const char* message, void (BoardHandler::*handle)(const base::ListValue&)) {
      web_ui()->RegisterMessageCallback(message,
                                        base::BindRepeating(handle, base::Unretained(this)));
    };
    on("hello", &BoardHandler::Hello);
    on("write", &BoardHandler::Write);
    on("slice", &BoardHandler::Slice);
    on("done", &BoardHandler::Done);
  }

  // A reload or a navigation away: what the page had on its way is dropped, a picture half-added
  // with it, and the answers it waited for go nowhere.
  void OnJavascriptDisallowed() override {
    weak_factory_.InvalidateWeakPtrs();
    if (!writes_.is_null()) {
      writes_.AsyncCall(&Writes::Drop);
    }
  }

 private:
  // The Board the frame's committed address names, or empty: a private window's, or an address
  // with no id.
  base::FilePath Committed() {
    return profile_dir_.empty()
               ? base::FilePath()
               : board::BoardDir(profile_dir_,
                                 IdIn(web_ui()->GetRenderFrameHost()->GetLastCommittedURL()));
  }

  // Whether this page may write: it holds its Board, and its address still names it.
  bool Writer() { return owner_ && !board_.empty() && Committed() == board_; }

  // Gives up the Board, telling the pages waiting for it.
  void Release() {
    if (!owner_) {
      return;
    }
    owner_ = false;
    Open().erase(board_);
    for (BoardHandler* waiting : std::set<BoardHandler*>(Waiting())) {
      if (waiting->board_ == board_ && waiting->IsJavascriptAllowed()) {
        waiting->FireWebUIListener("board-free");
      }
    }
  }

  void Hello(const base::ListValue& args) {
    if (args.empty()) {
      return;
    }
    AllowJavascript();
    if (base::FilePath board = Committed(); board != board_) {
      Release();
      Waiting().erase(this);
      board_ = std::move(board);
      // The last Board's writes go, on the sequence, and any temporary file with them.
      writes_.Reset();
      if (!board_.empty()) {
        writes_ = base::SequenceBound<Writes>(Sequence(), board_);
      }
    }
    if (board_.empty()) {
      Answer(args[0].Clone(), base::Value("missing"));
      return;
    }
    writes_.AsyncCall(&Writes::Here)
        .Then(base::BindOnce(&BoardHandler::Found, weak_factory_.GetWeakPtr(), board_,
                             args[0].Clone()));
  }

  void Found(base::FilePath board, base::Value id, bool here) {
    if (board != board_) {
      return;  // An answer for the Board the page was before.
    }
    if (!here) {
      Release();
      Answer(std::move(id), base::Value("missing"));
      return;
    }
    auto [open, opened] = Open().emplace(board_, this);
    if (open->second == this) {
      owner_ = true;
      Waiting().erase(this);
      Answer(std::move(id), base::Value("here"));
    } else {
      Waiting().insert(this);
      Answer(std::move(id), base::Value("elsewhere"));
    }
  }

  void Write(const base::ListValue& args) {
    if (args.empty()) {
      return;
    }
    const std::string* kind = args.size() == 3 ? args[1].GetIfString() : nullptr;
    const std::optional<double> size = args.size() == 3 ? args[2].GetIfDouble() : std::nullopt;
    const Writes::Kind writes = !kind              ? Writes::Kind::kNone
                                : *kind == "board" ? Writes::Kind::kBoard
                                : *kind == "thumb" ? Writes::Kind::kThumb
                                : *kind == "file"  ? Writes::Kind::kFile
                                                   : Writes::Kind::kNone;
    if (!Writer() || writes == Writes::Kind::kNone || !size || *size < 1 ||
        *size > static_cast<double>(Writes::Cap(writes))) {
      Answer(args[0].Clone(), base::Value(false));
      return;
    }
    writes_.AsyncCall(&Writes::Start)
        .WithArgs(writes, static_cast<uint64_t>(*size))
        .Then(base::BindOnce(&BoardHandler::Answered, weak_factory_.GetWeakPtr(), args[0].Clone()));
  }

  void Slice(const base::ListValue& args) {
    if (args.empty()) {
      return;
    }
    // A slice comes as an ArrayBuffer, which chrome.send carries as binary.
    if (!Writer() || args.size() != 2 || !args[1].is_blob()) {
      Answer(args[0].Clone(), base::Value(false));
      return;
    }
    // A wall against a page that goes wrong, not the budget: by now the UI thread has carried the
    // slice. The write it was part of goes.
    if (args[1].GetBlob().size() > board::kSlice) {
      writes_.AsyncCall(&Writes::Drop);
      Answer(args[0].Clone(), base::Value(false));
      return;
    }
    // The UI thread only hands the slice on.
    writes_.AsyncCall(&Writes::Append)
        .WithArgs(args[1].GetBlob())
        .Then(base::BindOnce(&BoardHandler::Answered, weak_factory_.GetWeakPtr(), args[0].Clone()));
  }

  void Done(const base::ListValue& args) {
    if (args.empty()) {
      return;
    }
    if (!Writer()) {
      Answer(args[0].Clone(), base::Value(false));
      return;
    }
    writes_.AsyncCall(&Writes::Finish)
        .Then(base::BindOnce(&BoardHandler::Finished, weak_factory_.GetWeakPtr(), args[0].Clone()));
  }

  void Answered(base::Value id, bool taken) { Answer(std::move(id), base::Value(taken)); }

  void Finished(base::Value id, std::optional<std::string> name) {
    LOG(ERROR) << "browser: board "
               << (!name           ? std::string("refused a write")
                   : name->empty() ? std::string("saved")
                                   : "added " + *name);
    Answer(std::move(id), name ? base::Value(std::move(*name)) : base::Value(false));
  }

  void Answer(base::Value id, base::Value answer) {
    if (IsJavascriptAllowed()) {
      ResolveJavascriptCallback(id, answer);
    }
  }

  // Empty where it keeps no Boards: a private window's profile, or a guest's.
  const base::FilePath profile_dir_;
  base::FilePath board_;
  bool owner_ = false;
  base::SequenceBound<Writes> writes_;
  base::WeakPtrFactory<BoardHandler> weak_factory_{this};
};

class BoardUI : public content::WebUIController {
 public:
  explicit BoardUI(content::WebUI* web_ui) : content::WebUIController(web_ui) {
    Profile* profile = Profile::FromWebUI(web_ui);
    content::URLDataSource::Add(profile, std::make_unique<BoardSource>(profile));
    content::URLDataSource::Add(
        profile, std::make_unique<FaviconSource>(profile, chrome::FaviconUrlFormat::kFavicon2));
    web_ui->AddMessageHandler(std::make_unique<BoardHandler>(ProfileDir(profile)));
  }

  WEB_UI_CONTROLLER_TYPE_DECL();
};

WEB_UI_CONTROLLER_TYPE_IMPL(BoardUI)

class BoardUIConfig : public content::WebUIConfig {
 public:
  BoardUIConfig() : content::WebUIConfig("chrome", kHost) {}

  std::unique_ptr<content::WebUIController> CreateWebUIController(content::WebUI* web_ui,
                                                                  const GURL& url) override {
    return std::make_unique<BoardUI>(web_ui);
  }
};

}  // namespace

void RegisterBoards() {
  content::WebUIConfigMap::GetInstance().AddWebUIConfig(std::make_unique<BoardUIConfig>());
}

}  // namespace browser_core
