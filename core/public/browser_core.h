// The core's API: what the app calls in Chrome's framework. Plain C, so the app binds it without
// sharing a C++ toolchain with Chromium. Everything runs on the main thread, callbacks included.

#ifndef BROWSER_CORE_PUBLIC_BROWSER_CORE_H_
#define BROWSER_CORE_PUBLIC_BROWSER_CORE_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifdef _WIN32
#define BC_EXPORT __declspec(dllexport)
#else
#define BC_EXPORT __attribute__((visibility("default")))
#endif

// Names a window, a Space or a tab for as long as the process runs; never reused. 0 names nothing.
typedef uint64_t bc_id;

// In points. A window's is on the screen, from the top-left of the main display; a tab's is in its
// window, from the top-left of the window's content.
typedef struct bc_rect {
  double x, y, width, height;
} bc_rect;

// Modifier keys held with a key, as flags.
#define BC_MODIFIER_SHIFT 1
#define BC_MODIFIER_CONTROL 2
// Option on the Mac.
#define BC_MODIFIER_ALT 4
// Command on the Mac.
#define BC_MODIFIER_META 8

// A key going down.
typedef struct bc_key {
  // The key as the web names it, for the keyboard's layout: "t", "T", "Enter", "ArrowLeft".
  const char* key;
  // The BC_MODIFIER_ flags held.
  uint32_t modifiers;
  // The page's focused element takes text, as a text field does, so editing keys such as ⌘Z are
  // its own.
  bool editing;
  // With ⌘ held on a layout that isn't Latin, such as Hebrew, Arabic or Armenian, the key as the
  // user's Latin layout types it, with ⇧ if it's held, as the Mac's menus match ⌘ shortcuts: "t"
  // for the key Hebrew types "א" with. Empty otherwise, and null for an extension's command.
  const char* latin_key;
} bc_key;

// What's wrong with a tab's page, if anything.
#define BC_PAGE_FINE 0
// Its renderer died: it crashed, was killed, ran out of memory or never started. The page shows
// nothing until bc_tab_reload. Only while a window shows the tab: one that isn't shown, or stops
// being, is unloaded instead and loads once it's shown.
#define BC_PAGE_CRASHED 1
// Its renderer hasn't answered the user's input for Chrome's hang timeout. It may come back by
// itself; bc_tab_wait waits longer, and bc_tab_reload ends the renderer and loads the page again.
#define BC_PAGE_HUNG 2
// It's Chromium's error page for a load that failed, such as for no network: net_error says why.
// Chromium loads it again once the network is back, while it's shown.
#define BC_PAGE_NETWORK_ERROR 3

// Where a tab's page is, and its sound: what the URL field, back, forward, reload and the tab's
// speaker show.
typedef struct bc_tab_state {
  // The URL the tab shows: the one it's going to if the user asked for it, else its page's.
  const char* url;
  // There's a page to go back or forward to.
  bool can_go_back;
  bool can_go_forward;
  // The page is loading, so reload shows as stop. A page changing its own URL in place isn't.
  bool loading;
  // The tab has no page loaded, as a tab put back and not yet shown, a pinned tab closed, or a tab
  // unloaded unseen (bc_unload_idle_tabs). It loads once it's shown.
  bool unloaded;
  // The tab is pinned or a favorite, and its page isn't the one it was pinned or made a favorite
  // at. bc_tab_back_to_pinned_url goes back there.
  bool away_from_pinned_url;
  // The page is playing sound, muted or not, or was in the last 2 s, as Chrome's speaker shows.
  bool audible;
  // The tab's sound is muted, by bc_tab_set_muted or an extension.
  bool muted;
  // The emoji bc_tab_set_icon gave the tab to show in place of its page's favicon, UTF-8, or empty.
  const char* icon;
  // What's wrong with the page: a BC_PAGE_ value. The UI and agents read the same one.
  int trouble;
  // With BC_PAGE_NETWORK_ERROR, Chromium's net error code, such as -106 for no network, else 0.
  int net_error;
  // The system's engine shows the page, WebKit on the Mac, in place of Chromium
  // (bc_tab_set_system_engine).
  bool system_engine;
  // The page uses the camera, or the microphone, as Chrome's tab strip marks it.
  bool camera;
  bool microphone;
  // The page shares a screen, a window or another tab, as a call does.
  bool sharing;
  // Another page shares this tab, as a call shows it.
  bool shared;
  // Another page shares a tab, and bc_tab_share_instead would have it share this one.
  bool share_instead;
  // The page's media, as Chrome's media controls show it: whether it has a session the browser
  // can play, pause and skip (bc_tab_media), whether that's playing, its title and artist, each
  // empty without one, whether it has a next track, and whether it can float in Picture in Picture
  // while nothing of the page floats.
  bool media;
  bool media_playing;
  const char* media_title;
  const char* media_artist;
  bool media_next;
  bool media_picture_in_picture;
  // The tab shows its page in Developer Mode (bc_tab_set_developer_mode): the user turned it on for
  // the page's site, or it's localhost, as Chrome counts it, and they didn't turn it off there.
  bool developer_mode;
} bc_tab_state;

// A tab's find in page, as its find bar shows it.
typedef struct bc_find_state {
  // The find bar is open. The tab going to another page closes it, as in Chrome.
  bool open;
  // The find bar's text.
  const char* text;
  // How many matches the page has for it, or -1 before it has looked. It counts as it goes.
  int matches;
  // The selected match, from 1, or 0 for none.
  int selected;
} bc_find_state;

// One of the command bar's suggestions, as Chrome's address bar lists them.
typedef struct bc_suggestion {
  // Where it goes: a page, or a search for what was typed.
  const char* url;
  // What its row shows: a page's title, else its URL, or the words a search is for.
  const char* text;
  // What follows, dimmer: the page's URL when the row shows its title, the search engine, or "".
  const char* detail;
  // A tab already at the page, other than the one the window shows, which the row switches to
  // with bc_window_select. Else 0, as for the first suggestion, which goes where what was typed
  // goes.
  bc_id tab;
  // For the first suggestion, what to show after the text typed, selected, to complete it to this
  // suggestion's URL or title, when bc_window_suggest allowed it. Else "".
  const char* completion;
  // Whether bc_window_remove_suggestion can take it out of the profile's history.
  bool removable;
  // The keyword of the search engine whose site search Tab enters on this row, as Chrome's address
  // bar hints "Search <site>": what was typed names the engine's site, or the row's page does.
  // Else "".
  const char* keyword;
  // That engine's name, such as "DuckDuckGo", or "".
  const char* site;
} bc_suggestion;

// A page in a tab's history, which bc_tab_history lists, as Chrome's back and forward buttons'
// menu does. Strings are UTF-8.
typedef struct bc_history_entry {
  // How far it is from the tab's page, for bc_tab_go_to_history and bc_tab_open_history: -1 is
  // the page back goes to, 1 the one forward goes to.
  int offset;
  const char* url;
  // Its page's title, or "" if it had none.
  const char* title;
  // The favicon the profile's history has for it, else for its host: a PNG |favicon_size| bytes
  // long, 32 pixels square for 16 points at 2×, or null and 0.
  const uint8_t* favicon;
  size_t favicon_size;
} bc_history_entry;

// One of the archive's tabs: a Today tab that closed, as Arc's Library lists them.
typedef struct bc_archived_tab {
  // For bc_window_restore_archived.
  bc_id id;
  // The page it was at, and its title.
  const char* url;
  const char* title;
  // When it closed, in milliseconds since the Unix epoch.
  double archived;
} bc_archived_tab;

// One of an extension's keyboard commands that has a key, as Chrome's Keyboard Shortcuts page
// lists them.
typedef struct bc_extension_command {
  // Its name in the extension's manifest, for bc_window_run_extension_command, or
  // "_execute_action" for its action, which bc_window_run_extension runs.
  const char* name;
  // What it does, in the extension's words.
  const char* description;
  // Its key, as bc_key names one, and the BC_MODIFIER_ flags with it.
  const char* key;
  uint32_t modifiers;
  // Whether the key is the one the extension suggested, not one the user set.
  bool suggested;
} bc_extension_command;

// One of a profile's extensions, as Chrome's extensions page lists them.
typedef struct bc_extension {
  // Chrome's id for it, for bc_window_run_extension.
  const char* id;
  const char* name;
  // Whether it's turned on. A turned-off extension's action doesn't run.
  bool enabled;
  // Whether its action is pinned, for the windows to show (bc_extension_set_pinned).
  bool pinned;
  // The page its options are on, for a tab to open as Chrome's Options does: its own, or Chrome's
  // extensions page showing them. "" if it has none.
  const char* options;
  const bc_extension_command* commands;
  size_t command_count;
  // Whether the user may remove it (bc_extension_remove). The ad blocker the browser comes with is
  // turned off in Settings instead (bc_set_ad_blocking).
  bool removable;
  // Whether the user allowed it in Incognito, as Chrome's extensions page does: it runs in private
  // windows, where the others don't, and its action and commands work there.
  bool incognito;
  // Whether the browser turned it off as its process crashed twice and the user hasn't been told
  // (bc_extension_crash_told). It stays set across launches until they are.
  bool crashed;
} bc_extension;

// An extension's action as a window shows it, for the tab it shows.
typedef struct bc_extension_action {
  const char* id;
  // What its button's tooltip says: the title the extension gave the tab, or else its name.
  const char* title;
  // Its icon for the tab, a PNG at twice its 16-point size.
  const uint8_t* icon;
  size_t icon_size;
  // The text of its badge over the icon's bottom, often "", and the badge's colour, 0xAARRGGBB,
  // or 0 for Chrome's.
  const char* badge;
  uint32_t badge_color;
  // Whether it acts on the tab. An action that doesn't shows dimmed, as in Chrome.
  bool enabled;
  // Whether it's pinned, for the URL field's end. The others come only while the Site Controls
  // show (bc_tab_site_controls).
  bool pinned;
} bc_extension_action;

// An extension's install, re-enable or new permissions, for the user to confirm, in Chrome's words.
typedef struct bc_extension_question {
  // For bc_extension_answer.
  bc_id id;
  // The extension's id and name.
  const char* extension;
  const char* name;
  // Its icon, a PNG of Chrome's 128-pixel install icon, or null and 0.
  const uint8_t* icon;
  size_t icon_size;
  // What Chrome's dialog says: its title, such as "Add "Name"?", the line over the warnings, such
  // as "It can:", and its buttons, such as "Add extension" and "Cancel". |accept| is "" for a
  // question with nothing to accept.
  const char* title;
  const char* heading;
  const char* accept;
  const char* cancel;
  // Chrome's permission warnings, such as "Read your browsing history", |warning_count| of them,
  // each with its details: "" or lines, one per site or device, split by "\n".
  const char* const* warnings;
  const char* const* details;
  size_t warning_count;
} bc_extension_question;

// Where a download is.
#define BC_DOWNLOAD_RUNNING 0
#define BC_DOWNLOAD_PAUSED 1
#define BC_DOWNLOAD_DONE 2
#define BC_DOWNLOAD_CANCELLED 3
#define BC_DOWNLOAD_FAILED 4

// One of the profile's downloads, as Arc's Library lists them.
typedef struct bc_download {
  // For bc_download_act and bc_download_icon.
  bc_id id;
  // Its file's name, and the URL it comes from.
  const char* name;
  const char* url;
  // Where its file is once it's done, a full path in UTF-8, or "" until that's known.
  const char* path;
  // The bytes it has, and the bytes it will have in all, or 0 while that isn't known.
  int64_t received;
  int64_t total;
  // A BC_DOWNLOAD_ state.
  int state;
  // It opens once it's done.
  bool open_when_done;
  // Chrome holds it, as a file that could harm the computer or one that came over plain HTTP,
  // until BC_DOWNLOAD_KEEP, and BC_DOWNLOAD_REMOVE deletes it.
  bool held;
  // When it started, in milliseconds since the Unix epoch.
  double started;
} bc_download;

// The values a site's permission takes in the Site Controls, as Chrome's page info words them: the
// site may, it may not, or it asks first.
#define BC_SITE_ALLOW 0
#define BC_SITE_BLOCK 1
#define BC_SITE_ASK 2

// A permission of the site of a tab's page, as Chrome's page info lists it.
typedef struct bc_site_permission {
  // Chrome's ContentSettingsType for it, which bc_tab_set_site_permission takes.
  int type;
  // Chrome's name for it, such as "Location".
  const char* name;
  // A BC_SITE_ value.
  int value;
  // The values the user may give it, a bit (1 << value) each. 0 for one that isn't the user's to
  // change, as a policy's or an extension's isn't.
  uint32_t choices;
  // Chrome's lasting name for it, such as "geolocation", which stays when the types renumber.
  const char* key;
  // Chrome's page info's words for its value under its name, such as "Allowed".
  const char* state;
} bc_site_permission;

// What the connection to a tab's page is, as the Site Controls' footer says.
#define BC_CONNECTION_SECURE 0
#define BC_CONNECTION_NOT_SECURE 1
// A file, or one of the browser's own pages.
#define BC_CONNECTION_INTERNAL 2

// What Picture in Picture can do for a tab's page: nothing, take its playing video, or bring the
// video it has back.
#define BC_PICTURE_IN_PICTURE_NONE 0
#define BC_PICTURE_IN_PICTURE_ENTER 1
#define BC_PICTURE_IN_PICTURE_EXIT 2

// A login Chrome's password manager keeps in a profile, without its password, which never leaves
// the core, or a site set to Never, where it doesn't offer to save one.
typedef struct bc_login {
  // The site as the user should see it, such as "accounts.example.com".
  const char* site;
  // The username, "" for a login without one and for a site set to Never.
  const char* username;
  bool never;
  // What bc_profile_remove_login takes back to name it, with the username; not for showing.
  const char* key;
} bc_login;

// An address or a card Chrome's autofill keeps in a profile, as Settings lists it. A card's number
// never leaves the core, not even its last digits.
typedef struct bc_autofill_entry {
  // An address's name, or its email for one without; a card's nickname, else the name on it.
  const char* title;
  // An address's first line and city; a card's network and expiry, such as "Visa, Exp: 06/27".
  const char* detail;
  bool card;
  // What bc_profile_remove_autofill takes back to name it; not for showing.
  const char* key;
} bc_autofill_entry;

// Whether the ad blocker runs on the site of a tab's page, as its Site Controls show: not the
// user's to change there (a page that isn't http(s), one the system's engine shows, a private
// window, or ad blocking off), on, or turned off for the site.
#define BC_AD_BLOCKING_NONE 0
#define BC_AD_BLOCKING_ON 1
#define BC_AD_BLOCKING_OFF 2

// Whether a site's tabs show in Developer Mode, for its Site Controls: none where the user can't
// turn it on or off, as for a page that isn't http(s) or in a private window, else on or off.
#define BC_DEVELOPER_MODE_NONE 0
#define BC_DEVELOPER_MODE_ON 1
#define BC_DEVELOPER_MODE_OFF 2

// The site of a tab's page, for its Site Controls.
typedef struct bc_site_controls {
  // The site as the user should see it, such as "example.com".
  const char* site;
  // A BC_CONNECTION_ value, and Chrome's words for it, such as "Connection is secure".
  int connection;
  const char* connection_summary;
  // Those Chrome's page info lists: the ones the site was given, has asked for or uses, and none
  // for a file or one of the browser's pages.
  const bc_site_permission* permissions;
  size_t count;
  // Chrome's words for the button that sets them back to their defaults, such as "Reset
  // permissions", or "" when it has none, as when the user set none of them.
  const char* reset;
  // A BC_PICTURE_IN_PICTURE_ value.
  int picture_in_picture;
  // The page came with a certificate, which bc_tab_show_certificate shows.
  bool certificate;
  // The site's pages open in the system's engine, which shows them (bc_tab_set_system_engine): a
  // listed service's, or a site the profile keeps. A tab showing one lists no permissions: the
  // system's engine asks for none.
  bool system_engine;
  // They don't, but the system's engine can show the tab's page: a page of the tab asked for what
  // only it plays, the user closed the site's note (bc_tab_close_media_note), or it's a listed
  // service the user turned off. bc_tab_set_system_engine turns it on.
  bool system_engine_offered;
  // The logins the profile keeps for the site, |login_count| of them: those saved for its domain,
  // as Chrome's password manager offers them there, then the site set to Never, if it is.
  const bc_login* logins;
  size_t login_count;
  // A BC_AD_BLOCKING_ value, which bc_tab_set_ad_blocking changes.
  int ad_blocking;
  // A BC_DEVELOPER_MODE_ value, which bc_tab_set_developer_mode changes.
  int developer_mode;
} bc_site_controls;

// What a device to cast to is doing, as Chrome's Cast dialog shows it.
#define BC_CAST_AVAILABLE 0
#define BC_CAST_CONNECTING 1
#define BC_CAST_CONNECTED 2
#define BC_CAST_DISCONNECTING 3
#define BC_CAST_UNAVAILABLE 4

// A device Chrome's media router found on the network, as its Cast dialog lists it.
typedef struct bc_cast_device {
  // Chrome's, for bc_tab_cast_to.
  const char* id;
  // The device's own name, such as "Living Room TV".
  const char* name;
  // A BC_CAST_ value.
  int state;
  // Chrome's words for what it's doing or what went wrong, such as "Casting tab", or "".
  const char* status;
} bc_cast_device;


// An agent's call the core checked, for the activity log (bc_agent_may_act).
typedef struct bc_agent_activity {
  // The agent's id, as its client gives it.
  const char* agent;
  bc_id tab;
  // What it asked to do, as the agent code names it, such as "navigate".
  const char* action;
  // Why the core refused it, for the user and the agent to read, or empty if the agent may act.
  const char* refused;
} bc_agent_activity;

// Why an agent's session with a tab ended (agent_session_ended).
// Its client disconnected (bc_agent_disconnected).
#define BC_AGENT_SESSION_DISCONNECTED 0
// It released the tab (bc_agent_release_tab).
#define BC_AGENT_SESSION_RELEASED 1
// The user took the tab back (bc_tab_take_back).
#define BC_AGENT_SESSION_TAKEN_BACK 2
// Its page asked for one of the user's passkeys, in Chrome's dialog, which is the user's: the
// tab is held as bc_tab_take_back holds it. This comes for a tab in an agent's Space even when
// the agent had no session there, as the app must know the tab is the user's.
#define BC_AGENT_SESSION_PASSKEY 3

