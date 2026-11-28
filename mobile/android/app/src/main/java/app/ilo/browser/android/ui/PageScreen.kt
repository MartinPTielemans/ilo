package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.times
import androidx.compose.ui.viewinterop.AndroidView
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Address
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.ExternalDecision
import app.ilo.browser.android.browser.PendingAsk
import app.ilo.browser.android.browser.SitePermission
import app.ilo.browser.android.browser.Tab

/**
 * A page, edge to edge, with the bar at its foot. The bar is one rounded region with the
 * deck of the Space's pages as a stack of their pictures on its own plate at the left, the
 * page's title beside it on its own plate with room above, and an (i) when the page didn't
 * load cleanly, and the + and the ... as small marks on the bar's own ground. The title
 * opens the URL field, which works as a search box. A tap on the deck opens the
 * strip of the Space's pages over the page, its cards carrying the pictures the browser kept.
 * While the page is read the bar folds to a pill with the site's name and the page runs under
 * it, the fold's state belonging to the browser so the pill survives a tab switch and the bar
 * comes back on a scroll up. A certificate the phone doesn't trust stands in the page's place,
 * its words at the foot over the Space's ground.
 */
@Composable
fun PageScreen(
    browser: Browser,
    tab: Tab,
    onLeave: () -> Unit,
    findingParam: Boolean,
    onFindDone: () -> Unit,
    onOpenSwitcher: () -> Unit,
    onOpenSettings: () -> Unit = {},
) {
    val base = LocalLook.current
    val spaces by browser.state.spaces.collectAsState()
    val currentSpace by browser.state.currentSpace.collectAsState()
    val faults by browser.state.tabFaults.collectAsState()
    val fault = faults[tab.id]
    val spaceColor = spaces.firstOrNull { it.id == currentSpace }?.color
    val look = Look.of(base.dark, spaceColor, isPrivate = tab.isPrivate, pageColor = tab.pageColor)
    val tucked by browser.state.barTucked.collectAsState()
    val taskbar = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    var urlOpen by remember { mutableStateOf(false) }
    var toolsOpen by remember { mutableStateOf(false) }
    var listOpen by remember { mutableStateOf<String?>(null) }
    // Whether the next field-open creates a tab, as the + asks.
    var plusNext by remember { mutableStateOf(false) }
    // The empty tab the + made, its page to be: the tab exists from the press, so the deck
    // counts it and a restart keeps it, and the field only says where it goes.
    var emptyTab by remember { mutableStateOf<Tab?>(null) }
    // The tools' own Find row opens the find bar here, its way out closing it.
    var findingHere by remember { mutableStateOf(false) }
    val finding = findingParam || findingHere
    // The page honors the taskbar always, and the bar while the bar is up; folded away, the pill
    // draws above the content instead. One spring, so the page tracks the bar's own fold.
    val pageBottom by animateDpAsState(
        targetValue = if (tucked) taskbar else taskbar + Metrics.Bar.gap + Metrics.Bar.height,
        animationSpec = spring(dampingRatio = 0.8f, stiffness = 380f),
        label = "pageBottom",
    )

    if (fault != null) {
        // The warning stands where the page would: back is the safe way out of it.
        BackHandler { browser.answerSsl(tab, fault, false) }
    } else if (!urlOpen && !toolsOpen && listOpen == null && !finding) {
        BackHandler(enabled = browser.engine.canGoBack(tab)) { browser.engine.goBack(tab) }
    }
    if (finding) BackHandler { findingHere = false; onFindDone() }

    Box(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
    ) {
        // The page starts under the status bar's own band, as iOS's page does: the ground shows
        // behind the bar and the page keeps the area it measures for what it draws.
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            // Keyed by the tab, so a tab switch trades the page for the new tab's own view:
            // the AndroidView's factory runs only when it enters composition, so without the
            // key a new tab opened over a page keeps the old one's view on screen.
            key(tab.id) {
                // The pull follows the bar's own progress flow, so the spinner shows only a
                // load the pull started and ends when the page has settled.
                val progress by browser.progressFlow.collectAsState()
                val userPulled = remember(tab.id) { mutableStateOf(false) }
                val atTop by browser.state.pageAtTop.collectAsState()
                if (progress >= 100) userPulled.value = false
                // Chrome's own pull: a drag down at the page's top becomes a refresh once the
                // icon has traveled from the page's top edge to the page's middle, where the
                // owner set its trip point, and it goes on spinning until the page has settled.
                val pullDistance = remember(tab.id) { mutableStateOf(0f) }
                // The pull's distances are pixels, the trip point half the page, so the height
                // converts where it is known and the threshold follows the page's own size.
                val boxHeightPx = remember(tab.id) { mutableStateOf(0f) }
                val pullThresholdPx = boxHeightPx.value / 2f
                // The connection reads the threshold from the state each time, since it is made
                // before the box has its size and would otherwise keep the zero it was born with.
                val pullConnection = remember(tab.id) {
                    object : NestedScrollConnection {
                        override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
                            if (source != NestedScrollSource.UserInput) return Offset.Zero
                            if (!atTop || available.y <= 0f) {
                                if (available.y < 0f) pullDistance.value = 0f
                                return Offset.Zero
                            }
                            val threshold = boxHeightPx.value / 2f
                            val taken = available.y.coerceAtMost(threshold * 2f - pullDistance.value)
                            if (taken > 0f) pullDistance.value += taken
                            return Offset(0f, taken)
                        }
                        override suspend fun onPreFling(available: Velocity): Velocity {
                            // A fast flick refreshes with less pull; a slow drag pays half the page.
                            val threshold = swipeCommitTravel(boxHeightPx.value, available.y, boxHeightPx.value * 0.5f)
                            val fired = threshold > 0f && pullDistance.value >= threshold
                            pullDistance.value = 0f
                            if (fired) {
                                userPulled.value = true
                                browser.engine.reload(tab)
                            }
                            return Velocity.Zero
                        }
                    }
                }
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .nestedScroll(pullConnection)
                        .onSizeChanged { boxHeightPx.value = it.height.toFloat() },
                ) {
                    AndroidView(
                        factory = { browser.engine.pageHost(it, tab) },
                        update = {},
                        modifier = Modifier
                            .fillMaxHeight()
                            .fillMaxWidth()
                            // The WebView paints its empty background past its bounds until a page
                            // commits, so it stays where the page sits and the ground shows below.
                            .clipToBounds(),
                    )
                    // The reload icon rides the pull from the page's top edge down toward
                    // its middle, and turns on its own once the trip point fires the reload.
                    val spinning = progress in 1..99 && userPulled.value
                    if (pullDistance.value > 0f || spinning) {
                        Box(
                            Modifier
                                .align(Alignment.TopCenter)
                                .offset { IntOffset(0, (pullDistance.value.coerceAtMost(pullThresholdPx)).toInt()) }
                                .size(28.dp)
                                .clip(CircleShape)
                                .background(look.overlay),
                            contentAlignment = Alignment.Center,
                        ) {
                            if (spinning) {
                                CircularProgressIndicator(
                                    modifier = Modifier.size(20.dp),
                                    color = look.accent,
                                )
                            } else {
                                CircularProgressIndicator(
                                    progress = {
                                        if (pullThresholdPx > 0f) (pullDistance.value / pullThresholdPx).coerceIn(0f, 1f) else 0f
                                    },
                                    modifier = Modifier.size(20.dp),
                                    color = look.accent,
                                )
                            }
                        }
                    }
                }
            }
            Spacer(Modifier.height(pageBottom))
        }
        fault?.let {
            // What stands in a page's place, over the Space's own ground as iOS's StatePage is.
            Box(
                Modifier
                    .fillMaxSize()
                    .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
            ) {
                Column(
                    Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .padding(bottom = pageBottom + 16.dp)
                ) {
                    CertificateSheet(
                        fault = it,
                        url = tab.url,
                        look = look,
                        onAnswer = { allow -> browser.answerSsl(tab, it, allow) },
                    )
                }
            }
        }
        val asks by browser.state.tabAsks.collectAsState()
        asks[tab.id]?.let { ask ->
            // The ask stands over the page itself, unlike the certificate's stand-in, so the
            // page stays under it and a back press is the same as Don't allow.
            BackHandler { browser.answerSite(tab, ask, false) }
            Box(
                Modifier
                    .fillMaxSize()
                    .background(Look.Ink.copy(alpha = 0.45f))
            ) {
                Column(
                    Modifier
                        .align(Alignment.Center)
                        .fillMaxWidth()
                        .padding(horizontal = Metrics.Column.side + Metrics.Column.inset)
                        .background(look.groundTop, RoundedCornerShape(Metrics.Menu.radius))
                        .padding(20.dp)
                ) {
                    AskSheet(ask, look, onAnswer = { allow -> browser.answerSite(tab, ask, allow) })
                }
            }
        }
        if (fault == null && tab.isPrivate && tab.url == null && !urlOpen && !toolsOpen && listOpen == null) {
            // A new private tab says what it is, once, where its page will be: ink over the
            // white an empty WebView shows, the words at the foot as iOS's PrivateWords are.
            Column(
                Modifier
                    .fillMaxSize()
                    .background(Look.Ink)
            ) {
                Spacer(Modifier.weight(1f))
                Column(
                    Modifier
                        .fillMaxWidth()
                        .padding(bottom = pageBottom + Metrics.PrivateMark.overField)
                ) {
                    PrivateWords(look)
                }
            }
        }
        PageBar(
            browser = browser,
            tab = tab,
            look = look,
            onLeave = {
                // The card keeps what the page last showed, as iOS's leave takes its picture.
                browser.keepPicture(tab)
                onLeave()
            },
            onDeck = onOpenSwitcher,
            onField = { urlOpen = true },
            onTools = { toolsOpen = true },
            onPlus = {
                // The tab exists from the press, Chrome-style, with nothing in it as iOS's does:
                // the count goes up now, the field says where it goes, and no page loads until
                // the words do. The start page the user set stays what Settings' "page" choice
                // and the empty Space open, not what + preloads.
                emptyTab = browser.state.newTab(isPrivate = tab.isPrivate)
                urlOpen = true
            },
            fault = fault,
            modifier = Modifier.align(Alignment.BottomCenter),
        )
        // Both asks stand over the bar as well as the page, so the dim covers it too:
        // drawn here, after the bar, the scrim reaches everything behind the ask.
        val pressedLink by browser.pressedLink.collectAsState()
        pressedLink?.let { link ->
            // The link's sheet stands over the page as the ask does, and a back press is
            // the same as leaving it alone.
            BackHandler { browser.dismissPressedLink() }
            Box(
                Modifier
                    .fillMaxSize()
                    .background(Look.Ink.copy(alpha = 0.45f))
                    .clickable(role = Role.Button) { browser.dismissPressedLink() }
            ) {
                Column(
                    Modifier
                        .align(Alignment.Center)
                        .fillMaxWidth()
                        .padding(horizontal = Metrics.Column.side + Metrics.Column.inset)
                        .background(look.groundTop, RoundedCornerShape(Metrics.Menu.radius))
                        .padding(20.dp)
                ) {
                    LinkSheet(link, look, onOpen = { private -> browser.openPressedLink(private) })
                }
            }
        }
        val externalAsk by browser.externalAsk.collectAsState()
        externalAsk?.let { ask ->
            // The app ask stands over the page as the link sheet does, and a back press is
            // the same as leaving it alone.
            BackHandler { browser.dismissExternal() }
            Box(
                Modifier
                    .fillMaxSize()
                    .background(Look.Ink.copy(alpha = 0.45f))
                    .clickable(role = Role.Button) { browser.dismissExternal() }
            ) {
                Column(
                    Modifier
                        .align(Alignment.Center)
                        .fillMaxWidth()
                        .padding(horizontal = Metrics.Column.side + Metrics.Column.inset)
                        .background(look.groundTop, RoundedCornerShape(Metrics.Menu.radius))
                        .padding(20.dp)
                ) {
                    OpenInAppSheet(
                        decision = ExternalDecision.OpenInApp(ask.url, ask.guess, ask.fallbackUrl),
                        appName = ask.appName,
                        icon = ask.icon,
                        look = look,
                        onAnswer = { open -> browser.answerExternal(open) },
                    )
                }
            }
        }
        if (urlOpen) {
            // The field's words and kind start over when + turns the modal into a new page's, so
            // the panel never reuses the page behind it as its own.
            key(plusNext) {
            UrlModal(
                browser = browser,
                // The + made its tab already; the field stands on it, saying where it goes.
                tab = emptyTab ?: tab,
                baseLook = look,
                createsTab = false,
                startWords = if (emptyTab != null) "" else tab.url ?: "",
                onClose = { urlOpen = false; plusNext = false; emptyTab = null },
                // The panel's header acts on the page behind it: its deck opens the switcher,
                // its dots the same menu the bar's dots open.
                onDeck = onOpenSwitcher,
                onTools = { toolsOpen = true },
            )
            }
        }
        if (toolsOpen) {
            ToolsModal(
                browser = browser,
                // The panel's dots stand for the page to be, so with it up the menu is the
                // new page's own, with no page rows behind it.
                tab = if (urlOpen) null else tab,
                look = look,
                onNewTab = { isPrivate ->
                    toolsOpen = false
                    // The tab exists from the row, with nothing in it as iOS's does; its kind is
                    // the row asked for, and the field says where it goes.
                    emptyTab = browser.state.newTab(isPrivate = isPrivate)
                    if (isPrivate) browser.state.privateMode.value = true
                    urlOpen = true
                },
                onOpenHistory = { toolsOpen = false; listOpen = "history" },
                onOpenBookmarks = { toolsOpen = false; listOpen = "bookmarks" },
                onOpenDownloads = { toolsOpen = false; listOpen = "downloads" },
                onOpenSiteSettings = { toolsOpen = false; listOpen = "site" },
                onOpenSettings = { toolsOpen = false; onOpenSettings() },
                onFind = { findingHere = true; toolsOpen = false },
                onClose = { toolsOpen = false },
            )
        }
        if (listOpen != null) {
            ListScreen(
                which = listOpen!!,
                browser = browser,
                look = look,
                tab = tab,
                onOpen = { url -> browser.go(url); listOpen = null; toolsOpen = false },
                onClose = { listOpen = null },
            )
        }
        if (finding) {
            // Both flags go down together, as the bar's two ways in share one way out.
            FindBar(browser, tab, look) {
                findingHere = false
                onFindDone()
            }
        }
        LoginLayer(browser, tab, look, pageBottom, clear = fault == null && !urlOpen && !toolsOpen && listOpen == null && !finding)
    }
}

