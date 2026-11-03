# VoiceOver reads ilo's own tree: the sidebar's Space name is a heading, and the tab shown a
# selected tab named by its page's title in the Today tab group, after New Tab. ⌘T's bar has the
# keyboard in a text field called "Search or Enter URL", lists its suggestions with the first
# selected, and ↓ selects the next; ⇧F10 opens its field's menu on the first item. A tab's menu is
# the app's window in front and a menu of menu items and separators, whose Move to opens a submenu.
# ⌘F's find bar names its buttons and reads its count as Matches, a status that says it politely.
# It reads the tree from outside, as an assistive app does: on the Mac through tools/ax-tree.sh,
# which needs this terminal to have the Accessibility permission, on Linux as Orca does, over
# AT-SPI through tools/atspi-tree.sh, which needs dbus-daemon and at-spi2-core, and on Windows as
# Narrator does, over UI Automation through tools/uia-tree.sh, which needs Python's comtypes.
# Without them the run says it skipped, and it never asks for the permission.

# Each role as the run's reader writes it, and how the tab shown says it's selected.
if [ "$(uname)" = Linux ]; then
  screen_reader=Orca reader=../tools/atspi-tree.sh
  heading=heading button=button tab_group='page tab list' tab='page tab' shown=selected
  field=entry suggestion='list item' menu_role=menu menu_at=' menu "Menu"'
  submenu_role=menu menu_item='menu item' separator=separator
elif windows; then
  # Control type/ARIA role: UI Automation has no heading type, and AccessKit says it in the role.
  screen_reader=Narrator reader=../tools/uia-tree.sh
  heading=Text/heading button=Button/button tab_group=Tab/tablist tab=TabItem/tab shown=selected
  field=Edit/textbox suggestion=ListItem/ menu_role=Menu/menu menu_at=' Menu/menu "Menu"'
  submenu_role=Menu/menu menu_item=MenuItem/menuitem separator=Separator/separator
else
  screen_reader=VoiceOver reader=../tools/ax-tree.sh
  heading=AXHeading/- button=AXButton/- tab_group=AXTabGroup/- tab=AXRadioButton/AXTabButton
  shown=value=1 field=AXTextField/- suggestion=AXStaticText/
  menu_role='AXMenu/[^ ]*' menu_at=' AXMenu/' submenu_role=AXMenu/- menu_item='AXMenuItem/[^ ]*'
  separator=AXSplitter/
fi
session=
a11y=
skipped=
# Off the Mac GPUI publishes its tree only while the session bus's org.a11y.Status says AT-SPI is
# on, which on the desktop's bus would put every app there on it, so the app gets a session bus of
# its own. That bus starts at-spi-bus-launcher, which starts AT-SPI's own bus, and the run starts
# its registry: where that bus is dbus-broker's, it would ask the desktop's systemd for it.
if [ "$(uname)" = Linux ]; then
  started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
  session=$(echo "$started" | cut -d' ' -f1)
  buses=$(echo "$started" | cut -d' ' -f2)
  [ -z "$session" ] || a11y=$(DBUS_SESSION_BUS_ADDRESS=$session busctl --user call \
    org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress 2> /dev/null | sed -n 's/^s "\(.*\)"$/\1/p')
  registry=
  for each in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-registryd \
    /usr/lib/at-spi2-core/at-spi2-registryd; do
    [ -x "$each" ] && registry=$each && break
  done
  if [ -z "$a11y" ] || [ -z "$registry" ]; then
    skipped="no AT-SPI bus answers, as dbus-daemon or at-spi2-core isn't installed"
  else
    AT_SPI_BUS_ADDRESS=$a11y "$registry" > "$out/access-registry.log" 2>&1 &
    buses="$buses $!"
    for _ in $(seq 50); do
      busctl --address="$a11y" status org.a11y.atspi.Registry > /dev/null 2>&1 && break
      sleep 0.1
    done
    DBUS_SESSION_BUS_ADDRESS=$session busctl --user set-property org.a11y.Bus /org/a11y/bus \
      org.a11y.Status IsEnabled b true || skipped="AT-SPI's bus didn't turn on"
  fi
