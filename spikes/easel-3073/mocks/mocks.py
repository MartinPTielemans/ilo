# #3073's screens. Each takes make.py (m) and a theme (t) and writes <screen>-<theme>.html over the
# preview's capture. The card is the preview's page card, [shell] card-inset 10 inside the window at
# the sidebar's 228. The patches' fills are sampled from the captures, so they sit flush.
CARD = (228, 10, 1042, 780)
# Sampled: the sidebar's URL field and selected row; the Library's rail, its selected plate, the
# column, the search field and the card.
CAP = {
    "light": dict(ground="#cccccc", url="#b9b9b9", row="#e5e5e5", rail="#c1c1c1", rail_plate="#b2b2b1",
                  column="#cccccc", search="#bcbcbb"),
    "dark": dict(ground="#000000", url="#191818", row="#0a0909", rail="#0e0e0e", rail_plate="#232322",
                 column="#000000", search="#161615"),
}


def row_box(m, t, x, y, w, h):
    c = CAP[t["name"]]
    shadow = ";box-shadow:0 0 0 0.5px rgba(0,0,0,0.08),0 1px 2px rgba(0,0,0,0.2)" if t["name"] == "light" else ""
    return f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;border-radius:12px;background:{c["row"]}{shadow}"></div>'


def url_field(m, t, label):
    # The URL field names the Board: it has no address to copy until #680 shares one, so its copy
    # button isn't there (Quiet controls).
    c = CAP[t["name"]]
    return (f'<div class=a style="left:8px;top:46px;width:212px;height:36px;border-radius:10px;background:{c["url"]};'
            f'display:flex;align-items:center;gap:6px;padding:0 13px;font:400 13px Inter;color:{m.ink(t, 0.72)}">'
            f'{m.icon("presentation", 14, m.ink(t, 0.6))}{label}</div>')


def row_patch(m, t, label, field=False, beside=None):
    """The selected pinned row, its tab a Board's: the presentation glyph and the Board's name. With
    field, its name in the row's field, all selected, as New Folder's is. With beside, the row is a
    split's, as the preview draws one: the first tab's glyph and name, a rule, the second's glyph."""
    x, y, w, h = 8, 246, 212, 36
    out = url_field(m, t, label) + row_box(m, t, x, y, w, h)
    glyph = m.icon("presentation", 16, m.ink(t, 0.75))
    if field:
        out += (f'<div class=a style="left:{x + 10}px;top:{y}px;height:{h}px;display:flex;align-items:center">{glyph}</div>'
                f'<div class=a style="left:{x + 32}px;top:{y + 6}px;width:{w - 40}px;height:24px;border-radius:6px;'
                f'background:{t["field"]};box-shadow:0 0 0 1.5px {m.ACCENT};display:flex;align-items:center;padding:0 5px">'
                f'<span style="font:500 13px Inter;color:{m.ink(t, 0.9)};background:rgba(40,144,84,0.3)">{label}</span></div>')
        return out
    name = f'<span style="font:500 13px Inter;color:{m.ink(t, 0.9)};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;flex:1">{label}</span>'
    tail = ""
    if beside:
        tail = (f'<span style="width:1px;height:16px;background:{m.ink(t, 0.2)}"></span>'
                f'{m.icon(beside, 16, m.ink(t, 0.75))}')
    out += (f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;display:flex;align-items:center;'
            f'gap:9px;padding:0 6px 0 10px">{glyph}{name}{tail}</div>')
    return out


def window(m, t, base):
    x, y, w, h = CARD
    inner = m.board_full(t, w, h) + m.strip(t, w / 2, 12)
    body = row_patch(m, t, "Kyoto trip") + m.card(x, y, w, h, inner)
    m.write(f"board-window-{t['name']}", m.page(body, f"{base}/sidebar-{t['name']}.png"))