/**
 * The passwords' offers over the page, as iOS's LoginLayer is: a chip at the foot to save
 * the login the page was signed in with, or to update the one the account has, and one to
 * sign in with a saved login when the page shows a field. Nothing shows over a sheet, a
 * modal or the find bar; the save chip says it's saved once the account took it, and goes
 * with the bar when the page is tucked.
 */
@Composable
private fun LoginLayer(
    browser: Browser,
    tab: Tab,
    look: Look,
    pageBottom: androidx.compose.ui.unit.Dp,
    clear: Boolean,
) {
    // A private page keeps nothing, and only a real page carries offers: its address field's
    // words and an empty tab are no page.
    if (!clear || tab.isPrivate || !tab.url.orEmpty().startsWith("http")) return
    val lane = browser.passwords.of(tab.id)
    val tucked by browser.state.barTucked.collectAsState()
    val offer by lane.save.offer.collectAsState()

    // The chip leaves for the sheet's row when its time passes, as iOS's does, and "Saved"
    // goes too.
    LaunchedEffect(offer, tab.id) {
        val next = lane.save.next() ?: return@LaunchedEffect
        kotlinx.coroutines.delay((next - System.currentTimeMillis()).coerceAtLeast(0))
        lane.save.tick()
    }
    // The chip never sits on the page's own foot: it goes with the bar at the first scroll.
    LaunchedEffect(tucked) { if (tucked) lane.save.putAway() }

    // Asked for again each recomposition, since the never-list can change while it waits.
    val shown = if (!tucked) lane.shownOffer() else null
    val logins = if (!tucked && shown == null) lane.fillOffers(tab.url) else emptyList()

    Column(
        Modifier
            .fillMaxWidth()
            .padding(bottom = pageBottom + Metrics.Bar.gap),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (shown != null && shown.phase != app.ilo.browser.android.browser.LoginSave.Phase.ROW) {
            SaveChip(lane, shown, look)
        } else if (logins.isNotEmpty()) {
            LoginChip(browser, lane, logins, look, tab)
        }
    }
}

