# Builds #3073's Board mocks, light and dark, as HTML for Firefox's headless screenshot, at 1x in a
# 1280 by 800 window. The window round each Board is the preview's own (tools/parity.sh on the
# spike branch, Linux under sway); the Board is drawn from app/design/tokens.toml: [palette],
# [radius], [elevation], [text], [shell] and the theme picker's swatches. Content is made up.
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BASE = sys.argv[1]  # the folder of the preview's captures
OUT = sys.argv[2]
FONT = os.path.normpath(os.environ.get("MOCK_FONTS") or f"{HERE}/../../../app/assets/fonts")

ACCENT = "#289054"  # [palette] no-theme-accent
SW = dict(sand="#f0eae5", pink="#e59eba", plum="#9e749b", red="#e1676f", orange="#ef8c63",
          yellow="#f2d46c", green="#73e49b", blue="#7eb6d5", slate="#656786")
T = {
    "light": dict(
        name="light", ground="#fbf9f4", ink="22,20,15", dots=0.10, note="#ffffff",
        strip="#f3f0e8", plate=0.06, plate_on=0.09, line=0.10,
        resting="0 0 0 0.5px rgba(0,0,0,0.18), 0 1px 1.5px rgba(0,0,0,0.20)",
        popover="0 0 0 0.5px rgba(0,0,0,0.22), 0 8px 12px rgba(0,0,0,0.14)",
        page="#ffffff", field="#ffffff",
    ),
    "dark": dict(
        name="dark", ground="#131313", ink="243,240,232", dots=0.09, note="#2a2720",
        strip="#1f1d17", plate=0.06, plate_on=0.09, line=0.10,
        resting="inset 0 0 0 0.5px rgba(255,255,255,0.08), 0 0 0 0.5px rgba(0,0,0,0.7), 0 1px 1.5px rgba(0,0,0,0.2)",
        popover="inset 0 0 0 0.5px rgba(255,255,255,0.12), 0 0 0 0.5px rgba(0,0,0,0.7), 0 8px 12px rgba(0,0,0,0.35)",
        page="#1b1b1b", field="#2a2720",
    ),
}


def icon(name, size=16, color="currentColor", sw=2.0):
    s = open(f"{HERE}/{name}.svg").read()
    s = re.sub(r"<!--.*?-->", "", s, flags=re.S)
    # Only the <svg> tag's own size and class go; a <rect>'s width is its shape.
    s = re.sub(r"<svg\b[^>]*>", lambda t: re.sub(r'\s(width|height|class)="[^"]*"', "", t.group(0)), s, count=1)
    s = s.replace("<svg", f'<svg width="{size}" height="{size}" style="color:{color};display:block;flex:none"', 1)
    s = s.replace('stroke-width="2"', f'stroke-width="{sw}"')
    return s


def ink(t, a):
    return f"rgba({t['ink']},{a})"


FACE = "".join(
    f"@font-face{{font-family:Inter;src:url('file://{FONT}/Inter-{w}.ttf');font-weight:{n}}}"
    for w, n in (("Regular", 400), ("Medium", 500), ("SemiBold", 600))
)


def page(body, base=None):
    bg = f"background:url('file://{base}') no-repeat 0 0;" if base else ""
    return (f"<!doctype html><meta charset=utf-8><style>{FACE}*{{margin:0;box-sizing:border-box}}"
            f"html,body{{width:1280px;height:800px;overflow:hidden;font-family:Inter,Noto Sans CJK JP,IPAGothic,sans-serif;"
            f"-webkit-font-smoothing:antialiased}}body{{{bg}position:relative}}"
            f".a{{position:absolute}}</style>{body}")


def ground(t, w, h):
    # Ours: the canvas's ground is a sheet's (dialog-light; the dark stand-in), with a dot every 24 pt
    # in the ink at a tenth, so a pan reads as motion even over an empty stretch.
    return (f"background-color:{t['ground']};background-image:radial-gradient(circle,{ink(t, t['dots'])} 1px,"
            f"transparent 1.3px);background-size:24px 24px;background-position:12px 12px;")