def page_pane(m, t, w, h):
    """A made-up article at example.org, in the page's own look."""
    fg = "#1d1d1f" if t["name"] == "light" else "#ececec"
    sub = "#555" if t["name"] == "light" else "#a8a8a8"
    para = ("The path follows a canal lined with cherry trees for about two kilometres, from "
            "Ginkaku-ji in the north to Nanzen-ji. Go early: by ten the narrow stretches fill.")
    more = ("Halfway along, Honen-in sits up a short flight of steps under the trees. Its gate is "
            "thatched, and the sand beds inside are raked into a new pattern each season.")
    return (f'<div class=a style="inset:0;background:{t["page"]};padding:36px 36px;color:{fg};font:16px/1.55 Georgia,serif">'
            f'<div style="font:600 12px Inter;letter-spacing:.06em;color:{sub};text-transform:uppercase">Walks · Kyoto</div>'
            f'<div style="font:700 30px/1.15 Georgia,serif;margin:10px 0 18px">The Philosopher\'s Path</div>'
            f'<div style="position:relative;width:{w - 72}px;height:220px">{m.photo(t, 0, 0, w - 72, 220, "temple")}</div>'
            f'<p style="margin-top:18px">{para}</p><p style="margin-top:12px;color:{sub}">{more}</p></div>')


def split(m, t, base):
    x, y, w, h = CARD
    gap = 10  # [shell] split-gap
    pw = (w - gap) / 2
    # The page's picture, dragged into the Board's empty stretch under the first day's notes. Nothing
    # snaps in the first cut, so there's no slot: the picture rides the pointer half seen, as a drag
    # from a page does, and lands where it's held.
    dx, dy = 70, 420
    drag = (f'<div class=a style="left:{dx + 14}px;top:{dy + 12}px;width:220px;height:146px;opacity:.7">'
            f'{m.photo(t, 0, 0, 220, 146, "temple")}</div>'
            f'<svg class=a style="left:{dx + 124}px;top:{dy + 84}px" width="18" height="24" viewBox="0 0 18 24">'
            f'<path d="M1 1 L1 19 L6 14 L9.5 22 L12.5 20.8 L9.2 13 L16 13 Z" fill="#000" stroke="#fff" stroke-width="1.3"/></svg>')
    lpane = (f'<div class=a style="left:0;top:0;width:{pw}px;height:{h}px;overflow:hidden">'
             f'{m.board_full(t, w, h, selected=False)}{m.strip(t, pw / 2, 12)}</div>')
    body = (row_patch(m, t, "Kyoto trip", beside="globe")
            + f'<div class=a style="left:{x + pw}px;top:0;width:{gap}px;height:800px;background:{CAP[t["name"]]["ground"]}"></div>'
            + m.card(x, y, pw, h, lpane + drag) + m.card(x + pw + gap, y, pw, h, page_pane(m, t, pw, h)))
    m.write(f"board-split-{t['name']}", m.page(body, f"{base}/sidebar-{t['name']}.png"))


def chip(m, t, k):
    return (f'<span style="display:inline-grid;place-items:center;min-width:22px;height:20px;padding:0 5px;'
            f'border-radius:6px;background:{m.ink(t, 0.08)};font:500 11px Inter;color:{m.ink(t, 0.7)};margin-right:6px">{k}</span>')


def empty(m, t, base):
    x, y, w, h = CARD
    hint = (f'<div class=a style="left:0;right:0;top:{h / 2 - 34}px;text-align:center;font:400 15px/20px Inter;color:{m.ink(t, 0.5)}">'
            f'Type to write, or paste and drop pictures and links.</div>'
            f'<div class=a style="left:0;right:0;top:{h / 2 + 2}px;display:flex;justify-content:center;gap:18px;'
            f'font:400 13px Inter;color:{m.ink(t, 0.5)}">'
            f'<span>{chip(m, t, "⌘V")}Paste</span><span>{chip(m, t, "P")}Draw</span><span>{chip(m, t, "⌘+")}Zoom in</span></div>')
    inner = (f'<div class=a style="left:0;top:0;width:{w}px;height:{h}px;{m.ground(t, w, h)}"></div>'
             + hint + m.strip(t, w / 2, 12, chosen="type"))
    body = row_patch(m, t, "Untitled Board", field=True) + m.card(x, y, w, h, inner)
    m.write(f"board-empty-{t['name']}", m.page(body, f"{base}/sidebar-{t['name']}.png"))


