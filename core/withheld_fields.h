#ifndef BROWSER_CORE_WITHHELD_FIELDS_H_
#define BROWSER_CORE_WITHHELD_FIELDS_H_

#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_f.h"

class SkBitmap;

namespace browser_core {

// What the core knows of a field that holds something, to tell whether an agent may see it. The
// outline and an agent's pictures of the page follow this one rule.
struct FieldFacts {
  // What it holds now, as the accessibility tree reads it.
  std::u16string value;
  // A password field, now or ever (the page may have made one a text field), or a card's or a
  // one-time code's, by its autocomplete.
  bool secret = false;
  // Chromium's autofill filled it, or shows a fill in it.
  bool autofilled = false;
  // Disabled or read-only, which neither the user nor autofill can fill.
  bool fixed = false;
  // Its value in the page's markup: an input's value attribute, a textarea's text, or a select's
  // option marked selected.
  std::optional<std::u16string> markup;
  // What the agent's own typing or choosing left in it, as the field read after it settled.
  std::optional<std::u16string> typed;
};

// Whether what the field holds is withheld from an agent: anything the core can't name as the
// agent's own unchanged typing or the page's own value, so the user's typing and what autofill or a
// password manager put there, and a secret field always. An empty one has nothing to withhold.
bool Withheld(const FieldFacts& field);

// Whether an autocomplete attribute's tokens name a card's or a one-time code's field.
bool SecretAutocomplete(std::string_view autocomplete);

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

// Paints each of |boxes| over with a hatch in its own background, as a filled field whose contents
// are withheld looks in an agent's picture: filled, with nothing of the text, not even its length.
// |scale| is the picture's pixels per CSS pixel.
void Hatch(SkBitmap& picture, const std::vector<gfx::Rect>& boxes, float scale);

}  // namespace browser_core

#endif  // BROWSER_CORE_WITHHELD_FIELDS_H_
