#include "browser_core/withheld_fields.h"

#include <vector>

#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "third_party/skia/include/core/SkColor.h"

namespace browser_core {
namespace {

FieldFacts Holding(std::u16string value) {
  FieldFacts field;
  field.value = std::move(value);
  return field;
}

// One list, as the outline and the agent's pictures follow one rule: which fields' contents an
// agent sees.
TEST(WithheldFieldsTest, WhatTheAgentSees) {
  // Its own typing, while the field still holds it.
  FieldFacts typed = Holding(u"Ada");
  typed.typed = u"Ada";
  EXPECT_FALSE(Withheld(typed));

  // Its typing, changed since by the user or anything else.
  FieldFacts edited = Holding(u"Ada Lovelace");
  edited.typed = u"Ada";
  EXPECT_TRUE(Withheld(edited));

  // The user's typing, which the core can't tell from a password manager's fill.
  EXPECT_TRUE(Withheld(Holding(u"ada@example.com")));

  // What autofill filled, even the agent's own value or the page's default.
  FieldFacts autofilled = Holding(u"Ada");
  autofilled.autofilled = true;
  autofilled.typed = u"Ada";
  autofilled.markup = u"Ada";
  EXPECT_TRUE(Withheld(autofilled));

  // The page's own value, as its markup has it.
  FieldFacts page_default = Holding(u"Netherlands");
  page_default.markup = u"Netherlands";
  EXPECT_FALSE(Withheld(page_default));

  // A read-only or disabled field, which nobody but the page fills.
  FieldFacts fixed = Holding(u"Order 1234");
  fixed.fixed = true;
  EXPECT_FALSE(Withheld(fixed));

  // A password field the page made a text field, as a "show password" button does, and a card's
  // or one-time code's field: always, even holding the agent's typing or the page's default.
  FieldFacts shown_password = Holding(u"hunter2");
  shown_password.secret = true;
  shown_password.typed = u"hunter2";
  shown_password.markup = u"hunter2";
  shown_password.fixed = true;
  EXPECT_TRUE(Withheld(shown_password));
  EXPECT_TRUE(SecretAutocomplete("cc-number"));
  EXPECT_TRUE(SecretAutocomplete("section-pay billing CC-CSC"));
  EXPECT_TRUE(SecretAutocomplete("one-time-code"));
  EXPECT_FALSE(SecretAutocomplete("email"));
  EXPECT_FALSE(SecretAutocomplete("off"));

  // An empty field has nothing to withhold.
  EXPECT_FALSE(Withheld(Holding(u"")));
}

// Point's picture hatches every value, as its text and the note's panel send none: the page's
// default, a read-only field's and the agent's own typing too, which a screenshot shows.
TEST(WithheldFieldsTest, PointsPictureHatchesEveryValue) {
  FieldFacts page_default = Holding(u"Netherlands");
  page_default.markup = u"Netherlands";
  FieldFacts fixed = Holding(u"Order 1234");
  fixed.fixed = true;
  FieldFacts typed = Holding(u"Ada");
  typed.typed = u"Ada";
  for (const FieldFacts& field : {page_default, fixed, typed}) {
    EXPECT_FALSE(Hatches(field, Hatched::kWithheld));
    EXPECT_TRUE(Hatches(field, Hatched::kEveryValue));
  }
  EXPECT_TRUE(Hatches(Holding(u"ada@example.com"), Hatched::kWithheld));
  EXPECT_FALSE(Hatches(Holding(u""), Hatched::kEveryValue));
}

// A field in a frame, as one in a cross-origin frame is, lands where the frame shows it, and never
// past the frame's box.
TEST(WithheldFieldsTest, AFieldInAFrameIsCutToTheFrame) {
  const gfx::RectF frame(100, 200, 300, 150);
  EXPECT_EQ(IntoParent(gfx::RectF(10, 20, 50, 30), frame), gfx::RectF(110, 220, 50, 30));
  // Half past the frame's bottom edge.
  EXPECT_EQ(IntoParent(gfx::RectF(10, 135, 50, 30), frame), gfx::RectF(110, 335, 50, 15));
  // Scrolled out of the frame altogether.
  EXPECT_TRUE(IntoParent(gfx::RectF(10, -60, 50, 30), frame).IsEmpty());
}

// A field scrolled half out of view covers the half in the picture; at a zoom and on a 2x screen,
// every pixel its box touches; and in a picture of part of the view, as a capture of an area is,
// where it is in that part.
TEST(WithheldFieldsTest, ItsBoxInThePicture) {
  const gfx::Rect view(800, 600);
  const gfx::Size picture(800, 600);
  EXPECT_EQ(InPicture(gfx::RectF(10, 580, 100, 40), 1, view, picture),
            gfx::Rect(10, 580, 100, 20));
  EXPECT_EQ(InPicture(gfx::RectF(10, -20, 100, 40), 1, view, picture), gfx::Rect(10, 0, 100, 20));
  // At 2x: twice the pixels.
  EXPECT_EQ(InPicture(gfx::RectF(10, 20, 100, 40), 1, view, gfx::Size(1600, 1200)),
            gfx::Rect(20, 40, 200, 80));
  // At 125% zoom on a 2x screen, 2.5 pixels a CSS pixel, with fractions rounded outwards.
  EXPECT_EQ(InPicture(gfx::RectF(10.5f, 20, 100, 40.2f), 1.25f, view, gfx::Size(1600, 1200)),
            gfx::Rect(26, 50, 251, 101));
  // An area of the view at 2x: the field from the area's corner, and only what's in it.
  const gfx::Rect area(100, 200, 200, 100);
  EXPECT_EQ(InPicture(gfx::RectF(110, 220, 50, 30), 1, area, gfx::Size(400, 200)),
            gfx::Rect(20, 40, 100, 60));
  EXPECT_EQ(InPicture(gfx::RectF(250, 280, 100, 40), 1, area, gfx::Size(400, 200)),
            gfx::Rect(300, 160, 100, 40));
  EXPECT_TRUE(InPicture(gfx::RectF(10, 20, 50, 30), 1, area, gfx::Size(400, 200)).IsEmpty());
}

// The hatch leaves nothing of the text, keeps the field's background and paints nothing outside.
TEST(WithheldFieldsTest, TheHatchHidesTheText) {
  for (const int scale : {1, 2}) {
    SkBitmap picture;
    picture.allocN32Pixels(200 * scale, 100 * scale);
    picture.eraseColor(SK_ColorWHITE);
    const gfx::Rect box(20 * scale, 20 * scale, 120 * scale, 30 * scale);
    // The field's text, in red, away from its corners.
    picture.erase(SK_ColorRED, SkIRect::MakeXYWH(box.x() + 6 * scale, box.y() + 6 * scale,
                                                 60 * scale, box.height() - 12 * scale));
    Hatch(picture, {box}, static_cast<float>(scale));
    int stripes = 0;
    for (int y = 0; y < picture.height(); ++y) {
      for (int x = 0; x < picture.width(); ++x) {
        const SkColor color = picture.getColor(x, y);
        if (!box.Contains(x, y)) {
          ASSERT_EQ(color, SK_ColorWHITE) << x << "," << y;
          continue;
        }
        // Greys of the white background only: no red left.
        ASSERT_EQ(SkColorGetR(color), SkColorGetG(color)) << x << "," << y;
        stripes += color != SK_ColorWHITE;
      }
    }
    // Stripes cover a fair part of the box, so it reads as filled, not blank.
    EXPECT_GT(stripes, box.size().GetArea() / 8);
  }
}

}  // namespace
}  // namespace browser_core