/** The chip that offers to keep the login, with its three ways out. */
@Composable
private fun SaveChip(
    lane: app.ilo.browser.android.browser.PasswordsLane.TabLogins,
    offer: app.ilo.browser.android.browser.LoginSave.Offer,
    look: Look,
) {
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Bar.side)
            .background(look.overlay, RoundedCornerShape(Metrics.Bar.radius))
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        if (offer.phase == app.ilo.browser.android.browser.LoginSave.Phase.SAVED) {
            Text(
                text = stringResource(R.string.password_saved),
                color = look.text,
                fontSize = Type.small,
            )
            return@Column
        }
        val site = hostOf(offer.login.origin)
        Text(
            text = when (offer.kind) {
                app.ilo.browser.android.browser.LoginKind.SAVE ->
                    stringResource(R.string.save_password_for, site)
                app.ilo.browser.android.browser.LoginKind.UPDATE ->
                    stringResource(R.string.update_password_for, site)
            },
            color = look.text,
            fontSize = Type.small,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(
                text = stringResource(R.string.save_password),
                color = look.accent,
                fontSize = Type.small,
                fontWeight = FontWeight.Medium,
                modifier = Modifier
                    .clickable(role = Role.Button) { lane.takeOffer() }
                    .padding(vertical = 10.dp),
            )
            Text(
                text = stringResource(R.string.not_now),
                color = look.accent,
                fontSize = Type.small,
                modifier = Modifier
                    .clickable(role = Role.Button) { lane.putAwayOffer() }
                    .padding(vertical = 10.dp),
            )
            Text(
                text = stringResource(R.string.never_for_this_site),
                color = look.soft,
                fontSize = Type.small,
                modifier = Modifier
                    .clickable(role = Role.Button) { lane.neverSave() }
                    .padding(vertical = 10.dp),
            )
        }
    }
}

