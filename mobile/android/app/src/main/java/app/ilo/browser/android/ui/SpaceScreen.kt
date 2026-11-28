package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.Tab

/**
 * The Space: its open pages in a grid that grows up from the dock, and in the dock its tiles
 * over the search field, all within the thumb's reach. Typing swaps the grid for matches. A
 * private tab is among them only while the mode is on, its card ink on the Space's own ground.
 */
@Composable
fun SpaceScreen(browser: Browser, onSettings: () -> Unit, onOpenSwitcher: () -> Unit = {}) {
    val base = LocalLook.current
    val tabs by browser.state.tabs.collectAsState()
    val spaces by browser.state.spaces.collectAsState()
    val currentSpace by browser.state.currentSpace.collectAsState()
    val selectedId by browser.state.selectedId.collectAsState()
    val lastShownId by browser.state.lastShownId.collectAsState()
    val privateMode by browser.state.privateMode.collectAsState()
    var query by remember { mutableStateOf("") }
    val pager = rememberPagerState(pageCount = { spaces.count() })
    // The pager's page turns the Space: a swipe that settles changes it, and so does the dots'
    // Space changing under the pager (the sync lane can).
    LaunchedEffect(spaces, currentSpace) {
        val index = spaces.indexOfFirst { it.id == currentSpace }.coerceAtLeast(0)
        if (pager.currentPage != index && !pager.isScrollInProgress) pager.scrollToPage(index)
    }
    LaunchedEffect(pager) {
        snapshotFlow { pager.settledPage }.collect { page ->
            val space = spaces.getOrNull(page) ?: return@collect
            if (space.id != currentSpace) browser.state.selectSpace(space.id)
        }
    }
    val spaceColor = spaces.getOrNull(pager.currentPage)?.color
    // The Space keeps its own ground while the mode is on; only its private cards are ink, as iOS does.
    val look = Look.of(base.dark, spaceColor)
    val typing = query.isNotEmpty()
    // The grid shows the kind the screen is in: a private mode shows private tabs alone.
    val gridKind = privateMode
    // The search field takes the focus for a "search" start page, the app's own search surface.
    val searchFocus = remember { androidx.compose.ui.focus.FocusRequester() }
    // The count of the Space's own kind, so a Space emptied again runs the start page once more.
    val kindCount = browser.state.tabsIn(currentSpace).count { it.isPrivate == privateMode }
    // The new page panel over the Space while it stands as the start page.
    var panelOpen by remember { mutableStateOf(false) }
    // The panel's own dots menu and the lists it opens, as the page's bar has behind it.
    var toolsOpen by remember { mutableStateOf(false) }
    var listOpen by remember { mutableStateOf<String?>(null) }
    // Whether the panel already stood for this entry into the Space: the back key must be able
    // to close it, so the effect does not bring it straight back, and it stands again only
    // after the Space has held a tab and lost it.
    var panelShown by remember(currentSpace, privateMode) { mutableStateOf(false) }
    LaunchedEffect(currentSpace, privateMode, typing, kindCount, panelOpen) {
        // An empty Space shows the start page the user chose: the panel by default, the most
        // recent page on "last", a "search" one asks the field to take the focus, a "page" one
        // opens its address as a tab, a "blank" one stays empty.
        if (kindCount > 0) {
            if (panelShown) panelShown = false
            return@LaunchedEffect
        }
        if (privateMode || typing || panelOpen || selectedId != null || panelShown) return@LaunchedEffect
        when (browser.settings.startPage) {
            "panel" -> { panelOpen = true; panelShown = true }
            "last" -> {
                val lastUrl = browser.stores.historyFlow.value.firstOrNull()?.url
                if (lastUrl != null) browser.openInNewTab(lastUrl) else { panelOpen = true; panelShown = true }
                panelShown = true
            }
            "page" -> browser.openInNewTab(browser.settings.startPageUrl)
            "search" -> runCatching { searchFocus.requestFocus() }
        }
    }

    if (panelOpen) {
        UrlModal(
            browser = browser,
            tab = null,
            baseLook = look,
            createsTab = true,
            startWords = "",
            onClose = { panelOpen = false },
            // The panel's header acts on the Space behind it: its deck opens the switcher,
            // its dots the same menu the bar's dots open, with no page rows to show.
            onDeck = onOpenSwitcher,
            onTools = { toolsOpen = true },
        )
        if (toolsOpen) {
            ToolsModal(
                browser = browser,
                tab = null,
                look = look,
                onNewTab = { isPrivate ->
                    // The panel is the new tab already; the row turns the kind it will take.
                    toolsOpen = false
                    browser.state.privateMode.value = isPrivate
                },
                onOpenHistory = { toolsOpen = false; listOpen = "history" },
                onOpenBookmarks = { toolsOpen = false; listOpen = "bookmarks" },
                onOpenDownloads = { toolsOpen = false; listOpen = "downloads" },
                onOpenSiteSettings = { toolsOpen = false; listOpen = "site" },
                onOpenSettings = { toolsOpen = false; onSettings() },
                onFind = {},
                onClose = { toolsOpen = false },
            )
        }
        if (listOpen != null) {
            ListScreen(
                which = listOpen!!,
                browser = browser,
                look = look,
                tab = null,
                onOpen = { url -> browser.openInNewTab(url); listOpen = null },
                onClose = { listOpen = null },
            )
        }
        return
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
    ) {
        // The grid: the Space's open pages as cards, growing up from the dock, one page of the
        // pager per Space.
        // The grid runs out under the status bar, into the ground's own colour, as iOS's does.
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (!typing) {
                HorizontalPager(
                    state = pager,
                    modifier = Modifier.fillMaxSize(),
                ) { page ->
                    val space = spaces.getOrNull(page) ?: return@HorizontalPager
                    val pageTabs = tabs.filter { it.space == space.id && it.isPrivate == gridKind }
                    // A kept picture landing or going moves the grid's cards with it.
                    val pictures by browser.pictures.version.collectAsState()
                    // The block stands at the foot: with few pages it hugs the dock, with many it
                    // scrolls, like the iOS page's minHeight-and-bottom column.
                    BoxWithConstraints(Modifier.fillMaxSize()) {
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .verticalScroll(rememberScrollState())
                                .heightIn(min = maxHeight)
                                .wrapContentHeight(Alignment.Bottom)
                                .padding(horizontal = Metrics.Card.side),
                        ) {
                            SpaceTitle(name = space.name, look = look)
                            if (pageTabs.isEmpty()) {
                                Text(
                                    text = stringResource(R.string.nothing_open),
                                    color = look.text,
                                    fontSize = 20.sp,
                                    fontWeight = FontWeight.SemiBold,
                                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp),
                                )
                                Text(
                                    text = stringResource(R.string.nothing_open_hint),
                                    color = look.soft,
                                    fontSize = 16.sp,
                                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp),
                                )
                            } else {
                                pageTabs.chunked(2).forEachIndexed { row, cardsInRow ->
                                    Row(
                                        horizontalArrangement = Arrangement.spacedBy(Metrics.Card.columnGap),
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .padding(top = if (row == 0) 8.dp else 0.dp),
                                    ) {
                                        cardsInRow.forEach { tab ->
                                            // The disk read runs off the composition path, once per card.
                                            LaunchedEffect(tab.id) { browser.pictures.load(of = tab.id) }
                                            PageCard(
                                                tab = tab,
                                                current = tab.id == lastShownId,
                                                picture = { browser.pictures.picture(of = tab.id) },
                                                look = look,
                                                onOpen = { browser.state.select(tab.id) },
                                                onClose = {
                                                    browser.thrown("closed", tab) {
                                                        browser.engine.destroy(tab)
                                                        browser.state.close(tab.id)
                                                    }
                                                },
                                                modifier = Modifier.weight(1f),
                                            )
                                        }
                                    }
                                    Spacer(Modifier.height(Metrics.Card.rowGap))
                                }
                            }
                        }
                    }
                }
            } else {
                SearchMatches(
                    browser = browser,
                    look = look,
                    query = query,
                    onGo = { url ->
                        browser.openInNewTab(url)
                        query = ""
                    },
                    onPickTab = { tab ->
                        browser.state.select(tab.id)
                        query = ""
                    },
                    modifier = Modifier.fillMaxSize(),
                )
            }
            // The ground's top colour, fading down over the grid's cards, non-hitting, as iOS's.
            Box(
                Modifier
                    .align(Alignment.TopCenter)
                    .fillMaxWidth()
                    .height(WindowInsets.statusBars.asPaddingValues().calculateTopPadding() + Metrics.Card.rowGap)
                    .background(
                        Brush.verticalGradient(
                            colorStops = arrayOf(
                                0.45f to look.groundTop,
                                1f to look.groundTop.copy(alpha = 0f),
                            )
                        )
                    )
            )
        }

        // The dock: the Space dots over it, then one glass with plain fills on it, the tiles over the field.
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = Metrics.Dock.margin)
                .navigationBarsPadding()
                .padding(bottom = Metrics.Dock.margin)
        ) {
            // The dots: one per Space, the current one the accent, present only when there's a choice.
            if (spaces.count() > 1) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 16.dp, bottom = 10.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                ) {
                    spaces.forEach { space ->
                        Box(
                            Modifier
                                .size(6.dp)
                                .background(
                                    if (space.id == currentSpace) look.accent else look.faint,
                                    CircleShape,
                                )
                        )
                    }
                }
            }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = Metrics.Dock.inset)
                    .padding(bottom = Metrics.Dock.gap),
                horizontalArrangement = Arrangement.spacedBy(Metrics.Dock.gap),
            ) {
                DockTile(
                    label = stringResource(R.string.new_tab),
                    look = look,
                    modifier = Modifier.weight(1f),
                    onClick = { browser.state.newTab(isPrivate = gridKind) },
                ) {
                    Icon(Icons.Filled.Add, null, Modifier.size(Metrics.Dock.tileIcon), tint = look.glassText)
                }
                DockTile(
                    label = stringResource(R.string.settings),
                    look = look,
                    modifier = Modifier.weight(1f),
                    onClick = onSettings,
                ) {
                    Icon(Icons.Filled.Settings, null, Modifier.size(Metrics.Dock.tileIcon), tint = look.glassText)
                }
            }
            SearchField(
                look = look,
                query = query,
                onQueryChange = { query = it },
                focus = searchFocus,
                onSubmit = {
                    val url = browser.resolve(query)
                    if (url != null) {
                        browser.openInNewTab(url)
                        query = ""
                    }
                },
            )
        }
    }

    BackHandler(enabled = typing) { query = "" }
}

