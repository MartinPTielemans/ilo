#!/usr/bin/env python3
"""Splits app/src/main.rs into modules by feature, by moving its top-level items by name.

Lanes keep changing main.rs, so this is a recipe rather than a branch: run it on the newest main
and the tree it leaves builds. It reads main.rs as Rust tokens, never by line or by layout, so it
works before and after `cargo fmt`. Each moved item keeps its text, comments and #[cfg]; what
moving requires is all that changes:

- a moved item, field or inherent method with no visibility gets `pub(crate)`, as it was visible
  to the whole crate from the crate root;
- each new module starts with `use super::*;`, so it sees main.rs's imports and items as before;
- main.rs declares each module and imports it whole with `use name::*;`, so `crate::item` paths
  in the other modules still resolve;
- once app/rustfmt.toml exists, rustfmt formats main.rs and the new files.

An item the recipe names and main.rs no longer has is reported and skipped; an item main.rs has
and the recipe doesn't name stays in main.rs. `--list` prints main.rs's items and where each goes.

    tools/split-main.py [--list]
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAIN = ROOT / "app" / "src" / "main.rs"

# ---------------------------------------------------------------------------------------------
# Tokens


def tokenize(src):
    """Returns (kind, start, end) for each token: 'ws', 'comment', 'ident', 'lifetime', 'lit'
    or 'punct'. Whitespace and comments are kept so items carry them."""
    tokens = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c.isspace():
            j = i
            while j < n and src[j].isspace():
                j += 1
            tokens.append(("ws", i, j))
            i = j
        elif src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            tokens.append(("comment", i, j))
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            tokens.append(("comment", i, j))
            i = j
        elif m := re.compile(r'(?:b|c)?r(#*)"').match(src, i):
            close = '"' + m.group(1)
            j = src.index(close, m.end()) + len(close)
            tokens.append(("lit", i, j))
            i = j
        elif m := re.compile(r'(?:b|c)?"').match(src, i):
            j = m.end()
            while src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            tokens.append(("lit", i, j + 1))
            i = j + 1
        elif m := re.compile(r"b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'])'").match(src, i):
            tokens.append(("lit", i, m.end()))
            i = m.end()
        elif c == "'":
            m = re.compile(r"'[A-Za-z_][A-Za-z0-9_]*").match(src, i)
            tokens.append(("lifetime", i, m.end()))
            i = m.end()
        elif c.isalpha() or c == "_" or ord(c) > 127:
            m = re.compile(r"(?:r#)?[\w]+").match(src, i)
            tokens.append(("ident", i, m.end()))
            i = m.end()
        elif c.isdigit():
            m = re.compile(r"[0-9][0-9A-Za-z_]*(?:\.[0-9][0-9A-Za-z_]*)?").match(src, i)
            tokens.append(("lit", i, m.end()))
            i = m.end()
        else:
            tokens.append(("punct", i, i + 1))
            i += 1
    return tokens


class Toks:
    """The meaningful tokens (no whitespace or comments) of a source, with their text."""

    def __init__(self, src):
        self.src = src
        self.all = tokenize(src)
        self.code = [t for t in self.all if t[0] not in ("ws", "comment")]

    def text(self, k):
        t = self.code[k]
        return self.src[t[1] : t[2]]

    def matching(self, k):
        """The index of the bracket closing the one at k."""
        pairs = {"(": ")", "[": "]", "{": "}"}
        stack = []
        while True:
            s = self.text(k)
            if s in pairs:
                stack.append(pairs[s])
            elif s in (")", "]", "}"):
                assert stack.pop() == s, f"unbalanced {s} at {self.code[k][1]}"
                if not stack:
                    return k
            k += 1


# ---------------------------------------------------------------------------------------------
# Items

ITEM_KEYWORDS = {"fn", "struct", "enum", "union", "trait", "impl", "mod", "const", "static",
                 "type", "use", "extern", "macro_rules"}


class Item:
    def __init__(self, kind, name, start, end, head, kw):
        self.kind, self.name = kind, name
        self.start, self.end = start, end  # code token range [start, end]
        self.head = head  # first token after the attributes
        self.kw = kw  # the token naming the kind (fn, struct, impl, ...)

    @property
    def key(self):
        return f"{self.kind} {self.name}"


def joined(t, ks):
    """Tokens' text with a space only between two words, as `dialog::Host for DialogHost`."""
    out, word = "", False
    for k in ks:
        w = t.code[k][0] in ("ident", "lit", "lifetime")
        out += (" " if w and word else "") + t.text(k)
        word = w
    return out