def note(t, x, y, w, title, body, fill=None, selected=False, size="M", editing=False):
    # A fill is a swatch at 30 % over the note's plate in light and 22 % in dark, so the theme
    # recolours it from the swatch's name; "none" has no plate and no shadow.
    bare = fill == "none"
    bg = "transparent" if bare else (t["note"] if not fill else f"color-mix(in srgb,{fill} {30 if t['name'] == 'light' else 22}%,{t['note']})")
    shadow = "none" if bare else t["resting"]
    heading, text = (("600 28px/34px", "400 15px/20px") if size == "L" else ("600 15px/20px", "400 13px/19px"))
    sel = ""
    if editing:
        sel = f'<div class=a style="inset:-5px;border-radius:15px;box-shadow:0 0 0 1.5px {ACCENT}"></div>'
    if selected:
        sel = (f'<div class=a style="inset:-5px;border-radius:15px;box-shadow:0 0 0 1.5px {ACCENT}"></div>'
               + "".join(f'<div class=a style="width:9px;height:9px;border-radius:50%;background:#fff;'
                         f'box-shadow:0 0 0 1.5px {ACCENT};{p}"></div>'
                         for p in ("left:-9.5px;top:-9.5px", "right:-9.5px;top:-9.5px",
                                   "left:-9.5px;bottom:-9.5px", "right:-9.5px;bottom:-9.5px")))
    return (f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;padding:12px 14px 13px;border-radius:12px;'
            f'background:{bg};box-shadow:{shadow};color:{ink(t, 0.9)}">'
            f'<div style="font:{heading} Inter,Noto Sans CJK JP,IPAGothic">{title}</div>'
            + (f'<div style="font:{text} Inter,Noto Sans CJK JP,IPAGothic;color:{ink(t, 0.72)};margin-top:4px">{body}</div>' if body else "")
            + f'{sel}</div>')


def photo(t, x, y, w, h, kind, extra=""):
    scenes = {
        # Made-up pictures, no one's photos: a sky, a hill and a pagoda's outline in the swatches.
        "temple": (f"linear-gradient(180deg,{SW['blue']} 0%,{SW['sand']} 70%)",
                   f'<svg class=a style="left:0;bottom:0" width="{w}" height="{h}" viewBox="0 0 300 200" preserveAspectRatio="none">'
                   f'<path d="M0 150 Q60 110 120 135 T240 120 T300 128 V200 H0Z" fill="{SW["slate"]}"/>'
                   f'<path d="M0 170 Q80 150 160 168 T300 160 V200 H0Z" fill="#4b4d6b"/>'
                   f'<path d="M196 128 h28 l-4 -10 h-20z M200 118 h20 l-3 -9 h-14z M203 109 h14 l-3 -8 h-8z M209 101 v-10" stroke="#3a3b55" stroke-width="2" fill="#3a3b55"/></svg>'),
        "bamboo": (f"linear-gradient(180deg,#cfe7c4 0%,{SW['green']} 100%)",
                   f'<svg class=a style="left:0;top:0" width="{w}" height="{h}" viewBox="0 0 160 200" preserveAspectRatio="none">'
                   + "".join(f'<rect x="{12 + i * 19}" y="0" width="{6 + (i % 3) * 2}" height="200" fill="#3f8a5a" opacity="{0.55 + (i % 4) * 0.1}"/>' for i in range(8))
                   + '</svg>'),
        "market": (f"linear-gradient(160deg,{SW['orange']} 0%,{SW['red']} 100%)",
                   f'<svg class=a style="left:0;top:0" width="{w}" height="{h}" viewBox="0 0 260 170" preserveAspectRatio="none">'
                   + "".join(f'<circle cx="{30 + (i % 6) * 40}" cy="{40 + (i // 6) * 50}" r="14" fill="{SW["yellow"]}" opacity="0.85"/>' for i in range(12))
                   + '</svg>'),
    }
    bg, art = scenes[kind]
    return (f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;border-radius:6px;overflow:hidden;'
            f'background:{bg};box-shadow:{t["resting"]}">{art}</div>{extra}')


