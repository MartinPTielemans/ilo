package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.ArchivedTab
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.DownloadEntry
import app.ilo.browser.android.browser.HistoryEntry
import app.ilo.browser.android.browser.Bookmark
import app.ilo.browser.android.browser.SiteAnswer
import app.ilo.browser.android.browser.SitePermission
import app.ilo.browser.android.browser.Tab

/**
 * The four lists the tools open and the archive entry joins: History, Bookmarks, Downloads,
 * Archive. Each is a screen of rows over the same ground, a header naming it, rows doing what
 * they say, empties saying so. History opens its page and clears by its own row; bookmarks
 * open and remove; downloads open their file through Android's own chooser and remove; the
 * archive brings a tab back and deletes it.
 */
@Composable
fun ListScreen(
    which: String,
    browser: Browser,
    look: Look,
    tab: Tab?,
    onOpen: (String) -> Unit,
    onClose: () -> Unit,
) {
    BackHandler(onBack = onClose)
    Column(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
            .statusBarsPadding()
    ) {
        ListHead(
            title = stringResource(
                when (which) {
                    "history" -> R.string.history
                    "bookmarks" -> R.string.bookmarks
                    "downloads" -> R.string.downloads
                    "site" -> R.string.site_settings
                    else -> R.string.archived
                }
            ),
            look = look,
            onClose = onClose,
        )
        when (which) {
            "history" -> HistoryList(browser, look, onOpen)
            "bookmarks" -> BookmarksList(browser, look, onOpen)
            "downloads" -> DownloadsList(browser, look)
            "site" -> SiteSettingsList(browser, tab, look)
            else -> ArchiveList(browser, look, onOpen)
        }
    }
}

/** The screen's own header: its way back, then its name. */
@Composable
private fun ListHead(title: String, look: Look, onClose: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        modifier = Modifier
            .fillMaxWidth()
            .height(56.dp)
            .padding(horizontal = 16.dp),
    ) {
        PlateButton(Glyph.BACK, stringResource(R.string.back), look, onClick = onClose)
        Text(title, color = look.text, fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
    }
}

// LISTS

/** How history heads a page's age: today, yesterday, then the week, the month, then older. */
internal enum class HistoryGroup { TODAY, YESTERDAY, WEEK, MONTH, OLDER }

internal const val DAY_MS = 24 * 60 * 60 * 1000L

/** The group a visit falls in, by the day's start the phone's calendar says: Chrome's own
 * Today, Yesterday, Last 7 days, Last 30 days, Older. */
internal fun historyGroupOf(at: Long, now: Long): HistoryGroup {
    val cal = java.util.Calendar.getInstance()
    cal.timeInMillis = now
    cal.set(java.util.Calendar.HOUR_OF_DAY, 0)
    cal.set(java.util.Calendar.MINUTE, 0)
    cal.set(java.util.Calendar.SECOND, 0)
    cal.set(java.util.Calendar.MILLISECOND, 0)
    val today = cal.timeInMillis
    return when {
        at >= today -> HistoryGroup.TODAY
        at >= today - DAY_MS -> HistoryGroup.YESTERDAY
        at >= today - 7 * DAY_MS -> HistoryGroup.WEEK
        at >= today - 30 * DAY_MS -> HistoryGroup.MONTH
        else -> HistoryGroup.OLDER
    }
}

/** Whether a visit matches the search field's words: its name or its address says them. */
internal fun historyMatches(entry: HistoryEntry, words: String): Boolean {
    val query = words.trim()
    if (query.isEmpty()) return true
    return entry.title.contains(query, ignoreCase = true) ||
        entry.url.contains(query, ignoreCase = true)
}

/** Chrome's own clear ranges, what each takes being everything since its span began; the
 * last one takes everything. */
