#!/usr/bin/env python3
"""Turns app/design/tokens.toml into pen.dev variables and back (tools/design-sync.md).

  design-tokens.py to-pen > variables.json      the payload for pen's SetVariables
  design-tokens.py from-pen variables.json      writes pen's values into tokens.toml, and says what differs

A token's pen name is its table and key joined with "-" ([shell.library] column is
shell-library-column), and an array's items add -0, -1 and so on. Hex of six or eight digits is a
colour, #rrggbb or #rrggbbaa; anything else is a number. It reads the same small TOML build.rs does.
A table named motion, such as [shell.task.motion], holds timings, which the design file has none of,
so it's left out both ways.
"""
import json, os, re, sys

TOKENS = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'app', 'design', 'tokens.toml')


def parse(lines):
    """Yields (pen name, line index, item index or None, value text) for every value in the file."""
    table = None
    for n, line in enumerate(lines):
        text = line.split('#', 1)[0].strip()
        if text.startswith('['):
            table = text[1:-1].replace('.', '-')
        elif table and (table == 'motion' or table.endswith('-motion')):
            continue
        elif '=' in text:
            key, value = (part.strip() for part in text.split('=', 1))
            if value.startswith('['):
                for i, item in enumerate(v.strip() for v in value[1:-1].split(',') if v.strip()):
                    yield f'{table}-{key}-{i}', n, i, item
            else:
                yield f'{table}-{key}', n, None, value


def digits(value):
    return value[2:].replace('_', '') if value.startswith('0x') else None


def to_pen(value):
    hex = digits(value)
    if hex is not None and len(hex) in (6, 8):
        return {'type': 'color', 'value': '#' + hex.lower()}
    if hex is not None:
        return {'type': 'number', 'value': int(hex, 16)}
    number = float(value.replace('_', ''))
    return {'type': 'number', 'value': int(number) if re.fullmatch(r'-?[0-9_]+', value) else number}


def from_pen(old, pen):
    """The TOML text for pen's value, in the old value's form, or None if pen's can't be written so."""
    hex = digits(old)
    if hex is not None and len(hex) in (6, 8):
        if not isinstance(pen, str) or not re.fullmatch(r'#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})', pen):
            return None
        new = pen[1:].lower()
        if len(hex) == 6 and len(new) == 8:
            if new[6:] != 'ff':
                return None  # rgb in the code: an alpha needs rgba there first
            new = new[:6]
        elif len(hex) == 8 and len(new) == 6:
            new += 'ff'
        if new == hex.lower():
            return old
        # Keep the old value's underscores where they were.
        out, i = '0x', 0
        for c in old[2:]:
            out += '_' if c == '_' else new[i]
            i += c != '_'
        return out
    if not isinstance(pen, (int, float)) or isinstance(pen, bool):
        return None
    if to_pen(old)['value'] == pen:
        return old
    if hex is not None or re.fullmatch(r'-?[0-9_]+', old):
        return str(int(pen)) if float(pen).is_integer() and pen >= 0 else None
    return repr(float(pen))


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ('to-pen', 'from-pen'):
        sys.exit(__doc__)
    lines = open(TOKENS).read().split('\n')
    tokens = list(parse(lines))
    if sys.argv[1] == 'to-pen':
        json.dump({name: to_pen(value) for name, _, _, value in tokens}, sys.stdout, indent=0)
        print()
        return
    pen = json.load(open(sys.argv[2]))
    pen = pen.get('variables', pen)
    changed, skipped = [], []
    for name, n, item, value in tokens:
        if name not in pen:
            continue
        new = from_pen(value, pen[name].get('value'))
        if new is None:
            why = 'an alias, a themed value, or an alpha on an rgb colour'
            skipped.append(f'{name}: {pen[name].get("value")!r} ({why})')
        elif new != value:
            changed.append(f'{name}: {value} -> {new}')
            key, rest = lines[n].split('=', 1)
            if item is None:
                lines[n] = f'{key}= {rest.strip().replace(value, new, 1)}'
            else:
                items = [v.strip() for v in rest.strip()[1:-1].split(',') if v.strip()]
                items[item] = new
                lines[n] = f'{key}= [{", ".join(items)}]'
    open(TOKENS, 'w').write('\n'.join(lines))
    names = {name for name, _, _, _ in tokens}
    for title, rows in (('Changed in tokens.toml', changed), ('Not written', skipped),
                        ('Only in pen', sorted(set(pen) - names)), ('Only in code', sorted(names - set(pen)))):
        print(f'{title} ({len(rows)}):', *rows, sep='\n  ')


main()
