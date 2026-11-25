package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.blocking.EasyListBlocker
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.ExternalDecision
import app.ilo.browser.android.browser.SiteAnswer
import app.ilo.browser.android.browser.SitePermission
import app.ilo.browser.android.sync.IloAccount
import kotlin.math.roundToInt

/**
 * Settings as a root list and its submenus, as the owner asked: each root row carries what its
 * submenu stands at and pushes it, the back gesture and the back button return to the root,
 * everything to press nearest the foot of each page. Dark mode is the phone's own choice here,
 * and the filter lists a user adds are kept with the blocking.
 */
@Composable
fun SettingsScreen(
    browser: Browser,
    account: IloAccount?,
    accountScreen: (@Composable (onBack: () -> Unit) -> Unit)? = null,
    onBack: () -> Unit,
) {
    val look = LocalLook.current
    // The account's screen and the engine sheet are destinations of their own, with their way
    // back to this list.
    var inAccount by remember { mutableStateOf(false) }
    var inEngines by remember { mutableStateOf(false) }
    var page by remember { mutableStateOf(Page.Root) }
    if (inAccount && account != null && accountScreen != null) {
        BackHandler { inAccount = false }
        accountScreen { inAccount = false }
        return
    }
    if (inEngines) {
        BackHandler { inEngines = false }
        EngineSheet(browser) { inEngines = false }
        return
    }
    BackHandler { if (page == Page.Root) onBack() else page = Page.Root }

    // The app draws edge to edge, so content must clear the status bar's window:
    // taps inside it go to SystemUI and never reach the app.
    Column(
        Modifier
            .fillMaxSize()
            .background(look.overlay)
            .windowInsetsPadding(WindowInsets.statusBars)
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Metrics.Column.side)
    ) {
        Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
            PlateButton(Glyph.BACK, stringResource(R.string.back), look, onClick = {
                if (page == Page.Root) onBack() else page = Page.Root
            })
        }
        // The root names itself as its submenus do, so the screen's place is read, not remembered.
        if (page == Page.Root) {
            Text(
                stringResource(R.string.settings),
                color = look.text,
                fontSize = 20.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
            )
        }
        when (page) {
            Page.Root -> Root(browser, account, look, inAccount = { inAccount = true }) { page = it }
            Page.Appearance -> AppearancePage(browser, look)
            Page.AdBlocking -> AdBlockingPage(browser, look)
            Page.Search -> SearchPage(browser, look) { inEngines = true }
            Page.Translation -> TranslationPage(browser, look)
            Page.StartPage -> StartPagePage(browser, look)
            Page.Gestures -> GesturesPage(browser, look)
            Page.SiteSettings -> SiteSettingsPage(browser, look)
            Page.Undo -> UndoPage(browser, look)
            Page.About -> AboutPage(look)
        }
        Spacer(Modifier.height(16.dp))
    }
}

/** The submenus the root pushes, in the order their rows stand in. */
private enum class Page { Root, Appearance, AdBlocking, Search, Translation, StartPage, Gestures, SiteSettings, Undo, About }

/** A submenu's own heading, so each page names itself as the root's row did. */
@Composable
private fun SubTitle(text: String, look: Look) {
    Text(
        text,
        color = look.text,
        fontSize = 20.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    )
}

/** The root list: the account lane and a row per submenu, each carrying what it stands at. */
@Composable
private fun Root(
    browser: Browser,
    account: IloAccount?,
    look: Look,
    inAccount: () -> Unit,
    open: (Page) -> Unit,
) {
    if (account != null) {
        TextRow(stringResource(R.string.manage_account), look) { inAccount() }
    }
    Rule(look)

    val dark = listOf(
        "system" to stringResource(R.string.dark_mode_system),
        "light" to stringResource(R.string.dark_mode_light),
        "dark" to stringResource(R.string.dark_mode_dark),
    ).firstOrNull { it.first == browser.settings.darkMode }?.second
        ?: stringResource(R.string.dark_mode_system)
    PushRow(stringResource(R.string.appearance), dark, look) { open(Page.Appearance) }

    val blockingOn by browser.blocking.isOnFlow.collectAsState()
    PushRow(
        stringResource(R.string.ad_blocking),
        stringResource(if (blockingOn) R.string.on else R.string.off),
        look,
    ) { open(Page.AdBlocking) }

    PushRow(stringResource(R.string.search_engine), browser.settings.searchEngine.name, look) {
        open(Page.Search)
    }

    PushRow(
        stringResource(R.string.translation),
        translationStateName(browser.settings.translationLanguage)
            ?: stringResource(R.string.translate_phone_language),
        look,
    ) { open(Page.Translation) }

    val startPage = listOf(
        "panel" to stringResource(R.string.start_panel),
        "last" to stringResource(R.string.start_last),
        "search" to stringResource(R.string.start_search),
        "page" to stringResource(R.string.start_page_url),
    ).firstOrNull { it.first == browser.settings.startPage }?.second
        ?: stringResource(R.string.start_panel)
    PushRow(stringResource(R.string.start_page), startPage, look) { open(Page.StartPage) }

    PushRow(stringResource(R.string.gestures), null, look) { open(Page.Gestures) }

    var openInApps by remember { mutableStateOf(browser.settings.openInApps) }
    SwitchRow(stringResource(R.string.open_links_in_apps), openInApps, look) {
        openInApps = it
        browser.settings.openInApps = it
    }

    val answered by browser.sitePermissions.answersFlow.collectAsState()
    PushRow(
        stringResource(R.string.site_settings_global),
        if (answered.isEmpty()) null else answered.keys.size.toString(),
        look,
    ) { open(Page.SiteSettings) }

    PushRow(stringResource(R.string.undo), "${browser.settings.undoSeconds}s", look) {
        open(Page.Undo)
    }

    PushRow(stringResource(R.string.about), null, look) { open(Page.About) }
}

