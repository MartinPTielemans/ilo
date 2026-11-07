#include "browser_core/error_page.h"

#include <string>

#include "base/json/json_writer.h"
#include "base/strings/escape.h"
#include "base/strings/string_util.h"

namespace browser_core {

namespace {

// The page's look is ours (docs/ux-spec.md › Pages that break): the words in the middle of the
// pane, on its colour, which the tab gives the page (Tab::SetPageColor), at the sizes and colours of
// the app's cards, in white, or in Light the palette's ink, as the pane is light then. One script
// draws it from Chromium's strings, at first and again when Chromium's DNS probe sends new ones.
// Offline, it offers ilo's game (docs/ux-spec.md › Offline game).
constexpr char kPage[] = R"html(<!doctype html>
<html dir="$dir" lang="$lang">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>$title</title>
<style>
/* The words' and plates' colour, at the alphas below: white, and in Light, as websites are told
   it, the palette's ink (app/design/tokens.toml), as the pane is then (Tab::SetPageColor). */
:root { --ink: 255 255 255; }
@media (prefers-color-scheme: light) {
  :root { --ink: 22 20 15; }
}
html, body { height: 100%; margin: 0; background: transparent; overflow: hidden; }
body {
  display: flex; align-items: center; justify-content: center;
  font: 14px system-ui; color: rgb(var(--ink)); cursor: default; -webkit-user-select: none;
}
main {
  display: flex; flex-direction: column; align-items: center;
  max-width: 400px; padding: 0 24px; text-align: center;
  transition: opacity 200ms 600ms;
}
h1 { margin: 0; font-size: 14px; font-weight: 500; }
#note { margin-top: 4px; font-size: 13px; color: rgb(var(--ink) / 0.6); }
button {
  margin-top: 12px; height: 28px; padding: 0 12px; border: 0; border-radius: 6px;
  background: rgb(var(--ink) / 0.1); color: rgb(var(--ink)); font: 500 13px system-ui;
}
button:hover { background: rgb(var(--ink) / 0.12); }
#play, #code { margin-top: 12px; font-size: 11px; color: rgb(var(--ink) / 0.32); }
#field {
  position: fixed; inset: 0; width: 100%; height: 100%; pointer-events: none;
  opacity: 0; transition: opacity 200ms 600ms;
}
#corner {
  position: fixed; top: 14px; inset-inline-end: 16px; display: flex; gap: 12px;
  font-size: 11px; font-variant-numeric: tabular-nums; color: rgb(var(--ink) / 0.32);
}
#score { color: rgb(var(--ink) / 0.6); }
#online { color: #0E9F55; }
.running main { opacity: 0; transition: none; }
.running #field { opacity: 1; transition: none; }
[hidden] { display: none; }
</style>
</head>
<body>
<main>
<h1 id="heading"></h1>
<div id="note" hidden></div>
<div id="play" hidden>Press Space to play</div>
<button id="reload" hidden>Reload</button>
<div id="code"></div>
</main>
<canvas id="field" aria-hidden="true"></canvas>
<div id="corner" aria-hidden="true">
<span id="online" hidden>Back online</span><span id="score"></span><span id="best"></span>
</div>
<script>
// The offline game: Space hops ilo's green dot through the gaps in columns of tab rows. It draws
// only while a run is going, and sets no timer, so an idle page costs nothing.
const game = (() => {
  // In CSS pixels and seconds. The rows are the sidebar's: 36 tall on a 41 pitch, radius 12.
  const PITCH = 41, ROW = 36, ROUND = 12, COLUMN = 64, SPACING = 210, GAP = 3;
  const DOT = 6, GRAVITY = 1300, HOP = 360, FALL = 640, LEAD = 2.4;
  const SPEED = 150, TOP_SPEED = 300, EASE = 45, STEP = 1 / 120, REST = 0.4;
  // The rows' fills are the ink at these (--ink in the style).
  const GREEN = '#0E9F55', PLATE = 0.12, HIT = 0.2;
  const field = document.getElementById('field');
  const context = field.getContext('2d');
  const play = document.getElementById('play');
  let offered = false, running = false, played = false, back = false;
  let frame = 0, last = 0, lag = 0, ended = -Infinity;
  let width = 0, height = 0, scale = 1, rows = 0, top = 0, bottom = 0, x = 0;
  let columns = [], y = 0, velocity = 0, time = 0, score = 0, shown = -1, best = 0, hit = null;
  let plate = '', struck = '';

  // The rows' fills in the ink, again as the appearance changes.
  function ink() {
    const value = getComputedStyle(document.documentElement).getPropertyValue('--ink').trim();
    plate = 'rgb(' + value + ' / ' + PLATE + ')';
    struck = 'rgb(' + value + ' / ' + HIT + ')';
  }

  // Fits the field to the pane, at a resize too: as many rows as fit, 5 to 11, centred, and the
  // dot a little left of the middle.
  function fit() {
    width = innerWidth;
    height = innerHeight;
    scale = devicePixelRatio || 1;
    field.width = Math.round(width * scale);
    field.height = Math.round(height * scale);
    rows = Math.max(5, Math.min(11, Math.floor((height - 96) / PITCH)));
    top = Math.round((height - rows * PITCH + PITCH - ROW) / 2);
    bottom = top + rows * PITCH - (PITCH - ROW);
    x = Math.round(Math.max(64, Math.min(200, width * 0.28)));
    y = Math.max(top + DOT, Math.min(bottom - DOT, y));
    for (const column of columns) {
      column.gap = Math.min(column.gap, rows - 1 - GAP);
    }
  }

  // A gap's first row: never the band's first or last row, and at most three rows from the gap
  // before, so each can be reached from the last.
  function gapAfter(previous) {
    const low = Math.max(1, previous - 3);
    const high = Math.min(rows - 1 - GAP, previous + 3);
    return low + Math.floor(Math.random() * (high - low + 1));
  }

  // Columns come in from the right, SPACING apart. The first reaches the dot LEAD after the start,
  // with its gap in the middle, where the dot starts.
  function spawn() {
    while (columns.length && columns[0].x + COLUMN < 0) {
      columns.shift();
    }
    let previous = columns[columns.length - 1];
    while (!previous || previous.x < width) {
      const column = previous
          ? {x: previous.x + SPACING, gap: gapAfter(previous.gap), passed: false}
          : {x: x + DOT + SPEED * LEAD, gap: (rows - GAP) >> 1, passed: false};
      columns.push(column);
      previous = column;
    }
  }

  // Whether the dot touches a row, as a circle on a rounded rectangle, 1.5 short so a graze passes.
  function touches(left, rowTop) {
    const dx = Math.max(left + ROUND - x, 0, x - (left + COLUMN - ROUND));
    const dy = Math.max(rowTop + ROUND - y, 0, y - (rowTop + ROW - ROUND));
    return Math.hypot(dx, dy) < ROUND + DOT - 1.5;
  }

  function step(dt) {
    time += dt;
    const speed = SPEED + (TOP_SPEED - SPEED) * (1 - Math.exp(-time / EASE));
    velocity = Math.min(velocity + GRAVITY * dt, FALL);
    y += velocity * dt;
    // The band's edges hold the dot: only a row ends a run.
    if (y > bottom - DOT) {
      y = bottom - DOT;
      velocity = 0;
    } else if (y < top + DOT) {
      y = top + DOT;
      velocity = Math.max(velocity, 0);
    }
    for (const column of columns) {
      column.x -= speed * dt;
      if (!column.passed && column.x + COLUMN < x - DOT) {
        column.passed = true;
        score++;
      }
      if (column.x > x + DOT || column.x + COLUMN < x - DOT) {
        continue;
      }
      for (let row = 0; row < rows; row++) {
        if ((row < column.gap || row >= column.gap + GAP) && touches(column.x, top + row * PITCH)) {
          hit = {column, row};
          return;
        }
      }
    }
    spawn();
  }

  function draw() {
    context.setTransform(scale, 0, 0, scale, 0, 0);
    context.clearRect(0, 0, width, height);
    for (const column of columns) {
      // On whole device pixels, so a moving column's edges don't shimmer.
      const left = Math.round(column.x * scale) / scale;
      if (left > width || left + COLUMN < 0) {
        continue;
      }
      for (let row = 0; row < rows; row++) {
        if (row >= column.gap && row < column.gap + GAP) {
          continue;
        }
        // The row the dot hit takes the selected row's fill.
        context.fillStyle = hit && hit.column == column && hit.row == row ? struck : plate;
        context.beginPath();
        context.roundRect(left, top + row * PITCH, COLUMN, ROW, ROUND);
        context.fill();
      }
    }
    context.fillStyle = GREEN;
    context.beginPath();
    context.arc(x, y, DOT, 0, 2 * Math.PI);
    context.fill();
  }

  function tick(now) {
    frame = 0;
    // In fixed steps, so the run plays the same at any refresh rate, and at most 0.1 s a frame.
    lag = Math.min(lag + Math.max(now - last, 0) / 1000, 0.1);
    last = now;
    while (lag >= STEP && !hit) {
      step(STEP);
      lag -= STEP;
    }
    draw();
    if (score != shown) {
      corner();
    }
    if (hit) {
      end();
    } else {
      frame = requestAnimationFrame(tick);
    }
  }

  function corner() {
    shown = score;
    document.getElementById('score').textContent = played ? score : '';
    document.getElementById('best').textContent = best ? 'Best ' + best : '';
    document.getElementById('online').hidden = !back || !played;
  }

  // Chromium's auto-reload waits while a run is going, and loads the page once it ends.
  function tell(going) {
    if (window.errorPageController && errorPageController.setGameRunning) {
      errorPageController.setGameRunning(going);
    }
  }

  function start() {
    // Keys mashed as a run ends don't start the next.
    if ((performance.now() - ended) / 1000 < REST) {
      return;
    }
    columns = [];
    hit = null;
    score = 0;
    time = 0;
    ink();
    fit();
    y = (top + bottom) / 2;
    velocity = -HOP;
    spawn();
    running = played = true;
    document.body.classList.add('running');
    tell(true);
    corner();
    last = performance.now();
    lag = 0;
    frame = requestAnimationFrame(tick);
  }

  function end() {
    running = false;
    ended = performance.now();
    cancelAnimationFrame(frame);
    frame = 0;
    best = Math.max(best, score);
    play.textContent = 'Press Space to play again';
    document.body.classList.remove('running');
    tell(false);
    corner();
  }

  function hop() {
    velocity = -HOP;
  }

  addEventListener('keydown', event => {
    if (!offered || event.metaKey || event.ctrlKey || event.altKey ||
        (event.key != ' ' && event.key != 'ArrowUp')) {
      return;
    }
    event.preventDefault();
    if (!event.repeat) {
      running ? hop() : start();
    }
  });
  // A click hops, but doesn't start a run: the line says Space, and a click may only be focusing.
  addEventListener('pointerdown', event => {
    if (running && event.button == 0) {
      hop();
    }
  });
  // A run ends once the keys go elsewhere: the command bar or another overlay, window or app, or
  // another tab, which hides this one. Not on visibilitychange: a listener for it, as for pagehide,
  // keeps the page out of the fast shutdown at a quit, and the quit then never finished.
  addEventListener('blur', () => {
    if (running) {
      end();
    }
  });
  addEventListener('resize', () => {
    if (played) {
      fit();
      draw();
    }
  });
  matchMedia('(prefers-color-scheme: light)').addEventListener('change', () => {
    if (played) {
      ink();
      draw();
    }
  });
  addEventListener('online', () => {
    back = true;
    corner();
  });
  addEventListener('offline', () => {
    back = false;
    corner();
  });

  return {
    // Only an offline error offers it: the others have something to do instead.
    offer(offline) {
      offered = offline;
      play.hidden = !offline;
      if (!offline && running) {
        end();
      }
    },
  };
})();

