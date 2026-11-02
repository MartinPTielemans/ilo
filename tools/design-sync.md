# Design sync

ilo's look lives in `app/design/tokens.toml`, which the code reads (as `crate::tokens`) and the pen.dev design file copies. Code is the source of truth for values, and pen is where new designs start. This is the procedure an agent runs to carry values both ways.

- **The design file:** `~/.cache/browser-explore/ilo-chrome.pen`, kept outside the repo by the owner's decision (#325). Boards from it are linked from issues and PRs.
- **The tools:** pen has no command line, and its file is reachable only through the running pen.dev app's MCP server, `pencil`. Read `pencil`'s `execute.md` skill once; `GetVariables`, `SetVariables`, `Get` and `Print` run inside its `execute` tool. `tools/design-tokens.py` converts between the two formats, using only Python's standard library.
- **Names:** a token's pen variable is its table and key joined with `-`, so `[shell.library] column` is `shell-library-column`, and an array's items add `-0`, `-1` and so on. Six or eight hex digits make a colour (`#rrggbb` or `#rrggbbaa`); every other value is a number. A table named `motion`, such as `[shell.task.motion]`, holds timings, which the design file has none of, so the script leaves it out both ways.
- **When:** the lane that merges runs it after any merge that touches `app/design/tokens.toml` or a file named in a component's `context`.

## Code to pen

1. **Make the payload:** `tools/design-tokens.py to-pen > ~/.cache/browser-app-work/design-sync/variables.json`.
2. **Set the variables:** in one `execute` on the design file, call `SetVariables(<the payload>)`. Leave `replace` false: the file also holds design-only variables, such as the `ilo-*` proposals, which a merge keeps. Every property bound to a variable updates at once.
3. **List the components to check by hand.** Each reusable component names its code in `context`, such as `"app/src/shell/rows.rs › Tab Row"`.
   - List them with `Get(n => n.reusable && n.context && Print(n.id, "|", n.name, "|", n.context))`.
   - The last sync's commit is the pen variable `sync-commit`. For each file named, `git log --oneline <sync-commit>..origin/main -- <file>` says whether it changed. A changed file's components get compared with the code and a capture, and fixed in pen.
   - Report every `context` whose file no longer exists, so its component gets its new path or is retired.
4. **Captures:** place the latest check captures (`app/build/check/*.png`) under their matching screens as hidden reference layers, replacing the last ones. Drift shows when a layer is switched on.
5. **Record the sync:** `SetVariables({"sync-commit": {type: "string", value: "<the origin/main SHA>"}})`.

## Pen to code

1. **Read pen's variables:** `Print(GetVariables())` in `execute`, and save the printed JSON to `~/.cache/browser-app-work/design-sync/pen.json`.
2. **Write them into the token file:** `tools/design-tokens.py from-pen ~/.cache/browser-app-work/design-sync/pen.json`. It rewrites changed values in place, keeping each value's form and the file's comments, and lists:
   - **Changed in tokens.toml:** the values pen changed. `git diff app/design/tokens.toml` is the PR's token change.
   - **Not written:** aliases (`$other`), themed values, and an alpha on a colour the code draws with `rgb`. Those need a code change first: the token file holds one plain value per token.
   - **Only in pen:** variables with no token, such as a new design's. A new token goes into its surface's table by hand, together with the code that draws with it.
   - **Only in code:** tokens pen doesn't have yet; the next code-to-pen adds them.
3. **Open the PR:** it links the board and carries before and after captures, as every UI change does. `cargo test --release --lib` has to pass.
4. **Structural changes stay a normal lane.** A new component or a changed layout is written by hand, with the board as its spec.

## The first sync

The design file predates `tokens.toml`. Its variables carry hand-made names (`plate-radius`, `bar-fill`, `row-selected`) where the tokens are `shell-plate-radius`, `command-bar-fill` and `shell-selected`. The first sync renames them once.

1. **Map the names:** match each old variable to its token by its components' `context` and its value. Keep the map in `~/.cache/browser-app-work/design-sync/`.
2. **Add the tokens:** run "Code to pen".
3. **Move the references:** with a `Get` visitor, rebind every `$old` property to `$new`. Then set the variables again with `replace: true` and the full set minus the old names, which drops them.
4. **Check the boards:** take a screenshot of a few boards, before and after, to confirm nothing moved.
