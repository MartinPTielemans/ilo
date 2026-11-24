// The window's material under a floating window's plate (app/src/glass.rs, #4090).

#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Puts the HUD window's material under the GPUI window drawn in |view|, in the shape of its plate
// at |left|, |top|, |width|, |height| points from the window's top left with corners of |radius|,
// as opaque as |opacity|, in a |light| or dark look. Asked again as the plate moves, scales or
// fades.
void host_set_glass(void* view,
                    double left,
                    double top,
                    double width,
                    double height,
                    double radius,
                    double opacity,
                    bool light);

#ifdef __cplusplus
}
#endif
