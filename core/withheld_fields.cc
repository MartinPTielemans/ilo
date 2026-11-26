#include "browser_core/withheld_fields.h"

#include <algorithm>

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

// The hatch's two greys, the same on every page: a colour taken from the field's pixels would
// follow what it holds, and the picture of a withheld field shows nothing of that. A mid grey reads
// as covered on a light page and a dark one.
constexpr SkColor kGround = SkColorSetRGB(0xC8, 0xC8, 0xC8);
constexpr SkColor kStripeColor = SkColorSetRGB(0xA6, 0xA6, 0xA6);

}  // namespace

Seen SeenOf(const FieldFacts& field) {
  if (field.value.empty()) {
    return Seen::kEmpty;
  }
  if (field.secret || field.autofilled) {
    return Seen::kFilled;
  }
  if ((field.typed && *field.typed == field.value) || field.fixed ||
      (field.select && field.markup == field.value)) {
    return Seen::kValue;
  }
  return field.typed ? Seen::kChanged : Seen::kFilled;
}

bool CouldShow(const FieldFacts& field) {
  return !field.value.empty() && !field.secret && !field.autofilled &&
         (field.typed || field.fixed || field.select);
}

bool Described(const FieldFacts& field) {
  return CouldShow(field) || (field.select && !field.secret && !field.autofilled);
}

bool Withheld(const FieldFacts& field) {
  const Seen seen = SeenOf(field);
  return seen == Seen::kFilled || seen == Seen::kChanged;
}

bool Hatches(const FieldFacts& field, Hatched hatched) {
  return hatched == Hatched::kEveryValue ? !field.value.empty() : Withheld(field);
}

bool SecretAutocomplete(std::string_view autocomplete) {
  for (std::string_view token :
       base::SplitStringPiece(autocomplete, base::kWhitespaceASCII, base::TRIM_WHITESPACE,
                              base::SPLIT_WANT_NONEMPTY)) {
    if (base::StartsWith(token, "cc-", base::CompareCase::INSENSITIVE_ASCII) ||
        base::EndsWith(token, "password", base::CompareCase::INSENSITIVE_ASCII) ||
        base::EqualsCaseInsensitiveASCII(token, "one-time-code")) {
      return true;
    }
  }
  return false;
}

FieldKind KindOf(const std::optional<std::string>& type,
                 const std::optional<std::string>& autocomplete) {
  const bool password = type && base::EqualsCaseInsensitiveASCII(*type, "password");
  return {password, password || (autocomplete && SecretAutocomplete(*autocomplete))};
}

void TellKinds(base::DictValue& node, bool secret, bool autofilled) {
  node.Set("secret", secret);
  node.Set("autofilled", autofilled);
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
    canvas.save();
    canvas.clipRect(
        SkRect::Make(SkIRect::MakeXYWH(box.x(), box.y(), box.width(), box.height())));
    canvas.drawColor(kGround);
    SkPaint stripe;
    stripe.setColor(kStripeColor);
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
