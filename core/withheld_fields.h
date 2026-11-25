#ifndef BROWSER_CORE_WITHHELD_FIELDS_H_
#define BROWSER_CORE_WITHHELD_FIELDS_H_

#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/values.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_f.h"

class SkBitmap;

namespace browser_core {

// What the core knows of a field that holds something, to tell whether an agent may see it. The
// outline and an agent's pictures of the page follow this one rule.
struct FieldFacts {
  // What it holds now, as the accessibility tree reads it.
  std::u16string value;
  // A password field, now or ever in its document (the page may have made one a text field), or
  // a password's, a card's or a one-time code's by its autocomplete.
  bool secret = false;
  // Chromium's autofill filled it, or shows a fill in it.
  bool autofilled = false;
  // Disabled or read-only, which neither the user nor autofill can fill.
  bool fixed = false;
  // A select, by its role: the only field whose markup counts. A field is the page's own only
  // while the page's markup is what filled it, which the core can't tell of an input or a textarea.
  bool select = false;
  // A select's value in the page's markup: its option marked selected, else its first.
  std::optional<std::u16string> markup;
  // What the agent's own typing or choosing left in it, as the field read after it settled.
  std::optional<std::u16string> typed;
};

// What an agent sees of a field, in its outline and its pictures alike, so the two can't drift.
enum class Seen {
  // It holds nothing.
  kEmpty,
  // What it holds: the agent's own typing, unchanged, or the page's own value, a read-only or
  // disabled field's or a select's option its markup marks selected.
  kValue,
  // That it holds something, which the core can't name as either: the user's typing, what
  // autofill or a password manager put there, and a secret field's contents always, even the
  // agent's own typing.
  kFilled,
  // That it holds something else than the agent typed there.
  kChanged,
};
Seen SeenOf(const FieldFacts& field);

// Whether reading the field's attributes and markup could let SeenOf show what it holds: what the
// agent typed, a read-only or disabled field's, or a select's. Any other filled field reads filled
// whatever they say, so the page isn't asked about it.
bool CouldShow(const FieldFacts& field);

// Whether what the field holds is withheld from an agent: anything SeenOf doesn't show.
bool Withheld(const FieldFacts& field);

// Which fields a picture hatches: those whose contents an agent doesn't see (Withheld), as its
// screenshot is, or every field holding a value, as Point's is, whose text has none (#2352).
enum class Hatched { kWithheld, kEveryValue };
bool Hatches(const FieldFacts& field, Hatched hatched);

// Whether an autocomplete attribute's tokens name a password's, a card's or a one-time code's
// field: a token ending in "password", as "current-password" and "new-password", one starting
// "cc-", or "one-time-code".
bool SecretAutocomplete(std::string_view autocomplete);

// What a field is by its type and autocomplete, as its markup has them: a password field, and
// secret, which a password field is and one whose autocomplete SecretAutocomplete names.
struct FieldKind {
  bool password = false;
  bool secret = false;
};
FieldKind KindOf(const std::optional<std::string>& type,
                 const std::optional<std::string>& autocomplete);

// What the app, and no agent, is told of a text field on its node of the outline: "secret" and
// "autofilled", each there as true or false, so the app can tell no from not said. They decide
// what the app keeps of what was typed there, and nothing an outline or a picture shows.
void TellKinds(base::DictValue& node, bool secret, bool autofilled);

// |box|, in a frame's CSS pixels, in its parent's: cut to the frame's |frame| box there, then moved
// to it, so a field never reaches past the frame it's seen through.
gfx::RectF IntoParent(const gfx::RectF& box, const gfx::RectF& frame);

// The pixels of a picture of |picture|'s size, of the view's |source| area, that |box| covers, in
// the main frame's CSS pixels at |zoom| view points each: every pixel it touches, inside the
// picture.
gfx::Rect InPicture(const gfx::RectF& box,
                    float zoom,
                    const gfx::Rect& source,
                    const gfx::Size& picture);

// Paints each of |boxes| over with a grey hatch, as a filled field whose contents are withheld
// looks in an agent's picture: filled, with nothing of the text, not even its length, and the
// same whatever the field holds. |scale| is the picture's pixels per CSS pixel.
void Hatch(SkBitmap& picture, const std::vector<gfx::Rect>& boxes, float scale);

}  // namespace browser_core

#endif  // BROWSER_CORE_WITHHELD_FIELDS_H_