/** The app's own look: dark mode, then what the pages themselves do while it is dark. */
@Composable
private fun AppearancePage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.appearance), look)
    var dark by remember { mutableStateOf(browser.settings.darkMode) }
    listOf(
        "system" to stringResource(R.string.dark_mode_system),
        "light" to stringResource(R.string.dark_mode_light),
        "dark" to stringResource(R.string.dark_mode_dark),
    ).forEach { (id, label) ->
        ChoiceRow(
            label = label,
            chosen = dark == id,
            look = look,
        ) {
            dark = id; browser.settings.darkMode = id
        }
    }

    // What the pages themselves do while the app is dark: each algorithm the engine
    // offers, as the owner asked, with the page-following one ilo ships as its default.
    var pageDark by remember { mutableStateOf(browser.settings.pageDark) }
    Text(
        stringResource(R.string.dark_pages),
        color = look.soft,
        fontSize = Type.small,
        modifier = Modifier.padding(top = 12.dp),
    )
    listOf(
        Triple("follow", R.string.dark_pages_follow, R.string.dark_pages_follow_hint),
        Triple("darken", R.string.dark_pages_darken, R.string.dark_pages_darken_hint),
        Triple("off", R.string.dark_pages_off, R.string.dark_pages_off_hint),
    ).forEach { (id, label, hint) ->
        ChoiceRow(
            label = stringResource(label),
            hint = stringResource(hint),
            chosen = pageDark == id,
            look = look,
        ) {
            pageDark = id; browser.settings.pageDark = id
        }
    }

    // The two switcher dials: the cards' size and how far the stack overlaps, each a share
    // with the owner's number as its default, so the deck the user sees is the one asked for.
    var cardShare by remember { mutableStateOf(browser.settings.switcherCardShare) }
    var overlap by remember { mutableStateOf(browser.settings.switcherOverlap) }
    Text(
        stringResource(R.string.switcher_cards),
        color = look.soft,
        fontSize = Type.small,
        modifier = Modifier.padding(top = 12.dp),
    )
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(stringResource(R.string.card_size), color = look.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
        Chip(
            "-", look, description = stringResource(R.string.card_size_decrease),
        ) {
            cardShare = (cardShare - 0.05f).coerceAtLeast(0.10f)
            browser.settings.switcherCardShare = cardShare
        }
        Text(
            "${(cardShare * 100).roundToInt()}%",
            color = look.text,
            fontSize = 16.sp,
            modifier = Modifier.padding(horizontal = 12.dp),
        )
        Chip(
            "+", look, description = stringResource(R.string.card_size_increase),
        ) {
            cardShare = (cardShare + 0.05f).coerceAtMost(0.95f)
            browser.settings.switcherCardShare = cardShare
        }
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Metrics.Switcher.controlGap),
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(stringResource(R.string.card_overlap), color = look.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
        Chip(
            "-", look, description = stringResource(R.string.card_overlap_decrease),
        ) {
            overlap = (overlap - 0.05f).coerceAtLeast(0.10f)
            browser.settings.switcherOverlap = overlap
        }
        Text(
            "${(overlap * 100).roundToInt()}%",
            color = look.text,
            fontSize = 16.sp,
            modifier = Modifier.padding(horizontal = 12.dp),
        )
        Chip(
            "+", look, description = stringResource(R.string.card_overlap_increase),
        ) {
            overlap = (overlap + 0.05f).coerceAtMost(0.95f)
            browser.settings.switcherOverlap = overlap
        }
    }
}

