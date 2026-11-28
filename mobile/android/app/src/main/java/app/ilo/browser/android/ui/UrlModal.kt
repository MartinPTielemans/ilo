package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Address
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.FaviconStore
import app.ilo.browser.android.browser.SearchEngine
import app.ilo.browser.android.browser.Tab

/**
 * The new page panel, Chrome's new tab in ilo's own look: a header with the deck plate and the
 * menu when other tabs are open, a field with the chosen engine's mark at its left (pressing
 * the mark picks another), the incognito hat at its right, and while the field is empty the
 * recent pages as tiles, as many as the settings say. Typing swaps the tiles for the open
 * row, the search the words become and the matches, each history match with a cross that
 * forgets it. With no tab of its own it is what an empty Space shows,
 * so every use of it either goes to the page that's up or creates a tab, as iOS's openNew does.
 */
@Composable
fun UrlModal(
    browser: Browser,
    tab: Tab?,
    baseLook: Look,
    createsTab: Boolean,
    startWords: String,
    onClose: () -> Unit,
    onDeck: (() -> Unit)? = null,
    onTools: (() -> Unit)? = null,
) {
    var words by remember { mutableStateOf(startWords) }
    val focus = remember { FocusRequester() }
    var engine by remember { mutableStateOf(browser.settings.searchEngine) }
    var engineOpen by remember { mutableStateOf(false) }
    val history by browser.stores.historyFlow.collectAsState()
    val tabs by browser.state.tabs.collectAsState()
    // A tab to be takes the kind the app is in, not the page behind it: the menu's incognito
    // row turns privateMode on before the panel opens.
    var private by remember {
        mutableStateOf(if (createsTab) browser.state.privateMode.value else (tab?.isPrivate ?: browser.state.privateMode.value))
    }
    val modePrivate by browser.state.privateMode.collectAsState()
    LaunchedEffect(modePrivate) {
        // While the panel stands for a tab to be, the app's mode change reaches it: the menu's
        // incognito row turns it after the panel is already up.
        if (createsTab) private = modePrivate
    }
    val look = if (private) baseLook.privateVersion() else baseLook
    // The panel stands for a tab to be, so the deck counts the Space's other tabs of its kind:
    // the siblings the new tab will have, not the page behind it.
    val others = tabs.filter {
        it.space == browser.state.currentSpace.value && it.isPrivate == private && it.id != tab?.id
    }
    val otherTabs = pluralStringResource(R.plurals.other_tabs_open, others.count(), others.count())
    val clearName = stringResource(R.string.clear_field)
    val privateName = stringResource(R.string.toggle_private)

    LaunchedEffect(Unit) { focus.requestFocus() }
    BackHandler(onBack = onClose)

    // A fixed panel, not a modal: the ground never closes it, only the back key and its rows.
    Column(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
            .statusBarsPadding()
            .imePadding()
    ) {
        // The header: the deck plate and the menu, present only when other tabs are open, so
        // an empty Space keeps its plain panel; both dim and do nothing without a callback.
        if (others.isNotEmpty()) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = Metrics.Column.side, vertical = 10.dp),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
                        .clickable(enabled = onDeck != null, role = Role.Button) { onDeck?.invoke() }
                        .padding(horizontal = 12.dp, vertical = 10.dp)
                        .semantics { contentDescription = otherTabs },
                ) {
                    Canvas(Modifier.size(20.dp)) { drawTabsGlyph(look.glassText, size.width, size.height) }
                    Spacer(Modifier.width(8.dp))
                    Text(otherTabs, color = look.text, fontSize = Type.small, maxLines = 1)
                }
                Spacer(Modifier.weight(1f))
                PlateButton(
                    glyph = Glyph.MORE,
                    label = stringResource(R.string.more),
                    look = look,
                    onClick = { onTools?.invoke() },
                    dimmed = onTools == null,
                )
            }
        }
        // The field with the chosen engine's mark at its left, as Chrome's new tab field.
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = Metrics.Bar.side)
                .padding(top = 12.dp)
                .height(Metrics.Bar.height + 8.dp)
                .background(look.overlay, RoundedCornerShape(Metrics.Bar.radius))
                .padding(horizontal = 10.dp),
        ) {
            EngineMark(
                engine = engine,
                favicons = browser.favicons,
                look = look,
                onClick = { engineOpen = !engineOpen },
            )
            BasicField(
                value = words,
                onValueChange = { words = it },
                hint = stringResource(R.string.search_or_enter),
                look = look,
                selectsAll = true,
                onSubmit = { go(browser, tab, words, createsTab, private, onClose) },
                modifier = Modifier
                    .weight(1f)
                    .padding(start = 8.dp)
                    .focusRequester(focus),
            )
            if (words.isNotEmpty()) {
                Box(
                    modifier = Modifier
                        .size(Metrics.Bar.height)
                        .clickable(role = Role.Button) { words = "" }
                        .semantics { contentDescription = clearName },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.Filled.Close, null, tint = look.glassSoft)
                }
            }
            // The incognito mark at the field's right, the app's standard shape: the hat
            // worn for the kind the words would go to now, lifted when that kind is regular.
            Spacer(Modifier.width(4.dp))
            Box(
                modifier = Modifier
                    .size(Metrics.Bar.height)
                    .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
                    .clickable(role = Role.Button) { private = !private }
                    .semantics { contentDescription = privateName },
                contentAlignment = Alignment.Center,
            ) {
                Canvas(Modifier.size(22.dp)) {
                    if (private) drawIncognitoGlyph(look.text, size.width, size.height)
                    else drawIncognitoOffGlyph(look.text, size.width, size.height)
                }
            }
        }
        // The engine chooser, open under the field: one row per engine, a letter mark and the
        // name with the chosen one's radio, closing on the pick.
        if (engineOpen) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = Metrics.Bar.side)
                    .padding(top = 2.dp)
                    .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
                    .padding(vertical = 4.dp),
            ) {
                browser.settings.allEngines().forEach { choice ->
                    val chosen = engine.id == choice.id
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .selectable(
                                selected = chosen,
                                role = Role.RadioButton,
                                onClick = {
                                    engine = choice
                                    browser.settings.searchEngine = choice
                                    engineOpen = false
                                },
                            )
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    ) {
                        EngineMark(
                            engine = choice,
                            favicons = browser.favicons,
                            look = look,
                        )
                        Text(
                            choice.name,
                            color = look.text,
                            fontSize = Type.bar,
                            modifier = Modifier
                                .weight(1f)
                                .padding(start = 12.dp),
                        )
                        RadioButton(
                            selected = chosen,
                            onClick = null,
                            colors = RadioButtonDefaults.colors(selectedColor = look.accent),
                        )
                    }
                }
            }
        }
        // What the words name: the open row and the matches once typed, the recent pages as
        // tiles while the field is empty, newest first.
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = Metrics.Column.side)
                .padding(top = 8.dp, bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.Top),
        ) {
            // The tiles share the row's width: fewer per row on a narrow screen, so none is cut.
            val perRow = maxOf(1, (LocalConfiguration.current.screenWidthDp - Metrics.Column.side.value.toInt() * 2) / 84)
            val trimmed = words.trim()
            val asUrl = remember(trimmed) { Address.asAddress(trimmed) }
            // The page's own words name it, whether the field carries its whole address or its host.
            val pageWords = trimmed.isNotEmpty() && tab != null && (trimmed == tab.url || trimmed == hostOf(tab.url))
            if (trimmed.isEmpty()) {
                // The recent pages as tiles, as many as the settings say, 0 or an empty history hiding the section.
                val recents = history.take(browser.settings.recentTiles)
                if (browser.settings.recentTiles > 0 && recents.isNotEmpty()) {
                    Text(
                        stringResource(R.string.recent_pages),
                        color = look.soft,
                        fontSize = 13.sp,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                    recents.chunked(perRow).forEach { rowTiles ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            rowTiles.forEach { entry ->
                                RecentTile(
                                    url = entry.url,
                                    title = entry.title.ifEmpty { hostOf(entry.url) },
                                    favicons = browser.favicons,
                                    look = look,
                                ) {
                                    go(browser, tab, entry.url, createsTab, private, onClose)
                                }
                            }
                        }
                    }
                }
            } else {
                // The open row: the page's own words name its page, otherwise the words as an
                // address or the search they become.
                EntryRow(
                    url = asUrl ?: trimmed,
                    title = if (pageWords) tab!!.title.ifEmpty { trimmed } else trimmed,
                    detail = if (asUrl != null) hostOf(asUrl)
                    else stringResource(R.string.search_on, engine.name),
                    look = look,
                    accentIcon = asUrl != null || pageWords,
                ) {
                    go(browser, tab, words, createsTab, private, onClose)
                }
                // What the words become as a search on the chosen engine, only when they are
                // not an address: an address names a page, not a search.
                if (asUrl == null && !pageWords) {
                    EntryRow(
                        url = trimmed,
                        title = trimmed,
                        detail = stringResource(R.string.search_on, engine.name),
                        look = look,
                    ) {
                        go(browser, tab, words, createsTab, private, onClose)
                    }
                }
                // The pages the phone remembers that match, newest first, each with its site's
                // face and a cross that forgets it from the history.
                history.filter { entry ->
                    entry.url.contains(trimmed, ignoreCase = true) ||
                        entry.title.contains(trimmed, ignoreCase = true)
                }.take(5).forEach { entry ->
                    HistoryRow(
                        url = entry.url,
                        title = entry.title.ifEmpty { hostOf(entry.url) },
                        favicons = browser.favicons,
                        look = look,
                        onRemove = { browser.stores.removeHistory(entry.url) },
                    ) {
                        go(browser, tab, entry.url, createsTab, private, onClose)
                    }
                }
            }
        }
    }
}