/** The chip that offers a saved sign-in; several open a short list from the foot. */
@Composable
private fun LoginChip(
    browser: Browser,
    lane: app.ilo.browser.android.browser.PasswordsLane.TabLogins,
    logins: List<app.ilo.browser.android.sync.SavedLogin>,
    look: Look,
    tab: Tab,
) {
    var choosing by remember(tab.id) { mutableStateOf(false) }
    if (choosing) {
        Column(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = Metrics.Bar.side)
                .background(look.overlay, RoundedCornerShape(Metrics.Bar.radius))
                .padding(vertical = 6.dp),
        ) {
            Text(
                text = stringResource(R.string.sign_in_to, hostOf(tab.url)),
                color = look.soft,
                fontSize = Type.small,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            for (login in logins) {
                Text(
                    text = if (login.username.isEmpty()) stringResource(R.string.no_username) else login.username,
                    color = look.text,
                    fontSize = Type.small,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(role = Role.Button) {
                            choosing = false
                            fillLogin(browser, lane, login, tab)
                        }
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                )
            }
        }
        Spacer(Modifier.height(Metrics.Bar.gap))
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Bar.side)
            .background(look.overlay, RoundedCornerShape(Metrics.Bar.radius))
            .clickable(role = Role.Button) {
                if (logins.size > 1) choosing = true else fillLogin(browser, lane, logins.first(), tab)
            }
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Text(
            text = when {
                logins.size > 1 -> pluralStringResource(R.plurals.saved_signins, logins.size, logins.size)
                logins.first().username.isEmpty() -> stringResource(R.string.sign_in_saved)
                else -> stringResource(R.string.sign_in_as, logins.first().username)
            },
            color = look.text,
            fontSize = Type.small,
            modifier = Modifier.weight(1f),
        )
    }
}