// An extension's side panel as a window shows it, beside the page of the tab it shows.
typedef struct bc_side_panel {
  // The extension's id and name.
  const char* extension;
  const char* name;
  // The extension's icon, a PNG at twice its 16-point size, or null.
  const uint8_t* icon;
  size_t icon_size;
  // The title of the panel's page.
  const char* title;
  // The width the user last gave extensions' side panels in the profile, or 0 for none yet.
  double width;
} bc_side_panel;

// What a tab's offer offers: to save the password of a login its page submitted, or to update the
// password saved for it; or to save an address or a card the page's form submitted.
#define BC_OFFER_KIND_PASSWORD 0
#define BC_OFFER_KIND_PASSWORD_UPDATE 1
#define BC_OFFER_KIND_ADDRESS 2
#define BC_OFFER_KIND_CARD 3

// An offer from Chrome's password manager or its autofill, in Chrome's words. The password stays
// in the core, and a card's number but its last four digits.
typedef struct bc_offer {
  // A BC_OFFER_KIND_ kind.
  int kind;
  // Such as "Save password?", and the site whose password it is, as the user should see it.
  const char* title;
  const char* site;
  // The username it saves unless the user edits it, and the values the page's other fields had
  // that might be it instead, |username_count| of them, the first that one, for a picker.
  const char* username;
  const char* const* usernames;
  size_t username_count;
  // How many characters (code points) the password has, to show it masked.
  size_t password_length;
  // The labels of its buttons for BC_OFFER_ACCEPT, BC_OFFER_NOT_NOW and BC_OFFER_NEVER, the last ""
  // where it has none, as an update.
  const char* accept;
  const char* decline;
  const char* never;
  // An address's or a card's: what it saves, as Chrome's bubble lists it, a line each with '\n'
  // between, such as the card's name and last four digits, then its expiry. "" for a password's,
  // whose |username| and |password_length| say it, as theirs are "" and 0.
  const char* details;
} bc_offer;

// What a page's own dialog asks, in tab_dialog: its alert(), confirm() or prompt(), whether to
// leave it, from its beforeunload handler, which Chrome words, the user's name and password for
// the site's HTTP sign-in, or whether it may open another app for a link, such as Mail for
// mailto:, which Chrome asks only until the user ticks "Always allow".
#define BC_DIALOG_ALERT 0
#define BC_DIALOG_CONFIRM 1
#define BC_DIALOG_PROMPT 2
#define BC_DIALOG_LEAVE 3
#define BC_DIALOG_SIGN_IN 4
#define BC_DIALOG_OPEN_APP 5

// A page's own dialog, for the user to answer while its tab shows, in Chrome's words.
typedef struct bc_page_dialog {
  // Answer it with this.
  bc_id id;
  // A BC_DIALOG_ kind.
  int kind;
  // Chrome's title, such as "example.com says".
  const char* title;
  // The page's message, or Chrome's under the title.
  const char* message;
  // The text a prompt's field starts with, else "".
  const char* text;
  // Chrome's labels for its buttons. |cancel| is "" for none, as an alert has only OK.
  const char* accept;
  const char* cancel;
  // The label of a checkbox under the message, or "" for none.
  const char* check;
  // A sign-in's labels for its username field and its password field, else "".
  const char* username;
  const char* password;
} bc_page_dialog;

// A row of a page's context menu, as Chrome builds it.
typedef struct bc_page_menu_item {
  // What bc_tab_close_page_menu runs for it, or 0 for a line.
  int command;
  // What it says, UTF-8, or "" for a line between rows.
  const char* title;
  // The index of the row whose submenu it's in, which comes before it, or -1 for none.
  int parent;
  bool enabled;
  // It shows a check, as a spelling option that's on does.
  bool checked;
} bc_page_menu_item;

// Where a link dragged over a page is (bc_host's tab_link_dragged).
#define BC_LINK_DRAG_OVER 0
#define BC_LINK_DRAG_LEFT 1
#define BC_LINK_DRAG_DROP 2

// Where the browser's account is, in sync_changed. The account is one ilo server's, the browser's
// and not a profile's: it syncs the Spaces, and the bookmarks, passwords and open tabs of the
// profile the core started with. Other profiles keep theirs on the device.
// Signed out: bc_sync_sign_in starts.
#define BC_SYNC_SIGNED_OUT 0
// The sign-in window is open, or the core is getting the account's tokens with what it returned.
#define BC_SYNC_SIGNING_IN 1
// Signed in to an account whose data no device has a secret for yet, so this is its first device:
// bc_sync_recovery_words makes the secret and turns sync on.
#define BC_SYNC_FIRST_DEVICE 2
// Signed in to an account another device holds the secret of, which this one hasn't got: it gets
// it with bc_sync_request_pairing or bc_sync_enter_recovery_words, and can't read what's synced
// until then.
#define BC_SYNC_NEEDS_SECRET 3
// It asked the account's other devices for the secret (bc_sync_request_pairing), and waits for one
// to add it.
#define BC_SYNC_PAIRING 4
// Syncing.
#define BC_SYNC_ON 5

// Why the account's last step didn't go as asked, in sync_changed, for the app to say. It stays
// until the next step the user takes.
#define BC_SYNC_PROBLEM_NONE 0
// The recovery words are words of the list, with their checksum, but not this account's.
#define BC_SYNC_PROBLEM_WRONG_WORDS 1
// The other device answered Not me, or cancelled.
#define BC_SYNC_PROBLEM_DECLINED 2
// No device added this one in the pairing's 5 minutes, or the one that answered went away.
#define BC_SYNC_PROBLEM_EXPIRED 3
// This device was removed from the account on another (bc_sync_remove_device), or the server
// signed it out, so it's signed out now.
#define BC_SYNC_PROBLEM_REMOVED 4
// The server couldn't be reached, or refused, as when the account asked for too many pairings in
// an hour. The step can be tried again.
#define BC_SYNC_PROBLEM_SERVER 5

// The browser's account, as Settings shows it. Strings are UTF-8.
typedef struct bc_sync_state {
  // A BC_SYNC_ value.
  int status;
  // A BC_SYNC_PROBLEM_ value.
  int problem;
  // The server the browser signs in to, such as "https://sync.example.org": the build's own,
  // from its branding, until bc_sync_set_server gives another.
  const char* server;
  // The server is the build's own, which the app names by its own name.
  bool default_server;
  // The account's email while signed in, else "".
  const char* email;
  // With BC_SYNC_PAIRING, the match code once a device has answered: 6 digits, which that device
  // shows too. Else "".
  const char* code;
} bc_sync_state;

// Another device asks this one, through the server, to add it to the account, in
// sync_pairing_asked.
typedef struct bc_sync_pairing {
  // Answer it with this, in bc_sync_answer_pairing.
  const char* id;
  // The device's name and system, as it gives them, such as "Pixel 9" and "Android". Shown, never
  // trusted: the code is what the user checks.
  const char* name;
  const char* os;
  // The match code, 6 digits, which the asking device shows too.
  const char* code;
  // When it asked, and when it signed in to the account, by the server's word rather than its
  // own: milliseconds since the Unix epoch, 0 from a server that doesn't say.
  double asked_at;
  double signed_in_at;
} bc_sync_pairing;

// One of the account's devices, as Settings lists them.
typedef struct bc_sync_device {
  // For bc_sync_remove_device.
  const char* id;
  // As the device gave them when it signed in, such as "MacBook Air" and "macOS".
  const char* name;
  const char* os;
  // It's this one.
  bool current;
} bc_sync_device;