def link(t, x, y, w, title, host, tint):
    return (f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:56px;border-radius:12px;background:{t["note"]};'
            f'box-shadow:{t["resting"]};display:flex;align-items:center;gap:10px;padding:0 12px">'
            f'<div style="width:28px;height:28px;display:grid;place-items:center">{icon("globe", 16, ink(t, 0.7))}</div>'
            f'<div style="min-width:0"><div style="font:500 13px/17px Inter;color:{ink(t, 0.9)};white-space:nowrap;'
            f'overflow:hidden;text-overflow:ellipsis">{title}</div>'
            f'<div style="font:400 11px/15px Inter;color:{ink(t, 0.55)}">{host}</div></div></div>')


STRIP_W = 5 * 32 + 9 + 56 + 6 * 2 + 12  # five tools, a rule, the percent, the gaps and the padding


def strip(t, cx, bottom, chosen="mouse-pointer-2", hovered=None, zoom="100%"):
    tools = ["mouse-pointer-2", "hand", "type", "pencil", "image"]
    b = ""
    for n in tools:
        on = n == chosen
        plate = ink(t, t["plate_on"]) if on else (ink(t, t["plate"]) if n == hovered else "transparent")
        b += (f'<div style="width:32px;height:32px;border-radius:10px;background:{plate};display:grid;place-items:center">'
              f'{icon(n, 16, ACCENT if on else ink(t, 0.72))}</div>')
    b += f'<div style="width:1px;height:20px;background:{ink(t, t["line"])};margin:0 4px"></div>'
    b += (f'<div style="width:56px;height:32px;border-radius:10px;display:grid;place-items:center;'
          f'font:500 13px Inter;color:{ink(t, 0.72)};font-variant-numeric:tabular-nums">{zoom}</div>')
    w = STRIP_W
    return (f'<div class=a style="left:{cx - w / 2}px;bottom:{bottom}px;height:44px;padding:6px;border-radius:16px;'
            f'background:{t["strip"]};box-shadow:{t["popover"]};display:flex;align-items:center;gap:2px">{b}</div>')


def context_bar(t, x, y, chosen=2, size="M"):
    dots = ""
    fills = ["none", t["note"], SW["yellow"], SW["pink"], SW["blue"], SW["green"]]
    for i, c in enumerate(fills):
        on = f"0 0 0 1.5px {t['strip']},0 0 0 3px {ACCENT}"
        if c == "none":
            # None: the plate's outline crossed out, so it reads apart from Plain in both themes.
            ring = on if i == chosen else "none"
            dots += (f'<div style="width:28px;height:28px;display:grid;place-items:center"><div style="width:16px;height:16px;'
                     f'border-radius:50%;box-shadow:{ring}"><svg width="16" height="16" viewBox="0 0 16 16" style="display:block">'
                     f'<circle cx="8" cy="8" r="7.25" fill="none" stroke="{ink(t, 0.45)}" stroke-width="1.2" stroke-dasharray="2.2 1.8"/>'
                     f'<path d="M3.2 12.8 L12.8 3.2" stroke="{ink(t, 0.55)}" stroke-width="1.3" stroke-linecap="round"/></svg></div></div>')
            continue
        ring = on if i == chosen else f"inset 0 0 0 0.5px {ink(t, 0.25)}"
        dots += (f'<div style="width:28px;height:28px;display:grid;place-items:center"><div style="width:16px;height:16px;'
                 f'border-radius:50%;background:{c};box-shadow:{ring}"></div></div>')
    sep = f'<div style="width:1px;height:18px;background:{ink(t, t["line"])};margin:0 4px"></div>'
    size = (f'<div style="height:28px;padding:0 8px;border-radius:8px;display:flex;align-items:center;gap:4px;'
            f'font:500 13px Inter;color:{ink(t, 0.72)}">{size}'
            f'<svg width="10" height="10" viewBox="0 0 10 10"><path d="M2 4l3 3 3-3" stroke="currentColor" fill="none" stroke-width="1.5"/></svg></div>')
    more = f'<div style="width:28px;height:28px;display:grid;place-items:center">{icon("ellipsis", 16, ink(t, 0.72))}</div>'
    return (f'<div class=a style="left:{x}px;top:{y}px;height:40px;padding:6px;border-radius:16px;background:{t["strip"]};'
            f'box-shadow:{t["popover"]};display:flex;align-items:center;gap:0">{dots}{sep}{size}{sep}{more}</div>')