private enum class ClearRange(val label: Int, val spanMs: Long?) {
    LAST_15(R.string.clear_last_15, 15 * 60 * 1000L),
    LAST_HOUR(R.string.clear_last_hour, 60 * 60 * 1000L),
    LAST_DAY(R.string.clear_last_day, DAY_MS),
    LAST_WEEK(R.string.clear_last_week, 7 * DAY_MS),
    LAST_MONTH(R.string.clear_last_month, 30 * DAY_MS),
    ALL(R.string.clear_all_history, null),
}

/** History's rows, newest first under their day's heading, a search field over them and the
 * clear ranges at their foot; a tapped range asks first, for the pages it removes are gone
 * for good. */
@Composable
private fun HistoryList(browser: Browser, look: Look, onOpen: (String) -> Unit) {
    val history by browser.stores.historyFlow.collectAsState()
    var query by remember { mutableStateOf("") }
    var confirming by remember { mutableStateOf<ClearRange?>(null) }
    val found = remember(history, query) { history.filter { historyMatches(it, query) } }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = Metrics.Column.side, end = Metrics.Column.side, bottom = 16.dp
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        item(key = "search") {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = Metrics.Bar.height)
                    .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
                    .padding(horizontal = 12.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                BasicField(
                    value = query,
                    onValueChange = { query = it },
                    hint = stringResource(R.string.search_history),
                    look = look,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
        if (history.isEmpty()) {
            item { GroupHead(stringResource(R.string.history_empty), look) }
        } else if (found.isEmpty()) {
            item { GroupHead(stringResource(R.string.history_no_matches), look) }
        } else {
            var lastGroup: HistoryGroup? = null
            for (entry in found) {
                val group = historyGroupOf(entry.at, System.currentTimeMillis())
                if (group != lastGroup) {
                    lastGroup = group
                    item(key = "group:$group") {
                        GroupHead(
                            stringResource(
                                when (group) {
                                    HistoryGroup.TODAY -> R.string.group_today
                                    HistoryGroup.YESTERDAY -> R.string.group_yesterday
                                    HistoryGroup.WEEK -> R.string.group_this_week
                                    HistoryGroup.MONTH -> R.string.group_this_month
                                    HistoryGroup.OLDER -> R.string.group_older
                                }
                            ),
                            look,
                        )
                    }
                }
                item(key = entry.url) {
                    ListRow(
                        title = entry.title.ifEmpty { entry.url },
                        under = hostOf(entry.url),
                        look = look,
                        onOpen = { onOpen(entry.url) },
                    )
                }
            }
        }
        item(key = "clear-heading") { GroupHead(stringResource(R.string.clear_range), look) }
        for (range in ClearRange.entries) {
            item(key = "clear:$range") {
                ListRow(
                    title = stringResource(range.label),
                    under = null,
                    look = look,
                    onOpen = { confirming = range },
                )
            }
        }
    }
    confirming?.let { range ->
        // A range of history is destroyed, so the tap asks before it happens; the dialog's
        // own back dismissal is the same as Cancel.
        Dialog(onDismissRequest = { confirming = null }) {
            Column(
                Modifier
                    .background(look.groundTop, RoundedCornerShape(Metrics.Menu.radius))
                    .padding(20.dp)
            ) {
                Text(
                    stringResource(range.label),
                    color = look.text,
                    fontSize = 18.sp,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    stringResource(R.string.clear_confirm_body),
                    color = look.soft,
                    fontSize = 14.sp,
                    modifier = Modifier.padding(top = 8.dp, bottom = 20.dp),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    val clearLabel = stringResource(R.string.clear_confirm_yes)
                    val cancelLabel = stringResource(R.string.clear_confirm_no)
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .heightIn(min = Metrics.Bar.height)
                            .background(look.button, RoundedCornerShape(Metrics.Bar.radius / 2))
                            .clickable(role = Role.Button, onClick = {
                                confirming = null
                                if (range.spanMs == null) browser.stores.clearHistory()
                                else browser.stores.clearHistorySince(
                                    System.currentTimeMillis() - range.spanMs
                                )
                            })
                            .semantics { contentDescription = clearLabel },
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(text = clearLabel, color = look.text, fontSize = 16.sp)
                    }
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .heightIn(min = Metrics.Bar.height)
                            .background(look.plate, RoundedCornerShape(Metrics.Bar.radius / 2))
                            .clickable(role = Role.Button, onClick = { confirming = null })
                            .semantics { contentDescription = cancelLabel },
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(text = cancelLabel, color = look.text, fontSize = 16.sp)
                    }
                }
            }
        }
    }
}

