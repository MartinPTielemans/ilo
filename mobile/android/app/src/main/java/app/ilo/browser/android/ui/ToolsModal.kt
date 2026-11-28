package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.ArrowForward
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.Tab

/**
 * The tools the bar's dots open, one floating plate at the page's bottom right, as Chrome's own
 * menu stands: a tap outside or the back gesture closes it. The first row walks the history, back
 * and forward, each dimmed where it can't go. Under it the rows sit in groups that name
 * themselves, so the list is scanned rather than counted: the page's own rows; the lists the
 * account keeps; the ways on from here. Add to bookmarks, whose star fills when the page is kept,
 * and Reload belong to the page; Bookmarks, Downloads, History and Site controls open their
 * lists; New tab and New private tab make one; Share, Find on page, Translate, Reading mode,
 * Desktop site, Dark content, which turns the page itself dark whatever the app is, and Block
 * ads & trackers, the page's own pause of blocking, off only for this tab while it lives when the
 * tab is private, and for its site when it isn't, act on the page; Settings leaves it. The plate
 * grows up to the screen and then scrolls inside, with a clipped edge at the foot while it can
 * still scroll, the cue that the list goes on.
 */
@Composable
fun ToolsModal(
    browser: Browser,
    tab: Tab?,
    look: Look,
    onNewTab: (private: Boolean) -> Unit,
    onOpenHistory: () -> Unit,
    onOpenBookmarks: () -> Unit,
    onOpenDownloads: () -> Unit,
    onOpenSiteSettings: () -> Unit,
    onOpenSettings: () -> Unit,
    onFind: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val bookmarks by browser.stores.bookmarksFlow.collectAsState()
    // With no page behind it, the star follows nothing: it keeps its press but does nothing.
    val bookmarked = remember(bookmarks, tab) { tab?.url?.let { browser.stores.isBookmarked(it) } ?: false }

    BackHandler(onBack = onClose)

    Box(
        modifier
            .fillMaxSize()
            .clickable(role = Role.Button, onClick = onClose)
    ) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            // The top/left tenth stays free for the page, as the owner asked: the cap counts the
            // share off both sides of the screen, and the height cap keeps it above the bar too.
            val freeW = maxWidth * Metrics.Menu.freeShare
            val freeH = maxHeight * Metrics.Menu.freeShare
            val scroll = rememberScrollState()
            val density = LocalDensity.current
            // The cut lives in the draw, reading the scroll there: maxValue only exists after
            // the first layout, and a draw-side read redraws when it arrives, so the cue shows
            // from the sheet's first settled frame without a scroll to wake it.
            val peekPx = with(density) { 12.dp.toPx() }
            Column(
                Modifier
                    .align(Alignment.BottomEnd)
                    .padding(end = Metrics.Bar.side + Metrics.Menu.side)
                    .navigationBarsPadding()
                    // Above the bar itself, so the menu grows up from beside its dots.
                    .padding(bottom = Metrics.Bar.height + Metrics.Bar.gap)
                    .widthIn(max = maxWidth - freeW)
                    .width(Metrics.Menu.width)
                    .heightIn(max = maxHeight - freeH - (Metrics.Bar.height + Metrics.Bar.gap))
                    .shadow(Metrics.Menu.shadow, RoundedCornerShape(Metrics.Menu.radius))
                    .background(look.overlay, RoundedCornerShape(Metrics.Menu.radius))
                    // On the plate, before the scroll: inside it the lambda would measure the
                    // scrolled content, and the fade would land below every row.
                    .drawWithContent {
                        // The last row dissolves into the plate's edge, the cue that more rows
                        // wait below, gone once the list reaches its end.
                        drawContent()
                        if (scroll.value < scroll.maxValue - peekPx) {
                            drawRect(
                                brush = Brush.verticalGradient(
                                    0f to Color.Transparent,
                                    1f to look.overlay,
                                    startY = size.height - peekPx,
                                    endY = size.height,
                                ),
                            )
                        }
                    }
                    .verticalScroll(scroll)
                    .padding(vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(Metrics.Menu.gap),
            ) {
                if (tab != null) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = Metrics.Menu.rowSide),
                        horizontalArrangement = Arrangement.spacedBy(Metrics.Menu.gap),
                    ) {
                        NavButton(
                            glyph = Icons.Filled.ArrowBack,
                            label = stringResource(R.string.back),
                            look = look,
                            enabled = browser.engine.canGoBack(tab),
                        ) {
                            browser.engine.goBack(tab)
                            onClose()
                        }
                        NavButton(
                            glyph = Icons.Filled.ArrowForward,
                            label = stringResource(R.string.forward),
                            look = look,
                            enabled = browser.engine.canGoForward(tab),
                        ) {
                            browser.engine.goForward(tab)
                            onClose()
                        }
                    }
                }

                MenuHead(stringResource(R.string.menu_group_page), look)

                if (tab != null) {
                    MenuRow(
                        label = stringResource(if (bookmarked) R.string.remove_from_bookmarks else R.string.add_to_bookmarks),
                        look = look,
                        icon = { Icon(Icons.Filled.Star, null, tint = if (bookmarked) look.accent else look.soft) },
                    ) {
                        val url = tab.url ?: return@MenuRow
                        // The menu stays open, as the star is a state the page keeps, not a place it goes.
                        if (bookmarked) browser.stores.unbookmark(url) else browser.stores.bookmark(url, tab.title)
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = stringResource(R.string.reload),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawRefreshGlyph(look.soft, size.width, size.height) } },
                    ) {
                        browser.engine.reload(tab)
                        onClose()
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = stringResource(R.string.share),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawShareGlyph(look.soft, size.width, size.height) } },
                    ) {
                        val url = tab.url ?: return@MenuRow
                        // The page goes to Android's own share sheet, with its name, as any app shares it.
                        val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(android.content.Intent.EXTRA_TEXT, url)
                            putExtra(android.content.Intent.EXTRA_SUBJECT, tab.title)
                        }
                        context.startActivity(android.content.Intent.createChooser(send, null))
                        onClose()
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = stringResource(R.string.find_in_page),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawMagnifierGlyph(look.soft, size.width, size.height) } },
                    ) {
                        onClose()
                        onFind()
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = if (tab.originalUrl != null) stringResource(R.string.show_original)
                                else stringResource(R.string.translate_to,
                                    translationName(browser.settings.translationLanguage)),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawTranslateGlyph(look.soft, size.width, size.height) } },
                    ) {
                        if (tab.originalUrl != null) browser.showOriginal(tab)
                        else browser.translate(tab, translationCode(browser.settings.translationLanguage))
                        onClose()
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = stringResource(R.string.reading_mode),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawReaderGlyph(look.soft, size.width, size.height) } },
                    ) {
                        browser.engine.read(tab)
                        onClose()
                    }
                }

                if (tab != null) {
                    MenuRow(
                        label = if (tab.wantsDesktop) stringResource(R.string.request_mobile_site)
                                else stringResource(R.string.request_desktop_site),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawDesktopGlyph(look.soft, size.width, size.height) } },
                    ) {
                        browser.engine.requestDesktop(tab, !tab.wantsDesktop)
                        onClose()
                    }
                }

                if (tab != null) {
                    // The page's own dark, over whatever the app itself is: null follows the app's
                    // setting, so the row shows what the page now resolves to and writes its own.
                    val resolvedGeneral = browser.contentDarkDefault(isSystemInDarkTheme())
                    val darkNow = tab.contentDark ?: resolvedGeneral
                    MenuRow(
                        label = stringResource(R.string.dark_content),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawMoonGlyph(look.soft, size.width, size.height) } },
                        value = stringResource(if (darkNow) R.string.on else R.string.off),
                    ) {
                        // It stays open, as the page's dark is a state the page keeps, not a place it goes.
                        browser.engine.setContentDark(tab, !darkNow)
                    }
                }

                if (tab != null) {
                    // The page's own pause of blocking, as iOS's actions row: the value it stands
                    // at is the tab's own held choice first, its site's otherwise. A private tab's
                    // turn stays the tab's alone, in memory, gone with it; a normal tab's writes
                    // its site's choice, where Settings still lists the exception as the way
                    // back. Off in Settings says whose switch is off.
                    val masterOn by browser.blocking.isOnFlow.collectAsState()
                    // Keyed on the flow's turn so a choice made anywhere recomposes this row.
                    val sitesOff by browser.blocking.sitesOffFlow.collectAsState()
                    var heldVersion by remember { mutableStateOf(0) }
                    val url = tab.url
                    val site = remember(url) {
                        url?.takeIf { it.startsWith("http") }?.let { app.ilo.browser.android.blocking.EasyListBlocker.hostOf(it) }
                    }
                    val held = remember(tab.id, heldVersion, site) { site?.let { browser.engine.blockingHeldChoice(tab, it) } }
                    // The site's own choice as the blocker answers it, subdomains together.
                    val siteOff = remember(url, sitesOff) { url?.let { browser.blocking.siteOff(it) } ?: false }
                    val offNow = held?.not() ?: siteOff
                    MenuRow(
                        label = stringResource(R.string.block_ads),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawShieldGlyph(look.soft, size.width, size.height) } },
                        value = if (!masterOn) stringResource(R.string.off_in_settings)
                                else stringResource(if (offNow) R.string.off else R.string.on),
                        enabled = masterOn && site != null,
                    ) {
                        // It stays open, as the page's blocking is a state the page keeps, and the
                        // reload shows the difference behind the menu.
                        if (tab.isPrivate && site != null) {
                            browser.engine.setBlockingHeldChoice(tab, site, offNow)
                            heldVersion++
                        } else if (site != null) {
                            browser.blocking.setSite(site, offNow)
                            browser.engine.reload(tab)
                        }
                    }
                }

                MenuHead(stringResource(R.string.menu_group_library), look)

                MenuRow(
                    label = stringResource(R.string.bookmarks),
                    look = look,
                    icon = { Icon(Icons.Filled.Star, null, tint = look.soft) },
                ) {
                    onClose()
                    onOpenBookmarks()
                }

                MenuRow(
                    label = stringResource(R.string.downloads),
                    look = look,
                    icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawDownloadGlyph(look.soft, size.width, size.height) } },
                ) {
                    onClose()
                    onOpenDownloads()
                }

                MenuRow(
                    label = stringResource(R.string.history),
                    look = look,
                    icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawClockGlyph(look.soft, size.width, size.height) } },
                ) {
                    onClose()
                    onOpenHistory()
                }

                if (tab != null) {
                    MenuRow(
                        label = stringResource(R.string.site_settings),
                        look = look,
                        icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawBrokenLockGlyph(look.soft, size.width, size.height) } },
                    ) {
                        onClose()
                        onOpenSiteSettings()
                    }
                }

                MenuHead(stringResource(R.string.menu_group_new), look)

                MenuRow(
                    label = stringResource(R.string.new_tab),
                    look = look,
                    icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawTabsGlyph(look.soft, size.width, size.height) } },
                ) {
                    onNewTab(false)
                }

                MenuRow(
                    label = stringResource(R.string.new_incognito_tab),
                    look = look,
                    icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawIncognitoGlyph(look.soft, size.width, size.height) } },
                ) {
                    onNewTab(true)
                }

                MenuRow(
                    label = stringResource(R.string.settings),
                    look = look,
                    icon = { Canvas(Modifier.size(Metrics.Column.icon)) { drawSettingsGlyph(look.soft, size.width, size.height) } },
                ) {
                    onClose()
                    onOpenSettings()
                }
            }
        }
    }
}