def first_note(m, t, base):
    x, y, w, h = CARD
    caret = f'<span style="display:inline-block;width:1.5px;height:18px;background:{m.ACCENT};vertical-align:-3px;margin-left:1px"></span>'
    inner = (f'<div class=a style="left:0;top:0;width:{w}px;height:{h}px;{m.ground(t, w, h)}"></div>'
             + m.note(t, w / 2 - 110, h / 2 - 24, 220, "Ky" + caret, "", editing=True)
             + m.strip(t, w / 2, 12, chosen="type"))
    body = row_patch(m, t, "Kyoto trip") + m.card(x, y, w, h, inner)
    m.write(f"board-first-note-{t['name']}", m.page(body, f"{base}/sidebar-{t['name']}.png"))


STRIP_W = None  # make.STRIP_W, read in tools()


def tools(m, t, base):
    """The tools at 1x on the canvas, in their states: at rest; Draw under the pointer with its
    tooltip; Draw chosen with its pen; the zoom menu; and the selection's bar for each kind."""
    W, H = 1280, 800
    s = f'<div class=a style="inset:0;{m.ground(t, W, H)}"></div>'
    lab = lambda x, y, text: (f'<div class=a style="left:{x}px;top:{y}px;font:600 11px Inter;letter-spacing:.04em;'
                              f'text-transform:uppercase;color:{m.ink(t, 0.45)}">{text}</div>')
    pop = lambda x, y, w, inner: (f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;border-radius:16px;background:{t["strip"]};'
                                  f'box-shadow:{t["popover"]};padding:6px">{inner}</div>')
    top = 180
    bottom = H - top - 44
    pen = lambda cx: cx - m.STRIP_W / 2 + 6 + 3 * 34 + 16  # the pencil's centre
    s += lab(48, 36, "At rest, Select chosen") + m.strip(t, 210, bottom)
    s += lab(448, 36, "Draw under the pointer") + m.strip(t, 610, bottom, hovered="pencil")
    s += m.tooltip(pen(610), top + 22, "Draw", "P")
    s += lab(848, 36, "Draw chosen opens its pen") + m.strip(t, 1010, bottom, chosen="pencil")
    cols = ["rgb(" + t["ink"] + ")"] + [m.SW[k] for k in ("red", "orange", "yellow", "green", "blue", "plum", "pink", "slate")]
    swatches = "".join(
        f'<div style="width:24px;height:24px;display:grid;place-items:center"><div style="width:16px;height:16px;border-radius:50%;'
        f'background:{c};box-shadow:{("0 0 0 1.5px " + t["strip"] + ",0 0 0 3px " + m.ACCENT) if i == 1 else "inset 0 0 0 0.5px " + m.ink(t, 0.25)}"></div></div>'
        for i, c in enumerate(cols))
    widths = "".join(
        f'<div style="width:36px;height:28px;border-radius:10px;display:grid;place-items:center;background:{m.ink(t, t["plate_on"]) if i == 1 else "transparent"}">'
        f'<div style="width:18px;height:{wd}px;border-radius:{wd}px;background:{m.ink(t, 0.75)}"></div></div>'
        for i, wd in enumerate((2, 3.5, 6)))
    pw = 9 * 24 + 4 + 12
    s += pop(pen(1010) - pw / 2, top - 8 - 84, pw,
             f'<div style="display:flex;padding:2px">{swatches}</div>'
             f'<div style="height:1px;background:{m.ink(t, t["line"])};margin:5px 2px"></div>'
             f'<div style="display:flex;gap:4px;padding:0 2px">{widths}</div>')
    # The zoom menu, from the strip's percent, its keys at the rows' ends.
    rows = [("Zoom In", "⌘+"), ("Zoom Out", "⌘−"), ("Actual Size", "⌘0"), ("Zoom to Fit", "⇧1")]
    menu = "".join(
        f'<div style="height:28px;display:flex;align-items:center;justify-content:space-between;padding:0 10px;border-radius:10px;'
        f'font:400 13px Inter;color:{m.ink(t, 0.85)};background:{m.ink(t, t["plate"]) if i == 0 else "transparent"}">'
        f'<span>{a}</span><span style="color:{m.ink(t, 0.45)}">{k}</span></div>' for i, (a, k) in enumerate(rows))
    s += lab(48, 300, "The zoom menu") + pop(48, 324, 200, menu)
    bar = lambda x, y, items: (f'<div class=a style="left:{x}px;top:{y}px;height:40px;padding:6px;border-radius:16px;background:{t["strip"]};'
                               f'box-shadow:{t["popover"]};display:flex;align-items:center;gap:2px">'
                               + "".join(f'<div style="height:28px;padding:0 8px;border-radius:10px;display:flex;align-items:center;gap:6px;'
                                         f'font:500 13px Inter;color:{m.ink(t, 0.72)}">{i}</div>' for i in items) + "</div>")
    ring = lambda x, y, w, h, r: f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;border-radius:{r}px;box-shadow:0 0 0 1.5px {m.ACCENT}"></div>'
    more = m.icon("ellipsis", 16, m.ink(t, 0.72))
    s += lab(300, 300, "A note: fill, size, more") + m.context_bar(t, 300, 324, chosen=2)
    s += m.note(t, 300, 380, 200, "Bring", "Rail pass · coin purse · umbrella", fill=m.SW["yellow"], selected=True)
    s += lab(620, 300, "A picture") + bar(620, 324, ["Copy", more])
    s += m.photo(t, 620, 380, 200, 136, "bamboo") + ring(615, 375, 210, 146, 9)
    s += lab(900, 300, "A link") + bar(900, 324, ["Open", more])
    s += m.link(t, 900, 380, 280, "Nishiki Market: hours and stalls", "example.com", None) + ring(895, 375, 290, 66, 15)
    s += lab(48, 580, "A drawing") + bar(48, 604, [
        f'<div style="width:16px;height:16px;border-radius:50%;background:{m.SW["red"]}"></div>',
        f'<div style="width:18px;height:3.5px;border-radius:3px;background:{m.ink(t, 0.75)}"></div>', more])
    s += m.drawing(t, f'<path d="M64 720 C 124 668, 204 676, 264 726" stroke="{m.SW["red"]}" stroke-width="3.5" fill="none" stroke-linecap="round"/>', W, H)
    s += ring(54, 664, 220, 72, 6)
    s += lab(448, 580, "Several selected") + bar(448, 604, ["Copy", more])
    s += m.note(t, 448, 664, 150, "Day 4", "Fushimi Inari, early.") + m.note(t, 614, 664, 150, "Day 5", "Pack, Nishiki, train.")
    s += ring(442, 658, 328, 84, 15)
    s += lab(840, 580, "A note with no fill, size L") + m.context_bar(t, 840, 604, chosen=0, size="L")
    s += m.note(t, 840, 664, 360, "Kyoto in April", "Five days, mostly on foot.", fill="none", size="L", selected=True)
    m.write(f"board-tools-{t['name']}", m.page(s))