// What the app gives the core when it starts Chromium. Any callback may be null, and a string it's
// passed lasts only for the call.
typedef struct bc_host {
  // Passed back to every callback.
  void* context;
  // Chromium has made NSApp and the browser is about to start: the first moment the app can put a
  // window on screen. None of the core's calls work yet.
  void (*launching)(void* context);
  // Chromium's run loop is running and the core's other calls work, bc_window_restore included.
  void (*started)(void* context);
  // Chromium is quitting. The core's calls stop working once this returns.
  void (*stopping)(void* context);
  // The window has closed, by bc_window_close, the user or quitting. With other windows open, its
  // tabs went to the one active before it, their pages live. Else they closed first.
  void (*window_closed)(void* context, bc_id window);
  // Chrome opened a tab in the window that the app didn't ask for: a page's link to a new tab, or
  // its popup or new window, which open as tabs. |opener| is the tab it came from, or 0. In a window
  // that shows a Space, it goes in its opener's Space where Chrome would put it, under the opener,
  // and the window shows it if |foreground| and the window shows that Space. A link ⌥- or
  // ⌥⌘-clicked goes in a split view with its page instead, unless the split has three tabs; then
  // space_changed. Elsewhere it isn't shown until bc_tab_show; show it if |foreground|, since
  // Chrome made it the window's active tab.
  void (*tab_opened)(void* context, bc_id window, bc_id tab, bc_id opener, bool foreground);
  // The tab's page finished loading |url|.
  void (*tab_loaded)(void* context, bc_id tab, const char* url);
  // The tab's URL, its history or whether it's loading changed. A burst of changes is one call.
  void (*tab_state_changed)(void* context, bc_id tab, const bc_tab_state* state);
  // The tab has a new title: its page's, or the name bc_tab_rename gave it.
  void (*tab_title_changed)(void* context, bc_id tab, const char* title);
  // The tab's page has a new favicon: a PNG |size| bytes long, 32 pixels square for 16 points at
  // 2×, or null and 0 once a page that has none has loaded. While a page loads, the tab keeps the
  // favicon it had until the new page's arrives.
  void (*tab_favicon_changed)(void* context, bc_id tab, const uint8_t* png, size_t size);
  // The pages bc_tab_history asked for, back from the tab's or, if |forward|, forward of it,
  // nearest first, |count| of them.
  void (*tab_history)(void* context, bc_id tab, bool forward, const bc_history_entry* entries,
                      size_t count);
  // The tab's page went full screen, as a video's full-screen button takes it, or came back. Show
  // it filling its window while it's full screen. The window goes full screen with it unless it
  // already was, and comes back with it if it went for it, after this call either way, so the
  // window's resize can lay the page out. Escape, the user taking the window out of full screen,
  // and switching or closing the tab bring the page back; the last two report it for a tab no
  // longer shown, or gone. Each change is reported once.
  void (*tab_fullscreen_changed)(void* context, bc_id tab, bool fullscreen);
  // The tab's find in page changed: its find bar opened or closed, its text changed, or the page
  // counted matches or selected another.
  void (*tab_find_changed)(void* context, bc_id tab, const bc_find_state* state);
  // The tab's page asks to use something, such as the camera: |site| is who asks, as the user
  // should see it, and |permissions| what it asks for, |count| of them, each a phrase that follows
  // "wants to", such as "Use your camera". Ask the user while the tab is shown, and answer with
  // bc_tab_answer_permission. It comes again with |count| 0 once the question has gone: answered,
  // or the tab went to another page, closed or was no longer shown. Chrome keeps the request of a
  // tab no longer shown, and it comes again once the tab is.
  void (*tab_permission_asked)(void* context,
                               bc_id tab,
                               const char* site,
                               const char* const* permissions,
                               size_t count);
  // The window's suggestions for |text|, the text bc_window_suggest was last given, after its
  // keyword and a space if it had one, best first. The first goes where Chrome's address bar goes
  // for what was typed. They come again as more are found.
  void (*window_suggestions)(void* context, bc_id window, const char* text,
                             const bc_suggestion* suggestions, size_t count);
  // The tab has closed, by bc_tab_close, its page, its window closing or quitting.
  void (*tab_closed)(void* context, bc_id tab);
  // The Space's tabs changed: one opened, closed, moved, pinned or unpinned. Read them with
  // bc_space_pinned and bc_space_today.
  void (*space_changed)(void* context, bc_id space);
  // A Space was made, renamed, deleted or given another profile, or a profile was made, renamed or
  // deleted. Read them with bc_spaces and bc_profiles.
  void (*spaces_changed)(void* context);
  // A tab was made a favorite, moved among them or taken out of them, or the favorites' scope
  // changed. Read them with bc_favorites.
  void (*favorites_changed)(void* context);
  // The window shows another Space. Which of its tabs, if any, follows in window_selected.
  void (*window_space_shown)(void* context, bc_id window, bc_id space);
  // The window shows another tab, or none (0): the app asked, a page opened one in front, the tab
  // shown closed, or the window shows another Space, which may have no tabs.
  void (*window_selected)(void* context, bc_id window, bc_id tab);
  // A key went down while the tab's page has the keyboard, before the page gets it. Return true to
  // keep it from the page, as browsers keep their reserved shortcuts (⌘T, ⌘W, ⌘Q). Keys an input
  // method is composing with don't come here.
  bool (*tab_key_before_page)(void* context, bc_id tab, const bc_key* key);
  // The tab's page didn't keep a key, such as ⌘F on a page without a find of its own. Return true
  // if the app used it; otherwise the window's menus get it, as ⌘M does on the Mac.
  bool (*tab_key_unhandled)(void* context, bc_id tab, const bc_key* key);
  // No field of the app's opens for the key the keys typed after it are held for (bc_hold_keys):
  // the page kept it, neither the page nor the app used it, or it got no answer in a second. A host
  // that holds them itself gives them to the page now.
  void (*held_keys_unclaimed)(void* context);
  // The archive changed, or what the last launch archived was read: |tabs| is all of it, newest
  // first.
  void (*archive_changed)(void* context, const bc_archived_tab* tabs, size_t count);
  // The favicon bc_page_favicon asked for, for |url| as the app wrote it: a PNG |size| bytes long,
  // 32 pixels square for 16 points at 2×, or null and 0 if the profile's history has none.
  void (*page_favicon)(void* context, const char* url, const uint8_t* png, size_t size);
  // The profile's downloads changed, or the ones Chrome kept from the last launch were read:
  // |downloads| is all of them, newest first. While one runs, about twice a second.
  void (*downloads_changed)(void* context, const bc_download* downloads, size_t count);
  // The icon bc_download_icon asked for: a PNG |size| bytes long, 32 pixels square for 16 points
  // at 2×, or null and 0 if the system has none.
  void (*download_icon)(void* context, bc_id download, const uint8_t* png, size_t size);
  // Chrome asked to show |window|'s downloads, such as after the user stopped a quit to let them
  // finish. In place of Chrome's downloads page, which the window never opens.
  void (*window_show_downloads)(void* context, bc_id window);
  // The profile's extensions changed, or were first read: one was added, removed, turned on or
  // off, pinned or unpinned, a command's key changed, or bc_extensions_report asked. |extensions|
  // is all of them that Chrome's extensions page lists, by name.
  void (*extensions_changed)(void* context,
                             bc_id profile,
                             const bc_extension* extensions,
                             size_t count);
  // An extension's popup showed over the window's page, |extension| being its id, or the popup
  // shown went, and |extension| is "". Chrome's popup goes when it loses the keyboard, when the
  // window shows another tab, or when its page closes it.
  void (*window_extension_popup)(void* context, bc_id window, const char* extension);
  // The actions the window shows changed: those pinned in the profile of the tab it shows, in
  // their order, as they are for that tab, then, while the Site Controls show for the tab
  // (bc_tab_site_controls), every other turned-on extension's of the profile, by name. Once the tab
  // shown changes, if they're different for it.
  void (*window_extension_actions)(void* context,
                                   bc_id window,
                                   const bc_extension_action* actions,
                                   size_t count);
  // The private session of |profile| ended: its last private window closed, and Chrome destroyed
  // the off-the-record profile they shared, with its cookies, cache and the rest. Not at quit. The
  // user's alone: agents hear none of it.
  void (*private_session_ended)(void* context, bc_id profile);
  // The site of the tab's page, for its Site Controls, after bc_tab_site_controls, and again as it
  // changes, until bc_site_controls_close: a permission changed or was used, the tab went to
  // another page, or a video started or stopped.
  void (*tab_site_controls)(void* context, bc_id tab, const bc_site_controls* controls);
  // The tab's Cast dialog, in place of Chrome's: after bc_tab_cast, the page menu's Cast… or the
  // page asking to cast, and again as the devices found change, with Chrome's header, such as
  // "Cast tab", and the devices, |count| of them. Once it closes, with |header| null: on
  // bc_cast_close, another tab's, the tab going, or casting starting, as Chrome's closes then.
  void (*tab_cast)(void* context,
                   bc_id tab,
                   const char* header,
                   const bc_cast_device* devices,
                   size_t count);
  // The box of the element at (|x|, |y|) bc_tab_element_at asked for, its border's in points from
  // the top-left of the page's view, or a box 0 wide if there's none or the page went.
  void (*tab_element_box)(void* context, bc_id tab, double x, double y, bc_rect box);
  // The picture bc_tab_capture asked for as |request|: a PNG |size| bytes long, |width| by
  // |height| pixels, or null and 0 if the page couldn't give one.
  void (*tab_captured)(void* context,
                       bc_id tab,
                       uint64_t request,
                       const uint8_t* png,
                       size_t size,
                       int width,
                       int height);
  // The user chose Open Link in New Window or Open Link in Incognito Window in the page menu of a
  // tab in |window|. Open |url| in a window as ⌘N's, or with |private_window| in a private one in
  // |profile|'s session, as ⇧⌘N's. A link from a private window asks for a private one either way,
  // as Chrome's does. The user's alone: agents have no page menu.
  void (*window_open_link)(void* context,
                           bc_id window,
                           const char* url,
                           bc_id profile,
                           bool private_window);
  // The user ⌥⌘-clicked a link to |url| in a page of |window|, a regular window or a Little ilo
  // window: open it in a new Little ilo window for |profile| (bc_window_open_little), as Arc's
  // Little Arc does. A private window's link opens beside its page instead, as ⌥-click's does. The
  // user's alone: agents' clicks hold no keys.
  void (*window_open_little)(void* context, bc_id window, const char* url, bc_id profile);
  // The page of |tab|, the tab |window| has selected, went to another window, which shows it live,
  // and |window| shows no page; or |tab| is 0, and |window| shows a page again, or has selected
  // another tab. Once for each change. window_selected doesn't say it: the tab stays selected.
  // bc_tab_picture gives a picture of the page, for the window to show in its place. The user's
  // alone: agents have no windows.
  void (*window_page_elsewhere)(void* context, bc_id window, bc_id tab);
  // The picture bc_tab_picture asked for as |request|: |width| by |height| pixels, row after row,
  // each 4 bytes in BGRA order, in sRGB, or null and 0 if the page couldn't give one. The pixels
  // are the app's to copy during the call.
  void (*tab_pictured)(void* context,
                       bc_id tab,
                       uint64_t request,
                       const uint8_t* bgra,
                       int width,
                       int height);
  // Chrome asks the user to confirm an extension's install, its re-enabling once it asks for more
  // permissions, or more permissions it asks for, over the window, in place of its dialog. Answer
  // with bc_extension_answer. A window asks one at a time, and this comes again once the question
  // has gone, answered or because the tab that asked closed, with the window's next, or with
  // |question| null if it has none. Chrome hears none was given if the window closes first.
  void (*window_extension_question)(void* context,
                                    bc_id window,
                                    const bc_extension_question* question);
  // The side panel the window shows changed: an extension opened or closed it, its page's title
  // or the extension's icon changed, or the window shows a tab with another panel or none. |panel|
  // is null for none. Once the tab shown changes, if it's different for it.
  void (*window_side_panel)(void* context, bc_id window, const bc_side_panel* panel);
  // A page of |tab| asked for a protected-media key system ilo can't play, such as Widevine, which
  // Netflix and Spotify play their video with: |key_system| is "com.widevine.alpha" or
  // "com.microsoft.playready", never the page's own string. Once for each key system a document is
  // refused, unless it's the one the app was last told, and never once a page at the site was given
  // one, as a site may ask for Widevine and then play with PlayReady on Windows. It comes with an
  // empty |key_system| once the tab goes to another site, or once bc_tab_set_system_engine switches
  // it: the note it asks for is gone. bc_tab_set_system_engine answers it where
  // bc_system_engine_available. Never at a site whose note the user closed
  // (bc_tab_close_media_note). Agents get none: the note hands the user's page to another engine or
  // app. For a tab in the system's engine, |key_system| is "output-restricted" once its page's
  // protected video is kept off the screen, as the display connection's protection (HDCP) isn't
  // enough for it: once a tab and site, with the empty one as the tab goes to another site or back
  // to Chromium. Closing that note is the app's alone.
  void (*tab_media_unsupported)(void* context, bc_id tab, const char* key_system);
  // Google's Widevine module is now |state|, a BC_WIDEVINE_ value (bc_widevine).
  void (*widevine_changed)(void* context, int state);
  // The tab's offer changed: Chrome's password manager or its autofill offers |offer|, or with
  // null, it no longer does: it was answered, or another tab was shown or the tab closed. Show it
  // while the tab is, and answer with bc_tab_answer_offer. A tab shown again doesn't get it back.
  // The user's alone: agents hear none, as it names the user's login, address or card.
  void (*tab_offer_changed)(void* context, bc_id tab, const bc_offer* offer);
  // The tab's key icon changed: what Chrome's password manager has for its page, a BC_PASSWORDS_
  // value, as Chrome's toolbar would show its key for it. The user's alone: agents hear none.
  void (*tab_passwords_changed)(void* context, bc_id tab, int passwords);
  // The profile's logins, after bc_profile_read_logins, and again each time they change: the saved
  // ones by site, then the sites set to Never. The user's alone: agents hear none.
  void (*profile_logins_changed)(void* context,
                                 bc_id profile,
                                 const bc_login* logins,
                                 size_t count);
  // The profile's saved addresses, most recently changed first, then its cards, after
  // bc_profile_read_autofill, and again each time they change. The user's alone: agents hear none.
  void (*profile_autofill_changed)(void* context,
                                   bc_id profile,
                                   const bc_autofill_entry* entries,
                                   size_t count);
  // bc_profile_import_passwords is done with its file: |result| is a BC_PASSWORDS_IMPORT_ value,
  // and with BC_PASSWORDS_IMPORTED, |imported| logins came in, or were here already, |conflicted|
  // were kept as the profile had them, as it holds another password for that site and username,
  // and |skipped| rows weren't logins it could take, such as one without a password. The user's
  // alone: agents hear none, and no file, row or count reaches them.
  void (*passwords_imported)(void* context,
                             bc_id profile,
                             int result,
                             size_t imported,
                             size_t conflicted,
                             size_t skipped);
  // Another app asked the browser to open |urls|, |count| http, https or file URLs: a link clicked
  // in Mail or Slack, `open https://…`, an .html file from Finder. Open each in a new tab of the
  // user's, in front. Those that launched the browser come as AppKit finishes launching the app,
  // before started or just after; open them over the Spaces bc_window_restore brings back. The
  // user's alone: a link a page opens, an agent's too, comes in tab_opened, and agents get no way
  // to make the browser the system's default, which only the user does, in the app's Settings or
  // the system's.
  void (*open_links)(void* context, const char* const* urls, size_t count);
  // The pointer is on a link in the page of |tab|, or has left it and |url| is "". |url| is the
  // link's address as the app shows it: UTF-8, without https://, and a name in another script as
  // Chrome's address bar shows it, which is as ASCII if it could pass for another site's. The
  // bytes [|domain_begin|, |domain_end|) of it are the host's registrable domain, such as
  // example.co.uk in login.example.co.uk, or the whole host if it has no registry; both are 0 for
  // an address with no host. Told as the link changes, and "" when the page navigates away, never
  // polled. The user's alone: agents have no pointer.
  void (*tab_link_target)(void* context,
                          bc_id tab,
                          const char* url,
                          size_t domain_begin,
                          size_t domain_end);
  // The user right-clicked the page of |tab|, which |window| shows, or pressed its menu key: show
  // its context menu, its top left at (|x|, |y|) in the window's content, in place of Chrome's.
  // |items| is Chrome's menu for what's there, as it builds it with its extensions' items and its
  // spelling suggestions, |count| rows in order, each submenu's after the row that opens it. Answer
  // with bc_tab_close_page_menu once it closes, chosen from or not: until then the page has its
  // menu open and opens no other. It comes again with |window| 0 and |count| 0 if the menu went
  // first, as its tab closed; close it then, without answering. The user's alone: agents have no
  // page menu.
  void (*tab_page_menu)(void* context,
                        bc_id window,
                        bc_id tab,
                        double x,
                        double y,
                        const bc_page_menu_item* items,
                        size_t count);
  // An entry for the activity log: the core checked an agent's call with bc_agent_may_act, and
  // let it act or refused it.
  void (*agent_activity)(void* context, const bc_agent_activity* entry);
  // On Linux under Wayland: |window|'s content is |width| by |height| points now. The app's UI is a
  // subsurface there (bc_window_wayland_surface), which no compositor sizes.
  void (*window_resized)(void* context, bc_id window, double width, double height);
  // The agent with the id |agent| is done in |tab|: its session there ended for |why|, a
  // BC_AGENT_SESSION_ value. Also an entry for the activity log. Not for a tab that closed or left
  // the agent's Space, which tab_closed and spaces_changed tell.
  void (*agent_session_ended)(void* context, const char* agent, bc_id tab, int why);
  // The tab's page asks with a dialog of its own, in place of Chrome's: show it over the tab's
  // card while the tab shows, and answer with bc_tab_answer_dialog. A tab asks one at a time, and
  // this comes again once the dialog has gone, answered or taken back, with the tab's next, or
  // with |dialog| null if it has none. Chrome's rules stay: a page that isn't the front window's
  // tab shown can't ask confirm() or prompt(), its alert() waits until the tab shows, and a
  // confirm() or prompt() is cancelled once its tab no longer shows. A "Leave site?" holds up
  // every other app-wide dialog, as Chrome's, so its tab comes to the front window if no window
  // shows it. A site's sign-in waits until its tab shows, as an alert(), and goes once another of
  // the profile's tabs signs in to the same realm. An agent's tab asks the user the same way, and
  // the agent can't answer: a sign-in is the user's to give. A page in an agent's Space never opens
  // another app, so it asks none.
  void (*tab_dialog)(void* context, bc_id tab, const bc_page_dialog* dialog);
  // The user clicked a link in the tab's page to |url|, on another site than the page's, which is
  // about to load in the tab: a plain click, as a link opened with a modifier goes to a new tab,
  // and not a form or a page's script without a click. |x|, |y| is where the user last pressed in
  // the page, in points from the top-left of its view. Return true to keep it from the tab, and
  // open it in a link preview with bc_tab_open_preview after this call returns. A site is a
  // registrable domain, such as example.co.uk.
  bool (*tab_link_to_site)(void* context, bc_id tab, const char* url, double x, double y);
  // The browser crashed the time before, and the user has Send Crash Reports off: offer to send
  // that crash's report, and answer with bc_answer_crash_report. Once the browser has started, at
  // most once a launch, and never twice for a report.
  void (*crash_report_offered)(void* context);
  // On Linux under Wayland, while a text field of the app's UI in |window| has the keys
  // (bc_window_ui_text_input): the input method composes |text| there, shown underlined at the
  // caret, or stops composing with |text| empty.
  void (*ui_composed)(void* context, bc_id window, const char* text);
  // As ui_composed: the input method commits |text| in place of what it composed.
  void (*ui_committed)(void* context, bc_id window, const char* text);
  // bc_profile_add_history added |added| pages to |profile|'s history, once for each call that
  // returned true, in order.
  void (*history_added)(void* context, bc_id profile, size_t added);
  // The text bc_tab_read_text asked for as |request|, UTF-8, or null if the tab had no page loaded
  // or closed first.
  void (*tab_text)(void* context, bc_id tab, uint64_t request, const char* text);
  // The answer to bc_agent_page's |request| for |tab|: with |error|, |text| says why it couldn't,
  // for the agent; else a screenshot's JPEG in base64, or for every other action the page's
  // accessibility tree, DevTools' Accessibility.getFullAXTree nodes as a JSON array.
  void (*agent_page_done)(void* context, bc_id tab, uint64_t request, const char* text, bool error);
  // The element bc_agent_page's |request| for |tab| is about to click or type in, scrolled into
  // view: its box in points from the top-left of the tab's page view, for the agent's pointer.
  void (*agent_page_element)(void* context, bc_id tab, uint64_t request, bc_rect box);
  // On Linux and Windows, the browser is quitting, or with |last_private| the last window of a
  // private profile closing, with |count| downloads running, which it would cancel: ask the user
  // over |window|, and answer with bc_window_answer_downloads. A window asks one at a time. The Mac
  // asks in Chrome's own alert as the app quits. The user's: agents get none, as they can't quit.
  void (*window_downloads_question)(void* context, bc_id window, int count, bool last_private);
  // The user drags a link to |url|, an http or https address, over the page of |tab|, at (|x|,
  // |y|) in points from the top-left of its view: BC_LINK_DRAG_OVER as it moves there, and
  // BC_LINK_DRAG_DROP as it's let go there. BC_LINK_DRAG_LEFT, with |url| "", as a drag leaves the
  // page or ends without a drop on it. Over, return true to take the drag there from the page,
  // which then doesn't see it, as over a split view's targets; let go, return whether the app
  // took the drop, opening the link itself. The user's alone: agents don't drag.
  bool (*tab_link_dragged)(void* context, bc_id tab, int phase, const char* url, double x,
                           double y);
  // The note on the first Today tabs auto-archive took came or went (bc_archive_note).
  void (*archive_note_changed)(void* context);
  // The trace bc_trace_stop ended: Chromium's JSON, |size| bytes, or null and 0 if it gave none.
  // The user's: save it where they pick, or drop it.
  void (*trace_recorded)(void* context, const char* json, size_t size);
  // On Linux: |window| became the active one. Under X11 the UI's window in it hears nothing of that
  // when the keyboard goes to the page, as a click on another window's page gives it.
  void (*window_activated)(void* context, bc_id window);
  // On Linux under X11: ⌥ was pressed and let go alone, with no other key or click while it was
  // down, as the page in |window| had the keyboard. GPUI hears only the UI's keys there.
  void (*window_option_tapped)(void* context, bc_id window);
  // bc_profile_add_addresses added |added| addresses to |profile|'s, once for each call that
  // returned true, in order.
  void (*addresses_added)(void* context, bc_id profile, size_t added);
  // The browser's account changed, or was first read, once the browser has started. The user's
  // alone: agents hear none.
  void (*sync_changed)(void* context, const bc_sync_state* state);
  // Another device asks to be added to the account: ask the user over the front window whether
  // to, showing its code, and answer with bc_sync_answer_pairing. It comes only once the core has
  // the code, after it answered the request with this device's key, which it does while one of the
  // browser's windows is active, or once one becomes so, so the device the user is at takes it. It
  // comes again with |pairing| null when the request ends unanswered: it expired, the asking device
  // cancelled, or another device took it. One at a time. The user's alone: agents hear none.
  void (*sync_pairing_asked)(void* context, const bc_sync_pairing* pairing);
  // The account's devices, after bc_sync_read_devices, and again after bc_sync_remove_device:
  // |count| of them, this one first. The user's alone: agents hear none.
  void (*sync_devices_changed)(void* context, const bc_sync_device* devices, size_t count);
} bc_host;

// Runs Chromium in this process with these arguments (Chromium's switches) and returns its exit
// code once it has quit. Call it once, on the main thread; it runs the main run loop. Not on
// Windows and Linux, where the sandbox needs Chromium's own executable to start every process
// (chrome.exe, and chrome through its zygote): there the app is BC_APP_LIBRARY, beside it, and the
// core calls its BC_APP_ENTRY in the browser process, as Chromium starts, for what the app would
// pass bc_main. |argc| and |argv| come in as the arguments the process was given, which there the
// app can't read itself once Chromium has written its process title over them, and the entry sets
// them to the switches. It returns the callbacks, which it keeps for as long as the process runs.
BC_EXPORT int bc_main(int argc, const char** argv, const bc_host* host);

#if defined(_WIN32)
#define BC_APP_LIBRARY L"Browser.dll"
#else
#define BC_APP_LIBRARY "libBrowser.so"
#endif
#define BC_APP_ENTRY "bc_app_host"
typedef const bc_host* (*bc_app_entry)(int* argc, const char*** argv);

// Asks Chromium to quit. Pages get their unload handlers first, then bc_main returns.
BC_EXPORT void bc_quit(void);

// Quits as bc_quit does, then starts the browser again with the switches it was given, and the app
// brings back its windows and Spaces as after any quit.
BC_EXPORT void bc_restart(void);

// Whether Chromium is quitting: after bc_quit, the Dock's Quit or logging out. The windows it
// closes then still call window_closed.
BC_EXPORT bool bc_quitting(void);

// Opens a window at |bounds| with the app's UI view in it, under the pages it shows: an NSView on
// the Mac, which the window keeps. |ui_view| may be null. Returns 0 if Chromium is quitting. Every
// window shows the same Spaces. The user's: agents get none, as it changes what the user sees.
BC_EXPORT bc_id bc_window_open(bc_rect bounds, void* ui_view);

// On Linux under Wayland, Chromium's wl_display, which the app's UI shares, and null otherwise: on
// X11 the UI has a connection of its own. From then on the core calls |dispatched| each time
// Chromium has dispatched the display's events, from its loop and never while it has a read
// prepared, as what Chromium reads can include the UI's, which wait in the UI's queue.
BC_EXPORT void* bc_wayland_display(void (*dispatched)(void));

// The scale Chromium draws at on the main display, in pixels a point, as the desktop sets it (on X11
// through GTK, from GDK_SCALE and the font resolution) or --force-device-scale-factor does. On X11,
// where the app's UI has a connection of its own, the UI draws at it too, so its points are the
// pages'. Works from started.
BC_EXPORT double bc_display_scale(void);

// On Linux under Wayland, |window|'s wl_surface, for the app's UI to be a subsurface of, under the
// pages, with its content's size in points; else null. The app's UI, not bc_window_open's
// |ui_view|, which is null there, then follows window_resized.
BC_EXPORT void* bc_window_wayland_surface(bc_id window, double* width, double* height);

// On Linux under Wayland, where every key reaches both the browser window, for its pages, and the
// app's UI: gives the keyboard in |window| to the UI, as a click in the UI does, and the page that
// had it lets it go. A click in a page, or the core focusing one, takes it back. On X11 the X
// server's focus says whose the keys are, and the core follows it. On Windows the app calls it
// before it focuses the UI, so the core knows whether the UI kept the keyboard already.
BC_EXPORT void bc_window_ui_takes_keyboard(bc_id window);

// On Linux and Windows, as |window| shows no page: gives the keyboard to the app's UI, as
// bc_window_ui_takes_keyboard does, until a page shows, which then has it, as the Mac's window
// holds the keys for the next page shown.
BC_EXPORT void bc_window_ui_holds_keyboard(bc_id window);

// On Linux, whether the keys in |window| are the app's UI's rather than a page's.
BC_EXPORT bool bc_window_keyboard_is_ui(bc_id window);

// On Linux, whether the keys in |window| are a window of Chromium's own in it, such as an
// extension's popup or a tab's dialog, rather than a page's or the UI's. For the test driver, as
// the Mac's sees it as a child window with the keyboard.
BC_EXPORT bool bc_window_keyboard_in_child(bc_id window);

// On Linux, whether one of |window|'s pages has the keys, and if so its pane's frame in |frame|,
// from the top-left of the window's content. For the test driver, as the Mac's finds the page of
// the view with the keyboard.
BC_EXPORT bool bc_window_keyboard_page(bc_id window, bc_rect* frame);

// On Linux and Windows, where the core draws a split view's ring around the focused page: whether
// it shows in |window|, and if so its frame in |frame|, from the top-left of the window's content.
// For the test driver, which sees the Mac's ring as a view.
BC_EXPORT bool bc_window_focus_ring(bc_id window, bc_rect* frame);

// On Linux, whether a tab's dialog, such as print preview, shows over |window|, and if so its frame
// in |frame|, from the top-left of the window's content. For the test driver, which can't see it,
// as the Mac's sees it as a child window.
BC_EXPORT bool bc_window_dialog(bc_id window, bc_rect* frame);

// On Linux, while an overlay of the app's over |window|, such as the command bar, has the keyboard:
// neither the window's pages nor its UI get keys, and the page or UI that had them keeps its focus,
// to have them back once the overlay gives them up (|takes| false).
BC_EXPORT void bc_window_overlay_takes_keyboard(bc_id window, bool takes);

// On Linux, where the window is Chromium's and the app can't reach it under Wayland: minimizes
// |window|, as its minimize button does, where the window manager can.
BC_EXPORT void bc_window_minimize(bc_id window);

