package app.ilo.browser.android.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.ArrowForward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The glyphs the app draws: its own tabs and incognito marks, and Material's for the rest. */
enum class Glyph { TABS, INCOGNITO, CHEVRON_UP, CHEVRON_DOWN, MAGNIFIER, CLOSE, BACK, FORWARD, REFRESH, PLUS, SHIELD, SETTINGS, DESKTOP, BROKEN_LOCK, MORE }

/**
 * The field the whole app writes in, so the field in the dock and the one on the page's bar
 * are one field with the same keys. The page bar's opens with its words all selected, as
 * iOS's selectsAll does when its field gains the focus, so typing replaces them.
 */
@Composable
fun BasicField(
    value: String,
    onValueChange: (String) -> Unit,
    hint: String,
    look: Look,
    modifier: Modifier = Modifier,
    onSubmit: (() -> Unit)? = null,
    onFocus: ((Boolean) -> Unit)? = null,
    selectsAll: Boolean = false,
    focus: androidx.compose.ui.focus.FocusRequester? = null,
) {
    // The field's own selection state, so the words stay selected while the page's host
    // changes under them.
    var words by remember { mutableStateOf(TextFieldValue(value)) }
    // Every text this field has pushed out since it gained the focus. A parent echo of
    // any of them, however many frames late, is not an outside change: taking one rolls
    // the field back under newer words, which dropped the fast typist's characters.
    val pushedTexts = remember { mutableSetOf<String>() }
    var selectedThisFocus by remember { mutableStateOf(false) }
    // An outside change of the words (the page's host under the bar's field, the cross clearing
    // them) always reaches the field: while focused the change came from outside, so the caret
    // goes to the words' end rather than the field holding stale words.
    LaunchedEffect(value) {
        if (value !in pushedTexts && words.text != value) {
            words = TextFieldValue(text = value, selection = TextRange(value.length))
        }
    }
    BasicTextField(
        value = words,
        onValueChange = { typed ->
            words = typed
            // An empty push is not remembered: the cross clears to the empty text, and that
            // clear must always reach the field, not be taken for a deleted-out text's echo.
            if (typed.text.isNotEmpty()) pushedTexts.add(typed.text)
            onValueChange(typed.text)
        },
        textStyle = TextStyle(color = look.glassText, fontSize = Type.bar),
        cursorBrush = SolidColor(look.accent),
        keyboardOptions = KeyboardOptions(
            autoCorrect = false,
            keyboardType = KeyboardType.Uri,
            imeAction = if (onSubmit != null) ImeAction.Go else ImeAction.None,
        ),
        keyboardActions = KeyboardActions(onGo = { onSubmit?.invoke() }),
        singleLine = true,
        modifier = modifier
            .let { if (focus != null) it.focusRequester(focus) else it }
            .onFocusChanged { gained ->
            if (!gained.isFocused) pushedTexts.clear()
            // The words are selected the moment the focus lands, not a frame later: a gap
            // between the two lets the first keystrokes in before the replacement, garbling
            // what fast typing leaves in the field.
            if (gained.isFocused && selectsAll && !selectedThisFocus && words.text.isNotEmpty()) {
                selectedThisFocus = true
                words = TextFieldValue(text = words.text, selection = TextRange(0, words.text.length))
            }
            if (!gained.isFocused) selectedThisFocus = false
            onFocus?.invoke(gained.isFocused)
        },
        decorationBox = { inner ->
            Box(contentAlignment = Alignment.CenterStart) {
                if (words.text.isEmpty()) Text(hint, color = look.glassSoft, fontSize = Type.bar, maxLines = 1)
                inner()
            }
        },
    )
}