/** Ad blocking: the master switch, each kind, the lists with when each last fetched, and the
 * trusted sites. */
@Composable
private fun AdBlockingPage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.ad_blocking), look)
    val blockingOn by browser.blocking.isOnFlow.collectAsState()
    var kinds by remember { mutableStateOf(EasyListBlocker.Kind.entries.associateWith { browser.blocking.kindOn(it) }) }
    val sitesOff by browser.blocking.sitesOffFlow.collectAsState()
    var newListUrl by remember { mutableStateOf("") }
    val status by browser.blocking.listStatus.collectAsState()
    val listTimes by browser.blocker.listTimesFlow.collectAsState()

    // uBlock-style updateAfter, 7 days as the simple cadence: opening this submenu refetches
    // once when any tracked list is older, so the rules never go far past their week.
    LaunchedEffect(Unit) {
        val now = System.currentTimeMillis()
        val tracked = EasyListBlocker.Kind.entries.flatMap { browser.blocking.builtInLists(it) } +
            browser.blocking.customListUrls()
        if (tracked.any { (browser.blocker.lastUpdatedOf(it) ?: 0L) < now - 7L * 24 * 3600 * 1000 }) {
            browser.blocking.updateLists()
        }
    }

    // The one switch every site's own row in the actions sheet follows.
    SwitchRow(stringResource(R.string.ad_blocking), blockingOn, look) {
        browser.blocking.toggle(it)
    }

    // Element hiding has its own switch under the master one, rebuilt like a kind's.
    if (blockingOn) {
        var hides by remember { mutableStateOf(browser.blocker.hidesOn()) }
        Column {
            KindRow(
                label = stringResource(R.string.hide_ad_elements),
                on = hides,
                look = look,
            ) { on ->
                hides = on; browser.blocker.setHides(on)
            }
            Text(
                stringResource(R.string.hide_ad_elements_hint),
                color = look.soft,
                fontSize = Type.small,
                modifier = Modifier.padding(horizontal = Metrics.Column.inset),
            )
        }
    }

    // Each kind of blocking has its own switch, kept under the master one as the iPhone's.
    if (blockingOn) {
        listOf(
            EasyListBlocker.Kind.ADS to R.string.block_ads_trackers,
            EasyListBlocker.Kind.TRACKERS to R.string.block_trackers,
            EasyListBlocker.Kind.COOKIES to R.string.block_cookie_banners,
        ).forEach { (kind, label) ->
            KindRow(
                label = stringResource(label),
                on = kinds[kind] == true,
                look = look,
            ) { on ->
                kinds = kinds + (kind to on); browser.blocking.setKind(kind, on)
            }
        }
    }

    Text(
        stringResource(R.string.filter_lists),
        color = look.text,
        fontSize = 16.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    )
    // The built-in lists, friendly names from the store's strings, each with when it last
    // fetched well, so a reader sees how old its rules are.
    val friendly = mapOf(
        "easylist.txt" to stringResource(R.string.easylist_name),
        "easyprivacy.txt" to stringResource(R.string.easyprivacy_name),
        "easylist-cookies.txt" to stringResource(R.string.easylist_cookies_name),
        "ublock-cookies.txt" to stringResource(R.string.ublock_cookies_name),
        "ublock-filters.txt" to stringResource(R.string.ublock_filters_name),
        "adguard-mobile.txt" to stringResource(R.string.adguard_mobile_name),
    )
    (browser.blocking.builtInLists(EasyListBlocker.Kind.ADS) +
        browser.blocking.builtInLists(EasyListBlocker.Kind.TRACKERS) +
        browser.blocking.builtInLists(EasyListBlocker.Kind.COOKIES))
        .forEach { file ->
            Column(Modifier.padding(horizontal = Metrics.Column.inset)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        friendly[file] ?: file,
                        color = look.text,
                        fontSize = 14.sp,
                        maxLines = 1,
                        modifier = Modifier.weight(1f),
                    )
                    val kind = EasyListBlocker.Kind.entries.firstOrNull { file in it.files }
                    Text(
                        if (kinds[kind] == true) stringResource(R.string.on) else stringResource(R.string.off),
                        color = look.soft,
                        fontSize = 14.sp,
                    )
                }
                Text(
                    when (val at = listTimes[file] ?: 0L) {
                        0L -> stringResource(R.string.never_updated)
                        else -> stringResource(R.string.updated, describeAge(at))
                    },
                    color = look.soft,
                    fontSize = Type.small,
                )
            }
        }
    Text(
        stringResource(R.string.list_stale),
        color = look.soft,
        fontSize = Type.small,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
    )

    // The lists a user added, each with its way out.
    browser.blocking.customListUrls().sorted().forEach { url ->
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .padding(start = Metrics.Column.inset, end = Metrics.Column.inset),
        ) {
            Text(url, color = look.soft, fontSize = 14.sp, maxLines = 1, modifier = Modifier.weight(1f))
            Chip(stringResource(R.string.remove), look) { browser.blocking.removeList(url) }
        }
    }

    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Column.inset)
            .padding(top = 4.dp, bottom = 12.dp),
    ) {
        Box(
            Modifier
                .weight(1f)
                .heightIn(min = Metrics.Bar.height)
                .background(look.plate, RoundedCornerShape(Metrics.Bar.radius)),
            contentAlignment = Alignment.CenterStart,
        ) {
            BasicField(
                value = newListUrl,
                onValueChange = { newListUrl = it },
                hint = stringResource(R.string.filter_list_url),
                look = look,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp),
                onSubmit = {
                    addList(browser, newListUrl) { newListUrl = "" }
                },
            )
        }
        Chip(stringResource(R.string.add), look) {
            addList(browser, newListUrl) { newListUrl = "" }
        }
    }
    var listsUpdated by remember { mutableStateOf(false) }
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Column.inset)
            .padding(bottom = 8.dp),
    ) {
        // Update refetches the built-ins; the status flow says how it went while it runs,
        // and the flag catches the success the flow reports only as a return to null.
        Chip(stringResource(R.string.filter_list_update), look) {
            listsUpdated = false
            browser.blocking.updateLists()
        }
        LaunchedEffect(Unit) {
            var wasUpdating = false
            browser.blocking.listStatus.collect { s ->
                if (s == "update") wasUpdating = true
                if (wasUpdating && s == null) {
                    listsUpdated = true
                    wasUpdating = false
                }
            }
        }
        when {
            status == "update" ->
                Text(stringResource(R.string.filter_list_updating), color = look.soft, fontSize = 14.sp)
            status != null && status!!.endsWith(" failed") ->
                Text(stringResource(R.string.filter_list_failed), color = look.alarm, fontSize = 14.sp)
            listsUpdated ->
                Text(stringResource(R.string.filter_list_updated), color = look.soft, fontSize = 14.sp)
            status != null ->
                Text(stringResource(R.string.filter_list_loading), color = look.soft, fontSize = 14.sp)
        }
    }

    Text(
        stringResource(R.string.trusted_sites),
        color = look.text,
        fontSize = 16.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    )
    // The sites blocking is off for, each with the way back to blocking it.
    if (sitesOff.isEmpty()) {
        Text(
            stringResource(R.string.trusted_sites_empty),
            color = look.soft,
            fontSize = 14.sp,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
        )
    }
    sitesOff.sorted().forEach { site ->
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .padding(start = Metrics.Column.inset, end = Metrics.Column.inset),
        ) {
            Text(site, color = look.soft, fontSize = 14.sp, maxLines = 1, modifier = Modifier.weight(1f))
            Chip(stringResource(R.string.block_on_site), look) { browser.blocking.setSite(site, true) }
        }
    }
    var newSite by remember { mutableStateOf("") }
    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Column.inset)
            .padding(top = 4.dp, bottom = 12.dp),
    ) {
        Box(
            Modifier
                .weight(1f)
                .heightIn(min = Metrics.Bar.height)
                .background(look.plate, RoundedCornerShape(Metrics.Bar.radius)),
            contentAlignment = Alignment.CenterStart,
        ) {
            BasicField(
                value = newSite,
                onValueChange = { newSite = it },
                hint = stringResource(R.string.site_address),
                look = look,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp),
                onSubmit = { trustSite(browser, newSite) { newSite = "" } },
            )
        }
        Chip(stringResource(R.string.add), look) {
            trustSite(browser, newSite) { newSite = "" }
        }
    }
}