/** A group's heading, softer and smaller than a row, naming what follows it. */
@Composable
private fun GroupHead(words: String, look: Look) {
    Text(
        words,
        color = look.soft,
        fontSize = 13.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(start = 12.dp, top = 14.dp, bottom = 4.dp),
    )
}

/** Bookmarks' rows, the pages the user kept, each with its remove. */
@Composable
private fun BookmarksList(browser: Browser, look: Look, onOpen: (String) -> Unit) {
    val bookmarks by browser.stores.bookmarksFlow.collectAsState()
    if (bookmarks.isEmpty()) {
        EmptyRow(stringResource(R.string.bookmarks_empty), look)
        return
    }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = Metrics.Column.side, end = Metrics.Column.side, bottom = 16.dp
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        items(bookmarks, key = { it.url }) { mark ->
            ListRow(
                title = mark.title.ifEmpty { mark.url },
                under = hostOf(mark.url),
                look = look,
                onOpen = { onOpen(mark.url) },
                side = stringResource(R.string.remove),
                onSide = { browser.stores.unbookmark(mark.url) },
            )
        }
    }
}

/** Downloads' rows, each opening its file through Android's chooser. */
@Composable
private fun DownloadsList(browser: Browser, look: Look) {
    val downloads by browser.stores.downloadsFlow.collectAsState()
    if (downloads.isEmpty()) {
        EmptyRow(stringResource(R.string.downloads_empty), look)
        return
    }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = Metrics.Column.side, end = Metrics.Column.side, bottom = 16.dp
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        items(downloads, key = { it.id }) { download ->
            DownloadRow(download, browser, look)
        }
    }
}