/** The Space's name, a heading on its pages that scrolls with them. Ports the iOS SpaceTitle. */
@Composable
private fun SpaceTitle(name: String, look: Look) {
    Text(
        text = name,
        color = look.text,
        fontSize = 30.sp,
        fontWeight = FontWeight.Bold,
        maxLines = 1,
        modifier = Modifier
            .padding(horizontal = 4.dp)
            .padding(top = 12.dp, bottom = 14.dp),
    )
}

/** A page's card in the grid: a picture of its page with a title line under it, and a close on
 * its corner. A private tab's card is ink with the one glyph, and never a picture of its page. */
@Composable
fun PageCard(
    tab: Tab,
    current: Boolean,
    look: Look,
    picture: () -> android.graphics.Bitmap?,
    onOpen: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val privateName = stringResource(R.string.private_tab)
    val bitmap = if (!tab.isPrivate) picture() else null
    Column(modifier = modifier) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(Metrics.Card.pictureHeight)
                .shadow(
                    if (tab.isPrivate) Metrics.PrivateMark.cardShadowRadius else 8.dp,
                    RoundedCornerShape(Metrics.Card.radius),
                    spotColor = if (tab.isPrivate) Color.Black.copy(alpha = Metrics.PrivateMark.cardShadow) else look.cardShadow,
                    ambientColor = if (tab.isPrivate) Color.Black.copy(alpha = Metrics.PrivateMark.cardShadow) else look.cardShadow,
                )
                // A private tab's card is ink, whatever the phone's look, as iOS's PrivateCard is.
                .background(
                    if (tab.isPrivate) Look.Ink else look.overlay,
                    RoundedCornerShape(Metrics.Card.radius),
                )
                .then(
                    if (current) {
                        Modifier.border(
                            Metrics.Card.ring,
                            look.accent,
                            RoundedCornerShape(Metrics.Card.radius + Metrics.Card.ring),
                        )
                    } else if (tab.isPrivate) {
                        Modifier.border(
                            0.5.dp,
                            Look.Paper.copy(alpha = Metrics.PrivateMark.cardRim),
                            RoundedCornerShape(Metrics.Card.radius),
                        )
                    } else {
                        Modifier.border(1.dp, look.rim, RoundedCornerShape(Metrics.Card.radius))
                    }
                )
                .clickable(role = Role.Button, onClick = onOpen)
        ) {
            if (bitmap != null) {
                Image(
                    bitmap = bitmap.asImageBitmap(),
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier
                        .fillMaxSize()
                        .clip(RoundedCornerShape(Metrics.Card.radius)),
                )
            } else {
            // The favicon dot stands for the picture until the phone has one.
            Box(
                Modifier
                    .align(Alignment.Center)
                    .size(Metrics.PrivateMark.plate)
                    .background(
                        if (tab.isPrivate) Look.Paper.copy(alpha = Metrics.PrivateMark.plateFill) else look.accent.copy(alpha = 0.22f),
                        RoundedCornerShape(Metrics.PrivateMark.plateRadius),
                    )
                    .let { if (tab.isPrivate) it.semantics { contentDescription = privateName } else it }
            ) {
                if (tab.isPrivate) {
                    Canvas(Modifier.size(Metrics.PrivateMark.onCard)) {
                        drawIncognitoGlyph(Look.Paper.copy(alpha = Metrics.PrivateMark.cardGlyph), size.width, size.height)
                    }
                } else {
                    Text(
                        text = tab.title.take(1).uppercase().ifEmpty { "?" },
                        color = look.glassText,
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.align(Alignment.Center),
                    )
                }
            }
            }
            CardClose(onClose, Modifier.align(Alignment.TopEnd))
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
        ) {
            if (tab.isPrivate && tab.url == null) {
                // A private tab with no page is called by its mark, as iOS's PrivateCard foot is.
                Canvas(
                    Modifier.size(16.dp),
                ) { drawIncognitoGlyph(look.soft, size.width, size.height) }
            } else {
                Box(
                    Modifier
                        .size(16.dp)
                        .background(Color(0xFF000000 + tab.faviconColor), CircleShape)
                )
            }
            Text(
                text = tab.title.ifEmpty { tab.url ?: if (tab.isPrivate) privateName else "" },
                color = look.text,
                fontSize = Type.card,
                maxLines = 1,
                modifier = Modifier.padding(start = 7.dp),
            )
        }
    }
}