/** A round button on a plate, its glyph drawn or picked from Material's set. */
@Composable
fun PlateButton(
    glyph: Glyph,
    label: String,
    look: Look,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    dimmed: Boolean = false,
) {
    val tint = if (dimmed) look.glassText.copy(alpha = 0.26f) else look.glassText
    Box(
        modifier = modifier
            .size(Metrics.Bar.height)
            .background(look.button, CircleShape)
            .clickable(enabled = !dimmed, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        when (glyph) {
            Glyph.TABS -> Canvas(Modifier.size(34.dp)) { drawTabsGlyph(tint, size.width, size.height) }
            Glyph.INCOGNITO -> Canvas(Modifier.size(34.dp)) { drawIncognitoGlyph(tint, size.width, size.height) }
            Glyph.CHEVRON_UP -> Icon(Icons.Filled.KeyboardArrowUp, null, tint = tint)
            Glyph.CHEVRON_DOWN -> Icon(Icons.Filled.KeyboardArrowDown, null, tint = tint)
            Glyph.MAGNIFIER -> Icon(Icons.Filled.Search, null, tint = tint)
            Glyph.CLOSE -> Icon(Icons.Filled.Close, null, tint = tint)
            Glyph.BACK -> Icon(Icons.Filled.ArrowBack, null, tint = tint)
            Glyph.FORWARD -> Icon(Icons.Filled.ArrowForward, null, tint = tint)
            Glyph.REFRESH -> Icon(Icons.Filled.Refresh, null, tint = tint)
            Glyph.PLUS -> Icon(Icons.Filled.Add, null, tint = tint)
            Glyph.SHIELD -> Canvas(Modifier.size(34.dp)) { drawShieldGlyph(tint, size.width, size.height) }
            Glyph.DESKTOP -> Canvas(Modifier.size(34.dp)) { drawDesktopGlyph(tint, size.width, size.height) }
            Glyph.BROKEN_LOCK -> Canvas(Modifier.size(34.dp)) { drawBrokenLockGlyph(tint, size.width, size.height) }
            Glyph.SETTINGS -> Icon(Icons.Filled.Settings, null, tint = tint)
            Glyph.MORE -> Icon(Icons.Filled.MoreVert, null, tint = tint)
        }
    }
}

/**
 * The tabs glyph, two cards one over the other, as the iOS app draws it: the back one at half
 * transparency turned ten degrees one way, the front one turned eight the other.
 */
internal fun DrawScope.drawTabsGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    fun card(xoff: Float, yoff: Float, degrees: Float, alpha: Float) {
        rotate(degrees, pivot = Offset(width / 2f, height / 2f)) {
            drawRoundRect(
                color = tint.copy(alpha = alpha),
                topLeft = Offset(width / 2f - 6.5f * scale + xoff, height / 2f - 8.5f * scale + yoff),
                size = Size(13f * scale, 17f * scale),
                cornerRadius = CornerRadius(3.5f * scale, 3.5f * scale),
                style = Fill,
            )
        }
    }
    card(-3f * scale, 0.5f * scale, -10f, 0.5f)
    card(3f * scale, -0.5f * scale, 8f, 1f)
}

/** A search engine's mark: the icon its own site gave, where the phone learned one, else the one
 * its /favicon.ico hands over, else the one the app ships with, else its letter on a tinted
 * plate. Pressing it is the chooser's own, so this is only what it looks like, shared by the
 * field, its chooser and Settings. */