def skip_vis(t, k):
    if t.text(k) == "pub":
        k += 1
        if t.text(k) == "(":
            k = t.matching(k) + 1
    return k


def items(t):
    """Splits a file's tokens into its top-level items."""
    out = []
    k, n = 0, len(t.code)
    while k < n:
        start = k
        # Inner attributes (#![...]) are items of their own; outer ones belong to the next item.
        if t.text(k) == "#" and t.text(k + 1) == "!":
            end = t.matching(k + 2)
            out.append(Item("inner", t.src[t.code[k][1] : t.code[end][2]], start, end, k, k))
            k = end + 1
            continue
        while t.text(k) == "#":
            k = t.matching(k + 1) + 1
        head = k
        k = skip_vis(t, k)
        # Qualifiers before the keyword.
        while t.text(k) in ("unsafe", "async", "default", "safe") or (
            t.text(k) == "const" and t.text(k + 1) in ("fn", "unsafe", "async", "extern")
        ) or (t.text(k) == "extern" and t.code[k + 1][0] == "lit" and t.text(k + 2) in ("fn", "unsafe")):
            k += 2 if t.text(k) == "extern" else 1
        kw = k
        word = t.text(k)
        if t.code[k][0] == "ident" and t.text(k + 1) == "!" and word != "macro_rules":
            # A macro call: thread_local! { static NAME ... }, actions!(ns, [A, B]), ...
            body = k + 2
            close = t.matching(body)
            end = close + 1 if close + 1 < n and t.text(close + 1) == ";" else close
            inner = [t.text(j) for j in range(body + 1, close) if t.code[j][0] == "ident"]
            if word == "thread_local":
                name = inner[inner.index("static") + 1]
            elif word == "actions":
                name = ",".join(x for x in inner[1:])
            else:
                name = " ".join(inner[:3])
            out.append(Item(word + "!", name, start, end, head, kw))
            k = end + 1
            continue
        if word == "macro_rules":
            name = t.text(k + 2)
            close = t.matching(k + 3)
            end = close + 1 if close + 1 < n and t.text(close + 1) == ";" else close
            out.append(Item("macro_rules!", name, start, end, head, kw))
            k = end + 1
            continue
        assert word in ITEM_KEYWORDS, f"unknown item at {t.src[t.code[k][1]:t.code[k][1]+60]!r}"
        if word == "extern" and t.text(k + 1) == "crate":
            word = "extern crate"
        # The item ends at a ; outside brackets, or at the close of its body.
        braced = word in ("fn", "struct", "enum", "union", "trait", "impl", "mod", "extern")
        j = k + 1
        while True:
            s = t.text(j)
            if s == ";":
                end = j
                break
            if s == "{" and braced:
                end = t.matching(j)
                break
            if s in ("(", "[", "{"):
                j = t.matching(j)
            j += 1
        if word == "impl":
            # Named by its header without generics: `impl dialog::Host for DialogHost`.
            header, j = [], k + 1
            # Skip impl<...> generics.
            if t.text(j) == "<":
                depth = 0
                while True:
                    s = t.text(j)
                    depth += (s == "<") - (s == ">")
                    j += 1
                    if depth == 0 and s == ">":
                        break
            while t.text(j) not in ("{", "where"):
                header.append(j)
                j += 1
            name = joined(t, header)
        elif word == "extern":
            # extern "C" { ... }: named by its first function.
            names = [t.text(x + 1) for x in range(k, end) if t.text(x) == "fn"]
            name = names[0] if names else "?"
        elif word == "use":
            name = "".join(t.text(x) for x in range(k + 1, end))
        else:
            name = t.text(k + 1)
        out.append(Item(word, name, start, end, head, kw))
        k = end + 1
    return out