/** One of the two history buttons, dimmed where the history has no step that way. */
@Composable
private fun NavButton(
    glyph: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    look: Look,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    val tint = if (enabled) look.text else look.soft.copy(alpha = 0.26f)
    Box(
        modifier = Modifier
            .size(Metrics.Menu.nav)
            .background(look.plate, CircleShape)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(glyph, null, tint = tint)
    }
}

/** A group's name, small and soft, the scan's hook that says what its rows share. */
@Composable
private fun MenuHead(text: String, look: Look) {
    Text(
        text,
        color = look.soft,
        fontSize = Type.small,
        modifier = Modifier.padding(start = Metrics.Menu.rowSide + Metrics.Column.inset, top = 8.dp),
    )
}

/** One row of the menu: its glyph, its name and what it stands at, tall enough for a thumb. */
@Composable
private fun MenuRow(
    label: String,
    look: Look,
    icon: @Composable () -> Unit,
    value: String? = null,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = Metrics.Menu.row)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = Metrics.Menu.rowSide)
            .semantics { contentDescription = label },
    ) {
        Box(Modifier.size(Metrics.Column.icon), contentAlignment = Alignment.Center) { icon() }
        Spacer(Modifier.width(Metrics.Column.gap))
        Text(
            label,
            color = look.text,
            fontSize = Type.bar,
            modifier = Modifier.weight(1f),
            maxLines = 1,
        )
        if (value != null) {
            Text(value, color = look.soft, fontSize = Type.small, maxLines = 1)
        }
    }
}