/** A saved login goes into the page's fields once the owner confirmed, as iOS's fill does:
 * the lane reads the password for this call alone and hands the engine the script, so it is
 * in no script's text and kept nowhere. */
private fun fillLogin(
    browser: Browser,
    lane: app.ilo.browser.android.browser.PasswordsLane.TabLogins,
    login: app.ilo.browser.android.sync.SavedLogin,
    tab: Tab,
) {
    val confirmed = { script: String ->
        lane.fill.putAwayOffer()
        browser.engine.runInPage(tab, script)
    }
    val asker = browser.fillConfirmer ?: return
    asker(hostOf(tab.url)) {
        lane.fillScript(login, tab.url)?.let(confirmed) ?: lane.fill.putAwayOffer()
    }
}

/**
 * The page's own find bar, what the actions sheet's Find opens: the words in a field at the
 * foot, the count beside it, and the way through the matches and out. It runs on the WebView's
 * own find, which highlights the matches on the page behind it.
 */
@Composable
private fun FindBar(
    browser: Browser,
    tab: Tab,
    look: Look,
    onClose: () -> Unit,
) {
    var query by remember { mutableStateOf("") }
    val count by browser.state.findCount.collectAsState()
    val focus = remember { FocusRequester() }

    LaunchedEffect(Unit) { focus.requestFocus() }
    // The highlights go when the bar closes, as the page's own find leaves nothing behind.
    DisposableEffect(tab.id) {
        onDispose { browser.engine.findDone(tab) }
    }

    Column(
        Modifier
            .fillMaxSize()
            .clickable(role = Role.Button, onClick = onClose)
            .imePadding(),
    ) {
        Box(Modifier.weight(1f))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = Metrics.Bar.side)
                .navigationBarsPadding()
                .padding(bottom = Metrics.Bar.gap)
                .height(Metrics.Bar.height + 8.dp)
                .background(look.overlay, RoundedCornerShape(Metrics.Bar.radius))
                .padding(horizontal = 16.dp),
        ) {
            BasicField(
                value = query,
                onValueChange = {
                    query = it
                    browser.engine.findAll(tab, it)
                },
                hint = stringResource(R.string.find_in_page),
                look = look,
                modifier = Modifier
                    .weight(1f)
                    .focusRequester(focus),
            )
            if (query.isNotEmpty()) {
                Text(
                    text = count.toString(),
                    color = look.soft,
                    fontSize = Type.small,
                    modifier = Modifier.padding(horizontal = 8.dp),
                )
                BarButton(
                    glyph = Glyph.CHEVRON_DOWN,
                    label = stringResource(R.string.previous_match),
                    look = look,
                    onClick = { browser.engine.findNext(tab, false) },
                )
                BarButton(
                    glyph = Glyph.CHEVRON_UP,
                    label = stringResource(R.string.next_match),
                    look = look,
                    onClick = { browser.engine.findNext(tab, true) },
                )
            }
            BarButton(
                glyph = Glyph.CLOSE,
                label = stringResource(R.string.close),
                look = look,
                onClick = onClose,
            )
        }
    }
}

/**
 * The bar at the page's foot, as Chrome's is: one rounded region with the deck on its own
 * plate at its left, the field's title on the plate that fills the rest with room above it,
 * and the + and the ... as small marks on the bar's own ground. Tucked, it's the small
 * pill with the site's name, and a tap anywhere on it brings the bar back.
 */