/** One download: its file's name, its size, its open and its remove. */
@Composable
private fun DownloadRow(download: DownloadEntry, browser: Browser, look: Look) {
    val context = LocalContext.current
    ListRow(
        title = download.fileName,
        under = sizeWords(download.sizeBytes),
        look = look,
        onOpen = {
            // Android's own chooser opens the file where Download Manager put it.
            runCatching {
                val file = java.io.File(
                    android.os.Environment.getExternalStoragePublicDirectory(
                        android.os.Environment.DIRECTORY_DOWNLOADS
                    ),
                    download.fileName
                )
                val uri = androidx.core.content.FileProvider.getUriForFile(
                    context, "${context.packageName}.files", file
                )
                val open = android.content.Intent(android.content.Intent.ACTION_VIEW).apply {
                    setDataAndType(uri, download.mime.ifEmpty { "application/octet-stream" })
                    addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                context.startActivity(android.content.Intent.createChooser(open, download.fileName))
            }
        },
        side = stringResource(R.string.remove),
        onSide = { browser.stores.forgetDownload(download.id) },
    )
}

/** The page's own site settings: what ilo does for this site alone. The trusted certificate's
 * way back comes first, then the site's own ad blocking, then the connection as it stands
 * (the certificate's facts fold out when the connection has a fault), when the phone was
 * last on the site, and each permission kind with its answer, which a tap steps through
 * Ask, Allow and Block, as Chrome's own site row does. */
@Composable
private fun SiteSettingsList(browser: Browser, tab: Tab?, look: Look) {
    if (tab == null) {
        EmptyRow("", look)
        return
    }
    val exception = browser.state.tabExceptions.value[tab.id]
    val off by browser.blocking.sitesOffFlow.collectAsState()
    val url = tab.url
    // The site's own name for its rows, the store's own spelling: the address's casing
    // lowered and its www dropped, as both stores' lookups take it.
    val host = remember(url) {
        url?.let { runCatching { java.net.URI(it).host?.lowercase()?.removePrefix("www.") }.getOrNull() } ?: ""
    }
    // The site's permission answers, and the fault on the page's connection, if any: both
    // read once, read again whenever an answer changes.
    val answers by browser.sitePermissions.answersFlow.collectAsState()
    val history by browser.stores.historyFlow.collectAsState()
    val fault = browser.state.tabFaults.value[tab.id]
    var faultOpen by remember(fault) { mutableStateOf(false) }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = Metrics.Column.side, end = Metrics.Column.side, bottom = 16.dp
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        if (exception != null) {
            item {
                ListRow(
                    title = stringResource(R.string.stop_trusting),
                    under = exception.host,
                    look = look,
                    onOpen = { browser.stopTrusting(tab) },
                )
            }
        }
        if (url != null && url.startsWith("http")) {
            item {
                val site = remember(url) {
                    app.ilo.browser.android.blocking.EasyListBlocker.hostOf(url)
                }
                ListRow(
                    title = stringResource(R.string.block_ads),
                    under = site,
                    look = look,
                    onOpen = {
                        if (site != null) {
                            browser.blocking.setSite(site, browser.blocking.siteOff(url))
                            browser.engine.reload(tab)
                        }
                    },
                    side = if (off.any { it == site }) stringResource(R.string.off_in_settings)
                    else stringResource(R.string.blocked_count, browser.state.blockedCount.value),
                )
            }
        }
        if (url != null) {
            item(key = "connection") {
                // A fault stands on the connection, so its row names that and turns up the
                // certificate's own facts; a clean connection says so.
                ListRow(
                    title = stringResource(R.string.connection),
                    under = if (fault != null) fault.facts.sentence
                    else if (url.startsWith("https")) stringResource(R.string.connection_secure)
                    else stringResource(R.string.connection_not_secure),
                    look = look,
                    onOpen = { if (fault != null) faultOpen = !faultOpen },
                    side = if (fault != null) stringResource(R.string.view_certificate) else null,
                )
            }
            if (fault != null && faultOpen) {
                for ((index, line) in fault.facts.lines(host = fault.exception.host).withIndex()) {
                    item(key = "fault:$index") {
                        ListRow(
                            title = line.label,
                            under = line.value,
                            look = look,
                            onOpen = {},
                        )
                    }
                }
            }
        }
        if (host.isNotEmpty()) {
            item(key = "last-visit") {
                // Read over the collected history, so a visit while the panel is open refreshes it.
                val visited = remember(history, host) { browser.stores.lastVisitOf(host) }
                ListRow(
                    title = stringResource(R.string.last_visit),
                    under = visited?.let { java.text.DateFormat.getDateTimeInstance().format(java.util.Date(it)) }
                        ?: stringResource(R.string.last_visit_never),
                    look = look,
                    onOpen = {},
                )
            }
        }
        if (host.isNotEmpty()) {
            for (kind in SitePermission.entries) {
                item(key = "perm:$kind") {
                    // The row shows what the page gets: the site's own answer, or the kind's
                    // default when it never gave one; the flow's turn re-reads it.
                    val current = remember(answers, host, kind) { browser.sitePermissions.answer(host, kind) }
                    ListRow(
                        title = stringResource(
                            when (kind) {
                                SitePermission.LOCATION -> R.string.permission_location
                                SitePermission.CAMERA -> R.string.permission_camera
                                SitePermission.MICROPHONE -> R.string.permission_microphone
                                SitePermission.NOTIFICATIONS -> R.string.permission_notifications
                            }
                        ),
                        under = host,
                        look = look,
                        onOpen = { browser.sitePermissions.setAnswer(host, kind, browser.sitePermissions.nextAnswer(current)) },
                        side = stringResource(
                            when (current) {
                                SiteAnswer.ASK -> R.string.permissions_ask
                                SiteAnswer.ALLOW -> R.string.permissions_allow
                                SiteAnswer.BLOCK -> R.string.permissions_block
                            }
                        ),
                    )
                }
            }
            if (answers[host] != null) {
                item(key = "reset-site") {
                    // The way back from a site's own answers: they go entirely, so every
                    // page of it asks again.
                    ListRow(
                        title = stringResource(R.string.reset_site),
                        under = host,
                        look = look,
                        onOpen = { browser.sitePermissions.clearSite(host) },
                    )
                }
            }
        }
    }
}

