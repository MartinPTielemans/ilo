package app.ilo.browser.android

import android.os.Bundle
import android.content.res.Configuration
import android.view.View
import android.view.ViewGroup
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.compose.BackHandler
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.core.view.WindowCompat
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.FullscreenHost
import app.ilo.browser.android.browser.SyncLane
import app.ilo.browser.android.browser.host
import app.ilo.browser.android.blocking.EasyListBlocker
import app.ilo.browser.android.sync.FileSecretStore
import app.ilo.browser.android.sync.FileStateStore
import app.ilo.browser.android.sync.SyncedAccount
import app.ilo.browser.android.ui.AccountScreen
import app.ilo.browser.android.ui.IloTheme
import app.ilo.browser.android.ui.ListScreen
import app.ilo.browser.android.ui.LocalLook
import app.ilo.browser.android.ui.Look
import app.ilo.browser.android.ui.PageScreen
import app.ilo.browser.android.ui.SettingsScreen
import app.ilo.browser.android.ui.SpaceScreen
import app.ilo.browser.android.ui.Switcher

/** The single-activity host: the Space, the page over it, Settings and the account. */
class IloActivity : ComponentActivity(), FullscreenHost {
    private lateinit var account: SyncedAccount
    private lateinit var browser: Browser

    /** The page's fullscreen video while it is up, over the Compose tree and the bars. */
    private var fullscreenView: View? = null

    // The system's own permission ask, standing behind a page's: its answer answers the page.
    // The contract's callback carries no request code, so the code rides in a field, which one
    // ask at a time keeps sound.
    private var osRequestCode = 0
    private val osPermissionAsk =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { answers ->
            browser.onOsPermissionResult(osRequestCode, answers.values.all { it })
        }

