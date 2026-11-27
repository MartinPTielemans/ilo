#ifndef BROWSER_CORE_AGENT_FILE_FIELD_RULE_H_
#define BROWSER_CORE_AGENT_FILE_FIELD_RULE_H_

#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace browser_core {

// A file for a page's file field in an agent's tab comes only from the user (#3656): the page's
// own file dialogs are refused there, and the user's Choose File… on the agent's card opens the
// browser's, whose files go into the field the card named, or nowhere. Apart from content so
// browser_core_unittests tests it alone.

// Whether a page may open a file dialog, a file input's or a File System Access picker, in a tab
// that is |agents| (in an agent's Space) and |held| (by the user, from agents). Not in an agent's
// tab the user doesn't hold, where nobody watches what it hands the page.
bool PageMayOpenFileDialog(bool agents, bool held);

// A field as DevTools describes it: its node's name and attributes, name then value.
struct FileFieldFacts {
  std::string name;
  std::vector<std::string> attributes;
};

// Why the user can't be asked for a file for |field|, for the agent, or none if they can: it must
// be a file input, and not one for a folder.
std::optional<std::string> FileFieldRefusal(const FileFieldFacts& field);
// Whether |field| takes more than one file.
bool FileFieldTakesMany(const FileFieldFacts& field);
// The types |field|'s accept attribute lists, lowercased, those a file dialog can filter by: MIME
// types, as image/png or image/*, and extensions, as .pdf. None means any file.
std::vector<std::string> FileFieldAccepts(const FileFieldFacts& field);

// Where the field was as its card showed, and where it is as the user picks: the DevTools id of
// its frame, and that frame's origin.
struct FileFieldPlace {
  std::string frame;
  std::string origin;
};

// A child of a document as DevTools describes it (DOM.describeNode, depth 1): its node type, name
// and local name, the frame id DevTools gives it, if any, and whether it holds a document of its
// own.
struct DocumentChild {
  int type = 0;
  std::string name;
  std::string local_name;
  std::string frame;
  bool holds_document = false;
};

// The DevTools id of the frame whose document has |children|, which DevTools gives that document's
// element child, or empty if it can't be told. An element child that is itself a frame owner, as
// a page's script can make one, gets its content frame's id too: Blink sets that first and the
// document's own over it (InspectorDOMAgent::BuildObjectForNode), so one more frame id there would
// name another site's frame for a field in this document. Such a child is refused, whatever the
// order.
std::string DocumentFrame(const std::vector<DocumentChild>& children);

// Why the user can't be asked for a file for a field at |place|, for the agent, or none if they
// can: its frame must have a site the card can name, which one with an opaque origin, as a
// sandboxed or data: frame's, hasn't.
std::optional<std::string> FileFieldPlaceRefusal(const FileFieldPlace& place);

// The field as the user picks |count| files for it.
struct FileFieldNow {
  // Whether the frame still has the document it had as the card showed, the tab is still in the
  // Space of the agent the card was for, and the user holds it.
  bool same_document = false;
  bool agents = false;
  bool held = false;
  // Whether DevTools still knows the field's node, in its document: one the page removed, or put
  // in place of another, isn't.
  bool in_page = false;
  FileFieldPlace place;
  FileFieldFacts facts;
};

// Whether the user's |count| files go into the field the card at |shown| named: the same node, in
// the same document of the same frame, from the same origin, one with a site, still a file field
// that takes them, in a tab still that agent's that the user doesn't hold. Any miss, and they go
// nowhere.
bool FilesLand(const FileFieldPlace& shown, const FileFieldNow& now, size_t count);

// The field as the app asks to put |count| files of its own into it (bc_file_field_put).
struct FileFieldPutNow {
  // Whether the frame still has the document it had as the field was described, and the frame's
  // origin now; whether the tab is still in the Space of the agent the field was for, and the user
  // holds it; and whether the field takes several files.
  bool same_document = false;
  std::string origin;
  bool agents = false;
  bool held = false;
  bool many = false;
};

// Whether the app's |count| files may start into the field described at |shown|: some, one unless
// it takes several, in the same document, from the same origin, one with a site, in a tab still
// that agent's that the user doesn't hold. Any miss refuses the call itself, with no answer, as
// nothing has begun; FilesLand checks the node again as they go in.
bool MayPutFiles(const FileFieldPlace& shown, const FileFieldPutNow& now, size_t count);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_FILE_FIELD_RULE_H_