/** The search engine, in its own sheet as before, reached from the root now. */
@Composable
private fun SearchPage(browser: Browser, look: Look, openEngines: () -> Unit) {
    SubTitle(stringResource(R.string.search_engine), look)
    val chosenEngine = browser.settings.searchEngine
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Metrics.Column.gap),
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(role = Role.Button) { openEngines() }
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(stringResource(R.string.search_engine), color = look.text, fontSize = 16.sp, maxLines = 1, modifier = Modifier.weight(1f))
        Text(chosenEngine.name, color = look.soft, fontSize = 14.sp, maxLines = 1)
        EngineMark(chosenEngine, browser.favicons, look)
    }
}

/** The language pages are put into: the phone's own by default, or one the user picks. */
@Composable
private fun TranslationPage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.translation_language), look)
    var target by remember { mutableStateOf(browser.settings.translationLanguage) }
    // The phone's language is the default, so the offer never needs a choice to work.
    ChoiceRow(
        label = stringResource(R.string.translate_phone_language),
        chosen = target == null,
        look = look,
    ) {
        target = null; browser.settings.translationLanguage = null
    }
    Text(
        stringResource(R.string.translation_choose),
        color = look.soft,
        fontSize = Type.small,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
    )
    pickableLanguages().forEach { (name, code) ->
        ChoiceRow(
            label = name,
            chosen = target != null &&
                java.util.Locale.forLanguageTag(target).language == code,
            look = look,
        ) {
            target = code
            browser.settings.translationLanguage = code
        }
    }
}