@Composable
private fun PageBar(
    browser: Browser,
    tab: Tab,
    look: Look,
    onLeave: () -> Unit,
    onDeck: () -> Unit,
    onField: () -> Unit,
    onTools: () -> Unit,
    onPlus: () -> Unit,
    fault: app.ilo.browser.android.browser.CertificateFault?,
    modifier: Modifier = Modifier,
) {
    val progress by browser.progressFlow.collectAsState()
    val tucked by browser.state.barTucked.collectAsState()
    // The address's own menu, what a long-press on the field opens: its link on the clipboard,
    // or in Android's share sheet, the same two the tools menu's Share does.
    var addressMenu by remember { mutableStateOf(false) }
    val context = androidx.compose.ui.platform.LocalContext.current
    if (addressMenu) {
        androidx.compose.ui.window.Dialog(onDismissRequest = { addressMenu = false }) {
            Column(
                Modifier
                    .background(look.groundTop, RoundedCornerShape(Metrics.Menu.radius))
                    .padding(vertical = 8.dp)
            ) {
                val url = tab.url
                MenuTextRow(stringResource(R.string.copy_link), look) {
                    if (url != null) {
                        val clip = android.content.ClipData.newPlainText("", url)
                        context.getSystemService(android.content.ClipboardManager::class.java)
                            ?.setPrimaryClip(clip)
                    }
                    addressMenu = false
                }
                MenuTextRow(stringResource(R.string.share), look) {
                    addressMenu = false
                    if (url != null) {
                        val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(android.content.Intent.EXTRA_TEXT, url)
                            putExtra(android.content.Intent.EXTRA_SUBJECT, tab.title)
                        }
                        context.startActivity(android.content.Intent.createChooser(send, null))
                    }
                }
            }
        }
    }
    val height by animateDpAsState(
        targetValue = if (tucked) Metrics.Bar.small else Metrics.Bar.height,
        animationSpec = spring(dampingRatio = 0.8f, stiffness = 380f),
        label = "bar",
    )
    // The load's wave runs along the bar's foot, left to right, as iOS's Morphed line does:
    // its own color at the load's own pace, there only while a load is on its way, and gone
    // at the finish, which the page itself says, not WebView's early 100.
    val loading = progress in 1..99

    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            // Full up, the bar spans the whole width as the owner drew it; tucked, it keeps
            // its small pill's side room, so it stays a pill and not the full-width band.
            .padding(horizontal = if (tucked) Metrics.Bar.side else 0.dp, vertical = Metrics.Bar.gap)
            .height(height)
            .background(look.tinted, RoundedCornerShape(Metrics.Bar.radius))
            .clip(RoundedCornerShape(Metrics.Bar.radius))
            .drawBehind {
                if (loading) {
                    drawRect(
                        color = look.glassText.copy(alpha = 0.8f),
                        size = Size(size.width * (progress / 100f), Metrics.Bar.loadedLine.toPx()),
                        topLeft = Offset(0f, size.height - Metrics.Bar.loadedLine.toPx()),
                    )
                }
            },
    ) {
        if (tucked) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(role = Role.Button) { browser.state.barTucked.value = false }
                    .padding(horizontal = Metrics.Bar.smallInset),
            ) {
                if (tab.isPrivate) {
                    // The pill says what the tab is, as iOS's HostLabel carries the mark at both sizes.
                    Canvas(Modifier.size(Metrics.PrivateMark.onBar)) {
                        drawIncognitoGlyph(look.glassText, size.width, size.height)
                    }
                    Spacer(Modifier.width(Metrics.PrivateMark.barGap))
                }
                Text(
                    // A private tab with no page yet is called what it is, as iOS's HostLabel does.
                    text = tab.title.ifEmpty { tab.url ?: if (tab.isPrivate) stringResource(R.string.private_tab) else "" },
                    color = look.glassText,
                    fontSize = Type.small,
                    maxLines = 1,
                )
            }
        } else {
            // The deck stands at the bar's leading edge, on the plate that grew from it.
            Box(
                modifier = Modifier
                    .padding(start = Metrics.Bar.side, top = Metrics.Bar.gap, bottom = Metrics.Bar.gap)
                    .background(look.plate, RoundedCornerShape(Metrics.Bar.radius - Metrics.Bar.gap))
                    .padding(horizontal = 2.dp),
            ) {
                TabDeck(
                    browser = browser,
                    tab = tab,
                    look = look,
                    onOpenStrip = onDeck,
                    onLeave = onLeave,
                )
            }
            Spacer(Modifier.size(Metrics.Bar.gap))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxHeight()
                    // More room above the words, so the label's plate keeps its border at the top.
                    .padding(top = Metrics.Bar.gap, bottom = Metrics.Bar.gap)
                    // While the page loads, the plate says it as its own color, light at the
                    // load's start and the Space's color washed in at its end, as the owner
                    // drew it in place of a line.
                    .background(
                        if (progress in 1..99) androidx.compose.ui.graphics.lerp(
                            look.plate, look.accent.copy(alpha = 0.40f), progress / 100f,
                        ) else look.plate,
                        RoundedCornerShape(Metrics.Bar.radius - Metrics.Bar.gap),
                    )
                    .combinedClickable(role = Role.Button, onClick = onField, onLongClick = { addressMenu = true })
                    .padding(horizontal = 10.dp),
            ) {
                if (tab.isPrivate) {
                    // The one glyph before the field's host, as iOS's HostLabel carries it.
                    Canvas(Modifier.size(Metrics.PrivateMark.onBar)) {
                        drawIncognitoGlyph(look.glassText, size.width, size.height)
                    }
                    Spacer(Modifier.width(Metrics.PrivateMark.barGap))
                }
                if (fault != null) {
                    // A page that didn't load cleanly carries its (i) beside the title, as Chrome's does.
                    val infoName = stringResource(R.string.page_info)
                    Canvas(
                        Modifier
                            .size(16.dp)
                            .semantics { contentDescription = infoName }
                    ) {
                        drawInfoGlyph(look.glassText, size.width, size.height)
                    }
                    Spacer(Modifier.width(Metrics.PrivateMark.barGap))
                }
                Text(
                    // The field names the page, its title where it has one as Chrome's bar does.
                    text = tab.title.ifEmpty { tab.url ?: if (tab.isPrivate) stringResource(R.string.private_tab) else "" },
                    color = look.glassText,
                    fontSize = Type.bar,
                    maxLines = 1,
                    modifier = Modifier.weight(1f),
                )
            }
            // The + and the ... sit at the bar's very end, their plates flush with the edge.
            BarButton(
                glyph = Glyph.PLUS,
                label = stringResource(R.string.new_tab),
                look = look,
                onClick = onPlus,
            )
            BarButton(
                glyph = Glyph.MORE,
                label = stringResource(R.string.more),
                look = look,
                onClick = onTools,
                modifier = Modifier.padding(end = Metrics.Bar.side),
            )
        }
    }
}