/** The moon, a ring with its inner edge bitten off, the page's own dark mark. */
internal fun DrawScope.drawMoonGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    drawCircle(color = tint, radius = 8f * scale, center = Offset(cx, cy), style = Stroke(1.7f * scale))
    // The bite is cleared out of the ring, so the crescent stands as the page's own dark.
    drawCircle(
        color = Color.Black,
        radius = 6.5f * scale,
        center = Offset(cx + 4.5f * scale, cy - 1.5f * scale),
        style = Fill,
        blendMode = androidx.compose.ui.graphics.BlendMode.Clear,
    )
}

/** The refresh mark: a broken circle with its arrow, as the bar's reload glyph is. */
internal fun DrawScope.drawRefreshGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    drawArc(
        color = tint,
        startAngle = 40f,
        sweepAngle = 280f,
        useCenter = false,
        topLeft = Offset(cx - 7f * scale, cy - 7f * scale),
        size = Size(14f * scale, 14f * scale),
        style = Stroke(1.8f * scale),
    )
    // The arrowhead at the arc's open end.
    drawLine(color = tint, start = Offset(cx + 5f * scale, cy - 6.5f * scale), end = Offset(cx + 7.5f * scale, cy - 4f * scale), strokeWidth = 1.8f * scale)
    drawLine(color = tint, start = Offset(cx + 5f * scale, cy - 6.5f * scale), end = Offset(cx + 5f * scale, cy - 3f * scale), strokeWidth = 1.8f * scale)
}