@Composable
fun EngineMark(
    engine: app.ilo.browser.android.browser.SearchEngine,
    favicons: app.ilo.browser.android.browser.FaviconStore,
    look: Look,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
) {
    val version by favicons.version.collectAsState()
    val icon = remember(engine.prefix, version) {
        favicons.load(engine.prefix)
        // With nothing on disk the engine's own site is asked for it, as iOS's SiteIcon asks.
        if (favicons.icon(of = engine.prefix) == null) favicons.find(engine.prefix)
        favicons.icon(of = engine.prefix)
    }
    Box(
        modifier = modifier
            .size(Metrics.EngineMark.plate)
            .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
            .semantics { contentDescription = engine.name },
        contentAlignment = Alignment.Center,
    ) {
        if (icon != null) {
            androidx.compose.foundation.Image(
                bitmap = icon.asImageBitmap(),
                contentDescription = null,
                contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                modifier = Modifier.size(Metrics.EngineMark.icon).clip(RoundedCornerShape(Metrics.EngineMark.radius)),
            )
        } else {
            val bundled = engine.bundledIcon
            if (bundled != null) {
                androidx.compose.foundation.Image(
                    painter = androidx.compose.ui.res.painterResource(bundled),
                    contentDescription = null,
                    contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                    modifier = Modifier.size(Metrics.EngineMark.icon).clip(RoundedCornerShape(Metrics.EngineMark.radius)),
                )
            } else {
                Box(
                    modifier = Modifier
                        .size(Metrics.EngineMark.plate)
                        .background(look.accent.copy(alpha = Metrics.EngineMark.plateFill), RoundedCornerShape(Metrics.EngineMark.radius)),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        engine.name.take(1).uppercase(),
                        color = look.glassText,
                        fontSize = Metrics.EngineMark.letter,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}

/** A page's mark on a tile or a row: the favicon its site gave, else its host's letter on a
 * tinted plate, so no surface ever shows a bare letter where the phone knows the site's
 * face, and no favicon shows nothing where a letter would have said something. */
@Composable
fun FaviconMark(
    url: String,
    favicons: app.ilo.browser.android.browser.FaviconStore,
    look: Look,
    modifier: Modifier = Modifier,
) {
    val version by favicons.version.collectAsState()
    val icon = remember(url, version) {
        favicons.load(url)
        favicons.icon(of = url)
    }
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        if (icon != null) {
            androidx.compose.foundation.Image(
                bitmap = icon.asImageBitmap(),
                contentDescription = null,
                contentScale = androidx.compose.ui.layout.ContentScale.Crop,
                modifier = Modifier.size(Metrics.EngineMark.icon).clip(RoundedCornerShape(Metrics.EngineMark.radius)),
            )
        } else {
            Box(
                modifier = Modifier
                    .size(Metrics.EngineMark.icon)
                    .background(look.accent.copy(alpha = Metrics.EngineMark.plateFill), RoundedCornerShape(Metrics.EngineMark.radius)),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    hostOf(url).take(1).uppercase().ifEmpty { "?" },
                    color = look.glassText,
                    fontSize = Metrics.EngineMark.letter,
                    maxLines = 1,
                )
            }
        }
    }
}

/** The private mark, the one shape everywhere: a fedora hat over round glasses. */
internal fun DrawScope.drawIncognitoGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    val stroke = Stroke(width = 2f * scale)
    // The hat's crown, pinched where a fedora's front dips, closed along the brim's line.
    val crown = Path().apply {
        moveTo(9.5f * scale, 11f * scale)
        lineTo(10.5f * scale, 5.5f * scale)
        cubicTo(12f * scale, 3.8f * scale, 14f * scale, 3.5f * scale, 17f * scale, 3.5f * scale)
        cubicTo(20f * scale, 3.5f * scale, 22f * scale, 3.8f * scale, 23.5f * scale, 5.5f * scale)
        lineTo(24.5f * scale, 11f * scale)
        close()
    }
    drawPath(crown, tint, style = stroke)
    // The brim runs both ways from the crown's foot and snaps down at its ends.
    drawLine(tint, Offset(17f * scale, 11f * scale), Offset(31f * scale, 13.5f * scale), 2f * scale)
    drawLine(tint, Offset(17f * scale, 11f * scale), Offset(3f * scale, 13.5f * scale), 2f * scale)
    // Round glasses under the hat, joined by their bridge.
    drawCircle(tint, 5.2f * scale, Offset(10.5f * scale, 23f * scale), style = stroke)
    drawCircle(tint, 5.2f * scale, Offset(23.5f * scale, 23f * scale), style = stroke)
    drawLine(tint, Offset(15.7f * scale, 21.5f * scale), Offset(18.3f * scale, 21.5f * scale), 2f * scale)
}

/** The incognito-off mark: the same hat and glasses with the hat lifted off, a change of
 * state the shape keeps whole. */
internal fun DrawScope.drawIncognitoOffGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    val stroke = Stroke(width = 2f * scale)
    // The hat sits tilted above the face, its brim to its crown's foot.
    val crown = Path().apply {
        moveTo(7f * scale, 9f * scale)
        lineTo(8f * scale, 3.5f * scale)
        cubicTo(9.5f * scale, 1.8f * scale, 11.5f * scale, 1.5f * scale, 14.5f * scale, 1.5f * scale)
        cubicTo(17.5f * scale, 1.5f * scale, 19.5f * scale, 1.8f * scale, 21f * scale, 3.5f * scale)
        lineTo(22f * scale, 9f * scale)
        close()
    }
    drawPath(crown, tint, style = stroke)
    drawLine(tint, Offset(4.5f * scale, 8.5f * scale), Offset(24.5f * scale, 10f * scale), 2f * scale)
    // Round glasses at rest below, joined by their bridge.
    drawCircle(tint, 5.2f * scale, Offset(10.5f * scale, 23.5f * scale), style = stroke)
    drawCircle(tint, 5.2f * scale, Offset(23.5f * scale, 23.5f * scale), style = stroke)
    drawLine(tint, Offset(15.7f * scale, 22f * scale), Offset(18.3f * scale, 22f * scale), 2f * scale)
}