fi
# The tree, read again until a line has $1.
read_tree() {
  if windows; then
    "$reader" "$(cat "/proc/$pid/winpid")" "$1"
  else
    AT_SPI_BUS_ADDRESS=$a11y "$reader" "$pid" "$1"
  fi
}

page='data:text/html,<title>Docs</title><p>A page for VoiceOver to find in the sidebar.'
log="$out/access.log"
step=0
rm -f "$saved" "$launch_frame"
if [ -n "$session" ]; then
  DBUS_SESSION_BUS_ADDRESS=$session BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" \
    < "$out/drive" > "$log" 2>&1 &
else
  BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
fi
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* Docs$' || fail "the page didn't draw its title"
a=$(today)
# The lines deeper than the first line of tree $2 that has $1, up to the next as shallow.
under() {
  awk -v at="$1" 'found && $1 <= depth { exit } found { print }
    !found && index($0, at) { found = 1; depth = $1 }' "$2"
}

tree="$out/access-sidebar.txt"
if [ -n "$skipped" ]; then
  echo "accessibility: skipped, $skipped" > "$tree"
else
  read_tree "$tab_group \"Today\"" > "$tree"
fi
if grep -q '^accessibility: skipped' "$tree"; then
  exec 3>&-
  quit
  echo "skip: access. Nothing read ilo's tree: $(sed -n 's/^accessibility: skipped, //p' "$tree")."
