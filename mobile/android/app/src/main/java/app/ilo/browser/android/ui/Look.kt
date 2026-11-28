package app.ilo.browser.android.ui

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * ilo's look on the phone, ported one to one from the iOS app's Look.swift, so both phones
 * feel like one browser. The Space's color washes the ground and the page's bar.
 */
data class Look(
    val dark: Boolean,
    /** The Space's color, or ilo's green when it has none. */
    val accent: Color,
    /** A private tab's chrome: no Space color in its ground or bar, ink in light too. */
    val isPrivate: Boolean = false,
    /** The color the page's own theme-color meta named: the bar takes it instead of the
     * Space's, as Chrome's bar does; the ground keeps the Space's wash. */
    val pageColor: Long? = null,
) {
    companion object {
        val Ink = Color(0xFF16140F)
        val Paper = Color(0xFFF3F0E8)
        /** ilo's green, the accent when a Space has no color of its own. */
        val GreenDark = Color(0xFF57B97A)
        val GreenLight = Color(0xFF289054)
        /** What the Space's color lies over. */
        val DarkBase = Color(0xFF131313)
        val OverlayLight = Color(0xFFFBF9F4)
        val OverlayDark = Color(0xFF1F1D17)
        val ClosePlate = Color(0x9E1E2026)
        val Dim = Color(0x590A0E14)

        /** A private tab's chrome is ink whatever the phone's look, as iOS's privateIn forces dark. */
        fun of(dark: Boolean, spaceColor: Long?, isPrivate: Boolean = false, pageColor: Long? = null): Look =
            Look(
                dark = if (isPrivate) true else dark,
                accent = spaceColor?.let { Color(0xFF000000 + it) }
                    ?: if (dark || isPrivate) GreenDark else GreenLight,
                isPrivate = isPrivate,
                pageColor = pageColor,
            )
    }

    val text: Color get() = if (dark) Paper else Ink
    val soft: Color get() = text.copy(alpha = if (isPrivate) 0.62f else 0.60f)
    val faint: Color get() = text.copy(alpha = 0.25f)
    /** What floats over the ground: the bar, a sheet. */
    val overlay: Color get() = if (dark) OverlayDark else Paper
    /** The page's bar, which takes the page's own color when it named one, else the Space's. */
    val tinted: Color
        get() {
            if (isPrivate) return Ink
            val wash = pageColor ?: (accent.toArgb() and 0xFFFFFF).toLong()
            return if (dark) blend(OverlayDark, Color(0xFF000000 + wash), 0.36f)
            else blend(Paper, Color(0xFF000000 + wash), 0.26f)
        }
    /** A tile's or a pressed row's plate. */
    val plate: Color get() = if (dark) Color.White.copy(alpha = 0.10f) else Ink.copy(alpha = 0.07f)
    /** A round button's plate on a bar: a private tab's is Paper, as iOS's glassButton is. */
    val button: Color
        get() = if (isPrivate) Paper.copy(alpha = 0.16f)
        else if (dark) Color.White.copy(alpha = 0.14f) else Ink.copy(alpha = 0.09f)
    val rim: Color get() = if (dark) Color.White.copy(alpha = 0.10f) else Ink.copy(alpha = 0.10f)
    val shadow: Color get() = Color.Black.copy(alpha = if (dark) 0.35f else 0.14f)
    val cardShadow: Color get() = Color.Black.copy(alpha = if (dark) 0.22f else 0.12f)
    val alarm: Color get() = if (dark) Color(0xFFE1676F) else Color(0xFFC9414B)
    /** The colors the ground is made of: the base with the Space's color washed in. */
    val groundTop: Color
        get() = if (isPrivate) Ink else blend(if (dark) DarkBase else Paper, accent, 0.20f)
    val groundBottom: Color
        get() = if (isPrivate) Ink else blend(if (dark) DarkBase else Paper, accent, 0.07f)
    /** What's written on glass: Paper on a private tab's ink, whatever the phone's look. */
    val glassText: Color get() = if (isPrivate) Paper else text
    val glassSoft: Color get() = if (isPrivate) Paper.copy(alpha = 0.55f) else soft

    /** The ink look for a private page's chrome, keeping the Space's color, as iOS's privateIn does. */
    fun privateVersion() = copy(dark = true, isPrivate = true)

    /** Straight lerp in sRGB, matching iOS's mix. */
    private fun blend(a: Color, b: Color, fraction: Float): Color = Color(
        red = a.red + (b.red - a.red) * fraction,
        green = a.green + (b.green - a.green) * fraction,
        blue = a.blue + (b.blue - a.blue) * fraction,
        alpha = a.alpha + (b.alpha - a.alpha) * fraction,
    )
}

/** The values of the parts, from the iOS Look.swift, in dp where the phone measures them. */
object Metrics {
    /** The bar at the foot of a page. */
    object Bar {
        val height = 52.dp
        val gap = 8.dp
        val side = 12.dp
        val radius = 26.dp
        /** The page's bar while the page is read: a pill with the site's name. */
        val small = 32.dp
        val smallLabel = 13.sp
        val smallInset = 18.dp
        /** From each end of the page's bar to the site's name: the room of the button there. */
        val nameInset = 49.dp
        /** The line along the bar's foot that says how far the page has loaded. */
        val loadedLine = 2.dp
        /** How far a sideways swipe along the bar must be heading to step a tab. */
        val stepSwipe = 90.dp
    }

