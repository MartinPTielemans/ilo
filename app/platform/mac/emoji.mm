#import <CoreText/CoreText.h>
#import <Foundation/Foundation.h>

#include "../host.h"

// Apple Color Emoji draws a sequence it knows as one glyph, and one it doesn't as the glyphs of its
// parts, or a missing glyph from the Last Resort font: so one glyph in that font is a drawn emoji.
bool host_can_draw_emoji(const char* emoji) {
  static CTFontRef font = CTFontCreateWithName(CFSTR("Apple Color Emoji"), 20, nullptr);
  NSDictionary* attributes = @{(__bridge id)kCTFontAttributeName : (__bridge id)font};
  NSAttributedString* text = [[NSAttributedString alloc] initWithString:@(emoji)
                                                             attributes:attributes];
  CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)text);
  CFArrayRef runs = CTLineGetGlyphRuns(line);
  bool drawn = false;
  if (CFArrayGetCount(runs) == 1) {
    CTRunRef run = (CTRunRef)CFArrayGetValueAtIndex(runs, 0);
    CTFontRef used = (CTFontRef)CFDictionaryGetValue(CTRunGetAttributes(run), kCTFontAttributeName);
    CFStringRef name = CTFontCopyPostScriptName(used);
    drawn = CTRunGetGlyphCount(run) == 1 && CFEqual(name, CFSTR("AppleColorEmoji"));
    CFRelease(name);
  }
  CFRelease(line);
  return drawn;
}