def thumb(m, t, w, h, kind):
    """An Board's tile in the Library: its first screen, drawn small."""
    if kind == "kyoto":
        s = w / 1042
        return (f'<div style="position:relative;width:{w}px;height:{h}px;border-radius:12px;overflow:hidden;box-shadow:{t["resting"]}">'
                f'<div style="position:absolute;left:0;top:0;width:1042px;height:780px;transform:scale({s});transform-origin:0 0">'
                f'{m.board_full(t, 1042, 780, selected=False)}</div></div>')
    shapes = {
        "kitchen": [(10, 10, 52, 36, "#c9b79c"), (68, 10, 60, 40, m.SW["sand"]), (10, 52, 40, 30, m.SW["green"]), (56, 58, 30, 24, "note")],
        "reading": [(12, 12, 34, 20, "note"), (12, 36, 34, 20, "note"), (12, 60, 34, 20, "note"), (54, 12, 74, 68, m.SW["blue"])],
        "q3": [(10, 12, 36, 28, m.SW["yellow"]), (52, 12, 36, 28, m.SW["yellow"]), (94, 12, 34, 28, m.SW["pink"]), (10, 48, 118, 34, "note")],
        "birthday": [(16, 14, 50, 62, m.SW["pink"]), (74, 18, 50, 26, "note"), (74, 50, 40, 26, m.SW["plum"])],
        "empty": [],
    }[kind]
    rects = "".join(
        f'<div style="position:absolute;left:{x}px;top:{y}px;width:{rw}px;height:{rh}px;border-radius:{2 if c != "note" else 3}px;'
        f'background:{t["note"] if c == "note" else c};box-shadow:0 0 0 0.5px {m.ink(t, 0.12)}"></div>'
        for x, y, rw, rh, c in shapes)
    return (f'<div style="position:relative;width:{w}px;height:{h}px;border-radius:12px;overflow:hidden;'
            f'box-shadow:{t["resting"]};background:{t["ground"]}">{rects}</div>')