// On Linux, as bc_window_minimize: maximizes |window|, or restores it, as the third window button
// does on every Linux desktop. Full screen, it leaves full screen instead, as Escape asks of that
// button (the Mac's zoom button, which leaves it there too).
BC_EXPORT void bc_window_toggle_maximized(bc_id window);

// On Linux under Wayland, where the compositor gives the process one input method, the window's:
// a text field of the app's UI in |window|, or of an overlay of it, is focused (|focused|), its
// caret at |x|, |y| in points from the top left of the window's surface, |height| high. While the
// keys are the UI's or the overlay's, the input method composes for it, into ui_composed and
// ui_committed, and puts its candidates at the caret. Losing them commits what it composed, as a
// page's field does.
BC_EXPORT void bc_window_ui_text_input(bc_id window, bool focused, double x, double y,
                                       double height);

// During tab_key_before_page: holds the keys typed in the tab's page after the one asked about,
// which opens a field of the app's, such as ⌃L's address field. A shortcut the page gets first
// comes back to the app only after the keys typed behind it reached the page, and they belong in
// the field. They're held until the page has answered the key: if the page kept it, or the app
// didn't use it, they go to the page as typed (held_keys_unclaimed), and if the app used it, until
// the field has the keyboard, however long that takes. Keys an input method composes aren't held.
// With |in_core| the core holds them, and bc_give_held_keys gives them to the field once it has the
// keyboard, for hosts that can't, as on Linux, where a page's keys reach Chromium's window first.
// Otherwise the host holds them, and the core only tells it when no field takes them.
BC_EXPORT void bc_hold_keys(bool in_core);

// Ends a hold (bc_hold_keys): |take| gets the keys held, |count| of them in the order typed, during
// the call, for the field that now has the keyboard. Not called if nothing was held.
BC_EXPORT void bc_give_held_keys(void (*take)(void* context, const bc_key* keys, size_t count),
                                 void* context);

// Puts back the favorites and the Spaces as the user left them when the browser last quit or its
// last window closed, in the window, which shows the Space the front window showed, with its
// sidebar's width, and goes full screen if it was. The core saves them as they
// change, in Chrome's user data directory: the favorites, the Spaces' names and whose they are,
// their pinned and Today tabs with each tab's history, and the tab each showed. A tab loads once
// it's shown. Does nothing and returns false if nothing was saved or there are Spaces already.
// Then favorites_changed, spaces_changed, space_changed, each tab's tab_state_changed and
// tab_title_changed, tab_favicon_changed once a tab's favicon is found, window_space_shown and
// window_selected.
BC_EXPORT bool bc_window_restore(bc_id window);

// The frames of the regular windows open when the browser last quit, or when its last window
// closed, front to back, which is the order they were last active in: each as it was out of full
// screen, in bc_window_open's terms, and empty (width 0) if it was saved before frames were kept.
// None of a private window. Copies up to |count| into |frames| and returns how many there are.
// Works from started. The first is the window bc_window_restore brings the Spaces back in; open it
// there. The core saves them as the windows open, move, resize and close.
BC_EXPORT size_t bc_saved_window_frames(bc_rect* frames, size_t count);

// Opens saved window |index|, 1 or more, at |bounds|, as bc_window_open does but behind the windows
// open and not key, once bc_window_restore has brought the Spaces back in the first. It shows the
// Space it showed, with the tab it showed there unless another window shows that, its sidebar's
// width (bc_window_sidebar_width), and goes full screen if it was. Returns 0 if there's no such
// window or Chromium is quitting. Then window_space_shown and window_selected. The user's: agents
// get none, as it changes what the user sees.
BC_EXPORT bc_id bc_window_open_saved(size_t index, bc_rect bounds, void* ui_view);

// Closes the window. With other windows open, its tabs go, their pages live, to the one active
// before it, which shows its tab live again if it showed one this window took. Else its pages get
// their unload handlers. Then window_closed. The user's: agents get none.
BC_EXPORT void bc_window_close(bc_id window);

// Answers window_downloads_question, if the window still asks: |cancel| cancels the downloads and
// goes on quitting or closing. Otherwise the browser stays open, and Chrome shows the downloads
// (window_show_downloads). The user's: agents get none.
BC_EXPORT void bc_window_answer_downloads(bc_id window, bool cancel);

// Titles the window as the window manager shows it, for a host that can't reach the window itself,
// as under Wayland, where the window is Chromium's.
BC_EXPORT void bc_window_set_title(bc_id window, const char* title);

// Brings the window to the front with the keyboard, restoring it if it was minimized. For a host
// that can't reach the window itself, as bc_window_set_title. The user's: agents get none, as it
// takes the user's focus.
BC_EXPORT void bc_window_activate(bc_id window);

// Makes an empty Space named |name|, in the profile the core started with, right after the Space
// |after|, as Arc puts a new Space after the one shown, or after the others if |after| is 0 or
// gone. Any window can show it. It's the user's. Then spaces_changed.
BC_EXPORT bc_id bc_space_create(const char* name, bc_id after);

// A Space is the user's or an agent's, from when it's made for as long as it lives, and saved with
// it (decision 8). An agent is an AI client, such as Claude Code, driving the browser through the
// core. The rules its calls keep, once there are any:
// - It works only in its own Space: bc_agent_may_act answers which tabs are its to act on, and
//   bc_agent_list_tabs lists that Space's tabs alone. The user's tabs and favorites never are,
//   until the user gives an agent one (bc_tab_give_to_agent), and no tab moves into an agent's
//   Space by bc_tab_move_to_space or bc_tab_move. A tab the user opens in one, while a window shows
//   it, is the agent's like the rest.
// - Its actions never move the user's focus: no window shows another Space or tab, and no page
//   takes the keyboard, because of one.
// - The user can watch it, as an agent's Space shows in the windows like the others, take over in
//   it, and hand back. To keep what an agent made, the user moves its tabs into a Space of theirs.
// An agent's Space shows, switches, renames, clears and deletes as the user's Spaces do, and
// bc_window_undo_delete_space brings it back the agent's.

// Makes an empty Space named |name| for the agent with the id |agent| and the name |agent_name|,
// UTF-8, as its client gives them, in the profile the core started with, after the others.
// bc_space_set_profile puts it in another. The user's actions never call this: New Space makes the
// user's. Returns 0, making none, if |agent| or |agent_name| is empty. Then spaces_changed.
BC_EXPORT bc_id bc_space_create_for_agent(const char* name,
                                          const char* agent,
                                          const char* agent_name);

// Copies the id of the agent the Space belongs to into |agent| as bc_space_name does, or nothing,
// an empty string, for the user's own. It's the one the agent's client gave, stable across its
// connections.
BC_EXPORT size_t bc_space_agent(bc_id space, char* agent, size_t capacity);

// Copies the name of the agent the Space belongs to, for the user to see, into |name| as
// bc_space_name does, or an empty string for the user's own Space.
BC_EXPORT size_t bc_space_agent_name(bc_id space, char* name, size_t capacity);

// Whether the agent with the id |agent| may do |action| to |tab|: only if |action| is one agents
// may do, the tab is in that agent's own Space, and Chromium shows its page, not the system's
// engine, where the browser can't keep an agent's rules. Agents may back, click, close_tab,
// forward, hand_off, navigate, press_key, read_page, screenshot, scroll, select_option, snapshot
// and type; ad_blocking, devtools, passwords, profile_settings (a profile's search engine, archive
// timing, download folder, offers to save and saved data, and Clear Browsing Data) and any other
// action are refused. Agent code calls it once for each call an agent makes on a tab, before
// acting. If refused, copies why into |reason| as bc_space_name does, else an empty string. Either
// way, then agent_activity, for the activity log.
BC_EXPORT bool bc_agent_may_act(const char* agent,
                                bc_id tab,
                                const char* action,
                                char* reason,
                                size_t capacity);

// An agent's session with a tab, which the app shows as the agent at work there, lasts from the
// first call bc_agent_may_act lets it make in the tab until one of these three ends it, each then
// calling agent_session_ended, or until the tab closes or leaves the agent's Space. A quiet spell
// ends none: an idle browser schedules nothing.

// The agent with the id |agent| released |tab|. Ends its session there and returns whether it had
// one. Agent code calls it.
BC_EXPORT bool bc_agent_release_tab(const char* agent, bc_id tab);

// The client of the agent with the id |agent| disconnected. Ends each of its sessions and returns
// how many it had. Agent code calls it.
BC_EXPORT size_t bc_agent_disconnected(const char* agent);

// Opens a tab loading the http(s) |url| at the top of the Today tabs of the Space of the agent with
// the id |agent|, without showing it: no window shows another Space or tab, and no page takes the
// keyboard, because of it. Returns 0, opening none, for any other URL, an agent without a Space or
// with no window open. The agent's session in the tab starts, as bc_agent_may_act's does, and
// tab_opened tells of the tab, then spaces_changed. Agent code calls it.
BC_EXPORT bc_id bc_agent_open_tab(const char* agent, const char* url);

// Loads the http(s) |url| in |tab|, if it's in the Space of the agent with the id |agent|, and
// returns whether it does: never another URL. Agent code calls it after bc_agent_may_act.
BC_EXPORT bool bc_agent_navigate(const char* agent, bc_id tab, const char* url);

// Copies up to |capacity| of the tabs of the Space of the agent with the id |agent| into |tabs|, its
// pinned tabs top to bottom without their folders, then its Today tabs, and returns how many it
// has: 0 for an agent with no Space yet. The core finds the Space, so no other Space's tabs, the
// user's or another agent's, are ever listed. Agent code calls it for an agent's list of tabs,
// never bc_space_pinned or bc_space_today.
BC_EXPORT size_t bc_agent_list_tabs(const char* agent, bc_id* tabs, size_t capacity);

// What bc_agent_page does in a tab. Each but SCREENSHOT then answers with the page's accessibility
// tree, once the page has settled, as an action can start a navigation.
// The tree alone.
#define BC_AGENT_PAGE_SNAPSHOT 0
// Clicks the element: its center, scrolled into view first.
#define BC_AGENT_PAGE_CLICK 1
// Types |text| into the element, in place of what it holds, and with SUBMIT presses Return. Never
// into a password field.
#define BC_AGENT_PAGE_TYPE 2
#define BC_AGENT_PAGE_TYPE_SUBMIT 3
// Presses the key |text| names: Enter, Tab, Escape, Backspace, Delete, Space, ArrowUp, ArrowDown,
// ArrowLeft, ArrowRight, PageUp, PageDown, Home or End.
#define BC_AGENT_PAGE_KEY 4
// Scrolls the element into view, or with no element the page by most of a screen.
#define BC_AGENT_PAGE_SCROLL_DOWN 5
#define BC_AGENT_PAGE_SCROLL_UP 6
// Goes back, or forward, in the tab's history.
#define BC_AGENT_PAGE_BACK 7
#define BC_AGENT_PAGE_FORWARD 8
// A JPEG of what the page's view shows.
#define BC_AGENT_PAGE_SCREENSHOT 9

// Does |action|, a BC_AGENT_PAGE_ value, in |tab| for the agent with the id |agent|, to |element|,
// a backendDOMNodeId from the tree, or 0, with |text| as the action says, else null. Then
// agent_page_done with |request|, from a task of its own. Refused, with an error, if the tab isn't
// in the agent's Space or its page isn't http(s). Through Chromium's DevTools protocol, inside the
// browser, never a remote-debugging port, and never with the agent's own script. The tree leaves
// out what fields hold: no node's value, and nothing under a field, where Chromium puts the text
// typed or filled into it, and it marks password fields. It has the page's frames, each under the
// element it's in, those from http(s) origins only: never an extension's or the browser's, nor
// what's in one. An element in a frame in a process of its own has its frame above its DOM node's
// id in the backendDOMNodeId, so ids stay unique. A tab whose page the browser unloaded gets
// it back, not shown. While an agent works in a tab its page keeps drawing, hidden, as a tab Chrome
// captures does, until a while after its last action or the end of its session there. Chrome's
// autofill suggestions close before each input, so an agent's keys never fill the user's data.
// Agent code calls it after bc_agent_may_act.
BC_EXPORT void bc_agent_page(const char* agent,
                             bc_id tab,
                             uint64_t request,
                             int action,
                             int64_t element,
                             const char* text);

// The user takes |tab| over from the agent at work in it. Ends that agent's session there, returns
// whether one had it, and holds the tab: no agent may act in it (bc_agent_may_act refuses) until
// bc_tab_hand_back. The user's: agents get none. Chrome fills, suggests and offers to save the
// user's passwords, addresses, cards and passkeys in a held tab as in any of theirs, and none of
// them in an agent's tab it doesn't hold; its sign-in form fills at once, without a reload.
BC_EXPORT bool bc_tab_take_back(bc_id tab);

// The agent with the id |agent| hands |tab|, in its Space, to the user, for a step only they may
// do, such as a sign-in: the tab is held as bc_tab_take_back holds it, and its session stays.
// Returns whether it was, not if the user holds it already. Agent code calls it after
// bc_agent_may_act.
BC_EXPORT bool bc_agent_hand_off(const char* agent, bc_id tab);

// The user hands |tab| back to agents, and returns whether they held it. The user's.
BC_EXPORT bool bc_tab_hand_back(bc_id tab);

// Whether the user holds |tab| from agents.
BC_EXPORT bool bc_tab_held(bc_id tab);

// The user gives the agent whose Space is |space| their |tab|, which moves there as
// bc_tab_move_to_space moves a tab, and the agent may act in it as in its own from then on. Only
// one of the user's tabs, into an agent's Space of the same profile, as the page goes as it is. A
// hold on it from an earlier agent ends. Returns whether it moved. The user's: an agent never
// takes the user's tabs, so agents get none. Moving the tab to a Space of the user's takes it
// back.
BC_EXPORT bool bc_tab_give_to_agent(bc_id tab, bc_id space);

// The profile the Space's tabs open in, which its favorites are in.
BC_EXPORT bc_id bc_space_profile(bc_id space);

// Puts the Space in |profile|, loading it first if it isn't, and returns whether it will: only an
// empty Space changes, as its tabs' pages are in their profile. A window showing a favorite the
// Space no longer shows shows nothing. Under Each Space its favorites open again in |profile|, and
// the old tabs close. Then spaces_changed, and favorites_changed for favorites.
BC_EXPORT bool bc_space_set_profile(bc_id space, bc_id profile);

// Copies up to |capacity| of the profiles into |profiles| and returns how many there are: the one
// the core started with, then the others in the order they were made. A profile holds cookies,
// history, logins and extensions, which the Spaces in it share (decision 5).
BC_EXPORT size_t bc_profiles(bc_id* profiles, size_t capacity);

// Copies the profile's name into |name| as bc_space_name does. "Default" for one no one named.
BC_EXPORT size_t bc_profile_name(bc_id profile, char* name, size_t capacity);

// Names the profile |name|, UTF-8, and returns whether it did: not with an empty name. Then
// spaces_changed.
BC_EXPORT bool bc_profile_rename(bc_id profile, const char* name);

// Makes a profile named |name|, UTF-8, and returns it, or 0. It's listed once Chrome has made it,
// then spaces_changed.
BC_EXPORT bc_id bc_profile_create(const char* name);

// Deletes the profile, with everything Chrome keeps in it, and returns whether it will. Not one a
// Space or a favorite is in, the one the core started with or the last. Then spaces_changed.
BC_EXPORT bool bc_profile_delete(bc_id profile);

// The settings below are the profile's own, as Arc's Settings › Profiles keeps them, and the
// user's: agents get none. A profile that isn't loaded, as one no Space or favorite is in after a
// relaunch, has none to read or set: it loads, then spaces_changed.

// Copies the folder the profile's downloads go to, UTF-8, into |path| as bc_space_name does, or
// nothing for a profile that isn't loaded.
BC_EXPORT size_t bc_profile_download_folder(bc_id profile, char* path, size_t capacity);
// Sends the profile's downloads to the folder at |path|, UTF-8, and returns whether it will: not
// for a profile that isn't loaded, a path that isn't absolute, or a folder a policy sets.
BC_EXPORT bool bc_profile_set_download_folder(bc_id profile, const char* path);
// What a profile offers to save, for bc_profile_offers_to_save: the password the user signs in
// with, as Chrome's Offer to save passwords, which still fills those it has; and the address or the
// card the user fills a form with, as Chrome's Save and fill addresses and Save and fill payment
// methods, which fill nothing while off.
#define BC_SAVE_PASSWORDS 0
#define BC_SAVE_ADDRESSES 1
#define BC_SAVE_CARDS 2
// Sets |offers| to whether the profile offers to save |what|, a BC_SAVE_ value, and returns
// whether it could: not for a profile that isn't loaded.
BC_EXPORT bool bc_profile_offers_to_save(bc_id profile, int what, bool* offers);
// Has the profile offer to save |what|, or not, and returns whether it will: not for a profile
// that isn't loaded, or one a policy sets.
BC_EXPORT bool bc_profile_set_offers_to_save(bc_id profile, int what, bool offer);
// Copies the search engines the user picks the profile's default from, each "keyword\tname" on a
// line of its own, UTF-8, into |engines| as bc_space_name does. Nothing for a profile that isn't
// loaded, until its engines have, or while a policy or an extension picks its default.
BC_EXPORT size_t bc_profile_search_engines(bc_id profile, char* engines, size_t capacity);
// Copies the keyword of the profile's default search engine, which the command bar searches with in
// its Spaces, into |keyword| as bc_space_name does.
BC_EXPORT size_t bc_profile_search_engine(bc_id profile, char* keyword, size_t capacity);
// Makes the engine of |keyword|, one of bc_profile_search_engines, the profile's default, and
// returns whether it did.
BC_EXPORT bool bc_profile_set_search_engine(bc_id profile, const char* keyword);
// How long, in seconds, a Today tab in the profile's Spaces that no window shows waits before it
// goes in the archive, or 0 for a profile that isn't loaded.
BC_EXPORT double bc_profile_archive_after(bc_id profile);
// Sets it, and returns whether it did: not for a profile that isn't loaded or a time that isn't
// positive. The tabs that have waited that long go in the archive at once.
BC_EXPORT bool bc_profile_set_archive_after(bc_id profile, double seconds);