/** Where the words go: the page that's up, or a new tab, in the kind the row asked for. */
private fun go(browser: Browser, tab: Tab?, words: String, createsTab: Boolean, private: Boolean, onClose: () -> Unit) {
    if (words.isBlank()) return
    if (createsTab) {
        // The kind the panel takes is the mode the app holds after it, as the menu's rows set
        // it both ways, so the Space and the switcher agree with the tab the panel opened.
        browser.state.privateMode.value = private
        browser.openInNewTab(words, isPrivate = private)
    } else if (private && tab?.isPrivate != true) {
        // A private page opened this way turns the mode with it, as the menu's incognito row
        // does, so the Space and the switcher both show the tab the hat opened.
        browser.state.privateMode.value = true
        browser.openInNewTab(words, isPrivate = true)
    }
    else browser.go(words)
    onClose()
}

/** One recent page as a tile: the site's favicon, its title under it, as the dock's tiles. */
@Composable
private fun RecentTile(url: String, title: String, favicons: FaviconStore, look: Look, onClick: () -> Unit) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .width(76.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = title },
    ) {
        Box(
            modifier = Modifier
                .size(Metrics.Dock.tileHeight)
                .background(look.plate, RoundedCornerShape(Metrics.Dock.tileRadius)),
            contentAlignment = Alignment.Center,
        ) {
            FaviconMark(url, favicons, look, Modifier.size(Metrics.Dock.tileIcon))
        }
        Text(
            title,
            color = look.soft,
            fontSize = Type.small,
            maxLines = 1,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

/** One history match: the site's favicon, its name over its host, and a cross that removes
 * the entry from the history, no dialog in the way. */
@Composable
private fun HistoryRow(
    url: String,
    title: String,
    favicons: FaviconStore,
    look: Look,
    onRemove: () -> Unit,
    onClick: () -> Unit,
) {
    val removeName = stringResource(R.string.remove_from_history)
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 50.dp)
            .background(look.plate, RoundedCornerShape(12.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 12.dp),
    ) {
        FaviconMark(url, favicons, look, Modifier.size(Metrics.Column.icon))
        Spacer(Modifier.width(Metrics.Column.gap))
        Column(Modifier.weight(1f)) {
            Text(title, color = look.text, fontSize = Type.bar, maxLines = 1)
            Text(hostOf(url), color = look.soft, fontSize = 13.sp, maxLines = 1)
        }
        Box(
            modifier = Modifier
                .size(Metrics.Bar.height)
                .clickable(role = Role.Button, onClick = onRemove)
                .semantics { contentDescription = removeName },
            contentAlignment = Alignment.Center,
        ) {
            Icon(Icons.Filled.Close, null, tint = look.glassSoft)
        }
    }
}

/** One row of the modal: the globe or the search's mark, a name over its detail, as the iOS Go row. */
@Composable
private fun EntryRow(
    url: String,
    title: String,
    detail: String,
    look: Look,
    accentIcon: Boolean = false,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 50.dp)
            .background(look.plate, RoundedCornerShape(12.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 12.dp),
    ) {
        Box(Modifier.size(Metrics.Column.icon), contentAlignment = Alignment.Center) {
            if (accentIcon) Canvas(Modifier.size(20.dp)) { drawGlobeGlyph(look.accent, size.width, size.height) }
            else Icon(Icons.Filled.Search, null, tint = look.accent)
        }
        Spacer(Modifier.width(Metrics.Column.gap))
        Column(Modifier.weight(1f)) {
            Text(title, color = look.text, fontSize = Type.bar, maxLines = 1)
            Text(detail, color = look.soft, fontSize = 13.sp, maxLines = 1)
        }
    }
}