/** The archive's rows, the tabs put away, each with its restore and its delete. */
@Composable
private fun ArchiveList(browser: Browser, look: Look, onOpen: (String) -> Unit) {
    val archived by browser.stores.archivedFlow.collectAsState()
    if (archived.isEmpty()) {
        // The empty state names the direction that archives, so the words follow the setting,
        // and says nothing of swipes when no direction is set to archive.
        val upWord = stringResource(R.string.card_direction_up)
        val downWord = stringResource(R.string.card_direction_down)
        val word = when {
            browser.settings.swipeUpAction == "archive" -> upWord
            browser.settings.swipeDownAction == "archive" -> downWord
            else -> null
        }
        EmptyRow(
            if (word == null) stringResource(R.string.archive_empty_none)
            else stringResource(R.string.archive_empty, word),
            look,
        )
        return
    }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(
            start = Metrics.Column.side, end = Metrics.Column.side, bottom = 16.dp
        ),
        modifier = Modifier.fillMaxSize(),
    ) {
        items(archived, key = { it.id }) { archivedTab ->
            ArchiveRow(archivedTab, browser, look, onOpen)
        }
    }
}

/** One archived tab: its name, its restore and its delete. */
@Composable
private fun ArchiveRow(
    archivedTab: ArchivedTab,
    browser: Browser,
    look: Look,
    onOpen: (String) -> Unit,
) {
    ListRow(
        title = archivedTab.title.ifEmpty { archivedTab.url ?: "" },
        under = archivedTab.url?.let { hostOf(it) },
        look = look,
        // A restore opens the tab, as iOS's archive row does, so the screens that led here go
        // with it; a delete leaves the user looking at the list.
        onOpen = {
            browser.restoreArchived(archivedTab.id)
            onOpen(archivedTab.url ?: "")
        },
        side = stringResource(R.string.archive_delete),
        onSide = { browser.stores.forgetArchived(archivedTab.id) },
    )
}

// ROWS

/** A row of any list: its title, the host under it, a side word and what it does. */
@Composable
private fun ListRow(
    title: String,
    under: String?,
    look: Look,
    onOpen: () -> Unit,
    side: String? = null,
    onSide: (() -> Unit)? = null,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .background(look.plate, RoundedCornerShape(12.dp))
            .clickable(role = Role.Button, onClick = onOpen)
            .padding(horizontal = 12.dp)
            .semantics { contentDescription = title },
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, color = look.text, fontSize = Type.bar, maxLines = 1)
            under?.let {
                Text(it, color = look.soft, fontSize = 13.sp, maxLines = 1)
            }
        }
        side?.let { word ->
            Spacer(Modifier.width(12.dp))
            Text(
                word,
                color = look.accent,
                fontSize = 14.sp,
                modifier = onSide?.let {
                    Modifier
                        .background(look.overlay, RoundedCornerShape(14.dp))
                        .clickable(role = Role.Button, onClick = it)
                        .padding(horizontal = 12.dp, vertical = 6.dp)
                } ?: Modifier,
            )
        }
    }
}

/** What a list says when it holds nothing. */
@Composable
private fun EmptyRow(words: String, look: Look) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(words, color = look.soft, fontSize = 17.sp)
    }
}

/** A size as the phone's own words say it. */
private fun sizeWords(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024) return "${"%.0f".format(kb)} KB"
    val mb = kb / 1024.0
    if (mb < 1024) return "${"%.1f".format(mb)} MB"
    return "${"%.1f".format(mb / 1024.0)} GB"
}