/** One of the address menu's own rows: its words, one press, nothing else. */
@Composable
private fun MenuTextRow(label: String, look: Look, onClick: () -> Unit) {
    Text(
        label,
        color = look.text,
        fontSize = 16.sp,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 12.dp),
    )
}

/**
 * The deck at the field's left end: the Space's pages of the bar's tab's kind as a stack of
 * their pictures, with the count beside them. A private page's deck keeps the tabs glyph, as
 * its pages are never pictured. A tap opens the strip of the Space's pages over the page; a
 * long press goes to the Space, the way the deck always went.
 */
@Composable
private fun TabDeck(
    browser: Browser,
    tab: Tab,
    look: Look,
    onOpenStrip: () -> Unit,
    onLeave: () -> Unit,
) {
    val tabs by browser.state.tabs.collectAsState()
    val pictures by browser.pictures.version.collectAsState()
    // The Space's tabs of the page's kind, as the Space's grid separates them.
    val deckTabs = remember(tabs, tab) { tabs.filter { it.space == tab.space && it.isPrivate == tab.isPrivate } }
    // The button's spoken name, hoisted: the semantics block runs where composables can't.
    val deckName = if (tab.isPrivate) {
        pluralStringResource(R.plurals.deck_tabs_private, deckTabs.size, deckTabs.size)
    } else {
        pluralStringResource(R.plurals.deck_tabs, deckTabs.size, deckTabs.size)
    }
    // The pages nearest the top of the deck, newest first like the Space's grid.
    val deck = remember(deckTabs, pictures) {
        deckTabs.filter { !it.isPrivate }
            .sortedByDescending { deckTabs.indexOf(it) }
            .take(3)
            .mapNotNull { browser.pictures.picture(of = it.id) }
    }
    Box(
        modifier = Modifier
            .size(Metrics.Bar.height)
            .combinedClickable(role = Role.Button, onClick = onOpenStrip, onLongClick = onLeave)
            .semantics {
                contentDescription = deckName
            },
        contentAlignment = Alignment.Center,
    ) {
        if (deck.isEmpty()) {
            // The tabs glyph is what the button is, a private tab's as any other's.
            Canvas(Modifier.size(30.dp)) { drawTabsGlyph(look.glassText, size.width, size.height) }
        } else {
            // The front card is the newest page; each one behind it stands a little up and over.
            deck.asReversed().forEachIndexed { index, picture ->
                val lift = (deck.size - 1 - index) * 2.dp
                Box(
                    Modifier
                        .align(Alignment.Center)
                        .offset(y = lift)
                        .size(24.dp, 30.dp)
                        .background(look.overlay, RoundedCornerShape(6.dp))
                        .border(1.dp, Look.Paper, RoundedCornerShape(6.dp))
                        .clip(RoundedCornerShape(6.dp))
                ) {
                    Image(
                        bitmap = picture.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
        }
        Box(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(1.dp)
                .size(18.dp)
                .background(look.plate, RoundedCornerShape(9.dp)),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = deckTabs.size.toString(),
                color = look.glassText,
                fontSize = 10.sp,
                maxLines = 1,
            )
        }
    }
}

/** A new private tab's words: the one glyph with the heading, and what private means here,
 * both starting where the field does, as the iOS PrivateWords are. */
@Composable
private fun PrivateWords(look: Look) {
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.PrivateMark.side)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Canvas(Modifier.size(Metrics.PrivateMark.headingGlyph)) {
                drawIncognitoGlyph(look.glassText, size.width, size.height)
            }
            Spacer(Modifier.width(Metrics.PrivateMark.headingGap))
            Text(
                text = stringResource(R.string.private_tab),
                color = look.glassText,
                fontSize = Metrics.PrivateMark.heading,
                fontWeight = FontWeight.SemiBold,
            )
        }
        Spacer(Modifier.height(Metrics.PrivateMark.wordsGap))
        Text(
            text = stringResource(R.string.private_tab_words),
            color = look.soft,
            fontSize = 16.sp,
        )
    }
}