/** The languages a page can be put into, as the phone itself names them, one of each. */
private fun pickableLanguages(): List<Pair<String, String>> =
    java.util.Locale.getAvailableLocales()
        .filter { it.displayLanguage.isNotBlank() }
        .distinctBy { it.displayLanguage }
        .map { it.displayLanguage to it.language }
        .sortedBy { it.first }

/** The target's name as ToolsModal's translate action names it: the phone's language or the
 * chosen one, resolved, since the action is about the language it puts the page into. */
internal fun translationName(target: String?): String =
    if (target == null) phoneLanguage()
    else java.util.Locale.forLanguageTag(target).displayLanguage.ifBlank { target }

/** The Settings root row's own value: the state itself, so a reset reads as a reset and not as
 * a language picked behind the user's back. */
internal fun translationStateName(target: String?): String? =
    if (target == null) null
    else java.util.Locale.forLanguageTag(target).displayLanguage.ifBlank { target }

/** The target's BCP-47 code, what the translate call takes. */
internal fun translationCode(target: String?): String =
    target ?: phoneCode()

/** The age in the few words every span needs, for the updated line under a list. */
private fun describeAge(at: Long): String {
    val days = java.util.concurrent.TimeUnit.MILLISECONDS.toDays(System.currentTimeMillis() - at).toInt()
    return when {
        days <= 0 -> "today"
        days == 1 -> "yesterday"
        days < 30 -> "$days days ago"
        else -> "${days / 30} months ago"
    }
}

/** The phone's own language, the one a page is put into. */
internal fun phoneCode() = java.util.Locale.getDefault().language

/** The phone's own language's name, as the phone itself says it. */
internal fun phoneLanguage() = java.util.Locale.getDefault().displayLanguage

/** What a Space with no tabs opens, and how many recent pages the panel shows as tiles. */
@Composable
private fun StartPagePage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.start_page), look)
    var startPage by remember { mutableStateOf(browser.settings.startPage) }
    listOf(
        "panel" to stringResource(R.string.start_panel),
        "last" to stringResource(R.string.start_last),
        "search" to stringResource(R.string.start_search),
        "page" to stringResource(R.string.start_page_url),
    ).forEach { (id, label) ->
        ChoiceRow(
            label = label,
            chosen = startPage == id,
            look = look,
        ) {
            startPage = id; browser.settings.startPage = id
        }
    }
    // Only the page choice has an address; the others would carry a dead field.
    if (startPage == "page") {
        var pageUrl by remember { mutableStateOf(browser.settings.startPageUrl) }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = Metrics.Column.inset)
                .padding(top = 4.dp, bottom = 12.dp),
        ) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = Metrics.Bar.height)
                    .background(look.plate, RoundedCornerShape(Metrics.Bar.radius)),
                contentAlignment = Alignment.CenterStart,
            ) {
                BasicField(
                    value = pageUrl,
                    onValueChange = { pageUrl = it },
                    hint = browser.settings.startPageUrl,
                    look = look,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp),
                    onSubmit = { browser.settings.startPageUrl = pageUrl.trim() },
                )
            }
        }
    }
    // How many recent pages the panel shows as tiles, 0 hiding them.
    var recentTiles by remember { mutableStateOf(browser.settings.recentTiles) }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(
            stringResource(R.string.recent_tiles),
            color = look.text,
            fontSize = 16.sp,
            modifier = Modifier.weight(1f),
        )
        Chip("-", look, description = stringResource(R.string.recent_tiles_decrease)) { recentTiles = (recentTiles - 1).coerceAtLeast(0); browser.settings.recentTiles = recentTiles }
        Text("$recentTiles", color = look.text, fontSize = 16.sp, modifier = Modifier.padding(horizontal = 12.dp))
        Chip("+", look, description = stringResource(R.string.recent_tiles_increase)) { recentTiles = (recentTiles + 1).coerceAtMost(16); browser.settings.recentTiles = recentTiles }
    }
}