let reloadUrl = '';
// Chromium's heading is HTML, with the host in a tag: its text, which an inert template can't run.
function text(html) {
  const template = document.createElement('template');
  template.innerHTML = html || '';
  return template.content.textContent;
}
function show(strings) {
  const offline = !!strings.isOfflineError;
  document.getElementById('heading').textContent =
      offline ? 'You’re offline' : text(strings.heading && strings.heading.msg);
  const note = document.getElementById('note');
  // Chromium's auto-reload loads the page once the network is back, so there's nothing to press.
  note.textContent = 'Reloads when you’re back';
  note.hidden = !offline;
  reloadUrl = strings.reloadButton ? strings.reloadButton.reloadUrl : '';
  document.getElementById('reload').hidden = offline || !reloadUrl;
  document.getElementById('code').textContent = strings.errorCode || '';
  game.offer(offline);
}
document.getElementById('reload').addEventListener('click', () => {
  // Chromium's controller reloads as its own page's button does, else the page goes to the URL.
  if (window.errorPageController) {
    errorPageController.reloadButtonClick();
  } else if (reloadUrl) {
    location.href = reloadUrl;
  }
});
window.updateForDnsProbe = show;
show($strings);
</script>
</body>
</html>
)html";

std::string Text(const base::DictValue& strings, const char* key) {
  const std::string* value = strings.FindString(key);
  return value ? *value : std::string();
}

}  // namespace

std::string ErrorPageHtml(const base::DictValue& strings) {
  std::string json = base::WriteJson(strings).value_or("{}");
  // In a script element, where "</script>" in a host or URL would end it.
  base::ReplaceSubstringsAfterOffset(&json, 0, "<", "\\u003c");
  std::string dir = Text(strings, "textdirection");
  if (dir != "rtl") {
    dir = "ltr";
  }
  std::string html = kPage;
  // Last first, so each placeholder's first match is its own, not one in a value put in before it.
  base::ReplaceFirstSubstringAfterOffset(&html, 0, "$strings", json);
  base::ReplaceFirstSubstringAfterOffset(&html, 0, "$title",
                                         base::EscapeForHTML(Text(strings, "title")));
  base::ReplaceFirstSubstringAfterOffset(&html, 0, "$lang",
                                         base::EscapeForHTML(Text(strings, "language")));
  base::ReplaceFirstSubstringAfterOffset(&html, 0, "$dir", dir);
  return html;
}

}  // namespace browser_core