/** One of the bar's own buttons, on the bar's ground rather than a plate of its own. */
@Composable
private fun BarButton(
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
            .clickable(enabled = !dimmed, role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Box(Modifier.size(22.dp), contentAlignment = Alignment.Center) {
            // PlateButton's glyphs at 22dp instead of 34dp, so every bar mark matches its plate sibling.
            when (glyph) {
                Glyph.TABS -> Canvas(Modifier.size(22.dp)) { drawTabsGlyph(tint, size.width, size.height) }
                Glyph.INCOGNITO -> Canvas(Modifier.size(22.dp)) { drawIncognitoGlyph(tint, size.width, size.height) }
                Glyph.CHEVRON_UP -> Icon(Icons.Filled.KeyboardArrowUp, null, tint = tint)
                Glyph.CHEVRON_DOWN -> Icon(Icons.Filled.KeyboardArrowDown, null, tint = tint)
                Glyph.MAGNIFIER -> Icon(Icons.Filled.Search, null, tint = tint)
                Glyph.CLOSE -> Icon(Icons.Filled.Close, null, tint = tint)
                Glyph.BACK -> Icon(Icons.Filled.ArrowBack, null, tint = tint)
                Glyph.FORWARD -> Icon(Icons.Filled.ArrowForward, null, tint = tint)
                Glyph.REFRESH -> Icon(Icons.Filled.Refresh, null, tint = tint)
                Glyph.PLUS -> Icon(Icons.Filled.Add, null, tint = tint)
                Glyph.SHIELD -> Canvas(Modifier.size(22.dp)) { drawShieldGlyph(tint, size.width, size.height) }
                Glyph.SETTINGS -> Icon(Icons.Filled.Settings, null, tint = tint)
                Glyph.DESKTOP -> Canvas(Modifier.size(22.dp)) { drawDesktopGlyph(tint, size.width, size.height) }
                Glyph.BROKEN_LOCK -> Canvas(Modifier.size(22.dp)) { drawBrokenLockGlyph(tint, size.width, size.height) }
                Glyph.MORE -> Icon(Icons.Filled.MoreVert, null, tint = tint)
            }
        }
    }
}

/**
 * A long-pressed link's address with the two ways ilo opens it elsewhere, as Chrome's own
 * long-press menu offers them: a new tab, or a new private one. The address is shown so the
 * user can see where each goes before choosing.
 */
@Composable
private fun LinkSheet(
    link: String,
    look: Look,
    onOpen: (private: Boolean) -> Unit,
) {
    Text(
        text = link,
        color = look.text,
        fontSize = 18.sp,
        fontWeight = FontWeight.SemiBold,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.padding(bottom = 20.dp),
    )
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        listOf(
            false to stringResource(R.string.open_in_new_tab),
            true to stringResource(R.string.open_in_new_private_tab),
        ).forEach { (private, label) ->
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = Metrics.Bar.height)
                    .background(if (private) look.plate else look.button, RoundedCornerShape(Metrics.Bar.radius / 2))
                    .clickable(role = Role.Button, onClick = { onOpen(private) })
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) {
                Text(text = label, color = look.text, fontSize = 16.sp)
            }
        }
    }
}

/**
 * A permission ask standing over a page, as Chrome's own ask does: the sentence naming the
 * host and what it wants, then the two ways to answer. The answer is the user's for the
 * site, kept by the store, not only for the request standing here.
 */
@Composable
private fun AskSheet(
    ask: PendingAsk,
    look: Look,
    onAnswer: (allow: Boolean) -> Unit,
) {
    Text(
        text = when (ask.kind) {
            SitePermission.LOCATION -> stringResource(R.string.ask_location, ask.host)
            SitePermission.CAMERA -> stringResource(R.string.ask_camera, ask.host)
            SitePermission.MICROPHONE -> stringResource(R.string.ask_microphone, ask.host)
            SitePermission.NOTIFICATIONS -> stringResource(R.string.ask_notifications, ask.host)
        },
        color = look.text,
        fontSize = 18.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(bottom = 20.dp),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        val allowLabel = stringResource(R.string.ask_allow)
        val dontAllowLabel = stringResource(R.string.ask_dont_allow)
        Box(
            modifier = Modifier
                .weight(1f)
                .heightIn(min = Metrics.Bar.height)
                .background(look.button, RoundedCornerShape(Metrics.Bar.radius / 2))
                .clickable(role = Role.Button, onClick = { onAnswer(true) })
                .semantics { contentDescription = allowLabel },
            contentAlignment = Alignment.Center,
        ) {
            Text(text = allowLabel, color = look.text, fontSize = 16.sp)
        }
        Box(
            modifier = Modifier
                .weight(1f)
                .heightIn(min = Metrics.Bar.height)
                .background(look.plate, RoundedCornerShape(Metrics.Bar.radius / 2))
                .clickable(role = Role.Button, onClick = { onAnswer(false) })
                .semantics { contentDescription = dontAllowLabel },
            contentAlignment = Alignment.Center,
        ) {
            Text(text = dontAllowLabel, color = look.text, fontSize = 16.sp)
        }
    }
}