/** What the switcher's own swipes do, as the owner's spec named them. */
@Composable
private fun GesturesPage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.gestures), look)
    var swipeUp by remember { mutableStateOf(browser.settings.swipeUpAction) }
    var swipeDown by remember { mutableStateOf(browser.settings.swipeDownAction) }
    Text(
        stringResource(R.string.swipe_up),
        color = look.soft,
        fontSize = 14.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
    )
    listOf(
        "close" to stringResource(R.string.swipe_action_close),
        "archive" to stringResource(R.string.swipe_action_archive),
        "none" to stringResource(R.string.swipe_action_none),
    ).forEach { (id, label) ->
        ChoiceRow(
            label = label,
            chosen = swipeUp == id,
            look = look,
        ) {
            swipeUp = id; browser.settings.swipeUpAction = id
        }
    }
    Text(
        stringResource(R.string.swipe_down),
        color = look.soft,
        fontSize = 14.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
    )
    listOf(
        "archive" to stringResource(R.string.swipe_action_archive),
        "none" to stringResource(R.string.swipe_action_none),
    ).forEach { (id, label) ->
        ChoiceRow(
            label = label,
            chosen = swipeDown == id,
            look = look,
        ) {
            swipeDown = id; browser.settings.swipeDownAction = id
        }
    }

    var swipeReach by remember { mutableStateOf(browser.settings.swipeReach) }
    Text(
        stringResource(R.string.swipe_reach),
        color = look.soft,
        fontSize = 14.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
    )
    listOf(
        0.25f to stringResource(R.string.swipe_reach_light),
        0.5f to stringResource(R.string.swipe_reach_half),
        1f to stringResource(R.string.swipe_reach_full),
    ).forEach { (reach, label) ->
        ChoiceRow(
            label = label,
            chosen = swipeReach == reach,
            look = look,
        ) {
            swipeReach = reach; browser.settings.swipeReach = reach
        }
    }
}

/** Every site's own answers, apart: each kind's default, and the sites that answered for
 * themselves, each with the way back to asking. */
@Composable
private fun SiteSettingsPage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.site_settings_global), look)
    Text(
        stringResource(R.string.permissions_title),
        color = look.text,
        fontSize = 16.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    )
    // A press walks a kind's default round: asking, allowed, blocked, back to asking, as the
    // three answers are all a default can be.
    SitePermission.entries.forEach { kind ->
        val label = stringResource(
            when (kind) {
                SitePermission.LOCATION -> R.string.permission_location
                SitePermission.CAMERA -> R.string.permission_camera
                SitePermission.MICROPHONE -> R.string.permission_microphone
                SitePermission.NOTIFICATIONS -> R.string.permission_notifications
            }
        )
        val answers = listOf(
            SiteAnswer.ASK to stringResource(R.string.permissions_ask),
            SiteAnswer.ALLOW to stringResource(R.string.permissions_allow),
            SiteAnswer.BLOCK to stringResource(R.string.permissions_block),
        )
        var chosen by remember { mutableStateOf(browser.sitePermissions.defaultOf(kind)) }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .clickable(role = Role.Button) {
                    val at = answers.indexOfFirst { it.first == chosen }
                    chosen = answers[(at + 1) % answers.size].first
                    browser.sitePermissions.setDefault(kind, chosen)
                }
                .padding(horizontal = Metrics.Column.inset),
        ) {
            Text(
                label,
                color = look.text,
                fontSize = 16.sp,
                modifier = Modifier.weight(1f),
            )
            Text(
                answers.firstOrNull { it.first == chosen }?.second ?: "",
                color = look.soft,
                fontSize = 14.sp,
            )
        }
    }

    Text(
        stringResource(R.string.site_exceptions),
        color = look.text,
        fontSize = 16.sp,
        modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
    )
    val answered by browser.sitePermissions.answersFlow.collectAsState()
    if (answered.isEmpty()) {
        Text(
            stringResource(R.string.site_exceptions_empty),
            color = look.soft,
            fontSize = 14.sp,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 4.dp),
        )
    }
    answered.keys.sorted().forEach { site ->
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .padding(horizontal = Metrics.Column.inset),
        ) {
            Column(Modifier.weight(1f)) {
                Text(site, color = look.text, fontSize = 14.sp, maxLines = 1)
                val kinds = answered[site] ?: emptyMap()
                Text(
                    kinds.entries.joinToString { it.key.name.lowercase() },
                    color = look.soft,
                    fontSize = Type.small,
                    maxLines = 1,
                )
            }
            // The site's answers go entirely, so every page of it asks again.
            Chip(stringResource(R.string.remove), look) { browser.sitePermissions.clearSite(site) }
        }
    }
}