// Copies the profile's notes service, which New Note opens a new document in, into |service| as
// bc_space_name does: the app's name for it, as bc_profile_set_notes_service was given, or "" for
// none picked. The app keeps the services and their URLs; the profile keeps the name.
BC_EXPORT size_t bc_profile_notes_service(bc_id profile, char* service, size_t capacity);
BC_EXPORT void bc_profile_set_notes_service(bc_id profile, const char* service);

// A page of another browser's history, for bc_profile_add_history. Strings are UTF-8.
typedef struct bc_history_page {
  const char* url;
  const char* title;
  // When it was last visited, in microseconds since 1970.
  int64_t last_visit;
  int32_t visits;
  // How many of the visits were typed into the address bar, which ranks it among suggestions.
  int32_t typed;
} bc_history_page;

// Adds |count| pages from another browser's history to the loaded profile's, as Chrome's importer
// does, and returns whether it will: not to a private profile or one that hasn't loaded. A page
// already there with a visit as late is skipped, so importing the same history again adds
// nothing. Then history_added, with how many it added. The core copies what it needs.
BC_EXPORT bool bc_profile_add_history(bc_id profile, const bc_history_page* pages, size_t count);

// A field of another browser's saved address, for bc_profile_add_addresses, as a Chromium-based
// browser's Web Data keeps it: Chromium's FieldType and VerificationStatus numbers, and the value,
// UTF-8.
typedef struct bc_address_field {
  int32_t type;
  const char* value;
  int32_t status;
} bc_address_field;

typedef struct bc_address {
  const bc_address_field* fields;
  size_t count;
} bc_address;

// Adds |count| of another browser's saved addresses to the loaded profile's, once they've loaded,
// and returns whether it will: not to a private profile, one that hasn't loaded or one whose user
// turned addresses off. A field of a type or status Chromium doesn't keep is left out, and an
// address the profile has, or one with nothing in it, is skipped, so importing the same addresses
// again adds nothing. Then addresses_added, with how many it added. The core copies what it needs.
BC_EXPORT bool bc_profile_add_addresses(bc_id profile, const bc_address* addresses, size_t count);

// Copies up to |capacity| of the Spaces into |spaces|, in order, and returns how many there are.
BC_EXPORT size_t bc_spaces(bc_id* spaces, size_t capacity);

// Copies up to |capacity| bytes of the Space's name into |name|, UTF-8 and ending in a NUL, and
// returns the name's length without it.
BC_EXPORT size_t bc_space_name(bc_id space, char* name, size_t capacity);

// Renames the Space to |name|, UTF-8. Then spaces_changed.
BC_EXPORT void bc_space_set_name(bc_id space, const char* name);

// Puts the Space's colour, 0xRRGGBB in sRGB, in |color|, and returns true. False if it has none,
// and the window follows the system's accent colour.
BC_EXPORT bool bc_space_color(bc_id space, uint32_t* color);

// Gives the Space |color|, 0xRRGGBB in sRGB, or no colour if |has_color| is false. Then
// spaces_changed.
BC_EXPORT void bc_space_set_color(bc_id space, bool has_color, uint32_t color);

// Copies up to |capacity| bytes of the Space's icon into |icon| as bc_space_name does: what
// bc_space_set_icon gave it, UTF-8, or empty.
BC_EXPORT size_t bc_space_icon(bc_id space, char* icon, size_t capacity);

// Gives the Space |icon|, UTF-8, kept as its name is, for the UI to show in place of its dot and
// before its name: an emoji, or "glyph:" and the name of one of the UI's glyphs, such as
// "glyph:star". The core keeps it as it comes; empty takes the icon away. Then spaces_changed.
BC_EXPORT void bc_space_set_icon(bc_id space, const char* icon);

// Copies up to |capacity| bytes of the Space's theme into |theme| as bc_space_name does: what
// bc_space_set_theme gave it, UTF-8, or empty.
BC_EXPORT size_t bc_space_theme(bc_id space, char* theme, size_t capacity);

// Gives the Space |theme|, UTF-8, kept as its name is: the UI's description of the colours the
// window is drawn in, of which bc_space_set_color gives the main one. The core keeps it as it
// comes; empty takes the theme away. Then spaces_changed.
BC_EXPORT void bc_space_set_theme(bc_id space, const char* theme);

// Deletes the Space, unless it's the last, and returns whether it will. A window showing it shows
// the Space before it, else the one after it. Its tabs close, and it goes with the last of them; a
// page that keeps its tab open keeps the Space until it closes. Its Today tabs reopen with
// bc_window_reopen_closed_tab, in the Space shown, and the Space with
// bc_window_undo_delete_space. Under BC_FAVORITES_SPACE its favorites close with it and come back
// with it. Then spaces_changed.
BC_EXPORT bool bc_space_delete(bc_id space);

// Copies up to |capacity| of the Space's Today tabs into |tabs|, top to bottom, and returns how many
// it has. The ones the user opened are newest first; one a page opened sits under the page's tab.
BC_EXPORT size_t bc_space_today(bc_id space, bc_id* tabs, size_t capacity);

// Copies up to |capacity| of the Space's pinned tabs and folders into |tabs|, top to bottom, and
// returns how many it has. Each folder is followed by what's in it. They sit above its Today tabs.
BC_EXPORT size_t bc_space_pinned(bc_id space, bc_id* tabs, size_t capacity);

// Whether the Space's pinned tabs and folders show, which collapsing its pinned tabs hides but for
// the tab shown, as a closed folder does. They show until bc_space_set_pinned_open hides them, and
// the session keeps it.
BC_EXPORT bool bc_space_pinned_open(bc_id space);

// Then space_changed.
BC_EXPORT void bc_space_set_pinned_open(bc_id space, bool open);

// Whether |id| is a folder, and if it is, whether it shows what's in it in |open|.
BC_EXPORT bool bc_folder(bc_id id, bool* open);

// Copies the folder's name into |name| as bc_space_name does.
BC_EXPORT size_t bc_folder_name(bc_id folder, char* name, size_t capacity);

// The folder holding the pinned tab or folder |id|, else 0.
BC_EXPORT bc_id bc_folder_of(bc_id id);

// Makes an open folder named |name| at |index| in |folder| if it isn't 0, else in the Space's
// pinned tabs, counted as bc_tab_move counts, or at the end past it. Returns it. Then
// space_changed.
BC_EXPORT bc_id bc_folder_create(bc_id space, const char* name, bc_id folder, size_t index);

// Then space_changed.
BC_EXPORT void bc_folder_rename(bc_id folder, const char* name);
BC_EXPORT void bc_folder_set_open(bc_id folder, bool open);

// Copies the folder's icon, an emoji, UTF-8, into |icon| as bc_space_name does; empty for none,
// for which the sidebar draws a folder.
BC_EXPORT size_t bc_folder_icon(bc_id folder, char* icon, size_t capacity);

// Gives the folder |icon|, an emoji, UTF-8, kept across a quit and a duplicate; empty takes it
// away. Then space_changed.
BC_EXPORT void bc_folder_set_icon(bc_id folder, const char* icon);

// Makes a copy of the folder under it, in the folder holding it, with its name and icon, and a copy
// of each pinned tab and folder in it, in their order, each tab with its history, name, icon and
// pinned URL, unloaded until it's shown. Nothing is shown or loads. Returns the copy, or 0. Then
// space_changed.
BC_EXPORT bc_id bc_folder_duplicate(bc_id folder);

// Moves the folder, with what's in it, to |index| in |folder| if it isn't 0, else in the pinned
// tabs, counted as bc_tab_move counts. Not into itself. Then space_changed.
BC_EXPORT void bc_folder_move(bc_id folder, bc_id to, size_t index);

// Deletes the folder, leaving what was in it where it was, in the folder that held it. Then
// space_changed.
BC_EXPORT void bc_folder_delete(bc_id folder);

// Where the favorites are kept, as the user chose: who shows the same ones above their own tabs. A
// favorite is a site, and each profile that shows it has a tab of its own for it, in no Space,
// which a Space opens in its own profile.
// - BC_FAVORITES_EVERY: every Space shows the same sites, whatever its profile.
// - BC_FAVORITES_PROFILE: the Spaces of one profile show the same favorites, as Arc's do. The
//   default.
// - BC_FAVORITES_SPACE: every Space has its own.
#define BC_FAVORITES_EVERY 0
#define BC_FAVORITES_PROFILE 1
#define BC_FAVORITES_SPACE 2

// Copies up to |capacity| of the favorites into |tabs|, and returns how many there are: every
// profile's, and under BC_FAVORITES_SPACE every Space's. A Space shows those that are its profile's
// or, under BC_FAVORITES_SPACE, its own (bc_favorite_space), above its own tabs, in their order
// among them.
BC_EXPORT size_t bc_favorites(bc_id* tabs, size_t capacity);

// The Space the favorite belongs to, under BC_FAVORITES_SPACE, which alone shows it. Else, or for a
// tab that isn't a favorite, 0: every Space of the favorite's profile (bc_tab_profile) shows it.
BC_EXPORT bc_id bc_favorite_space(bc_id tab);

// The scope of the favorites, a BC_FAVORITES_ value, and changes it, which the core keeps with the
// Spaces. Switching never loses a favorite. Narrowing gives each Space or profile its own copy, its
// tabs unloaded but for those already loaded, which stay with one Space. Widening merges the lists
// by URL: the list the Space the window active last shows first, then each other list's extras in
// the order of the Spaces, and a duplicate's extra tabs close, keeping a loaded one over an
// unloaded one. A window showing a favorite its Space no longer shows shows the Space's top tab
// instead. Then favorites_changed. The user's preference: agents have no call to change it, and
// the calls they have for favorites follow the scope.
BC_EXPORT int bc_favorites_scope(void);
BC_EXPORT void bc_set_favorites_scope(int scope);

// The profile the tab's page is in, or 0.
BC_EXPORT bc_id bc_tab_profile(bc_id tab);

// Copies up to |capacity| of the tabs shown with |tab| into |tabs|, left to right or top to
// bottom, |tab| among them, and returns how many there are: those of its split view, else |tab|
// alone. 0 for a tab in no Space. A split's tabs sit together in their list, in this order, as one
// row.
BC_EXPORT size_t bc_tab_split(bc_id tab, bc_id* tabs, size_t capacity);

// Puts |tab| in a split view with |with|, at its left end if |left|, else its right end, making
// one if |with| is in none: a window showing either shows them side by side. |tab| leaves any
// split it was in and moves into the split's place in its list, pinned at its page if |with| is
// pinned. Returns false, doing nothing, if |with|'s split has three tabs already, or the two are
// in different Spaces. Then space_changed.
BC_EXPORT bool bc_tab_split_with(bc_id tab, bc_id with, bool left);

// Takes |tab| out of its split view, to just below the tabs left in it. A window showing it shows
// it alone. The split goes once one tab is left. Then space_changed.
BC_EXPORT void bc_tab_separate(bc_id tab);

// Moves |tab| to |index| in its split view, counted from the left: a window showing it shows the
// pages in their new order. Returns false, doing nothing, if |tab| is in no split view or |index|
// is past its end. Then space_changed.
BC_EXPORT bool bc_tab_move_in_split(bc_id tab, size_t index);

// Whether |tab|'s split view shows its pages stacked, top to bottom, rather than side by side.
BC_EXPORT bool bc_tab_split_stacked(bc_id tab);

// Stacks the pages of |tab|'s split view top to bottom, or puts them side by side: a window
// showing it shows them so, in the same order. A split view starts side by side. Returns false,
// doing nothing, if |tab| is in no split view. Then space_changed.
BC_EXPORT bool bc_tab_set_split_stacked(bc_id tab, bool stacked);

// The share of its split view's width, or height stacked, that |tab|'s page takes with the 10 pt
// gap after it, so a split's shares add up to 1. They start even, and are again whenever a tab
// joins or leaves the split. 0 if |tab| is in no split view.
BC_EXPORT double bc_tab_split_share(bc_id tab);

// Gives the pages of |tab|'s split view |shares|, |count| of them in its order, scaled to add up
// to 1, or even shares if |shares| is null: a window showing it lays it out so. Returns false,
// doing nothing, if |tab| is in no split view, or |shares| aren't one above 0 for each page. Then
// space_changed.
BC_EXPORT bool bc_tab_set_split_shares(bc_id tab, const double* shares, size_t count);

// The tab of |tab|'s split view whose page took the card by bc_tab_set_split_expanded, else 0 while
// the split isn't expanded or |tab| is in none. The pages of an expanded split keep their widths
// as the window changes, and bc_tab_split_share doesn't tell them: the others are 220 pt each, or
// an even share of a card too short for that, and the expanded one has the rest. A tab joining or
// leaving the split, and bc_tab_set_split_shares, end it. Kept after a quit.
BC_EXPORT bc_id bc_tab_split_expanded(bc_id tab);

// Gives |tab|'s page the card of its split view, the others keeping 220 pt each, or if |expanded|
// is false ends that and leaves the pages even: a window showing it lays it out so. Returns false,
// doing nothing, if |tab| is in no split view. Then space_changed.
BC_EXPORT bool bc_tab_set_split_expanded(bc_id tab, bool expanded);

// Makes |space| the one the window shows: its tabs are the ones bc_window_select shows, and the
// window's new tabs go in it. The window shows the tab it last showed there, else the Space's top
// Today tab, else its top pinned tab, else no page, passing over tabs another window shows. Then
// window_space_shown and window_selected.
BC_EXPORT void bc_window_show_space(bc_id window, bc_id space);

// The width the app last gave the window's docked sidebar, or 0 for none. The core keeps it with
// the Spaces, and a window opens with the one the last launch saved.
BC_EXPORT double bc_window_sidebar_width(bc_id window);
BC_EXPORT void bc_window_set_sidebar_width(bc_id window, double width);

// The tab bc_window_show_space would show for |space|, or the tab shown for the Space shown. 0 for
// none.
BC_EXPORT bc_id bc_window_space_tab(bc_id window, bc_id space);

// Where the window shows its tab's page, in the window, and the radius of the page's corners, 0 for
// square ones. Set it again when the layout changes; the page keeps its distance from the window's
// edges as the window resizes. A split view's pages share it, 10 apart, each as wide as the others
// side by side, or as tall stacked.
BC_EXPORT void bc_window_set_page_frame(bc_id window, bc_rect frame, double corner_radius);

// The colour of the ring inside the edge of the focused page of a split view, 0xRRGGBB.
BC_EXPORT void bc_window_set_focus_color(bc_id window, uint32_t color);

// What the window's pages show until they draw their first frame, in place of white, 0xRRGGBB.
BC_EXPORT void bc_window_set_page_color(bc_id window, uint32_t color);

// Whether the app is moving the window's page frame on its own, as the sidebar hides or shows or
// the gap in a split view drags: each step is a new size for the pages. Chrome shows a page that
// hasn't drawn at its new size yet at its old size in the top-left corner, so its right edge and
// scrollbar trail the frame's and snap. While moving, the pages shown, their docked DevTools and
// the side panel's page show that frame stretched to fill the view instead. Call it with true
// before the motion's first bc_window_set_page_frame or bc_window_set_split_shares, and false after
// its last. A page full screen keeps Chrome's way.
BC_EXPORT void bc_window_page_moving(bc_id window, bool moving);

// Shows |tab| at the window's page frame, in place of the page shown before, and its Space if the
// window shows another. It takes the keyboard if that page had it. A tab in a split view shows with
// the rest of its split, focused. A click in another page of the split focuses that one. A page
// another window shows moves here live, with its split; that window keeps the tab selected and
// shows no page until it shows a tab again. Then window_space_shown if the Space changed, and
// window_selected. A switch to a tab that isn't the shown tab's opener, a tab it opened or a sibling
// has their Spaces forget which tabs opened which, as Chrome's tab strip does on a switch it's asked
// for. For a tab another window shows, the user's: agents get none.
BC_EXPORT void bc_window_select(bc_id window, bc_id tab);

// Gives the page the window shows the keyboard, from the UI or wherever in the window it is, as a
// press that shows a tab does, before it returns. The window holds it for a page still loading, as
// when the page shown before left with it, so that page takes it as bc_window_select's would, unless
// a click in the UI comes first. The user's: agents get none.
BC_EXPORT void bc_window_focus_page(bc_id window);

// Clears the Space the window shows: closes its Today tabs but those any window shows, as
// bc_tab_close does, for bc_window_undo_clear to bring back. While it shows every Today tab, a tab
// alone or a split that is all of Today, it closes those too, and the window shows the tab the
// close rule picks. Returns the id of the Clear, for bc_window_undo_clear, or 0 if it closed none.
// The window keeps its last ten.
BC_EXPORT bc_id bc_window_clear(bc_id window);

// Brings back the tabs the window's bc_window_clear |clear| closed, at their places in their
// Space's Today tabs and unloaded, once. Not those bc_window_reopen_closed_tab brought back. Then
// space_changed. If the clear closed the tab shown, it shows that tab again, with its split, and
// its Space, then window_selected. Returns how many it brought back, or 0 if that clear is gone:
// it was undone, or ten others came after it, or it was another window's.
BC_EXPORT size_t bc_window_undo_clear(bc_id window, bc_id clear);

// Brings back the Space |space|, which bc_space_delete deleted, with its id at its place among the
// Spaces, its name, colour, folders and pinned tabs, and the Today tabs that went to the archive
// and haven't come back some other way, all unloaded and leaving the archive. The window shows it,
// at the tab it showed there. The last 25 deleted, by any window, can come back, in any order. Not
// while a page keeps one of its tabs open. Then spaces_changed and window_space_shown. Returns the
// Space, or 0 if it can't: it is back, or 25 others were deleted since.
BC_EXPORT bc_id bc_window_undo_delete_space(bc_id window, bc_id space);

// Reopens the Today tab closed last, in any window, with its history: at its place in its Space's
// Today tabs, else at the top of the Space the window shows. One closed from a split view goes back
// to its place in it, if a tab of the split is open and it has room. The window shows it, and its
// Space, and it leaves the archive.
// Tabs that close with their window or as the browser quits come back at the next launch instead.
// Returns the tab, or 0 if none is closed.
BC_EXPORT bc_id bc_window_reopen_closed_tab(bc_id window);