# ---------------------------------------------------------------------------------------------
# Visibility


def has_vis(t, k):
    return t.text(k) == "pub"


def to_end(t, j, close, fn_ends_at_body):
    """The last token of the member, field or static starting at j, before `close`."""
    while j < close:
        s = t.text(j)
        if s == ";" or (s == "," and not fn_ends_at_body):
            return j
        if s == "{" and fn_ends_at_body:
            return t.matching(j)
        if s in ("(", "[", "{"):
            j = t.matching(j)
        j += 1
    return close - 1


def vis_inserts(t, item):
    """Where `pub(crate) ` goes in a moved item: before the item if it has no visibility, and
    before each field, inherent method or thread-local static that has none."""
    at = []
    kind = item.kind
    if kind == "thread_local!":
        body = item.kw + 2
        close = t.matching(body)
        j = body + 1
        while j < close:
            while t.text(j) == "#":
                j = t.matching(j + 1) + 1
            if not has_vis(t, j):
                at.append(j)
            j = to_end(t, j, close, False) + 1
        return at
    if kind not in ("fn", "struct", "enum", "union", "trait", "impl", "mod", "const", "static",
                    "type"):
        return at
    if kind != "impl" and not has_vis(t, item.head):
        at.append(item.head)
    if kind in ("struct", "union"):
        j = item.kw + 2
        if t.text(j) == "<":
            depth = 0
            while True:
                depth += (t.text(j) == "<") - (t.text(j) == ">")
                j += 1
                if depth == 0:
                    break
        while t.text(j) not in ("{", "(", ";"):
            j += 1
        if t.text(j) != ";":
            close = t.matching(j)
            j += 1
            while j < close:
                while t.text(j) == "#":
                    j = t.matching(j + 1) + 1
                if j >= close:
                    break
                if not has_vis(t, j):
                    at.append(j)
                # A field ends at a , outside brackets and generics.
                depth = 0
                while j < close:
                    s = t.text(j)
                    if s in ("(", "[", "{"):
                        j = t.matching(j)
                    elif s == "<":
                        depth += 1
                    elif s == ">" and t.text(j - 1) != "-":
                        depth -= 1
                    elif s == "," and depth == 0:
                        break
                    j += 1
                j += 1
    elif kind == "impl" and " for " not in f" {item.name} ":
        j = item.kw
        while t.text(j) != "{":
            j += 1
        close = t.matching(j)
        j += 1
        while j < close:
            while t.text(j) == "#":
                j = t.matching(j + 1) + 1
            if j >= close:
                break
            if not has_vis(t, j):
                at.append(j)
            k = j
            while t.text(k) in ("const", "unsafe", "async", "extern") or t.code[k][0] == "lit":
                if t.text(k) == "const" and t.text(k + 1) not in ("fn", "unsafe", "async", "extern"):
                    break
                k += 1
            j = to_end(t, j, close, t.text(k) == "fn") + 1
    return at


# ---------------------------------------------------------------------------------------------
# The split


def line_start(src, a):
    return src.rfind("\n", 0, a) + 1