PAPER, INK, OVERLAY_DARK = "#f3f0e8", "#16140f", "#1f1d17"


def tooltip(cx, cy, text, key):
    lip = f"color-mix(in srgb,{PAPER} 55%,{OVERLAY_DARK})"
    chip = (f'<span style="position:relative;display:inline-block;width:18.5px;height:18.25px;margin-left:8px">'
            f'<span style="position:absolute;left:0;top:2px;width:18.5px;height:16.25px;border-radius:6px;background:{lip}"></span>'
            f'<span style="position:absolute;left:0;top:0;width:18.5px;height:16.25px;border-radius:6px;background:{PAPER};'
            f'color:{INK};font:600 11px/16.25px Inter;text-align:center">{key}</span></span>')
    return (f'<div class=a style="left:{cx + 8}px;top:{cy + 7}px;height:27px;padding:0 10px 0 11px;display:flex;align-items:center;'
            f'border-radius:6px 13.5px 13.5px 13.5px;background:{OVERLAY_DARK};color:{PAPER};font:600 11px Inter;white-space:nowrap;'
            f'box-shadow:inset 0 0 0 0.5px rgba(255,255,255,0.12),0 8px 12px rgba(0,0,0,0.35)">{text}{chip}</div>')


def drawing(t, svg_body, w=1042, h=780):
    return f'<svg class=a style="left:0;top:0;overflow:visible" width="{w}" height="{h}">{svg_body}</svg>'


def board_full(t, w, h, selected=True):
    """A Kyoto trip's Board at 100 %, its first screen."""
    s = ""
    s += note(t, 42, 32, 400, "Kyoto in April", "Five days, mostly on foot.", fill="none", size="L")
    s += note(t, 56, 136, 236, "Day 1 · Higashiyama",
              "Kiyomizu-dera before 8, then down Ninenzaka and Sannenzaka. Lunch at the bottom of the hill.")
    s += note(t, 56, 300, 236, "清水寺 → 二年坂 → 三年坂", "朝8時前に着くこと。帰りは祇園まで歩く。")
    s += note(t, 316, 136, 200, "Bring", "Rail pass · coin purse · umbrella · good shoes", fill=SW["yellow"], selected=selected)
    s += note(t, 316, 276, 200, "Day 2 · Arashiyama", "Bamboo grove at 7. Train back by 11, nap.")
    s += photo(t, 556, 56, 300, 200, "temple")
    s += photo(t, 880, 112, 180, 230, "bamboo")
    s += link(t, 556, 290, 300, "The Philosopher's Path, a walking guide", "example.org", None)
    s += link(t, 556, 358, 300, "Nishiki Market: hours and stalls", "example.com", None)
    s += photo(t, 316, 430, 260, 170, "market")
    s += note(t, 600, 444, 220, "Ask", "Is the garden open on the 14th? If not, swap days 3 and 4.", fill=SW["blue"])
    red = SW["red"]
    s += drawing(t, f'<path d="M824 506 C 882 500, 922 440, 938 364" stroke="{red}" stroke-width="3" fill="none" stroke-linecap="round"/>'
                    f'<path d="M926 374 L939 361 L948 377" stroke="{red}" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"/>'
                    f'<path d="M590 610 C 640 586, 760 590, 812 612" stroke="{SW["plum"]}" stroke-width="3" fill="none" stroke-linecap="round"/>')
    if selected:
        s += context_bar(t, 316, 82, chosen=2)
    return f'<div class=a style="left:0;top:0;width:{w}px;height:{h}px;{ground(t, w, h)}overflow:hidden">{s}</div>'


def card(x, y, w, h, inner):
    return f'<div class=a style="left:{x}px;top:{y}px;width:{w}px;height:{h}px;border-radius:6px;overflow:hidden">{inner}</div>'


def write(name, html):
    path = f"{OUT}/{name}.html"
    open(path, "w").write(html)
    return path


if __name__ == "__main__":
    import mocks  # the screens, each a function of the theme
    for n in ("light", "dark"):
        mocks.build(sys.modules[__name__], T[n], BASE, OUT)