// As bc_window_reopen_closed_tab for the Today tab that had the id |closed| when it closed, rather
// than the one closed last, so a tab closed before another can come back first. It doesn't show
// the tab: the one shown stays, so several can come back behind it, and the caller shows the one
// that was shown when it closed. Returns the new tab's id, which is not |closed|, or 0 if that tab
// isn't among those that reopen: it came back already, or 25 others closed since.
BC_EXPORT bc_id bc_window_reopen_tab(bc_id window, bc_id closed);

// Reopens the archived tab at the top of the Today tabs of the Space the window shows, unloaded,
// and shows it: with its history if it closed since the browser started, else at its page. It
// leaves the archive. Returns the tab, or 0 if the archive has no such tab. Then space_changed,
// archive_changed and window_selected.
BC_EXPORT bc_id bc_window_restore_archived(bc_id window, bc_id archived);

// Runs the extension's action on the tab the window shows, as a click on its button in Chrome's
// toolbar does: the extension is granted the tab, and its popup opens, hanging from |anchor| in the
// window, or its action.onClicked fires. Returns false if it didn't run: the window shows no tab,
// or the extension isn't turned on in the tab's profile. Then window_extension_popup, for a popup.
BC_EXPORT bool bc_window_run_extension(bc_id window, const char* id, bc_rect anchor);

// Runs the extension's keyboard command |name| on the tab the window shows, as its key does in
// Chrome: the extension is granted the tab, and its commands.onCommand hears the name and the tab.
// Returns false if it didn't run: the window shows no tab, the extension isn't turned on in the
// tab's profile, or it isn't listening.
BC_EXPORT bool bc_window_run_extension_command(bc_id window, const char* id, const char* name);

// Pins the extension's action in the profile, after the others, or unpins it, as Chrome's
// extensions menu does. Then extensions_changed and window_extension_actions.
BC_EXPORT void bc_extension_set_pinned(bc_id profile, const char* id, bool pinned);

// Removes the extension from the profile, with the data it keeps, as Chrome's Remove does once the
// user confirms, so the app asks first. Returns false if it can't be removed, as one that policy
// installed or the ad blocker the browser comes with. Then extensions_changed.
BC_EXPORT bool bc_extension_remove(bc_id profile, const char* id);

// The user was told the browser turned the extension off as it crashed (bc_extension.crashed), so
// it isn't reported crashed again. Then extensions_changed.
BC_EXPORT void bc_extension_crash_told(bc_id profile, const char* id);

// Turns the extension on again, as Chrome's extensions page's switch does, its crashes counted
// from none. Returns whether it's on. Then extensions_changed.
BC_EXPORT bool bc_extension_turn_on(bc_id profile, const char* id);

// Answers the question window_extension_question put, accepting it or not, if it's still asked:
// the extension installs, is turned on again or gets the permissions, or not. Then
// window_extension_question, and extensions_changed once the extension is added or turned on.
// The user's: agents get none, as an extension can read what the user sees and does on the sites
// it's given.
BC_EXPORT void bc_extension_answer(bc_id question, bool accept);

// Tells the app again of the extensions of each profile the core uses, since the first reports may
// come before it can take them. Then extensions_changed, for each.
BC_EXPORT void bc_extensions_report(void);

// Empties the archive. Tabs closed since the browser started still reopen with
// bc_window_reopen_closed_tab. Then archive_changed. Returns the removal's id, for
// bc_archive_undo_remove, or 0 if the archive was empty.
BC_EXPORT bc_id bc_archive_clear(void);

// Takes the archived tab out of the archive, as bc_archive_clear does with all of them. Its tab,
// if it closed since the browser started, still reopens with bc_window_reopen_closed_tab. Then
// archive_changed, unless the archive has no such tab. Returns the removal's id, as
// bc_archive_clear does, or 0 if it has none.
BC_EXPORT bc_id bc_archive_remove(bc_id archived);

// Puts back what the bc_archive_remove or bc_archive_clear that returned |removal| took out, each
// tab with its id where it was. The last 25, by any window, can come back, in any order. Then
// archive_changed. Returns how many tabs it put back, or 0 if that removal is gone.
BC_EXPORT size_t bc_archive_undo_remove(bc_id removal);

// How long, in seconds, a Today tab no window shows waits before it goes in the archive in a new
// profile: 12 hours, else the time the user chose. Each profile takes it the first time it's used,
// then keeps its own (bc_profile_archive_after), as Arc's do. The core keeps it with the Spaces.
BC_EXPORT double bc_archive_after(void);
BC_EXPORT void bc_set_archive_after(double seconds);

// Whether to show the note that auto-archive took Today tabs: from the first time it takes some
// until the user dismisses it with bc_dismiss_archive_note, after which it never shows again, as
// the core keeps that with the Spaces. Each change comes with archive_note_changed.
BC_EXPORT bool bc_archive_note(void);
BC_EXPORT void bc_dismiss_archive_note(void);

// Whether the app taps the trackpad at its detents, as the user chose. The core keeps it with the
// Spaces.
BC_EXPORT bool bc_haptics(void);
BC_EXPORT void bc_set_haptics(bool haptics);
// View › Appearance's choice, which pages' prefers-color-scheme follows: 0 the system's, 1 light,
// 2 dark. The app gives it at launch and when it changes, except on the Mac, where pages follow
// the app's NSAppearance instead.
BC_EXPORT void bc_set_appearance(int appearance);
// Whether the app asks before the user's Quit quits, as the user chose. The core keeps it with the
// Spaces.
BC_EXPORT bool bc_warn_before_quitting(void);
BC_EXPORT void bc_set_warn_before_quitting(bool warn);
// Whether a launch brings back the windows open when the browser last quit, each Space's Today
// tabs and the tab each showed, as the user chose. Else it opens one window where the front one
// was, out of full screen, showing the Space it showed with no tab, as a new window does, and the
// Today tabs go in the archive. The Spaces, their folders and pinned tabs and the favorites come
// back either way. The core keeps it with the Spaces, and reads it before bc_saved_window_frames
// and bc_window_restore.
BC_EXPORT bool bc_restore_windows(void);
BC_EXPORT void bc_set_restore_windows(bool restore);
// Whether a link to another site clicked in a pinned tab or a favorite opens in a link preview
// (bc_tab_open_preview), as the user chose. The core keeps it with the Spaces.
BC_EXPORT bool bc_link_previews(void);
BC_EXPORT void bc_set_link_previews(bool previews);
// Whether ⌥ pressed and let go alone brings the hidden sidebar out over the page, as the user
// chose. The core keeps it with the Spaces.
BC_EXPORT bool bc_option_reveals_sidebar(void);
BC_EXPORT void bc_set_option_reveals_sidebar(bool reveals);
// What the Library button's hover lists, as the user picked from the button's menu: bits for
// images (1), videos (2), other files (4) and the captures saved to the Library (8), all by
// default. The core keeps it with the Spaces.
BC_EXPORT int bc_hover_kinds(void);
BC_EXPORT void bc_set_hover_kinds(int kinds);
// Whether the app warns before moving a tab to another profile's Space, which it does once, as Dia
// does. The core keeps it with the Spaces.
BC_EXPORT bool bc_warn_moving_profiles(void);
BC_EXPORT void bc_set_warn_moving_profiles(bool warn);
// Whether a tab no window has shown for 2 hours lets go of its page, as the user chose, on by
// default. It keeps its place, history, title and favicon, and is unloaded in its tab_state_changed
// until it's shown and loads again. Checked as a window shows another tab or becomes key, and
// under memory pressure, which unloads the tabs unseen longest first, whatever their age. Never a
// tab shown, or pictured in a window while another shows it, nor one playing sound, capturing, in
// picture-in-picture, with DevTools open, a beforeunload handler or typed form text, showing its
// page in the system's engine, in a private window or in an agent's Space. Turning it on checks
// at once. The core keeps it with the Spaces.
BC_EXPORT bool bc_unload_idle_tabs(void);
BC_EXPORT void bc_set_unload_idle_tabs(bool unload);
// For check.sh: makes every tab look |seconds| longer unseen, so the check needn't wait 2 hours.
// The next tab shown checks which to unload, as it would then, and from now each check logs
// "browser: idle tabs checked" once it's done.
BC_EXPORT void bc_age_tabs_for_testing(double seconds);
// Whether the browser blocks ads and trackers, in every profile and its private windows, with the
// uBlock Origin it comes with: on unless the user turned it off. Off turns uBlock Origin off in
// each profile, as its switch in Chrome's extensions page does. The core keeps it with the Spaces.
// The user's: agents get none, as it keeps trackers off the user's pages.
BC_EXPORT bool bc_ad_blocking(void);
BC_EXPORT void bc_set_ad_blocking(bool on);
// Whether a page may take its video or call into Picture in Picture as its tab is hidden, the
// Mini Player, in every profile: on unless the user turned it off. Off blocks it by default, as
// Chrome's settings do, and a site the user allows in its Site Controls still may. The core keeps
// it with the Spaces. The user's: agents get none.
BC_EXPORT bool bc_auto_picture_in_picture(void);
BC_EXPORT void bc_set_auto_picture_in_picture(bool on);

// Crash reports. Crashpad writes a minidump of every crash in the profile folder, and the core sends
// one only while the user has Settings › Privacy › Send Crash Reports on, which is off by default:
// then each report not yet sent goes to the collector, once the browser has started and as the user
// turns it on, and is deleted once the collector has it. Turning it off deletes every report not
// yet sent. While it's off, the launch after a crash offers to send that crash's report
// (crash_report_offered). The core keeps the choice with the Spaces. No agent tool reads, changes
// or answers it.
//
// The host of the collector the app named with --crash-upload-url, or nothing without one, when
// nothing is sent and Settings doesn't offer to. Nothing on Windows too, where Crashpad writes them
// in the default folder other Chromiums share. Copied as bc_profile_name copies a name.
BC_EXPORT size_t bc_crash_reports_collector(char* host, size_t capacity);
// Whether the user has Send Crash Reports on and there's a collector.
BC_EXPORT bool bc_send_crash_reports(void);
BC_EXPORT void bc_set_send_crash_reports(bool send);
// The user's answer to crash_report_offered: send that one report, though Send Crash Reports is
// off, or delete it. Nothing once answered, or without an offer.
BC_EXPORT void bc_answer_crash_report(bool send);

// What bc_download_act does: opens the file, or for one still running, opens it once it's done or
// no longer; shows it in the system's file manager; pauses, resumes or cancels it; takes it off
// the list, keeping a finished file and deleting any other; or keeps one Chrome held.
#define BC_DOWNLOAD_OPEN 0
#define BC_DOWNLOAD_SHOW 1
#define BC_DOWNLOAD_PAUSE 2
#define BC_DOWNLOAD_RESUME 3
#define BC_DOWNLOAD_CANCEL 4
#define BC_DOWNLOAD_REMOVE 5
#define BC_DOWNLOAD_KEEP 6

// Does what |action|, a BC_DOWNLOAD_ action, says to the download. Then downloads_changed. The
// user's: agents get none, as opening a file runs it outside the browser.
BC_EXPORT void bc_download_act(bc_id download, int action);

// Takes the downloads that aren't running or paused off the list, keeping their files. Then
// downloads_changed.
BC_EXPORT void bc_downloads_clear(void);

// Asks for the icon the system shows for the download's file. Then download_icon.
BC_EXPORT void bc_download_icon(bc_id download);

// Asks for the favicon the profile's history has for the page at |url|, else for its host. Then
// page_favicon.
BC_EXPORT void bc_page_favicon(const char* url);

// Lists suggestions for |text| typed in the window's command bar, as Chrome's address bar does:
// pages from the profile's history and bookmarks, open tabs among them, and a search for |text|.
// Well-known sites |text| could start fill the rows left, under those. Nothing typed leaves the
// device, since search suggestions are off. Then window_suggestions, and
// again as more are found. Empty text lists none. |complete| lets the first suggestion complete
// |text|, which Chrome allows only while the user types at the end of the text: not after deleting
// or pasting, with the caret elsewhere, or while an input method composes. Unless null, |keyword|
// names the search engine whose site search the user entered, and the suggestions are its searches
// for |text|, as in Chrome's keyword mode; window_suggestions then gives "<keyword> <text>".
BC_EXPORT void bc_window_suggest(bc_id window, const char* keyword, const char* text,
                                 bool complete);

// Takes the window's removable suggestion going to |url| out of the profile's history and
// bookmarks' suggestions, as ⇧⌦ does in Chrome's address bar. Then window_suggestions without it.
BC_EXPORT void bc_window_remove_suggestion(bc_id window, const char* url);

// Opens a tab in the window, loading |url|, in the default profile, at the top of the Today tabs of
// the Space it shows. It isn't shown until bc_window_select or bc_tab_show. With |when_shown|, an
// http(s) |url| loads only once the tab is first shown, as a link a page opens behind does. Returns
// 0 if the window is closing or gone.
BC_EXPORT bc_id bc_tab_open(bc_id window, const char* url, bool when_shown);

// Copies up to |capacity| bytes of the URL Chrome's address bar goes to for |text| typed in it into
// |url|, UTF-8 and ending in a NUL, and returns the URL's length without it: the page |text| names,
// else a search for it with the default search engine of the profile |window| shows, DuckDuckGo
// until the user picks another. With no window, the profile the core started with. Returns 0 if
// |text| names neither.
BC_EXPORT size_t bc_typed_url(bc_id window, const char* text, char* url, size_t capacity);

// As bc_typed_url, the URL of a search for |text| with the search engine whose keyword is
// |keyword|, or the default one if that's null, even when |text| names a page. Returns 0 if there's
// no such engine.
BC_EXPORT size_t bc_search_url(bc_id window,
                               const char* keyword,
                               const char* text,
                               char* url,
                               size_t capacity);

// Link previews: a page over the window, from a link the user clicked in a pinned tab or a
// favorite, which the user closes or keeps as a tab. The user's: agents get none, as a preview takes
// the window's keyboard. An agent's links open in its own Space, as the page asks.

// Opens |url| in a tab in no Space, in |opener|'s window and profile, loading it at once as the link
// in |opener|'s page would have, for the app to show with bc_tab_show_in. bc_tab_close closes it,
// which no archive or ⇧⌘T keeps, and bc_window_keep_preview keeps it. Quitting closes it. Returns 0
// if |opener|'s window is closing or gone.
BC_EXPORT bc_id bc_tab_open_preview(bc_id opener, const char* url);

// Shows the page of |tab|, which no window shows, such as bc_tab_open_preview's, in |view|, an
// NSView of the app's on the Mac, which keeps it from then on: at |frame| in points from the view's
// top-left, over what the view draws, with corners of |corner_radius|, and with the keyboard. Call
// it again as the frame changes.
BC_EXPORT void bc_tab_show_in(bc_id tab, void* view, bc_rect frame, double corner_radius);

// Puts |tab|, a link preview, at the top of the Today tabs of the Space the window shows, and shows
// it there, as it is: its page doesn't load again. Then space_changed and window_selected.
BC_EXPORT void bc_window_keep_preview(bc_id window, bc_id tab);

// Shows the tab's page at |frame| in its window, in place of the page the window showed. It keeps
// its distance from the window's edges as the window resizes, and takes the keyboard if that page
// had it. For a window that shows no Space; one that does uses bc_window_select.
BC_EXPORT void bc_tab_show(bc_id tab, bc_rect frame);

// Loads |url| in the tab. A pinned tab keeps its page, as in Arc: an address other than its page's
// opens in a tab it opened, at the top of its Space's Today tabs, shown if its window shows that
// Space, and closing that tab shows the pinned one again.
BC_EXPORT void bc_tab_navigate(bc_id tab, const char* url);

// Goes back, or forward, in the tab's history, if it has a page that way.
BC_EXPORT void bc_tab_go_back(bc_id tab);
BC_EXPORT void bc_tab_go_forward(bc_id tab);

// Lists the pages back from the tab's, or forward of it if |forward|, nearest first, up to the
// six each side Chrome's sessions keep, so bc_tab_open_history can open any of them. None while
// the system's engine shows the tab's page, whose history is WebKit's. Then tab_history, once
// their favicons are found.
BC_EXPORT void bc_tab_history(bc_id tab, bool forward);

// Goes to the page |offset| from the tab's in its history, as bc_tab_history numbers them, if it
// has one there: what picking it from the back or forward button's menu does.
BC_EXPORT void bc_tab_go_to_history(bc_id tab, int offset);

// Opens a copy of the tab, with its history, at the page |offset| from its own, as
// bc_tab_history numbers them, or 0 for its page again: what ⌘-click on back, forward or reload
// does. It goes where bc_tab_duplicate's would, without showing, and its page loads once it's
// shown. Returns its id, or 0 if the tab has no page there. Then space_changed.
BC_EXPORT bc_id bc_tab_open_history(bc_id tab, int offset);

// Loads the tab's page again. |bypass_cache| fetches all of it from the network, as a force reload
// does. A hung page's renderer is ended first, as Chrome's Exit page does, with every page it
// shows: a reload would wait on it.
BC_EXPORT void bc_tab_reload(bc_id tab, bool bypass_cache);

// For a tab whose page is BC_PAGE_HUNG: waits for it, as Chrome's Wait does. The tab is fine again
// until Chrome's hang timeout runs out once more. The UI's and agents', which may reload instead.
BC_EXPORT void bc_tab_wait(bc_id tab);

// Stops the tab loading.
BC_EXPORT void bc_tab_stop(bc_id tab);

// Deletes the profile's cookies for the site of the tab's page if |cookies|, else its cache for
// the site, then loads the page again, bypassing the cache for the cache. A site is a registrable
// domain, such as example.co.uk, or an IP address. Does nothing for a page that isn't http(s). The
// system's engine's store loses the site's too, once the profile has used it.
BC_EXPORT void bc_tab_clear_site(bc_id tab, bool cookies);

// The system's engine, WebKit on the Mac, plays the protected video Chromium can't (decision 1).
// A short built-in list of streaming services opens in it from the first visit, until
// bc_tab_set_system_engine turns one off, which the profile keeps. Any other site's pages show in
// it once the user switches a tab there, from the protected-media note, and the profile keeps the
// site, never a listed one: its pages open there from then on, in any of the profile's tabs,
// until bc_tab_set_system_engine turns it off. Its own store, apart from Chromium's and Safari's,
// one a profile, and one that keeps nothing for a private window. The user's session with the site
// goes along inside the profile: the site's cookies and localStorage in Chromium are copied into
// the system's store before a tab's first load there, and the system's back to Chromium as the
// site is turned off. A tab goes back to Chromium for a link the user follows to another site,
// a URL typed for one, or back from its first page; redirects through other sites, as a sign-in's,
// stay. Agents' tabs never switch, and bc_agent_may_act refuses agents a tab that has.