/** The globe, for the row that opens an address, as iOS's Go row shows it. */
internal fun DrawScope.drawGlobeGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    val r = 8.5f * scale
    drawCircle(color = tint, radius = r, center = Offset(cx, cy), style = Stroke(width = 1.6f * scale))
    // The equator and one meridian say it's a globe, not a plain ring.
    drawLine(
        color = tint,
        start = Offset(cx - r, cy),
        end = Offset(cx + r, cy),
        strokeWidth = 1.4f * scale,
    )
    drawOval(
        color = tint,
        topLeft = Offset(cx - r * 0.45f, cy - r),
        size = Size(r * 0.9f, r * 2f),
        style = Stroke(width = 1.4f * scale),
    )
}

/** The shield: a rounded plate pointed at its foot, the blocked count's own mark. */
internal fun DrawScope.drawShieldGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    val w = 16f * scale
    val h = 18f * scale
    val cx = width / 2f
    val cy = height / 2f
    val top = cy - h / 2f
    drawRoundRect(
        color = tint,
        topLeft = Offset(cx - w / 2f, top),
        size = Size(w, h),
        cornerRadius = CornerRadius(4f * scale, 4f * scale),
        style = Fill,
    )
}

/** The desktop: a screen with its stand, what asks a site for its desktop page. */
internal fun DrawScope.drawDesktopGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    drawRoundRect(
        color = tint,
        topLeft = Offset(8f * scale, 8f * scale),
        size = Size(18f * scale, 13f * scale),
        cornerRadius = CornerRadius(2f * scale, 2f * scale),
        style = Stroke(2f * scale),
    )
    drawLine(
        color = tint,
        start = Offset(12f * scale, 23f * scale),
        end = Offset(22f * scale, 23f * scale),
        strokeWidth = 2f * scale,
    )
    drawLine(
        color = tint,
        start = Offset(17f * scale, 21f * scale),
        end = Offset(17f * scale, 23f * scale),
        strokeWidth = 2f * scale,
    )
}

/** The broken lock: a lock a slash runs through, the mark of a certificate not trusted. */
internal fun DrawScope.drawBrokenLockGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 34f
    val cx = width / 2f
    val cy = height / 2f
    drawRoundRect(
        color = tint,
        topLeft = Offset(cx - 7f * scale, cy - 6f * scale),
        size = Size(14f * scale, 10f * scale),
        cornerRadius = CornerRadius(2f * scale, 2f * scale),
        style = Stroke(2f * scale),
    )
    drawArc(
        color = tint,
        startAngle = 180f,
        sweepAngle = 180f,
        useCenter = false,
        topLeft = Offset(cx - 5f * scale, cy - 10f * scale),
        size = Size(10f * scale, 10f * scale),
        style = Stroke(2f * scale),
    )
    drawLine(
        color = tint,
        start = Offset(cx + 8f * scale, cy + 9f * scale),
        end = Offset(cx - 8f * scale, cy - 9f * scale),
        strokeWidth = 2f * scale,
    )
}