else
  grep -q " $heading \"[^\"]*\" id=space-name\$" "$tree" ||
    fail "the Space's name isn't a heading: $tree"
  grep -q " $button \"New Tab\".* id=new-tab\$" "$tree" || fail "New Tab isn't a button: $tree"
  under "$tab_group \"Today\"" "$tree" | grep -q " $tab \"Docs\" $shown id=tab-$a\$" ||
    fail "the tab shown isn't the selected tab Docs in Today: $tree"
  # New Tab comes before Today, as it's drawn.
  [ "$(grep -n 'id=new-tab$' "$tree" | cut -d: -f1)" -lt \
    "$(grep -n "$tab_group \"Today\"" "$tree" | cut -d: -f1)" ] ||
    fail "New Tab doesn't come before Today: $tree"

  send click $page_point
  if [ "$active" != 1 ]; then
    exec 3>&-
    quit
    echo "skip: access's command bar. Browser wasn't the active app, so ⌘T would go elsewhere. The sidebar's tree: $tree"
  else
    send key t cmd
    expect '^browser: command bar shown$' "⌘T didn't open the command bar"
    send type docs
    expect '^browser: command bar suggestions "docs"' "the command bar didn't suggest for \"docs\""
    bar="$out/access-bar.txt"
    read_tree '"Suggestions"' > "$bar"
    grep -q " $field \"Search or Enter URL\" value=docs" "$bar" ||
      fail "the command bar's field isn't a text field with what was typed: $bar"
    rows=$(under '"Suggestions"' "$bar")
    [ "$(echo "$rows" | grep -c " $suggestion")" -ge 2 ] ||
      fail "the command bar doesn't list two suggestions: $bar"
    echo "$rows" | grep " $suggestion" | head -1 | grep -q ' selected' ||
      fail "the first suggestion isn't selected: $bar"
    send key down
    expect '^browser: command bar selected ' "↓ didn't select the next suggestion"
    moved="$out/access-bar-down.txt"
    read_tree '"Suggestions"' > "$moved"
    under '"Suggestions"' "$moved" | grep " $suggestion" | sed -n 2p | grep -q ' selected' ||
      fail "↓ didn't move the selected suggestion: $moved"
    # ⇧F10, Full Keyboard Access's way to a menu, opens the field's under the caret on its first
    # item, and escape gives the keyboard back to the field.
    send key f10 shift
    expect '^browser: menu shown Cut[^,]*, Copy[^,]*, Paste, ' "⇧F10 didn't open the field's menu"
    expect '^browser: menu highlights \(Cut\|Paste\) at ' \
      "the field's menu from ⇧F10 didn't highlight its first item"
    expect '^browser: menu window at ' "the field's menu didn't take the keyboard"
    send key escape
    expect '^browser: menu closed$' "escape didn't close the field's menu"
    none_since '^browser: command bar hidden$' || fail "escape in the field's menu closed the bar"
    send key escape
    expect '^browser: command bar hidden$' "escape didn't close the command bar"

    # ilo's menu is the app's window in front, and reads as a menu: a menu item for each row, a
    # separator between groups, and Move to saying it opens a submenu, which → opens.
    post rightclick $(row 0)
    expect "^browser: menu shown $(cased 'Copy Link, ')" "a right-click on the tab didn't show its menu"
    expect '^browser: menu window at ' "the tab's menu didn't open its window"
    size=$(grep '^browser: menu window at' "$log" | tail -1 | cut -d' ' -f6)
    menu="$out/access-menu.txt"
    read_tree '"Duplicate"' > "$menu"
    windows="$out/access-menu-windows.txt"
    if [ "$(uname)" = Linux ] || windows; then
      # AT-SPI has no window order, but the window in front is the one that's active, and the
      # Windows reader marks it alike.
      awk -v at="$menu_at" '$1 == 0 { window = $0 } index($0, at) { print window; exit }' \
        "$menu" > "$windows"
      grep -q ' active' "$windows" || fail "the tab's menu isn't in the active window: $menu"
    else
      ../tools/ax-windows.sh "$pid" > "$windows"
      sed -n '/^window server, front first:/{n;p;}' "$windows" | grep -q " $size\$" ||
        fail "the tab's menu, $size, isn't the app's window in front: $windows"
    fi
    rows=$(under "$menu_at" "$menu")
    grep -q " $menu_role \"Menu\"" "$menu" || fail "the tab's menu isn't a menu: $menu"
    echo "$rows" | grep -q " $menu_item \"Duplicate\"" || fail "Duplicate isn't a menu item: $menu"
    echo "$rows" | grep -q " $separator" || fail "the lines between groups aren't separators: $menu"
    echo "$rows" | grep -q " $menu_item \"Move to\" .*collapsed" ||
      fail "Move to doesn't say it opens a submenu: $menu"
    send key m
    expect '^browser: menu highlights Move to at ' "typing m didn't select Move to"
    send key right
    expect '^browser: menu submenu Move to shown at ' "→ didn't open Move to's submenu"
    submenu="$out/access-menu-submenu.txt"
    read_tree "$submenu_role \"Move to\"" > "$submenu"
    grep -q " $menu_item \"Move to\" .*expanded" "$submenu" ||
      fail "Move to doesn't say its submenu is open: $submenu"
    under " $submenu_role \"Move to\"" "$submenu" | grep -q " $menu_item \"New Space…\"" ||
      fail "Move to's submenu doesn't list New Space…: $submenu"
    send key escape
    send key escape
    expect '^browser: menu closed$' "escape didn't close the tab's menu"

    # The find bar's buttons say what they do, and its count is the status Matches.
    found="$out/access-find.txt"
    if windows; then
      # check/find.sh, which drives the find bar, isn't one of Windows' runs yet.
      echo "find bar: not read on Windows, where the find run isn't tried yet" > "$found"
    else
      send click $page_point
      send key f cmd
      send type VoiceOver
      expect '^browser: find bar drew "VoiceOver" 1/1$' "⌘F and typing didn't find VoiceOver on the page"
      read_tree '"Find Next"' > "$found"
      grep -q ' "Matches"' "$found" || fail "the find bar's count isn't called Matches: $found"
      for name in "Find Previous" "Find Next" "Close Find Bar"; do
        grep -q " $button \"$name\"" "$found" || fail "the find bar's $name isn't a button: $found"
      done
      send key escape
      expect "^browser: find $a closed\$" "escape didn't close the find bar"
    fi
    exec 3>&-
    quit
    echo "ok: $screen_reader reads the Space's name as a heading, the tab shown as the selected tab Docs in Today after New Tab, ⌘T's bar as a text field with its suggestions in a list whose selection ↓ moves, ⇧F10's field menu on its first item, the tab's menu, in front, as a menu with its items, separators and a submenu → opens, and the find bar's named buttons and its Matches. Trees: $tree, $bar, $moved, $menu, $submenu, $windows, $found"
  fi
fi
# The run's buses go with it.
[ -z "${buses:-}" ] || kill $buses 2> /dev/null || true
buses=