// Whether the platform has a system engine that keeps a store a profile: macOS 14 or later.
// Answers without loading it.
BC_EXPORT bool bc_system_engine_available(void);

// With |on|, shows the tab's page in the system's engine, loading its URL there, and has the
// profile keep its site for it, or take back its off for a listed service. Else has the profile
// forget the site, or keep a listed service off across launches and list updates, and each of the
// profile's tabs that shows one of its pages there goes back to Chromium at that page, once the
// site's cookies and those pages' localStorage are in Chromium's store; the site then counts as one
// whose note was closed (bc_tab_close_media_note). Returns whether it did: not for a page that
// isn't http(s), nor where there's no system engine, nor for an agent's tab. Then
// tab_state_changed, tab_media_unsupported with an empty key system, and tab_site_controls if
// watched. The user's: agents get none.
BC_EXPORT bool bc_tab_set_system_engine(bc_id tab, bool on);

// The user closed the tab's protected-media note: its profile keeps the site of its page from
// asking again, kept across launches, and a private window's profile for its session.
// tab_media_unsupported comes with an empty key system for each tab at the site that had it, and
// never again there. Site Controls offer the system's engine for the site instead
// (bc_site_controls). Does nothing for a page that isn't http(s). The user's: agents get none.
BC_EXPORT void bc_tab_close_media_note(bc_id tab);

// Google's Widevine module, which Netflix, Spotify and Disney+ play their video with in Chromium
// (#236). It's Google's proprietary file, and its licence forbids bundling it, so ilo never ships
// it. On Linux the user can have the browser download it from Google's component service, from the
// protected-media note, and remove it in Settings. Elsewhere it's never offered: the Mac plays these
// sites in its system engine, and Windows with its own PlayReady. Until the user says yes, the
// component updater leaves it out of its checks, and once they have, it keeps it up to date with
// the security components (engine/README.md, What reaches Google).
// - BC_WIDEVINE_NONE: not offered on this platform.
// - BC_WIDEVINE_ABSENT: offered, not downloaded, or removed.
// - BC_WIDEVINE_DOWNLOADING: downloading, after the user's yes.
// - BC_WIDEVINE_FAILED: the download failed, for want of a network or of a module for this
//   platform. bc_widevine_download tries again.
// - BC_WIDEVINE_RESTART: downloaded. Pages play with it once the browser restarts (bc_restart), as
//   Linux loads it at launch, before the sandbox closes.
// - BC_WIDEVINE_READY: loaded. Pages that ask for Widevine play, and tab_media_unsupported never
//   names it.
#define BC_WIDEVINE_NONE 0
#define BC_WIDEVINE_ABSENT 1
#define BC_WIDEVINE_DOWNLOADING 2
#define BC_WIDEVINE_FAILED 3
#define BC_WIDEVINE_RESTART 4
#define BC_WIDEVINE_READY 5

// Where Google's Widevine module is: a BC_WIDEVINE_ value. Then widevine_changed with each change.
BC_EXPORT int bc_widevine(void);

// The user said yes to Google's Widevine module: the browser downloads it from Google's component
// service, then keeps it up to date. Does nothing unless it's BC_WIDEVINE_ABSENT or
// BC_WIDEVINE_FAILED. Then widevine_changed. The user's alone: agents get no way to it.
BC_EXPORT void bc_widevine_download(void);

// Removes Google's Widevine module: deletes it, and the component updater stops asking for it. A
// module the browser loaded plays until it restarts. Then widevine_changed with
// BC_WIDEVINE_ABSENT. The user's: agents get none.
BC_EXPORT void bc_widevine_remove(void);

// What PlayReady, which Windows plays these sites with in Chromium, needs that this PC may lack,
// for the protected-media note to say why a site was refused. It plays on Windows 11 on an x64 PC
// whose graphics have PlayReady's hardware DRM (security level 3000), outside private windows,
// which can't keep its licences. Chromium finds the hardware out only as a page asks, so this
// names what it can rule out first.
// - BC_PLAYREADY_NONE: no PlayReady here, off Windows: the note says nothing of it.
// - BC_PLAYREADY_WINDOWS_11: Windows 10, which Chromium's PlayReady doesn't run on.
// - BC_PLAYREADY_X64: an ARM PC, which runs the browser emulated and has no PlayReady for it.
// - BC_PLAYREADY_HARDWARE: Windows 11 on x64, so the graphics have no hardware DRM, or Chromium
//   turned it off after it failed.
#define BC_PLAYREADY_NONE 0
#define BC_PLAYREADY_WINDOWS_11 1
#define BC_PLAYREADY_X64 2
#define BC_PLAYREADY_HARDWARE 3
BC_EXPORT int bc_playready_needs(void);

// Opens the tab's find bar, if it isn't open, with the text it had when it last closed, else the
// text last found in another of the profile's tabs. Text selected in the page takes its place, as
// in Chrome, unless the user has changed the text since the bar opened. Highlights the matches
// without selecting one, so the page doesn't scroll. Copies up to |capacity| bytes of the bar's
// text into |text|, UTF-8 and ending in a NUL, and returns its length without it. Then
// tab_find_changed.
BC_EXPORT size_t bc_tab_find_open(bc_id tab, char* text, size_t capacity);

// Finds |text| in the tab's page, opening its find bar if it isn't open: highlights the matches and
// selects the next, or with |forward| false the one before. Empty text finds nothing. Then
// tab_find_changed, as the page counts them.
BC_EXPORT void bc_tab_find(bc_id tab, const char* text, bool forward);

// Closes the tab's find bar. The highlights go, and the selected match stays selected. Then
// tab_find_changed.
BC_EXPORT void bc_tab_find_close(bc_id tab);

// Asks for the box of the element at (|x|, |y|), in points from the top-left of the tab's page
// view, as DevTools' element picker hit tests, without running the page's JavaScript. One question
// is in the page at a time, and a newer one replaces any still waiting, so a moving pointer asks
// only for where it is now. Then tab_element_box. The user's: agents get none, as Capture is.
BC_EXPORT void bc_tab_element_at(bc_id tab, double x, double y);

// Captures the tab's page as a PNG: |rect|, in points from the top-left of the page's view, or the
// whole view if |rect| is empty, as the compositor last drew it; or with |full_page| the whole
// page, beyond the view, as DevTools' full-size screenshot does, which lays the page out at its
// full height while it draws. In pixels at the display's scale. A page in the system's engine gives
// only its whole view, as WebKit draws it, with protected video black. Then tab_captured with
// |request|, always from a task of its own, never inside this call, so the app may ask from inside
// an update of what the answer goes to. The user's: agents get none, as a picture shows what a page
// holds, and agents read pages through their own tool, by its rules.
BC_EXPORT void bc_tab_capture(bc_id tab, uint64_t request, bool full_page, bc_rect rect);

// Pictures the tab's whole view as the compositor last drew it, or a page in the system's engine as
// WebKit draws it, in pixels at the display's scale, for a window to show in place of a page
// another window shows: then tab_pictured with |request|, as with bc_tab_capture from a task of its
// own, never inside this call. Its pixels as GPUI draws them rather than a PNG, which takes longer
// to make and to read than the move it's shown for. With |width| and |height| above 0, it's scaled
// down to fit them, its shape kept, as the compositor copies it, for a small picture such as a
// sidebar tab's hover card shows of a tab that's hidden, which draws once for it, unseen and still
// hidden to the page, as Chrome's tab previews do. The user's: agents get none, as with
// bc_tab_capture.
BC_EXPORT void bc_tab_picture(bc_id tab, uint64_t request, int width, int height);

// Reads the text of the tab's page as its accessibility tree has it, across its frames from
// http(s) origins, a line for each block: what a screen reader reads, but no extension's frame and
// no field's value, typed or autofilled, a password's
// above all, nor the text of an editable region. Chromium builds the tree for this one read,
// without running the page's JavaScript or turning accessibility on for the tab. A page that's
// loading is read once it stops, or after 15 s. A tab without its page loaded has nothing to read.
// Then tab_text with |request|, from a task of its own. Agents read pages through it, each read
// checked with bc_agent_may_act first.
BC_EXPORT void bc_tab_read_text(bc_id tab, uint64_t request);

// Ends what bc_tab_element_at and bc_tab_capture began: drops their questions and lets go of the
// page's DevTools, so a page that isn't being captured pays nothing.
BC_EXPORT void bc_tab_capture_end(bc_id tab);

// Answers to the question tab_permission_asked put: the site may, now and whenever it asks again;
// it may not, now or again; or not now, and it may ask again.
#define BC_PERMISSION_ALLOW 0
#define BC_PERMISSION_BLOCK 1
#define BC_PERMISSION_DISMISS 2

// Answers the tab's page's question with |answer|, a BC_PERMISSION_ answer, if it's still asking.
// The profile keeps an allow or a block for the site, which Chrome's site settings list and
// change. Then tab_permission_asked. The user's: agents get none, as an answer can give a site the
// camera or where the user is.
BC_EXPORT void bc_tab_answer_permission(bc_id tab, int answer);

// Answers the dialog tab_dialog put, if it's still asked: |accept| for its accept button, else its
// cancel, with the text of its first field in |text| and of its password field in |password|,
// either of which may be null for "", and whether its checkbox is ticked in |checked|. Then
// tab_dialog. The core keeps neither text, nor logs it. The user's: agents get none, as the page
// goes on with what it thinks the user chose.
BC_EXPORT void bc_tab_answer_dialog(bc_id tab,
                                    bc_id dialog,
                                    bool accept,
                                    const char* text,
                                    const char* password,
                                    bool checked);

// Reads the site of the tab's page for its Site Controls, as Chrome's page info does, and watches
// it until bc_site_controls_close or another tab's bc_tab_site_controls. Then tab_site_controls,
// and window_extension_actions for the window showing the tab, with every turned-on extension's
// action until then, as the panel lists them all and a tab switch with it closed needn't pay for
// them.
BC_EXPORT void bc_tab_site_controls(bc_id tab);

// Stops watching the site bc_tab_site_controls read. Then window_extension_actions, with the
// pinned actions alone.
BC_EXPORT void bc_site_controls_close(void);

// Opens the tab's Cast dialog, as Chrome's View › Cast… does, which looks for devices on the
// network with DIAL (ungoogled-chromium builds without mDNS). Then tab_cast. The last tab's closes
// first: one shows at a time. Does nothing for a profile Chrome's media router doesn't serve. The
// user's: agents get none, as casting shows the page to a screen others can see.
BC_EXPORT void bc_tab_cast(bc_id tab);

// Closes the Cast dialog. Then tab_cast with a null header.
BC_EXPORT void bc_cast_close(void);

// A click on the device |id| in the tab's Cast dialog, as on Chrome's: one with a problem clears
// it, one casting stops, and one available casts the tab, or the page's presentation where it
// has one. Then tab_cast as it changes.
BC_EXPORT void bc_tab_cast_to(bc_id tab, const char* id);

// Gives the site of the tab's page |value|, a BC_SITE_ value, for the permission of |type|, as
// Chrome's page info does, if it's one of the permission's choices and bc_tab_site_controls is
// watching that tab. The profile keeps it, as Chrome's site settings do. Then tab_site_controls.
// The user's: agents get none, as a change can give a site the camera or where the user is, and
// take back what the user blocked.
BC_EXPORT void bc_tab_set_site_permission(bc_id tab, int type, int value);

// Sets the site's permissions the user set back to their defaults, as Chrome's page info's "Reset
// permissions" does, if bc_tab_site_controls is watching the tab. Then tab_site_controls. The
// user's, as bc_tab_set_site_permission is.
BC_EXPORT void bc_tab_reset_site_permissions(bc_id tab);

// Lets the ad blocker run on the site of the tab's page or not, if bc_tab_site_controls is watching
// the tab and bc_site_controls' ad_blocking isn't BC_AD_BLOCKING_NONE, and reloads the page. The
// profile keeps it, for the site and its subdomains. Then tab_site_controls. The user's, as
// bc_set_ad_blocking is.
BC_EXPORT void bc_tab_set_ad_blocking(bc_id tab, bool on);
// Copies the sites the profile has ad blocking turned off on into |sites| as bc_space_name does,
// one a line in the order they were turned off: each a registrable domain, which covers its
// subdomains, or a host alone. Empty for a profile that isn't loaded.
BC_EXPORT size_t bc_ad_blocking_off_sites(bc_id profile, char* sites, size_t capacity);
// Lets the ad blocker run on |site|, one of bc_ad_blocking_off_sites, in the profile again. A page
// open there keeps what it loaded until it reloads. The user's, as bc_set_ad_blocking is.
BC_EXPORT void bc_ad_blocking_turn_on_site(bc_id profile, const char* site);

// Turns Developer Mode on or off for the site of the tab's page, its registrable domain or host, in
// its profile, which keeps it, if the page is http(s) and the tab isn't in a private window. Then
// tab_state_changed for each of the profile's tabs at the site, and tab_site_controls if
// bc_tab_site_controls is watching one. The user's: agents get none, as
// it's how the user sees their tabs.
BC_EXPORT void bc_tab_set_developer_mode(bc_id tab, bool on);

// Takes the tab's playing video into Picture in Picture, or brings it back, as
// bc_site_controls' picture_in_picture says it can. Then tab_site_controls.
BC_EXPORT void bc_tab_picture_in_picture(bc_id tab);

// Shows the certificate the tab's page came with in the system's viewer, a sheet on the tab's
// window on the Mac, as Chrome's page info does. Does nothing for a page with none, as http and
// the browser's own pages have.
BC_EXPORT void bc_tab_show_certificate(bc_id tab);

// Answers to the offer tab_offer_changed made: save or update the password, the address or the
// card; never save a password for the site, which only a password's save offers; or not now.
#define BC_OFFER_ACCEPT 0
#define BC_OFFER_NEVER 1
#define BC_OFFER_NOT_NOW 2

// Answers the tab's offer with |answer|, a BC_OFFER_ answer, with the username as the user left it,
// or null to keep the offer's, if the offer is still open; an address's or a card's ignores it.
// Then tab_offer_changed. The user's: agents get none, as saving a password, an address or a card
// is the user's to decide, and an agent's tab is offered none of them.
BC_EXPORT void bc_tab_answer_offer(bc_id tab, int answer, const char* username);

// What tab_passwords_changed says Chrome's password manager has for a tab's page, as its key icon:
// nothing, so no key shows;
#define BC_PASSWORDS_NONE 0
// the site's saved logins, filled in or just saved, which the Site Controls list;
#define BC_PASSWORDS_SAVED 1
// or an offer to save or update the page's login, which bc_tab_show_offer brings back once closed.
#define BC_PASSWORDS_OFFER 2

// Brings back the tab's offer, closed by showing another tab, as Chrome's key icon does, if the
// password manager still holds the login. Then tab_offer_changed. Does nothing for a tab without
// BC_PASSWORDS_OFFER. The user's: agents get none, as the offer names the user's login.
BC_EXPORT void bc_tab_show_offer(bc_id tab);

// Closes the page menu tab_page_menu showed for |tab|, after running its row with |command|, as
// Chrome's menu runs the row the user chose, or none with 0. Does nothing if the tab's menu isn't
// open. The user's: agents get none, as they have no page menu.
BC_EXPORT void bc_tab_close_page_menu(bc_id tab, int command);

// User-only on purpose, unlike the rest of the API: agents get sessions, never secrets.
// Reads the profile's logins, then profile_logins_changed, and again each time they change while
// the profile is loaded.
BC_EXPORT void bc_profile_read_logins(bc_id profile);
// Removes the profile's login of |key| and |username|, as bc_login gives them, or takes its site
// off Never if |never|, so the offer comes there again. Then profile_logins_changed, if read, and
// tab_site_controls for a site the Site Controls show.
BC_EXPORT void bc_profile_remove_login(bc_id profile,
                                       const char* key,
                                       const char* username,
                                       bool never);
// Reads the profile's saved addresses and cards, then profile_autofill_changed, and again each time
// they change while the profile is loaded. Nothing for a profile that isn't loaded.
BC_EXPORT void bc_profile_read_autofill(bc_id profile);
// Removes the profile's address or card of |key|, as bc_autofill_entry gives it. Then
// profile_autofill_changed.
BC_EXPORT void bc_profile_remove_autofill(bc_id profile, const char* key);
// What bc_profile_import_passwords did with its file, for passwords_imported.
#define BC_PASSWORDS_IMPORTED 0
// The file couldn't be read.
#define BC_PASSWORDS_IMPORT_UNREADABLE 1
// Its first row isn't a password export's: url, username and password columns, by any of the
// names Chrome, Firefox and Safari give them.
#define BC_PASSWORDS_IMPORT_NOT_EXPORT 2
// Over Chrome's limits for one file, in its size or its number of logins.
#define BC_PASSWORDS_IMPORT_TOO_LARGE 3
#define BC_PASSWORDS_IMPORT_FAILED 4
// Imports the logins in the file at |path|, a CSV another browser or password manager exported,
// into the profile, with Chrome's password importer, as its password manager's Import passwords
// does, parsing the file away from the browser. A login the profile holds with another password
// stays as it is. The core keeps no copy of the file and doesn't delete it. Returns whether it
// will: not without a loaded profile, or while the profile's last import isn't done. Then
// passwords_imported.
BC_EXPORT bool bc_profile_import_passwords(bc_id profile, const char* path);

// Scrolls the tab's page to put its selection, or its caret, in the middle of the page's view, as
// Edit › Find › Jump to Selection does. Does nothing if it has neither.
BC_EXPORT void bc_tab_jump_to_selection(bc_id tab);

// Pins the tab in its Space at the page it's at, at the bottom of the pinned tabs, or unpins it to
// the top of the Today tabs, with the rest of its split view. Then space_changed and
// tab_state_changed.
BC_EXPORT void bc_tab_set_pinned(bc_id tab, bool pinned);

// Makes the tab the favorite at |index| among those its Space shows, counted without it, else the
// last of them; or moves a favorite there. A tab of a Space leaves it and its split view, and its
// favorite's URL is its pinned URL if it's pinned, else the page it's at. A window showing it keeps
// showing it. Under BC_FAVORITES_EVERY the other profiles' Spaces get the site too, in tabs of
// their own, and moving it moves it there. Under BC_FAVORITES_SPACE a tab of an agent's Space isn't
// made one. Then favorites_changed, and space_changed and tab_state_changed for a tab of a Space.
BC_EXPORT void bc_tab_move_to_favorites(bc_id tab, size_t index);