SECTIONS = [("Media", "image"), ("Downloads", "arrow-down-to-line"), ("Boards", "presentation"),
            ("Spaces", "spaces"), ("History", "rotate-ccw-clock"), ("Agents", "square-mouse-pointer"),
            ("Boosts", "paintbrush")]


def library(m, t, base):
    """The Library on Boards: the seventh section, between Downloads and Spaces, as Arc's sits
    after Downloads. At the 800-tall window seven buttons 100 apart would reach the title, so the
    gap between them gives way to 10."""
    c = CAP[t["name"]]
    s = f'<div class=a style="left:0;top:84px;width:112px;height:676px;background:{c["rail"]}"></div>'
    pitch, top = 90, 85
    for i, (label, glyph) in enumerate(SECTIONS):
        on = label == "Boards"
        y = top + i * pitch
        s += (f'<div class=a style="left:8px;top:{y}px;width:97px;height:80px;border-radius:12px;'
              f'background:{c["rail_plate"] if on else "transparent"}">'
              f'<div class=a style="left:38.5px;top:21px">{m.icon(glyph, 20, m.ACCENT if on else m.ink(t, 0.6))}</div>'
              f'<div class=a style="left:0;right:0;top:48.5px;text-align:center;font:600 11px/15px Inter;color:{m.ink(t, 0.85)}">{label}</div></div>')
    s += f'<div class=a style="left:113px;top:0;width:304px;height:800px;background:{c["column"]}"></div>'
    s += (f'<div class=a style="left:120px;top:10px;width:289px;height:35px;border-radius:10px;background:{c["search"]};'
          f'display:flex;align-items:center;gap:14px;padding:0 14px;font:400 14px Inter;color:{m.ink(t, 0.6)}">'
          f'{m.icon("search", 15, m.ink(t, 0.6))}Search Boards…</div>')
    head = lambda y, text: f'<div class=a style="left:124px;top:{y}px;font:600 11px Inter;color:{m.ink(t, 0.62)}">{text}</div>'

    def tile(x, y, kind, name, sub, shown=False):
        plate = (f'<div class=a style="left:{x - 5}px;top:{y - 5}px;width:144px;height:146px;border-radius:16px;'
                 f'background:{m.ink(t, 0.09)}"></div>') if shown else ""
        return (plate + f'<div class=a style="left:{x}px;top:{y}px;width:134px">{thumb(m, t, 134, 94, kind)}'
                f'<div style="margin-top:7px;font:500 13px/17px Inter;color:{m.ink(t, 0.9)};white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{name}</div>'
                f'<div style="font:400 11px/15px Inter;color:{m.ink(t, 0.6)}">{sub}</div></div>')
    s += head(64, "Today")
    s += tile(126, 86, "kyoto", "Kyoto trip", "31 items", shown=True) + tile(274, 86, "empty", "Untitled Board", "Empty")
    s += head(252, "Yesterday")
    s += tile(126, 274, "kitchen", "Kitchen redo", "18 items") + tile(274, 274, "reading", "Reading list", "9 items")
    s += head(440, "Older")
    s += tile(126, 462, "q3", "Q3 planning", "24 items") + tile(274, 462, "birthday", "Birthday ideas", "7 items")
    # The card shows the Board the Library opened, as a click on a tile opens it as a tab.
    cw = 1280 - 417  # the Library's card runs to the window's edge, as the capture's does
    s += m.card(417, 10, cw, 780, m.board_full(t, cw, 780, selected=False) + m.strip(t, cw / 2, 12))
    m.write(f"library-boards-{t['name']}", m.page(s, f"{base}/library-{t['name']}.png"))


def build(m, t, base, out):
    for f in (window, split, empty, first_note, tools, library):
        f(m, t, base)
