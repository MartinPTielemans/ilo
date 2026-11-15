#include "browser_core/withheld_fields.h"

#include <algorithm>
#include <cstddef>

#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "third_party/skia/include/core/SkCanvas.h"
#include "third_party/skia/include/core/SkColor.h"
#include "third_party/skia/include/core/SkPaint.h"
#include "third_party/skia/include/core/SkRect.h"
#include "ui/gfx/geometry/rect_conversions.h"

namespace browser_core {

namespace {

// The hatch's stripes, in CSS pixels: as wide and as far apart as the capture card draws (#2352).
constexpr float kStripe = 2;
constexpr float kPeriod = 9;

// The field's background as drawn: the colour most of its four corners, just inside, have, as its
// text rarely reaches a corner.
SkColor Background(const SkBitmap& picture, const gfx::Rect& box) {
  const int left = std::min(box.x() + 1, box.right() - 1);
  const int right = std::max(box.right() - 2, box.x());
  const int top = std::min(box.y() + 1, box.bottom() - 1);
  const int bottom = std::max(box.bottom() - 2, box.y());
  const SkColor corners[] = {picture.getColor(left, top), picture.getColor(right, top),
                             picture.getColor(left, bottom), picture.getColor(right, bottom)};
  SkColor most = corners[0];
  std::ptrdiff_t count = 0;
  for (SkColor corner : corners) {
    const std::ptrdiff_t same = std::ranges::count(corners, corner);
    if (same > count) {
      most = corner;
      count = same;
    }
  }
  return SkColorSetA(most, SK_AlphaOPAQUE);
}

// The stripes' colour: a little darker than a light background, a little lighter than a dark one.
SkColor Stripe(SkColor background) {
  const int r = SkColorGetR(background);
  const int g = SkColorGetG(background);
  const int b = SkColorGetB(background);
  if (r * 299 + g * 587 + b * 114 > 128 * 1000) {
    return SkColorSetRGB(r * 85 / 100, g * 85 / 100, b * 82 / 100);
  }
  return SkColorSetRGB(r + (255 - r) / 4, g + (255 - g) / 4, b + (255 - b) / 4);
}

}  // namespace

bool Withheld(const FieldFacts& field) {
  if (field.value.empty()) {
    return false;
  }
  if (field.secret || field.autofilled) {
    return true;
  }
  if (field.typed && *field.typed == field.value) {
    return false;
  }
  return !field.fixed && field.markup != field.value;
}

bool Hatches(const FieldFacts& field, Hatched hatched) {
  return hatched == Hatched::kEveryValue ? !field.value.empty() : Withheld(field);
}

bool SecretAutocomplete(std::string_view autocomplete) {
  for (std::string_view token :
       base::SplitStringPiece(autocomplete, base::kWhitespaceASCII, base::TRIM_WHITESPACE,
                              base::SPLIT_WANT_NONEMPTY)) {
    if (base::StartsWith(token, "cc-", base::CompareCase::INSENSITIVE_ASCII) ||
        base::EqualsCaseInsensitiveASCII(token, "one-time-code")) {
      return true;
    }
  }
  return false;
}

gfx::RectF IntoParent(const gfx::RectF& box, const gfx::RectF& frame) {
  gfx::RectF seen = box;
  seen.Intersect(gfx::RectF(frame.size()));
  seen.Offset(frame.OffsetFromOrigin());
  return seen;
}

gfx::Rect InPicture(const gfx::RectF& box,
                    float zoom,
                    const gfx::Rect& source,
                    const gfx::Size& picture) {
  if (source.IsEmpty()) {
    return gfx::Rect();
  }
  gfx::RectF in_view = gfx::ScaleRect(box, zoom);
  in_view.Offset(-source.OffsetFromOrigin());
  const float scale = static_cast<float>(picture.width()) / static_cast<float>(source.width());
  gfx::Rect pixels = gfx::ToEnclosingRect(gfx::ScaleRect(in_view, scale));
  pixels.Intersect(gfx::Rect(picture));
  return pixels;
}

void Hatch(SkBitmap& picture, const std::vector<gfx::Rect>& boxes, float scale) {
  SkCanvas canvas(picture);
  const float period = std::max(kPeriod * scale, 3.f);
  for (const gfx::Rect& box : boxes) {
    if (box.IsEmpty()) {
      continue;
    }
    const SkColor background = Background(picture, box);
    canvas.save();
    canvas.clipRect(
        SkRect::Make(SkIRect::MakeXYWH(box.x(), box.y(), box.width(), box.height())));
    canvas.drawColor(background);
    SkPaint stripe;
    stripe.setColor(Stripe(background));
    stripe.setAntiAlias(true);
    stripe.setStrokeWidth(std::max(kStripe * scale, 1.f));
    // Lines rising to the right, each from the bottom edge, so the stripes meet every edge alike.
    const float top = static_cast<float>(box.y());
    const float bottom = static_cast<float>(box.bottom());
    const float height = bottom - top;
    const float right = static_cast<float>(box.right());
    for (float x = static_cast<float>(box.x()) - height; x < right + period; x += period) {
      canvas.drawLine(x, bottom, x + height, top, stripe);
    }
    canvas.restore();
  }
}

}  // namespace browser_core