// Moves the tab, with the rest of its split view, to |index| in its Space's Today tabs, or in its
// pinned tabs, inside |folder| if it isn't 0, counted without them, else to the bottom, and below a
// split |index| is inside. In the pinned tabs or a folder, |index| counts the tabs and folders
// directly in it. A Today tab moved to the pinned tabs is pinned at the page it's at, and a pinned
// tab moved to Today is unpinned. A favorite leaves the favorites for the Space its window shows,
// unless that's an agent's or doesn't show it, pinned at its favorite's URL in the pinned tabs, and
// a window showing it keeps showing it. Under BC_FAVORITES_EVERY the site goes from every profile,
// and its other tabs close. Then space_changed and tab_state_changed, and favorites_changed for a
// favorite.
BC_EXPORT void bc_tab_move(bc_id tab, bool pinned, bc_id folder, size_t index);

// Loads a pinned tab's pinned URL, or a favorite's URL, unless it's there already. Then
// tab_state_changed.
BC_EXPORT void bc_tab_back_to_pinned_url(bc_id tab);

// Copies the URL a pinned tab or a favorite goes back to into |url| as bc_space_name does, or an
// empty string for any other tab.
BC_EXPORT size_t bc_tab_pinned_url(bc_id tab, char* url, size_t capacity);

// Changes the URL a pinned tab or a favorite goes back to, which unpinning forgets, so an undo can
// give a tab back the one it had. Under BC_FAVORITES_EVERY a favorite's site gets it in every
// profile. Does nothing for another tab or an invalid |url|. Then
// space_changed or favorites_changed, and tab_state_changed. The user's: agents get none.
BC_EXPORT void bc_tab_set_pinned_url(bc_id tab, const char* url);

// Opens a copy of the tab, with its history, under it in Today, or at the top of Today for a
// pinned tab, or of the Space its window shows for a favorite. Shows it in its window, where its
// page loads. Returns its id, or 0. Then space_changed and window_selected.
BC_EXPORT bc_id bc_tab_duplicate(bc_id tab);

// Copies the name the user gave the tab into |name| as bc_space_name does, or an empty string if
// it shows its page's title.
BC_EXPORT size_t bc_tab_name(bc_id tab, char* name, size_t capacity);

// Names the tab, which shows |name| for its title from then on, kept across a quit, a reopen and a
// duplicate; an empty name gives it back its page's title. Then tab_title_changed.
BC_EXPORT void bc_tab_rename(bc_id tab, const char* name);

// Gives the tab |icon|, an emoji, UTF-8, to show in place of its page's favicon, kept as its name
// is; empty gives it back its favicon. Then tab_state_changed.
BC_EXPORT void bc_tab_set_icon(bc_id tab, const char* icon);

// Mutes or unmutes the tab, whatever page it's at. Duplicating it and ⇧⌘T keep it muted; a quit
// doesn't. Then tab_state_changed.
BC_EXPORT void bc_tab_set_muted(bc_id tab, bool muted);

// While a page shares a tab and the tab's state has share_instead, has the page share this tab's
// page in its place, as Chrome's Share this tab instead does. The user's: agents get none, as it
// would show a page to whoever the call reaches. Then tab_state_changed, for both tabs.
BC_EXPORT void bc_tab_share_instead(bc_id tab);

// What bc_tab_media does to the page's media, as Chrome's media controls do.
#define BC_MEDIA_PLAY 0
#define BC_MEDIA_PAUSE 1
#define BC_MEDIA_NEXT 2
#define BC_MEDIA_PICTURE_IN_PICTURE 3

// Plays, pauses, skips or floats the page's media in Picture in Picture, one of BC_MEDIA_, while
// its state has media, and media_picture_in_picture for the last. Then tab_state_changed. The
// user's: agents act in the page instead.
BC_EXPORT void bc_tab_media(bc_id tab, int action);

// Closes the Today tabs below the tab, each as bc_tab_close does, bottom up. A window showing one
// shows the tab first. Then tab_closed for each, and space_changed.
BC_EXPORT void bc_tab_close_below(bc_id tab);

// Moves the tab to |space|: a pinned tab to the bottom of its pinned tabs, still pinned, a Today
// tab to the top of its Today tabs. A window showing the tab shows |space| with it. Then
// space_changed for both Spaces, and window_space_shown and window_selected for a window that went
// with it. A page can't take its cookies and logins to another profile, so into another profile's
// Space the tab goes as a new one, with its history, name and mute, which loads once it's shown.
// The old one closes, kept out of the archive and ⇧⌘T's tabs, as it lives on in the new one; then
// tab_closed. Nothing moves into an agent's Space: a tab leaves one only, unless the user gives an
// agent the tab (bc_tab_give_to_agent).
BC_EXPORT void bc_tab_move_to_space(bc_id tab, bc_id space);

// Zooms the tab's page in (|step| 1) or out (-1) to Chrome's next level, or back to 100 % (0), for
// its whole site, as Chrome does, and the profile keeps it. Returns the zoom in percent after.
BC_EXPORT int bc_tab_zoom(bc_id tab, int step);

// What bc_tab_devtools does: opens or closes the DevTools, opens or closes them at the console, or
// opens them picking an element in the page.
#define BC_DEVTOOLS_TOGGLE 0
#define BC_DEVTOOLS_CONSOLE 1
#define BC_DEVTOOLS_INSPECT 2

// Opens Chrome's DevTools for the tab shown, docked in its window, as BC_DEVTOOLS_ |what| says.
// They're the user's: agents get none (decision 8).
BC_EXPORT void bc_tab_devtools(bc_id tab, int what);

// Opens the source of the tab's page in a new tab under it, at its view-source: URL.
BC_EXPORT void bc_tab_view_source(bc_id tab);

// Shows Chrome's print preview for the page of the tab shown, over the page, as ⌘P does in Chrome.
// Nothing while the tab shows another dialog, or its page has crashed. The user's: agents get none.
BC_EXPORT void bc_tab_print(bc_id tab);

// Asks where to save the page of the tab shown, as Chrome's Save Page As, and saves it there. The
// user's: agents get none.
BC_EXPORT void bc_tab_save_page(bc_id tab);

// Opens Chrome's Task Manager, or brings it to the front, with the page |window| shows selected in
// it. The user's: agents get none.
BC_EXPORT void bc_window_task_manager(bc_id window);

// Starts recording a trace of every process, as Help › Troubleshooting › Record a Trace does, with
// Chrome's default categories, keeping the latest events once its buffer is full. Nothing records
// until this. False if a trace is recording already. The user's: agents get no call for it.
BC_EXPORT bool bc_trace_start(void);

// Stops the trace bc_trace_start began, then trace_recorded with it, sent nowhere else. Nothing
// without one.
BC_EXPORT void bc_trace_stop(void);

// Closes the tab: its page gets its unload handlers, which may keep it open, then tab_closed, and
// a Today tab goes in the archive. The window stays open without it. A pinned tab stays in its
// Space instead, and a favorite in the favorites, and only its page goes: a window showing it
// shows the top Today tab of the Space it shows, else the nearest pinned tab still loaded, else no
// page. A tab in a split view leaves it, and a window
// showing it shows the tab beside it, unless |split| closes the rest of the split with it, as
// the ✕ on its row does: a pinned split stays a split. Then tab_state_changed, and
// window_selected.
BC_EXPORT void bc_tab_close(bc_id tab, bool split);

// Windows. Every window shows the same Spaces, and a tab's page is live in one window at a time:
// the one that showed it last. The reads are for the UI and agents alike.

// Copies up to |capacity| of the open windows into |windows|, the most recently active first, and
// returns how many there are. Private windows aren't among them.
BC_EXPORT size_t bc_windows(bc_id* windows, size_t capacity);
// The Space the window shows, and the tab it shows there, as window_space_shown and
// window_selected last said. 0 for none.
BC_EXPORT bc_id bc_window_space(bc_id window);
BC_EXPORT bc_id bc_window_selected(bc_id window);
// The window that shows |tab|'s page, focused or in its split view, else 0. A window whose page
// another window took doesn't count.
BC_EXPORT bc_id bc_tab_window(bc_id tab);
// Shows |space| with no page, as a window ⌘N opens does. The user's: agents get none.
BC_EXPORT void bc_window_show_space_empty(bc_id window, bc_id space);

// Private windows are the user's alone: agents get no path into them or their tabs, not even reads.
// bc_windows leaves them out, and the agent server is to refuse a private window's id, its Space's
// and its tabs'. The UI uses the rest of the API on them, as on any window, but they never show
// another window's Spaces, and closing one closes its tabs.

// Opens a private window at |bounds|, as bc_window_open does, for |profile|'s off-the-record
// profile: Chrome's incognito. Its private windows share one session, cookies and cache, that the
// last one's closing ends (private_session_ended), and nothing of it is written: no history, and
// its Space, which holds its tabs as Today tabs, is never saved, archived or listed by bc_spaces.
// It shows that Space alone, reported by window_space_shown, and no page until one opens. Its
// tabs never move to a regular window, nor regular tabs to it. Closing it closes its tabs. Returns
// 0 if |profile| isn't loaded or Chromium is quitting. The user's alone: agents get none.
BC_EXPORT bc_id bc_window_open_private(bc_rect bounds, void* ui_view, bc_id profile);

// Opens a Little ilo window at |bounds|, as bc_window_open does: Arc's Little Arc, a small window
// for one page, from ⌥⌘N or a link ⌥⌘-clicked (window_open_little). It shows a Space of its own in
// |profile|, which no other window shows, holds Today tabs alone and is never saved or listed by
// bc_spaces, reported by window_space_shown, and no page until one opens. bc_tab_move_to_space
// moves its tab to one of the user's Spaces, and bc_window_select then shows it live in the window
// that selects it; the Little ilo window shows no page after. Closing it closes its tabs, which no
// archive or ⇧⌘T brings back. Returns 0 if |profile| isn't loaded or Chromium is quitting. The
// user's alone, as a private window is: bc_windows leaves it out, and agents get none.
BC_EXPORT bc_id bc_window_open_little(bc_rect bounds, void* ui_view, bc_id profile);

// Extensions' side panels. Each profile's tabs have their own, as each Chrome window does: one for
// every tab, or one for a tab that its extension gave it, which shows only while that tab does. An
// extension opens its panel with sidePanel.open() or from its action, and the user does with
// bc_window_toggle_side_panel. The user's: agents get none, as its page isn't a web page, and a
// private window has none.

// Where the window shows the side panel's page, in the window, its bottom corners as round as the
// page's. The page keeps its distance from the window's edges as the window resizes, as the tab's
// does. An empty frame, as for a page in full screen, hides it.
BC_EXPORT void bc_window_set_side_panel_frame(bc_id window, bc_rect frame);

// Closes the side panel the window shows, as its ✕ in Chrome does. Then window_side_panel.
BC_EXPORT void bc_window_close_side_panel(bc_id window);

// Whether the extension has a side panel for the tab the window shows, as Chrome's extension menu
// asks before it offers Open Side Panel.
BC_EXPORT bool bc_window_has_side_panel(bc_id window, const char* id);

// Opens the extension's side panel for the tab the window shows, or closes it if it shows, as
// Chrome's extension menu does. Then window_side_panel.
BC_EXPORT void bc_window_toggle_side_panel(bc_id window, const char* id);

// Saves the width the user gave the side panel, in the profile of the tab the window shows, for
// window_side_panel to give from now on.
BC_EXPORT void bc_window_set_side_panel_width(bc_id window, double width);

// On Linux, where the app's UI is inside the browser window and can't move it: has the window
// manager or compositor move |window| with the pointer, while the button pressed in the UI is
// down, as a drag on a title bar does. Called on the drag's first move.
BC_EXPORT void bc_window_start_move(bc_id window);

// On Linux, where the UI draws the window's buttons: whether |window| is maximized, which its third
// button shows, as restore rather than maximize. The UI asks as the window's bounds change, as
// maximizing and restoring change them.
BC_EXPORT bool bc_window_is_maximized(bc_id window);

// On Linux and Windows: whether |window| is full screen, as the window manager's own full-screen
// key, bc_window_toggle_full_screen or a page can put it. The UI asks as the window's bounds change.
BC_EXPORT bool bc_window_is_full_screen(bc_id window);

// On Linux and Windows: puts |window| full screen, or takes it out, as the window manager's own
// full-screen key does on Linux, for the test driver, and on Windows, which has none, for Escape
// too. On Linux bc_window_toggle_maximized takes it out as Escape asks.
BC_EXPORT void bc_window_toggle_full_screen(bc_id window);
// The browser's account, which keeps the Spaces and the first profile's bookmarks, passwords and
// open tabs the same on each of its devices, through an ilo server (ilo-server's docs/protocol.md).
// The server stores what's synced encrypted with the account's secret, which only devices hold: the
// first device makes it and shows it once as 24 recovery words, and a new device gets it from one
// that has it, by pairing, or from the words. The core signs in, keeps the tokens and the secret,
// and syncs. The app sees neither, only the words when the user asks for them. Each step comes back
// in sync_changed. All of it is the user's alone: agents get no account or sync calls, as the
// account holds the user's passwords and its secret reads them, and the sign-in window is no tab an
// agent can act in.

// Signs in to |server| from now on, such as "https://sync.example.org", or to the build's own if it's
// empty, signing out first if signed in, and returns whether it will: only to https, or to http on
// a loopback address, for a server on the machine. Then sync_changed.
BC_EXPORT bool bc_sync_set_server(const char* server);

// Opens the server's sign-in page in a small window of the core's over the front window: the page
// alone, the server's host in its title, in a session of its own that keeps nothing in the user's
// profiles. Once the user has signed in, the core catches where the page sends it, closes the
// window and gets the account's tokens; the user closing the window cancels. Only while signed out.
// Then sync_changed, BC_SYNC_SIGNING_IN, then BC_SYNC_FIRST_DEVICE or BC_SYNC_NEEDS_SECRET, or
// BC_SYNC_SIGNED_OUT again, with BC_SYNC_PROBLEM_SERVER if it failed.
BC_EXPORT void bc_sync_sign_in(void);

// Stops what the account waits for: the sign-in window closes, back to BC_SYNC_SIGNED_OUT, or the
// pairing request is taken back from the other devices, back to BC_SYNC_NEEDS_SECRET. Then
// sync_changed.
BC_EXPORT void bc_sync_cancel(void);

// Signs this device out: the server forgets it, and the core its tokens and the secret. What synced
// stays on the device, and nothing more syncs. Then sync_changed.
BC_EXPORT void bc_sync_sign_out(void);

// Copies the account's recovery words into |words| as bc_space_name does: its secret as 24 words of
// BIP-39's English list, lowercase, one space between them (protocol.md, The account secret). With
// BC_SYNC_FIRST_DEVICE it makes the secret first and turns sync on, then sync_changed with
// BC_SYNC_ON; with BC_SYNC_ON it copies the secret's. Else it copies nothing and returns 0.
BC_EXPORT size_t bc_sync_recovery_words(char* words, size_t capacity);

// Gets the secret from recovery words the user typed, with BC_SYNC_NEEDS_SECRET, or BC_SYNC_PAIRING,
// whose request it takes back. Returns false, doing nothing, unless |words| are 24 words of the list
// with their checksum, in any case and spacing. Then sync_changed, with BC_SYNC_ON once they open
// the account's data, or BC_SYNC_NEEDS_SECRET with BC_SYNC_PROBLEM_WRONG_WORDS.
BC_EXPORT bool bc_sync_enter_recovery_words(const char* words);

// Starts the account over for a user with neither another device nor the recovery words, with
// BC_SYNC_NEEDS_SECRET, or BC_SYNC_PAIRING, whose request it takes back: it deletes everything the
// account keeps on the server, its synced data and the devices it knows (Chromium's
// ClearServerData), and makes this the account's first device again. What this device has stays,
// and syncs up again under the new secret. Then sync_changed, with BC_SYNC_FIRST_DEVICE, whose
// bc_sync_recovery_words makes the new secret and its words, or with the status it had and
// BC_SYNC_PROBLEM_SERVER if the server couldn't do it. Does nothing in any other status.
BC_EXPORT void bc_sync_start_over(void);

// Asks the account's other devices, through the server, to add this one, with BC_SYNC_NEEDS_SECRET:
// it sends a commitment to a key made for it, with this device's name and system (protocol.md,
// Pairing). Then sync_changed with BC_SYNC_PAIRING, again with its code once a device answers, then
// BC_SYNC_ON once that device adds it, or BC_SYNC_NEEDS_SECRET with BC_SYNC_PROBLEM_DECLINED,
// BC_SYNC_PROBLEM_EXPIRED or BC_SYNC_PROBLEM_SERVER.
BC_EXPORT void bc_sync_request_pairing(void);

// Answers the request sync_pairing_asked asked about last, |id|: |add| seals the secret to the
// asking device, and else the core declines it, which that device says.
BC_EXPORT void bc_sync_answer_pairing(const char* id, bool add);

// Asks the server for the account's devices, while signed in. Then sync_devices_changed.
BC_EXPORT void bc_sync_read_devices(void);

// Signs the device |id| out of the account, one other than this, which bc_sync_sign_out signs out.
// It keeps what synced, and syncs again only once it signs in and gets the secret. Then
// sync_devices_changed.
BC_EXPORT void bc_sync_remove_device(const char* id);

// On Linux, the title of the window of Chromium's own with the keys in |window|
// (bc_window_keyboard_in_child), such as a tab's dialog, copied into |title| as bc_space_name does,
// or an empty string. For the test driver, as the X server has none for it.
BC_EXPORT size_t bc_window_child_title(bc_id window, char* title, size_t capacity);

// On Linux, under X11: answers the requests for the clipboard's contents that other clients have
// sent Chromium, from what it holds, and dispatches no other event of its. The UI calls it while its
// own read of the clipboard waits, as the owner it asks can be Chromium, which answers on the UI's
// thread.
BC_EXPORT void bc_answer_clipboard_requests(void);

#ifdef __cplusplus
}
#endif

#endif  // BROWSER_CORE_PUBLIC_BROWSER_CORE_H_