/** How long a thrown card's undo notice waits, as the owner asked for the user's own span. */
@Composable
private fun UndoPage(browser: Browser, look: Look) {
    SubTitle(stringResource(R.string.undo), look)
    var undoSeconds by remember { mutableStateOf(browser.settings.undoSeconds) }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(
            stringResource(R.string.undo),
            color = look.text,
            fontSize = 16.sp,
            modifier = Modifier.weight(1f),
        )
        Chip("-", look, description = stringResource(R.string.undo_seconds_decrease)) { undoSeconds = (undoSeconds - 1).coerceAtLeast(1); browser.settings.undoSeconds = undoSeconds }
        Text("$undoSeconds", color = look.text, fontSize = 16.sp, modifier = Modifier.padding(horizontal = 12.dp))
        Chip("+", look, description = stringResource(R.string.undo_seconds_increase)) { undoSeconds = (undoSeconds + 1).coerceAtMost(30); browser.settings.undoSeconds = undoSeconds }
    }
}

/** The app's own version and licence, as the root's About row opens. */
@Composable
private fun AboutPage(look: Look) {
    SubTitle(stringResource(R.string.about), look)
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(
            stringResource(R.string.about_version),
            color = look.text,
            fontSize = 16.sp,
            maxLines = 1,
            modifier = Modifier.weight(1f),
        )
        Text(stringResource(R.string.about_license), color = look.soft, fontSize = 14.sp)
    }
}

/** Reads the field where it stands, then clears it: the fetch runs after, and would
 * otherwise carry the empty field into it. */
private fun addList(browser: Browser, field: String, clear: () -> Unit) {
    if (field.isBlank()) return
    val url = field.trim()
    clear()
    browser.blocking.addList(url)
}

/** A site typed by hand goes straight to the store, as the page's own row writes it: any
 * scheme or path the user typed is stripped to the host the store keeps. */
private fun trustSite(browser: Browser, field: String, clear: () -> Unit) {
    if (field.isBlank()) return
    val host = app.ilo.browser.android.blocking.EasyListBlocker.hostOf("https://${field.trim()}") ?: field.trim()
    browser.blocking.setSite(host, false)
    clear()
}

/** A row that pushes a submenu, its label and what the submenu stands at, with the chevron
 * every such row carries so it reads as a way in. */
@Composable
private fun PushRow(label: String, value: String?, look: Look, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(label, color = look.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
        if (value != null) {
            Text(value, color = look.soft, fontSize = 14.sp, maxLines = 1)
        }
        Icon(
            Icons.AutoMirrored.Filled.KeyboardArrowRight,
            null,
            tint = look.soft,
            modifier = Modifier.semantics { contentDescription = label },
        )
    }
}

/** The master switch row, the whole row turning it as the kind rows do. The row carries the
 * switch's checked state, so a screen reader gets one target that says on or off. */
@Composable
private fun SwitchRow(label: String, on: Boolean, look: Look, onCheckedChange: (Boolean) -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(
                value = on,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .padding(start = Metrics.Column.inset),
    ) {
        // No glyph: the switch is the row's own mark, and a shared glyph on every row said
        // nothing about any of them.
        Text(
            label,
            color = look.text,
            fontSize = 16.sp,
            modifier = Modifier
                .weight(1f)
                .padding(start = Metrics.Column.gap),
        )
        Switch(
            checked = on,
            onCheckedChange = null,
            colors = SwitchDefaults.colors(checkedTrackColor = look.accent),
            modifier = Modifier.padding(end = Metrics.Column.inset),
        )
    }
}

/** A switch row for one kind of blocking, the whole row turning it like the master's, its
 * state on the row as the master's is. */
@Composable
private fun KindRow(label: String, on: Boolean, look: Look, onCheckedChange: (Boolean) -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .toggleable(
                value = on,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .padding(start = Metrics.Column.inset),
    ) {
        Text(label, color = look.text, fontSize = 16.sp, modifier = Modifier.weight(1f))
        Switch(
            checked = on,
            onCheckedChange = null,
            colors = SwitchDefaults.colors(checkedTrackColor = look.accent),
            modifier = Modifier.padding(end = Metrics.Column.inset),
        )
    }
}