def split(plan, list_only=False):
    src = MAIN.read_text()
    t = Toks(src)
    its = items(t)
    by_key = {}
    for it in its:
        by_key.setdefault(it.key, []).append(it)

    where, missing = {}, []
    for module, _, keys in plan:
        for key in keys:
            if key not in by_key:
                missing.append((module, key))
                continue
            for it in by_key[key]:
                if id(it) in where:
                    sys.exit(f"{key} is in both {where[id(it)]} and {module}")
                where[id(it)] = module

    # A module declared `mod name;` is in its own file already.
    if all(it.kind == "mod" and t.text(it.end) == ";" for it in its if id(it) in where):
        sys.exit("main.rs is split already")

    if list_only:
        for it in its:
            lines = src.count("\n", t.code[it.start][1], t.code[it.end][2]) + 1
            print(f"{where.get(id(it), 'main'):20} {lines:5}  {it.key}")
        for module, key in missing:
            print(f"missing for {module}: {key}", file=sys.stderr)
        return

    # Each item's text runs from the comments before it, back to the previous item, to its end
    # and a comment on its closing line.
    edits = []  # (start, end, replacement) in src
    texts = {module: [] for module, _, _ in plan}
    own_files = {}
    prev_end = 0
    for it in its:
        start = t.code[it.start][1]
        lead = start
        for kind, a, _ in reversed([a for a in t.all if prev_end <= a[1] < start]):
            if kind not in ("comment", "ws"):
                break
            lead = a
        while src[lead] in " \t\r\n":
            lead += 1
        end = t.code[it.end][2]
        m = re.compile(r"[ \t]*//[^\n]*").match(src, end)
        if m:
            end = m.end()
        prev_end = end
        module = where.get(id(it))
        if not module:
            continue
        if src[line_start(src, lead) : lead].strip():
            sys.exit(f"{it.key} shares its first line with what's before it; give it a line")
        if it.kind == "mod" and t.text(it.end) == ";":
            continue  # A file of its own already.
        if it.kind == "mod" and plan_keys(plan, module) == [f"mod {module}"]:
            # An inline module moves to a file of its own; its declaration and docs stay.
            body_open = next(k for k in range(it.kw, it.end) if t.text(k) == "{")
            body = src[t.code[body_open][2] : t.code[it.end][1]]
            own_files[module] = dedent(body)
            edits.append((t.code[body_open - 1][2], t.code[it.end][2], ";"))
            continue
        inserts = sorted(t.code[k][1] for k in vis_inserts(t, it))
        text, pos = "", lead
        for at in inserts:
            text += src[pos:at] + "pub(crate) "
            pos = at
        texts[module].append(text + src[pos:end])
        # The cut takes its lines and the blank lines after them.
        cut_end = end
        while cut_end < len(src) and src[cut_end] in " \t\r":
            cut_end += 1
        if src.startswith("\n", cut_end):
            cut_end += 1
            while (n := re.compile(r"[ \t\r]*\n").match(src, cut_end)):
                cut_end = n.end()
        edits.append((line_start(src, lead), cut_end, ""))

    # The modules' declarations go after main.rs's last `mod name;`.
    decls = [it for it in its if it.kind == "mod" and t.text(it.end) == ";" and id(it) not in where]
    at = t.code[decls[-1].end][2] if decls else 0
    new = [m for m, _, _ in plan if texts[m]]
    edits.append((at, at, "".join(f"\nmod {m};\nuse {m}::*;" for m in new)))

    for module, _, _ in plan:
        if (texts[module] or module in own_files) and (MAIN.parent / f"{module}.rs").exists():
            sys.exit(f"app/src/{module}.rs exists already: run this on a clean checkout")
    written = [MAIN]
    for module, doc, _ in plan:
        body = own_files.get(module)
        if body is None:
            if not texts[module]:
                continue
            body = "use super::*;\n\n" + "\n\n".join(texts[module]) + "\n"
            body = "".join(f"//! {line}".rstrip() + "\n" for line in doc.splitlines()) + "\n" + body
        path = MAIN.parent / f"{module}.rs"
        path.write_text(body)
        written.append(path)
        print(f"app/src/{module}.rs: {len(texts[module]) or 'the module'}")

    out = src
    for a, b, text in sorted(edits, key=lambda e: (e[0], e[1]), reverse=True):
        out = out[:a] + text + out[b:]
    MAIN.write_text(out)
    # Once the app's Rust is kept formatted (app/rustfmt.toml), so is what moved: `pub(crate)` can
    # take a line past the width.
    if (MAIN.parent.parent / "rustfmt.toml").exists():
        subprocess.run(["rustfmt", "--config-path", str(MAIN.parent.parent / "rustfmt.toml"),
                        *map(str, written)], check=True)
    for module, key in missing:
        print(f"warning: main.rs has no {key}, for {module}; it's skipped", file=sys.stderr)