/** The close on a card's corner: a small plate, in a touch target bigger than itself. */
@Composable
internal fun CardClose(onClose: () -> Unit, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .size(Metrics.Card.touch)
            .clickable(role = Role.Button, onClick = onClose),
    ) {
        Box(
            Modifier
                .align(Alignment.Center)
                .size(Metrics.Card.close)
                .background(Look.ClosePlate, CircleShape)
        ) {
            Icon(
                Icons.Filled.Close,
                contentDescription = stringResource(R.string.close_tab),
                tint = Color.White,
                modifier = Modifier.align(Alignment.Center).size(12.dp),
            )
        }
    }
}

/** A tile in the dock: a round-cornered plate with an icon, and a label for what it does. */
@Composable
fun DockTile(
    label: String,
    look: Look,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = modifier
            .height(Metrics.Dock.tileHeight)
            .background(look.plate, RoundedCornerShape(Metrics.Dock.tileRadius))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(top = Metrics.Dock.top, start = 10.dp, end = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(Modifier.size(Metrics.Dock.tileIcon)) { content() }
        Text(
            text = label,
            color = look.soft,
            fontSize = Type.small,
            maxLines = 1,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

/** The field at the dock's foot: a pill with a magnifier and a hint. */
@Composable
private fun SearchField(
    look: Look,
    query: String,
    onQueryChange: (String) -> Unit,
    onSubmit: () -> Unit,
    focus: androidx.compose.ui.focus.FocusRequester? = null,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .height(Metrics.Dock.field)
            .background(look.plate, RoundedCornerShape(Metrics.Dock.field / 2))
            .padding(horizontal = 16.dp),
    ) {
        Icon(Icons.Filled.Search, null, tint = look.glassSoft)
        BasicField(
            value = query,
            onValueChange = onQueryChange,
            hint = stringResource(R.string.search_hint),
            look = look,
            onSubmit = onSubmit,
            focus = focus,
            modifier = Modifier.weight(1f).padding(start = 10.dp),
        )
    }
}

/** What typing finds, stacked up from the field: the Go row right above it, then matching tabs. */
@Composable
private fun SearchMatches(
    browser: Browser,
    look: Look,
    query: String,
    onGo: (String) -> Unit,
    onPickTab: (Tab) -> Unit,
    modifier: Modifier = Modifier,
) {
    val tabs by browser.state.tabs.collectAsState()
    val resolved = browser.resolve(query)
    // The matches are the screen's own kind's, as iOS keeps the kinds apart.
    val matches = tabs.filter {
        it.isPrivate == browser.state.privateMode.value &&
            (it.title.contains(query, ignoreCase = true) || it.url?.contains(query, ignoreCase = true) == true)
    }
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Column.side),
        verticalArrangement = Arrangement.spacedBy(2.dp, Alignment.Bottom),
    ) {
        matches.forEach { tab ->
            MatchRow(
                title = tab.title,
                detail = tab.url ?: "",
                look = look,
                onClick = { onPickTab(tab) },
            )
        }
        if (resolved != null) {
            MatchRow(
                title = query,
                detail = resolved,
                look = look,
                onClick = { onGo(resolved) },
            )
        }
    }
}

/** One row of matches: title over the site's address, in the space the field holds. */
@Composable
private fun MatchRow(
    title: String,
    detail: String,
    look: Look,
    onClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    ) {
        Text(
            text = title,
            color = look.text,
            fontSize = Type.bar,
            maxLines = 1,
        )
        Text(
            text = detail,
            color = look.soft,
            fontSize = Type.small,
            maxLines = 1,
        )
    }
}