/** The share mark: one dot and its two roads out, as the Android share glyph is. */
internal fun DrawScope.drawShareGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    drawCircle(color = tint, radius = 2.2f * scale, center = Offset(cx, height / 2f - 6.5f * scale), style = Fill)
    drawCircle(color = tint, radius = 2.2f * scale, center = Offset(cx - 5.5f * scale, height / 2f + 3f * scale), style = Fill)
    drawCircle(color = tint, radius = 2.2f * scale, center = Offset(cx + 5.5f * scale, height / 2f + 3f * scale), style = Fill)
    drawLine(color = tint, start = Offset(cx - 1.2f * scale, height / 2f - 5f * scale), end = Offset(cx - 4.3f * scale, height / 2f + 1.5f * scale), strokeWidth = 1.4f * scale)
    drawLine(color = tint, start = Offset(cx + 1.2f * scale, height / 2f - 5f * scale), end = Offset(cx + 4.3f * scale, height / 2f + 1.5f * scale), strokeWidth = 1.4f * scale)
}

/** The reader's mark: a page's own words as lines, what reading mode makes of them. */
internal fun DrawScope.drawReaderGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    drawLine(color = tint, start = Offset(3f * scale, height / 2f - 6f * scale), end = Offset(17f * scale, height / 2f - 6f * scale), strokeWidth = 1.5f * scale)
    drawLine(color = tint, start = Offset(3f * scale, height / 2f - 1.5f * scale), end = Offset(13f * scale, height / 2f - 1.5f * scale), strokeWidth = 1.5f * scale)
    drawLine(color = tint, start = Offset(3f * scale, height / 2f + 3f * scale), end = Offset(15f * scale, height / 2f + 3f * scale), strokeWidth = 1.5f * scale)
    drawLine(color = tint, start = Offset(3f * scale, height / 2f + 7.5f * scale), end = Offset(10f * scale, height / 2f + 7.5f * scale), strokeWidth = 1.5f * scale)
}

/** The settings mark: a cog, as the Android settings glyph is. */
internal fun DrawScope.drawSettingsGlyph(tint: Color, width: Float, height: Float) {
    val scale = width / 20f
    val cx = width / 2f
    val cy = height / 2f
    drawCircle(color = tint, radius = 3f * scale, center = Offset(cx, cy), style = Stroke(1.6f * scale))
    // The cog's teeth as strokes out from its rim.
    for (angle in 0 until 360 step 45) {
        val rad = Math.toRadians(angle.toDouble())
        val from = 4.5f * scale
        val to = 6.5f * scale
        drawLine(
            color = tint,
            start = Offset(cx + (from * Math.cos(rad)).toFloat(), cy + (from * Math.sin(rad)).toFloat()),
            end = Offset(cx + (to * Math.cos(rad)).toFloat(), cy + (to * Math.sin(rad)).toFloat()),
            strokeWidth = 1.6f * scale,
        )
    }
}