def plan_keys(plan, module):
    return next(keys for m, _, keys in plan if m == module)


def dedent(body):
    """An inline module's body as a file: its indent taken off, if every line has it."""
    lines = body.strip("\n").split("\n")
    indents = [len(line) - len(line.lstrip(" ")) for line in lines if line.strip()]
    cut = min(indents, default=0)
    if any(line.strip() and not line.startswith(" " * cut) for line in lines):
        cut = 0
    return "\n".join(line[cut:] for line in lines) + "\n"


# ---------------------------------------------------------------------------------------------
# The recipe: each module, its doc line, and the items it takes from main.rs, as `kind name`
# (`--list` prints main.rs's). What isn't named stays in main.rs: startup and wiring.

PLAN = [
    ("platform", "", ["mod platform"]),
    ("browser_windows", """\
The browser windows the app keeps, and the ways to reach a window's shell and view from
the core's ids, the host's views and GPUI's updates.""", [
        "struct BrowserWindow", "impl BrowserWindow", "enum Own",
        "fn front", "fn ui_view_of", "fn full_screen_changed", "fn swiped", "fn option_tapped",
        "fn window_with_view", "fn shell_with_view", "fn key_window_changed", "fn ui_keyboard_left",
        "fn ui_keyboard_left_in", "fn update_shells", "fn update_shells_in", "fn update_window",
        "fn update_window_in", "fn tab_window", "fn update_tab_window", "fn update_app",
        "fn update_app_later", "fn fading_view", "fn ns_view", "fn set_page_frame", "fn set_frames",
        "fn rect", "fn window_resized", "fn window_activated", "fn window_option_tapped",
    ]),
    ("window_opening", """\
Opening a browser window and the shell in it, where it opens, the overlays' windows it makes,
and its closing.""", [
        "const WINDOW_SIZE", "fn open_browser_window", "fn saved_frames", "fn placed", "fn screens",
        "fn log_frame", "fn open_window_after", "fn open_window", "fn open_window_as",
        "thread_local! FINISHED_LAUNCHING", "fn overlays_now", "fn with_overlays",
        "fn finish_opening", "fn elevate", "fn window_closed",
    ]),
    ("engine", """\
A window's shell's requests, as calls to the core, and the helpers that read the core's
answers.""", [
        "struct Engine", "struct Video", "impl Drop for Engine", "impl Browser for Engine",
        "fn name", "fn ids", "fn core_text",
    ]),
    ("tabs", """\
Tabs: opening and splitting them, what the core says of their pages, and what's dragged in
and out of a window.""", [
        "fn open_tab", "fn open_tab_behind", "fn split_tab", "fn split_opened", "fn open_split",
        "fn create_space", "fn url_for", "fn window_selected", "fn window_page_elsewhere",
        "fn tab_loaded", "fn tab_state_changed", "fn tab_title_changed", "fn tab_favicon_changed",
        "fn tab_history", "fn page_favicon", "fn tab_opened", "fn tab_closed", "fn tab_find_changed",
        "fn tab_fullscreen_changed", "fn control_released", "fn file_preview",
        "fn file_drag_ended", "fn dragged",
    ]),
    ("spaces", """\
What the core says of the Spaces, their favorites, the archive and the sidebar's backups.""", [
        "fn spaces_changed", "fn archive_note_changed", "fn show_archive_note",
        "fn favorites_changed", "fn space_changed", "fn private_name", "fn window_space_shown",
        "fn archive_changed", "fn sidebar_backups_changed", "fn export_spaces",
    ]),
    ("pages", """\
What a tab's page asks of the user: its dialogs, permissions, Cast, its menu, the Site
Controls, offers, protected media and sharing.""", [
        "fn ask", "fn has_share_sheet", "fn share_page", "fn use_system_engine",
        "fn tab_site_controls", "fn tab_offer_changed", "fn tab_passwords_changed",
        "fn answer_page_dialog", "fn tab_dialog", "fn tab_permission_asked", "fn tab_cast",
        "fn tab_page_menu", "fn tab_media_unsupported", "fn widevine_changed",
        "fn window_suggestions",
    ]),
    ("downloads", """\
Downloads: the list, their icons, and the question a quit with downloads running asks.""", [
        "fn downloads_changed", "fn download_icon", "fn window_downloads_question",
        "fn window_show_downloads",
    ]),
    ("extensions", """\
Extensions: their actions, questions, side panels and popups.""", [
        "const WEB_STORE", "fn extensions_changed", "fn window_extension_actions",
        "fn window_extension_question", "fn window_side_panel", "fn window_extension_popup",
    ]),
    ("profile_data", """\
What Settings asks the core for of a profile: its logins, addresses and cards, the account's
sync and devices, and what an import brought.""", [
        "fn remove_login", "fn logins", "fn profile_autofill_changed", "fn profile_logins_changed",
        "fn sync_changed", "fn sync_pairing_asked", "fn sync_devices_changed",
        "fn passwords_imported", "fn history_added", "fn addresses_added",
    ]),
    ("agent_events", """\
What the core says of agents' sessions and actions in tabs.""", [
        "fn agent_activity", "fn agent_session_ended", "fn agent_page_done",
        "fn agent_page_element", "fn tab_text",
    ]),
    ("links", """\
Links from other apps and from pages' menus, and the windows and Spaces they open in.""", [
        "const CLEAR_BROWSING_DATA", "fn open_links", "fn launched_again", "fn open_in_profile",
        "fn clear_browsing_data", "fn open_links_in", "fn open_link_in", "fn users_space_of",
        "fn window_open_link",
    ]),
    ("little_ilo", """\
Little ilo: its windows, their size and place, the Dock's menu and the menu bar's button.""", [
        "const LITTLE_SIZE", "fn little_size_path", "fn little_size", "fn main_window_for",
        "fn little_home", "fn little_tab_opened", "fn little_frame", "fn open_little",
        "fn open_links_in_little", "fn dock_little", "fn dock_chosen", "fn new_little_window",
        "fn status_item_clicked", "fn little_in_menu_bar_path", "fn little_in_menu_bar",
        "fn set_little_in_menu_bar", "fn window_open_little", "fn home_in_profile",
        "fn window_reopen_little", "fn save_little_size",
    ]),
    ("keys", """\
Keys: those a page's shortcut or the UI takes, those held for the UI, the input method's text,
and the Windows driver's input.""", [
        "fn focus_ui", "fn keystroke", "fn run_page_shortcut", "thread_local! SECOND_PRESS",
        "fn tab_key_before_page", "fn held_keystroke", "fn give_held_keys",
        "fn held_keys_unclaimed", "fn tab_key_unhandled", "fn ui_composed", "fn ui_committed",
        "fn browser_drive_compose", "fn browser_drive_commit", "fn browser_drive_marked",
        "fn browser_drive_fingers",
    ]),
    ("menu_bar", """\
The menu bar and its menus.""", [
        "actions! Hide,HideOthers,ShowAll,Minimize", "actions! CheckForUpdates", "struct Menus",
        "impl Menus", "fn show_menus", "fn menus_closed", "fn edit_title", "fn menus",
        "fn appearance_title", "fn give_host_appearance", "fn restore_sidebar",
        "fn change_profile",
    ]),
    ("hosts", """\
The hosts of the library's overlays and popups: dialogs, menus, the Site Controls, toasts,
the zoom HUD, offers, the pickers, tooltips, hover cards and the link target.""", [
        "const COMMAND_BAR", "const ICON_PICKER", "const THEME_PICKER", "const FOLDER_PREVIEW",
        "const SITE_CONTROLS", "fn show_menu",
        "struct DialogHost", "impl dialog::Host for DialogHost",
        "struct DockDot", "impl dock_dot::Host for DockDot",
        "struct SiteControlsHost", "fn set_site_controls_shape",
        "impl site_controls::Host for SiteControlsHost", "thread_local! MENU_OVER",
        "struct MenuHost", "impl menu::Host for MenuHost", "fn menu_pointer", "fn menu_dismissed",
        "fn menu_choose", "fn site_controls_dismissed",
        "struct ToastHost", "impl toast::Host for ToastHost",
        "struct HudHost", "impl zoom_hud::Host for HudHost",
        "struct OfferHost", "impl offer::Host for OfferHost",
        "struct PickerHost", "impl icon_picker::Host for PickerHost", "fn recent_emoji_path",
        "fn recent_emoji", "fn icon_picker_dismissed",
        "struct FolderPreviewHost", "impl folder_preview::Host for FolderPreviewHost",
        "fn folder_preview_dismissed",
        "struct ThemeHost", "impl theme_picker::Host for ThemeHost", "fn theme_picker_dismissed",
        "struct TooltipHost", "impl tooltip::Host for TooltipHost", "fn tooltip_away",
        "struct CardHost", "impl hover_card::Host for CardHost", "fn card_away",
        "fn memory_pressed",
        "struct LinkTargetHost", "impl link_target::Host for LinkTargetHost", "fn link_pointer",
        "fn tab_link_target", "fn crash_report_offered",
    ]),
    ("command_bar_host", """\
The command bar's host: its requests, as calls to the core and the host's overlay.""", [
        "struct Bar", "impl command_bar::Host for Bar", "fn rgba_u32", "fn command_bar_dismissed",
    ]),
    ("link_preview_host", """\
The link preview's host, and the links dragged out of pages to a Space or a preview.""", [
        "struct LinkPreviewHost", "impl link_preview::Host for LinkPreviewHost",
        "fn link_preview_dismissed", "thread_local! LINK_PREVIEWS", "fn tab_link_dragged",
        "fn tab_link_to_site", "fn tab_link_to_preview", "fn link_preview_key",
    ]),
    ("capture_host", """\
The capture's host: screenshots and recordings of pages, where they're kept, and the elements
and pictures the core takes of a page.""", [
        "struct CaptureHost", "impl capture::Host for CaptureHost", "fn media_dir",
        "fn save_capture", "fn list_captures", "fn capture_name", "fn capture_dismissed",
        "fn tab_element_box", "fn tab_captured", "fn trace_recorded", "fn tab_pictured",
    ]),
    ("updates_host", """\
Updates: Sparkle on the Mac, ilo's own on Windows and Flatpak's portal on Linux, the track the
user follows, and the release notes an update's first launch opens.""", [
        "fn first_launch_notes", "fn keep_update_notes", "fn updates_available", "fn start_updates",
        "fn followed_track_path", "fn followed_track", "fn check_for_updates", "fn updated_by_store",
        "struct SystemUpdater", "impl updates::Host for SystemUpdater", "fn update_changed",
        "fn relaunch_into_newest", "fn relaunch_into_update",
    ]),
    ("quitting", """\
Quitting and restarting, and what's kept for the next launch: the launch picture and frame.""", [
        "fn quit_because", "fn restart_because", "fn quit_going_ahead",
        "fn private_session_ended", "fn stopping", "fn restart_browser",
        "fn launch_picture_on_resigning", "fn save_launch_picture", "fn forget_launch_pictures",
        "fn save_launch_frame", "fn launch_frame_path",
    ]),
]


if __name__ == "__main__":
    split(PLAN, list_only="--list" in sys.argv[1:])