/** The clock, for what the phone remembers of pages it was on. */
internal fun DrawScope.drawClockGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    drawCircle(color = tint, radius = 8.5f * scale, center = Offset(cx, cy), style = Stroke(1.6f * scale))
    drawLine(color = tint, start = Offset(cx, cy - 5f * scale), end = Offset(cx, cy), strokeWidth = 1.4f * scale)
    drawLine(color = tint, start = Offset(cx, cy), end = Offset(cx + 3.5f * scale, cy + 2f * scale), strokeWidth = 1.4f * scale)
}

/** The download: an arrow down to a tray, what a page brought to the phone. */
internal fun DrawScope.drawDownloadGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    drawLine(color = tint, start = Offset(cx, height / 2f - 8f * scale), end = Offset(cx, height / 2f + 2f * scale), strokeWidth = 1.8f * scale)
    drawLine(color = tint, start = Offset(cx - 4f * scale, height / 2f - 2.5f * scale), end = Offset(cx, height / 2f + 2f * scale), strokeWidth = 1.8f * scale)
    drawLine(color = tint, start = Offset(cx + 4f * scale, height / 2f - 2.5f * scale), end = Offset(cx, height / 2f + 2f * scale), strokeWidth = 1.8f * scale)
    drawLine(color = tint, start = Offset(cx - 6f * scale, height / 2f + 6.5f * scale), end = Offset(cx + 6f * scale, height / 2f + 6.5f * scale), strokeWidth = 1.8f * scale)
}

/** The translate: two bubbles, one holding its words and one empty, as iOS's character.bubble. */
internal fun DrawScope.drawTranslateGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    drawRoundRect(
        color = tint,
        topLeft = Offset(width / 2f - 8f * scale, height / 2f - 8f * scale),
        size = Size(9f * scale, 7f * scale),
        cornerRadius = CornerRadius(3.5f * scale, 3.5f * scale),
        style = Fill,
    )
    drawLine(color = tint, start = Offset(width / 2f - 5f * scale, height / 2f - 1f * scale), end = Offset(width / 2f + 1f * scale, height / 2f - 1f * scale), strokeWidth = 1.2f * scale)
    drawLine(color = tint, start = Offset(width / 2f - 5f * scale, height / 2f - 3.5f * scale), end = Offset(width / 2f + 1f * scale, height / 2f - 3.5f * scale), strokeWidth = 1.2f * scale)
    drawRoundRect(
        color = tint,
        topLeft = Offset(width / 2f - 1f * scale, height / 2f + 0.5f * scale),
        size = Size(9f * scale, 7f * scale),
        cornerRadius = CornerRadius(3.5f * scale, 3.5f * scale),
        style = Stroke(1.6f * scale),
    )
}

/** The mark of a page that didn't load cleanly: a ring with its i, what the (i) beside the
 * field's title is. */
internal fun DrawScope.drawInfoGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    drawCircle(color = tint, radius = 8f * scale, center = Offset(cx, cy), style = Stroke(1.5f * scale))
    drawLine(color = tint, start = Offset(cx, cy - 3.5f * scale), end = Offset(cx, cy + 4f * scale), strokeWidth = 1.5f * scale)
    drawCircle(color = tint, radius = 1f * scale, center = Offset(cx, cy - 5.5f * scale), style = Fill)
}

/** The page's host, its www dropped, or the words themselves with no page. */
internal fun hostOf(url: String?): String {
    if (url == null) return ""
    val host = runCatching { java.net.URI(url).host }.getOrNull() ?: return url
    return if (host.startsWith("www.")) host.substring(4) else host
}

/** The magnifier: a ring with its handle, what looks for words on the page. */
internal fun DrawScope.drawMagnifierGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    drawCircle(
        color = tint,
        radius = 6.5f * scale,
        center = Offset(width / 2f - 1.5f * scale, height / 2f - 1.5f * scale),
        style = Stroke(1.7f * scale),
    )
    drawLine(
        color = tint,
        start = Offset(width / 2f + 3.2f * scale, height / 2f + 3.2f * scale),
        end = Offset(width / 2f + 7f * scale, height / 2f + 7f * scale),
        strokeWidth = 1.9f * scale,
    )
}