    /** An open page's card in the Space's grid. */
    object Card {
        val pictureHeight = 200.dp
        val radius = 18.dp
        val side = 16.dp
        val columnGap = 12.dp
        val rowGap = 16.dp
        val ring = 3.dp
        val close = 24.dp
        val closeInset = 7.dp
        val touch = 44.dp
        /** The narrowest a card is where the grid has room for more than two. */
        val least = 176.dp

        /** How many cards fit across `width`, two at least, as iOS's Card.across counts. */
        fun across(width: Float): Int =
            maxOf(2, ((width - 2 * side.value + columnGap.value) / (least.value + columnGap.value)).toInt())

        /** A card's width in a grid `width` across, as iOS's Card.width shares it out. */
        fun widthIn(width: Float): Float {
            val across = across(width).toFloat()
            return maxOf(1f, (width - 2 * side.value - (across - 1) * columnGap.value) / across)
        }
    }

    /** The dock at the foot of the Space. */
    object Dock {
        val margin = 8.dp
        val radius = 40.dp
        val top = 14.dp
        val inset = 10.dp
        val field = 48.dp
        val gap = 8.dp
        val tileHeight = 54.dp
        val tileRadius = 16.dp
        val tileIcon = 30.dp
        val tileLabel = 66.dp
        val rowGap = 10.dp
        val pinsFoot = 12.dp
        /** How far a sideways swipe on the dock must be heading to turn to the next Space. */
        val swipe = 90.dp
    }

    /** What stands at the foot of the screen. */
    object Column {
        val widest = 440.dp
        val side = 12.dp
        val inset = 12.dp
        val icon = 20.dp
        val gap = 12.dp
    }

    /** The tools menu the bar's dots open, a floating plate at the page's bottom right. */
    object Menu {
        val width = 320.dp
        val radius = 20.dp
        val side = 8.dp
        val row = 56.dp
        val rowSide = 16.dp
        val gap = 4.dp
        val nav = 40.dp
        val shadow = 8.dp
        /** The share of the screen's width and height the menu never reaches on its side, so
         * the top left stays free for the page, as the owner asked. */
        const val freeShare = 0.10f
    }

    /** A search engine's mark: the icon a site gave, or its own letter on a plate. */
    object EngineMark {
        val plate = 28.dp
        val radius = 8.dp
        val icon = 20.dp
        val letter = 15.sp
        /** The tinted plate behind the letter, as the field's own accent plate. */
        const val plateFill = 0.22f
    }

    /** The switcher's stacked deck: the cards climb in stairs behind the front one, each a
     * step sideways and tilted a little as if pinned to a wheel's rim. The card's size and
     * the stack's overlap are the owner's dials, so they live in BrowserSettings. */
    object Switcher {
        /** Each card behind stands this share of the card's height over the one in front of
         * it, the stairs that keep the deep cards in view. */
        const val stair = 0.10f
        /** Each card behind tilts this many degrees more than the one in front of it, as if
         * the stack curved around a wheel. */
        const val tilt = 6f
        /** A swipe must cross this share of the deck before its move applies, turn or throw. */
        const val reach = 0.30f
        /** How transparent the front card goes as a switch swipe drags it, so the card
         * behind it reads through it. */
        const val frontFade = 0.30f
        /** The card's width against its height, shared by every card and by the stair math. */
        const val aspect = 0.62f
        /** How many cards the stack shows behind the front one. */
        val behind = 2
        /** The three controls at the top right: the mode's incognito mark, the archive and the way out. */
        val control = 34.dp
        val controlGap = 8.dp
        val controlSide = 16.dp
    }

    /** What marks a private tab: the one glyph, on its bar and on its card, from the iOS PrivateMark. */
    object PrivateMark {
        val onBar = 13.dp
        val barGap = 6.dp
        val onCard = 20.dp
        val plate = 44.dp
        val plateRadius = 11.dp
        /** What the card's plate and rim have of Paper, and the glyph on it. */
        const val plateFill = 0.10f
        const val cardRim = 0.16f
        const val cardGlyph = 0.78f
        /** Under the card, darker than any picture and so needing less shadow to stand off. */
        const val cardShadow = 0.22f
        val cardShadowRadius = 6.dp
        /** A new private tab's words, where its page will be. */
        val heading = 24.sp
        val headingGlyph = 19.dp
        val headingGap = 10.dp
        val wordsGap = 8.dp
        val side = 24.dp
        val overField = 26.dp
    }
}

/** The Look the UI sits in, its colors picked for the Space that's up. */
val LocalLook = staticCompositionLocalOf { Look.of(dark = true, spaceColor = null) }

/** The app's own type sizes, from the iOS app's scaled(17/13) choices. */
object Type {
    val bar = 17.sp
    val small = 13.sp
    val card = 13.sp
}