    /** The fill a sign-in asked for, held while the phone's lock asks its owner to say yes. */
    private var pendingFill: (() -> Unit)? = null
    private val fillConfirm =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { answer ->
            if (answer.resultCode == RESULT_OK) pendingFill?.invoke()
            pendingFill = null
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        account = SyncedAccount(FileSecretStore(filesDir), FileStateStore(filesDir))
        browser = Browser(this, EasyListBlocker(this))
        // The lane is made once both ends exist: it keeps the browser's Spaces and tabs on the
        // account's word, and nothing else does.
        SyncLane(account, browser.state.host())
        browser.accountHook = account
        browser.osPermissionBridge = { permissions, requestCode ->
            osRequestCode = requestCode
            osPermissionAsk.launch(permissions)
        }
        // A fill asks the phone's owner to prove they're them first, as Chrome's does: a device
        // with no lock lets the system's own credential screen say yes or no.
        browser.fillConfirmer = { _, then ->
            val keyguard = getSystemService(android.app.KeyguardManager::class.java)
            if (keyguard?.isDeviceSecure == true) then()
            else {
                pendingFill = then
                val ask = keyguard?.createConfirmDeviceCredentialIntent(null, null)
                if (ask == null) { pendingFill = null } else fillConfirm.launch(ask)
            }
        }
        // A tab that never picked its own dark follows the general setting, resolved against
        // the system's dark as it is when each page is drawn, so the app's dark choice holds
        // even when the system's differs.
        browser.engine.contentDarkDefault = {
            browser.settings.resolvedDark(
                (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
                    Configuration.UI_MODE_NIGHT_YES
            )
        }
        browser.engine.fullscreenHost = this
        handleViewIntent(intent)
        setContent {
            // The theme follows the setting as it changes, not just at launch.
            val darkMode by browser.settings.darkModeFlow.collectAsState()
            val dark = when (darkMode) {
                "light" -> false
                "dark" -> true
                else -> null
            }
            var inSettings by remember { mutableStateOf(false) }
            var finding by remember { mutableStateOf(false) }
            // The switcher sits above the page and the Space both: its own throws can take the
            // last tab and unmount the page under it, and the switcher stays through that.
            var switcherOpen by remember { mutableStateOf(false) }
            var archiveOpen by remember { mutableStateOf(false) }
            // The status bar's icons follow the surface actually shown, which the app's own
            // dark-mode setting picks, not Android's uiMode.
            val systemDark = isSystemInDarkTheme()
            androidx.compose.runtime.LaunchedEffect(dark, systemDark) {
                val lightBars = dark == false || (dark == null && !systemDark)
                window?.let { WindowCompat.getInsetsController(it, it.decorView).isAppearanceLightStatusBars = lightBars }
            }
            val spaces by browser.state.spaces.collectAsState()
            val currentSpace by browser.state.currentSpace.collectAsState()
            val tabs by browser.state.tabs.collectAsState()
            val selectedId by browser.state.selectedId.collectAsState()
            // Compose tracks what's collected, so the tab comes out of these, never from a
            // StateFlow's value read straight: that would freeze the route where it started.
            val selectedTab = tabs.firstOrNull { it.id == selectedId }
            // The back key leaves a fullscreen video before anything else, as Chrome's does.
            val fullscreenTabId by browser.engine.fullscreenTab.collectAsState()
            BackHandler(enabled = fullscreenTabId != null) { browser.engine.exitFullscreen() }
            // The app sits in the Space's own look: a private page's chrome builds its ink look
            // itself, as iOS's privateChrome covers the page and its sheets, never the Space.
            val look = Look.of(dark ?: systemDark, spaces.firstOrNull { it.id == currentSpace }?.color)
            androidx.compose.runtime.CompositionLocalProvider(LocalLook provides look) {
                IloTheme(darkOverride = dark) {
                    if (inSettings) {
                        BackHandler { inSettings = false }
                        SettingsScreen(
                            browser = browser,
                            account = account,
                            accountScreen = { onBack -> AccountScreen(account, onBack = onBack) },
                            onBack = { inSettings = false },
                        )
                    } else {
                        if (selectedTab != null) {
                            PageScreen(
                                browser = browser,
                                tab = selectedTab,
                                onLeave = { browser.state.leaveToSpace() },
                                findingParam = finding,
                                onFindDone = { finding = false },
                                onOpenSwitcher = { switcherOpen = true },
                                onOpenSettings = { inSettings = true },
                            )
                        } else {
                            SpaceScreen(
                                browser = browser,
                                onSettings = { inSettings = true },
                                onOpenSwitcher = { switcherOpen = true },
                            )
                        }
                        // The switcher over both: its throws can take the last tab, and it
                        // stays through that, over the Space the throw leaves.
                        if (switcherOpen) {
                            Switcher(
                                browser = browser,
                                tab = selectedTab,
                                look = look,
                                account = account,
                                onOpen = { opened ->
                                    // The page left keeps its picture, as iOS's switch keeps it.
                                    selectedTab?.let { browser.keepPicture(it) }
                                    browser.state.select(opened.id)
                                    switcherOpen = false
                                },
                                onOpenArchive = { archiveOpen = true },
                                onClose = { switcherOpen = false },
                            )
                        }
                        // The archive list from the switcher's own row, closing back to it; a
                        // restore opens its tab, so both go with it, as iOS's archive row does.
                        if (archiveOpen) {
                            ListScreen(
                                which = "archive",
                                browser = browser,
                                look = look,
                                tab = selectedTab,
                                onOpen = { switcherOpen = false; archiveOpen = false },
                                onClose = { archiveOpen = false },
                            )
                        }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        handleViewIntent(intent)
    }

    /** The page's fullscreen video goes over the Compose tree itself, so no chrome shows through,
     * and the system bars go with it: the app already draws edge to edge. */
    override fun showFullscreen(view: View) {
        val root = window.decorView as? ViewGroup ?: return
        fullscreenView?.let { root.removeView(it) }
        root.addView(
            view,
            ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        )
        fullscreenView = view
        WindowCompat.getInsetsController(window, view).apply {
            systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            hide(WindowInsetsCompat.Type.systemBars())
        }
    }

    /** Taking the view down brings the bars back; with none up, a call does nothing. */
    override fun hideFullscreen() {
        val view = fullscreenView ?: return
        (view.parent as? ViewGroup)?.removeView(view)
        fullscreenView = null
        WindowCompat.getInsetsController(window, view).apply {
            systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            show(WindowInsetsCompat.Type.systemBars())
        }
    }

    /** A link sent from outside opens as its own page, cold start or running app the same: an
     * address the Space holds already is shown, not opened twice. */
    private fun handleViewIntent(intent: android.content.Intent?) {
        val url = intent?.dataString ?: return
        if (!url.startsWith("http")) return
        browser.openInNewTab(url)
    }

    override fun onStart() {
        super.onStart()
        // The relay listens only while the app is in front.
        account.setActive(true)
        // The pages the stop paused go on again, where they stood.
        browser.engine.resumeScripts()
    }

    override fun onStop() {
        super.onStop()
        account.setActive(false)
        // No page keeps its scripts and timers on the battery while the user is elsewhere.
        browser.engine.pauseScripts()
    }
}