/** The line between the sheet's groups, as the iOS settings sheet's rules are. */
@Composable
private fun Rule(look: Look) {
    Box(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Column.inset)
            .padding(vertical = 8.dp)
            .height(1.dp)
            .background(look.text.copy(alpha = 0.12f))
    )
}

/** A row of words to press, as the iOS RowLabel without an icon. */
@Composable
private fun TextRow(label: String, look: Look, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = Metrics.Column.inset),
    ) {
        Text(label, color = look.text, fontSize = 16.sp, maxLines = 1)
    }
}

/** A row that picks one of a few, with the mark of the one that's picked at its end. A hint
 * under the label says what the choice does, when one row needs its own words. The row
 * carries the picked state, so a screen reader gets one target that says picked or not. */
@Composable
private fun ChoiceRow(label: String, chosen: Boolean, look: Look, hint: String? = null, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .selectable(
                selected = chosen,
                role = Role.RadioButton,
                onClick = onClick,
            )
            .padding(start = Metrics.Column.inset, end = Metrics.Column.inset),
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, color = look.text, fontSize = 16.sp, maxLines = 1)
            if (hint != null) {
                Text(hint, color = look.soft, fontSize = Type.small, maxLines = 1)
            }
        }
        RadioButton(
            selected = chosen,
            onClick = null,
            colors = RadioButtonDefaults.colors(selectedColor = look.accent),
        )
    }
}

/** A small plate of words to press, as the iOS ChipStyle: 34 tall, radius 11, inset 12.
 * A symbol-only chip takes a description, so a screen reader says what it does. */
@Composable
private fun Chip(label: String, look: Look, description: String? = null, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .height(34.dp)
            .background(look.button, RoundedCornerShape(11.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { description?.let { contentDescription = it } }
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = look.text, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}
/** The ask a page's link names another app: the caller resolves the target's name and icon
 * when it can, and each answer goes back as one callback. */
@Composable
fun OpenInAppSheet(
    decision: ExternalDecision.OpenInApp,
    appName: String?,
    icon: android.graphics.drawable.Drawable?,
    look: Look,
    onAnswer: (open: Boolean) -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = Metrics.Column.side + Metrics.Column.inset)) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(top = 16.dp, bottom = 8.dp),
        ) {
            icon?.let { appIcon ->
                // Adaptive icons draw only onto a canvas, so the icon becomes a bitmap once,
                // the way every other picture in the app travels.
                val painted = remember(appIcon) {
                    val bmp = android.graphics.Bitmap.createBitmap(96, 96, android.graphics.Bitmap.Config.ARGB_8888)
                    val canvas = android.graphics.Canvas(bmp)
                    appIcon.setBounds(0, 0, 96, 96)
                    appIcon.draw(canvas)
                    bmp
                }
                Image(
                    painted.asImageBitmap(),
                    contentDescription = null,
                    modifier = Modifier.size(36.dp).clip(RoundedCornerShape(9.dp)),
                )
                Spacer(Modifier.width(12.dp))
            }
            Text(
                text = stringResource(R.string.open_in_app_title, appName ?: decision.appName),
                color = look.text,
                fontSize = 24.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
        if (appName == null) {
            // The guess is a package tail or a host: shown soft so the ask is still honest.
            Text(
                text = decision.appName,
                color = look.soft,
                fontSize = 13.sp,
                modifier = Modifier.padding(bottom = 12.dp),
            )
        }
        Plate(
            text = stringResource(R.string.open_in_app_confirm),
            look = look,
            primary = true,
        ) { onAnswer(true) }
        Spacer(Modifier.height(8.dp))
        Plate(
            text = stringResource(R.string.open_in_app_cancel),
            look = look,
        ) { onAnswer(false) }
        Spacer(Modifier.height(16.dp))
    }
}

/** A full-width button on a plate, CertificateSheet's own shape. The ask's answer it names
 * is the primary one when primary is set, so the two choices are told apart at a glance while
 * both stay equally easy to press. */
@Composable
private fun Plate(text: String, look: Look, primary: Boolean = false, onClick: () -> Unit) {
    val ground = if (primary) look.accent else look.plate
    val words = if (primary) Look.Paper else look.text
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = Metrics.Bar.height)
            .background(ground, RoundedCornerShape(Metrics.Bar.radius))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = text },
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = words, fontSize = 17.sp, fontWeight = if (primary) FontWeight.SemiBold else null)
    }
}
