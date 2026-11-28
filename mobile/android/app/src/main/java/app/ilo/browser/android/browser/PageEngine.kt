package app.ilo.browser.android.browser

import android.annotation.SuppressLint
import app.ilo.browser.android.blocking.EasyListBlocker
import android.graphics.Bitmap
import android.net.http.SslError
import android.os.Bundle
import android.view.View
import android.view.ViewTreeObserver
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebViewFeature
import java.security.cert.X509Certificate
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

/** How a page's engine talks to the app: loads, titles and certificate faults. */
interface PageListener {
    /** A page finished loading at `url`, with its `title`. */
    fun loaded(tabId: String, url: String, title: String) {}
    /** A page's progress, 0 to 100. */
    fun progress(tabId: String, value: Int) {}
    /** A page began loading: the load's indicator starts here, not at the first progress,
     * which can come late or never for a small page. */
    fun loadStarted(tabId: String) {}
    /** A page came to its end, drawn or failed: the load's indicator must stop here, since
     * WebView's own progress reaches 100 before subframes finish. */
    fun loadFinished(tabId: String) {}
    fun favicon(tabId: String, color: Long) {}
    /** The icon a page's site gave for itself, learned for its host; never asked of a private
     * tab's page, whose icon is kept of nothing. */
    fun pageIcon(tabId: String, url: String, icon: Bitmap) {}
    /** A page finished drawing, so its preview can be taken while the page is fresh, not
     * only when its tab is left. */
    fun pageReady(tabId: String, url: String) {}
    /** The color the page's own theme-color meta named, or null when it named none. */
    fun themeColor(tabId: String, color: Long?) {}
    /** The page wants a new window, which becomes a tab; the engine supplies the WebView for it. */
    fun newWindow(openerId: String, webView: WebView) {}
    /** A long press on a link: the address is given, and the sheet that offers the two ways
     * to open it elsewhere is the host's to show. */
    fun linkLongPressed(tabId: String, url: String) {}
    /** A page asked for an app outside the browser: the host resolves the target, shows the
     * ask, and either launches it or falls back to `fallbackUrl`. */
    fun externalLinkAsked(tabId: String, url: String, appName: String?, fallbackUrl: String?) {}
}

/** Whether a page's ad elements are hidden on it, from the choices that decide: the master
 * switch, the hide toggle, the site's exception with the tab's held choice over it, and the
 * warning standing in the page's place. Kept pure so a unit test answers it. */
internal fun hidesWanted(
    url: String?, blockingOn: Boolean, hidesOn: Boolean,
    siteOff: Boolean, heldChoice: Boolean?, faultUp: Boolean,
): Boolean {
    if (url == null || !blockingOn || !hidesOn || faultUp) return false
    if (heldChoice == false) return false
    // A tab's held-on choice runs the hides over the site's own exception.
    if (heldChoice == true) return true
    return !siteOff
}

/**
 * Owns the WebViews, one per tab, created once and reused: a tab switch only shows the cached
 * WebView again. Beyond [BrowserState.liveLimit] live tabs, the least recently used ones are
 * frozen: their WebView's state is saved onto the tab and the WebView destroyed, brought back
 * on the next visit.
 */
class PageEngine(
    private val state: BrowserState,
    private val blocker: app.ilo.browser.android.blocking.Blocker,
    private val listener: PageListener,
) {
    private val webViews = ConcurrentHashMap<String, WebView>()
    /** The blocking choices a tab holds of its own, by site, overriding the site's choice for
     * that tab alone, as iOS's per-view hold does: a private tab's choice is its own, never
     * written anywhere, and dies with the tab. True names blocking, false its pause. */
    private val heldChoices = ConcurrentHashMap<String, MutableMap<String, Boolean>>()
    /** What the phone remembers of pages it was on; private pages never reach it. */
    var stores: PersonalStores? = null
    /** Runs a download through the system's own service, so the file lands in the app's public
     * downloads with its own notification, and the record comes back to the list. */
    var downloadStarter: ((url: String, name: String) -> Long)? = null
    /** What a page's own dark is when its tab hasn't picked one: the app's general setting,
     * supplied by the host, which knows the system's dark. */
    var contentDarkDefault: (() -> Boolean)? = null
    /** A pending SSL decision for a tab, answered by the warning sheet's buttons. */
    val pendingSsl = ConcurrentHashMap<String, Pair<SslErrorHandler, CertificateFault>>()
    /** Whether the user chose links to open in other apps, supplied by the host that holds
     * the setting; false keeps every link in the page, as before. */
    var openInApps: (() -> Boolean)? = null
    /** The site permission answers, kept per host; the engine reads them when a page asks. */
    var sitePermissions: SitePermissionStore? = null
    /** A page asked for a permission its host holds no answer for: the app shows its own ask,
     * and answers the request through the returned grant or deny. Set by the host, which owns
     * the UI; null denies, as the engine's own default always was. */
    var permissionAsker: ((tab: Tab, host: String, kind: SitePermission, request: android.webkit.PermissionRequest) -> Unit)? = null
    /** Whether the app itself holds the OS grant for a kind, supplied by the host that has the
     * context; null counts as held, as the engine's own default always was. */
    var osGrantCheck: ((kind: SitePermission) -> Boolean)? = null
    /** The addresses whose warnings Go back turned away, kept until another load starts: the
     * failed load's own aftermath must not stand them back up, as iOS's turnedAway keeps the
     * empty page after a refusal from marking. */
    val turnedAway = ConcurrentHashMap<String, String>()
    /** The passwords lane, told here what the pages' sign-in forms say; set by the host, and
     * null leaves the pages' login script unheard, as a lane that was never made. */
    var passwords: PasswordsLane? = null
    /** Shows and takes down a page's fullscreen video; set by the host that owns the window.
     * Null leaves a fullscreen ask unanswered, as a host that never showed one. */
    var fullscreenHost: FullscreenHost? = null
    /** The custom view's state, so WebView's double hide and un-asked second show stay sound. */
    private val fullscreen = FullscreenState<View>()
    /** The tab whose page owns the custom view, as a flow so the UI's back key can follow it. */
    private val fullscreenTabId = MutableStateFlow<String?>(null)

    /** The tab whose fullscreen video is up, or null; the host's back key watches it. */
    val fullscreenTab = fullscreenTabId

    /**
     * Private tabs get a separate profile through androidx.webkit's ProfileStore when the
     * WebView build supports MULTI_PROFILE, because it is the only process-less way to keep
     * private cookies off disk-mapped-to-the-normal-profile. Where it isn't supported, this
     * path falls back to the normal profile: WebView on those builds shares one cookie jar,
     * so a private tab's isolation there is limited to history, session and certificate state,
     * which ilo keeps separately. A separate process or data dir suffix can't be had for
     * WebView without MULTI_PROFILE.
     */
    private val multiProfile = WebViewFeature.MULTI_PROFILE.let { feature ->
        try { androidx.webkit.WebViewFeature.isFeatureSupported(feature) } catch (_: Throwable) { false }
    }

    /** The host view for `tab`'s page: a plain container the WebView goes into. The WebView
     * itself is created posted on the container, because one made inside the window's first
     * layout pass, as the interop factory runs, makes the engine size the page's viewport
     * units against a zero height forever after, and the same fault then follows every later
     * WebView in the process: Speedometer's header, on `top: 50%`, rides above the screen.
     * Created after the first pass, as Chrome's own does, the units keep the true size. */
    @SuppressLint("SetJavaScriptEnabled")
    fun pageHost(context: android.content.Context, tab: Tab): android.view.ViewGroup {
        val container = android.widget.FrameLayout(context)
        // Compose's interop holder forwards nested scroll into the Compose tree only while the
        // interop view itself has nested scrolling enabled, and a plain container starts false.
        container.isNestedScrollingEnabled = true
        container.post {
            val cached = webViews[tab.id]
            val webView = cached ?: create(context, tab).also { webViews[tab.id] = it }
            state.touch(tab.id)
            (webView.parent as? android.view.ViewGroup)?.removeView(webView)
            container.addView(
                webView,
                android.widget.FrameLayout.LayoutParams(
                    android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                    android.view.ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
            // A live tab's WebView keeps its own page across switches, so moving it is all a
            // switch back asks of the engine: loading its address again would throw its place
            // in the page away.
            if (cached != null) {
                return@post
            }
            // A tab meeting its page for the first time comes up where it was: its saved state,
            // or its address. The page is put on only once the WebView has been measured, so a
            // page that fixes its own width is scaled against the size it really has, not a
            // size from before the insets settled; the listener stays until that size is real,
            // since a window laid out for the field or the keyboard can report the page empty
            // first, and one laid out only once would leave the tab blank forever.
            val resume = {
                tab.webViewStateBundle?.let { webView.restoreState(it) }
                    ?: tab.url?.let { webView.loadUrl(it) }
                Unit
            }
            if (webView.width > 0 && webView.height > 0) resume()
            else webView.viewTreeObserver.addOnGlobalLayoutListener(object : ViewTreeObserver.OnGlobalLayoutListener {
                override fun onGlobalLayout() {
                    if (webView.width <= 0 || webView.height <= 0) return
                    webView.viewTreeObserver.removeOnGlobalLayoutListener(this)
                    resume()
                }
            })
        }
        return container
    }

    fun load(tabId: String, url: String) {
        val tab = state.tabs.value.firstOrNull { it.id == tabId } ?: return
        state.update(tab.copy(url = url, title = ""))
        webViews[tabId]?.loadUrl(url) ?: run {
            // A frozen tab's WebView comes back at its new address.
            state.selectedId.value?.let { sel -> if (sel == tabId) recreate(tab) }
        }
    }

    private fun recreate(tab: Tab): WebView? {
        // The tab's WebView, when the tab still holds one; the host hands it back on selection.
        return webViews[tab.id]
    }

    private fun create(context: android.content.Context, tab: Tab): WebView {
        val webView = if (tab.isPrivate && multiProfile) {
            // setProfile must run before the WebView loads anything, so it can't touch a page first.
            NestedScrollWebView(context).also { androidx.webkit.WebViewCompat.setProfile(it, PRIVATE_PROFILE) }
        } else {
            NestedScrollWebView(context)
        }
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            safeBrowsingEnabled = true
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            mediaPlaybackRequiresUserGesture = true
            allowFileAccess = false
            allowContentAccess = false
            // A page that fixes its own width, as Speedometer's 850, is scaled to fit the screen,
            // as Chrome fits it, instead of rendering 1:1 past the display's edge.
            useWideViewPort = true
            loadWithOverviewMode = true
            // The page is pinchable, as Chrome's pages are, without the zoom buttons Chrome
            // doesn't show either.
            builtInZoomControls = true
            displayZoomControls = false
        }
        // The agent the tab asks with, kept with the tab as iOS keeps it with WebTab, so a
        // frozen tab's WebView comes back asking the same way.
        webView.settings.userAgentString = if (tab.wantsDesktop) desktopAgent else null
        // A tab's own dark choice is on its page from the first one it shows, as its zoom is.
        applyContentDark(webView, tab)
        webView.webViewClient = Client(tab, state.exceptionsFor(tab))
        webView.webChromeClient = Chrome(tab)
        // The bar folds to its pill while the page is read: scrolling down asks for it, scrolling
        // up gives the bar back. A tap on the pill brings it back too.
        webView.setOnScrollChangeListener { view, _, scrollY, _, oldY ->
            if (scrollY > oldY && scrollY > 4) state.barTucked.value = true
            else if (scrollY < oldY) state.barTucked.value = false
            state.pageAtTop.value = scrollY == 0
        }
        // The page's own find bar hears the count the WebView reports, when the sheet's Find runs.
        webView.setFindListener { _, count, _ -> state.findCount.value = count }
        // The page's own downloads go through the system's service, never into the app's own space.
        webView.setDownloadListener { url, _, contentDisposition, mime, size ->
            if (!tab.isPrivate) {
                val name = downloadName(url, contentDisposition)
                val id = downloadStarter?.invoke(url, name) ?: return@setDownloadListener
                stores?.downloadBegan(DownloadEntry(url = url, fileName = name, mime = mime, sizeBytes = size, id = id))
            }
        }
        // A long press on a link asks the WebView what it hit, and the sheet that offers where
        // to open it is the host's: the engine only carries the address. A hit that names no
        // address (blank space, an image with no link) leaves the page's own menu as it was.
        webView.setOnLongClickListener {
            val hit = webView.hitTestResult
            val pressed = (hit.type == android.webkit.WebView.HitTestResult.SRC_ANCHOR_TYPE ||
                hit.type == android.webkit.WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE) &&
                hit.extra?.startsWith("http") == true
            if (pressed) listener.linkLongPressed(tab.id, hit.extra!!)
            pressed
        }
        return webView
    }

    /** The name a download goes by: what its own headers say, else its address's last piece. */
    private fun downloadName(url: String, contentDisposition: String?): String {
        val headerName = contentDisposition?.let { disposition ->
            Regex("filename\\*=UTF-8''([^;]+)|filename=\"?([^;\"]+)\"?").find(disposition)
        }?.let { match ->
            match.groupValues[1].ifEmpty { match.groupValues[2] }
        }
        if (!headerName.isNullOrBlank()) return headerName
        return runCatching { java.net.URI(url).path }.getOrNull()?.substringAfterLast('/')?.ifEmpty { null } ?: "download"
    }

    /** Gives a tab's WebView up: its state rides on the tab, the WebView goes. A fullscreen
     * video of its page goes first, since its callback belongs to a WebView about to die. */
    fun freeze(tab: Tab) {
        if (fullscreenFor(tab.id)) hideCustomView()
        val webView = webViews.remove(tab.id) ?: return
        val bundle = Bundle()
        webView.saveState(bundle)
        webView.stopLoading()
        webView.destroy()
        if (!bundle.isEmpty) state.update(tab.copy(webViewStateBundle = bundle))
    }

    /** A tab is closed: nothing of it stays anywhere. */
    fun destroy(tab: Tab) {
        if (fullscreenFor(tab.id)) hideCustomView()
        webViews.remove(tab.id)?.destroy()
        heldChoices.remove(tab.id)
    }

    /** The blocking choice the tab holds for `site`, or null when the site's own choice is the
     * one that runs. True names blocking, false its pause. */
    fun blockingHeldChoice(tab: Tab, site: String): Boolean? = heldChoices[tab.id]?.get(site)

    /** Sets the tab's own blocking choice for `site`, in memory only, and reloads it: the page's
     * requests are already past the interceptor, so the difference shows on the reload. */
    fun setBlockingHeldChoice(tab: Tab, site: String, blocks: Boolean) {
        heldChoices.getOrPut(tab.id) { ConcurrentHashMap() }[site] = blocks
        webViews[tab.id]?.reload()
    }

    fun canGoBack(tab: Tab): Boolean = webViews[tab.id]?.canGoBack() ?: false
    fun goBack(tab: Tab) { webViews[tab.id]?.goBack() }
    fun canGoForward(tab: Tab): Boolean = webViews[tab.id]?.canGoForward() ?: false
    fun goForward(tab: Tab) { webViews[tab.id]?.goForward() }
    fun reload(tab: Tab) { webViews[tab.id]?.reload() }

    /** The app went from the screen: its pages' script timers stop, so no page wakes the phone
     * on its own account while the user is elsewhere. Timers only: media keeps playing in the
     * background, as Chrome's tabs do, so onPause is the wrong tool here. */
    fun pauseScripts() {
        webViews.values.firstOrNull()?.pauseTimers()
    }

    /** The app is back: the script timers the stop paused go on again. */
    fun resumeScripts() {
        webViews.values.firstOrNull()?.resumeTimers()
    }

    /** Puts the page in its reading mode right on it: the article the page holds is restyled
     * in place by a small script, so the page keeps its place and its links work. Reload brings
     * the page back as the site made it. */
    fun read(tab: Tab) {
        webViews[tab.id]?.evaluateJavascript(READER_JS, null)
    }

    /** What reading mode does, in the page: the article (readability's own pick) is kept and
     * the page is replaced by its words, restyled for reading. Self-contained, so nothing is
     * fetched and no site is told. */
    private val READER_JS = """(function(){
        var pick = document.querySelector('article') || document.querySelector('[role=main]') || document.querySelector('main') || document.body;
        if (!pick) return;
        var words = pick.innerText || '';
        if (!words.trim()) return;
        var page = document.createElement('div');
        page.style.cssText = 'margin:0 auto;padding:20px 16px 80px 16px;max-width:38rem;font-family:Georgia,serif;font-size:20px;line-height:1.6;color:#1a1a1a;background:#fbf9f4';
        var title = document.createElement('h1');
        title.textContent = document.title;
        title.style.cssText = 'font-size:28px;line-height:1.3;margin:16px 0 24px 0';
        page.appendChild(title);
        var body = document.createElement('div');
        body.textContent = words;
        page.appendChild(body);
        document.documentElement.style.background = '#fbf9f4';
        document.body.innerHTML = '';
        document.body.style.cssText = 'margin:0;background:#fbf9f4';
        document.body.appendChild(page);
    })();"""
    /** Puts a script in the tab's page, as the fill does: the account's word has already said
     * it may, so the engine only runs it. */
    fun runInPage(tab: Tab, script: String) {
        webViews[tab.id]?.evaluateJavascript(script, null)
    }

    fun stopLoading(tab: Tab) { webViews[tab.id]?.stopLoading() }

    /** Opens the page's own find bar on the words, with the count the WebView reports. */
    fun findAll(tab: Tab, query: String) {
        webViews[tab.id]?.findAllAsync(query)
    }

    /** Steps the page's find to the match beside the one shown, as the WebView's own flags say. */
    fun findNext(tab: Tab, forward: Boolean) {
        webViews[tab.id]?.findNext(forward)
    }

    /** Tells the page its find is over, so its highlights go. */
    fun findDone(tab: Tab) {
        webViews[tab.id]?.clearMatches()
    }

    /** Asks the site for a desktop's page, or a phone's, and loads it again: the row's label
     * flips with the tab, as iOS's requestDesktopSite does. */
    fun requestDesktop(tab: Tab, wanted: Boolean) {
        state.update(tab.copy(wantsDesktop = wanted))
        webViews[tab.id]?.let { webView ->
            webView.settings.userAgentString = if (wanted) desktopAgent else null
            // A fresh navigation, as Chrome's own agent change does: a bare reload after the
            // agent switch leaves some pages blank until a second reload. The WebView's own
            // address carries redirects the tab's record may not.
            val url = webView.url ?: tab.url
            if (url != null) webView.loadUrl(url) else webView.reload()
        }
    }

    /** The page's own dark, one tab at a time, as the owner asked: what the menu's row turns
     * for the tab alone. The darkening is the WebView's own algorithm, Blink's per-element
     * inversion, the same engine Chrome's "Darken websites" uses: the old invert-everything
     * filter turned every hue and darkened pages again when they had a dark theme of their
     * own, while this keeps a page's own dark when it has one and darkens only the rest. */
    fun setContentDark(tab: Tab, dark: Boolean) {
        state.update(tab.copy(contentDark = dark))
        webViews[tab.id]?.let { applyDarkening(it, dark) }
    }

    /** Applies the tab's own dark choice to a WebView being made or brought back, so a
     * frozen tab's page comes up as it was left; a tab that never picked follows the app's
     * general setting, as the owner asked. */
    private fun applyContentDark(webView: WebView, tab: Tab) {
        val dark = tab.contentDark ?: contentDarkDefault?.invoke() ?: false
        applyDarkening(webView, dark)
    }

    /** Asks the page for its theme-color meta and tells the listener, as Chrome's bar does:
     * the first meta wins, and a page without one keeps the bar on the Space's tint. */
    private fun readThemeColor(view: WebView, tab: Tab) {
        view.evaluateJavascript(THEME_COLOR_JS) { value ->
            val color = runCatching { parseCssColor(value) }.getOrNull()
            listener.themeColor(tab.id, color)
        }
    }

    /** Reads the first theme-color meta's content, ignoring media queries as the phone's
     * bar is one surface, not the page's own media list. */
    private val THEME_COLOR_JS = """(function(){
        var m = document.querySelector('meta[name=theme-color]');
        return m ? m.getAttribute('content') : null;
    })()"""

    /** Turns the engine's own darkening on or off for one WebView. It takes effect at the
     * page's next style pass, so a page already on screen shows the change as it lays out
     * again; a WebView build without the feature keeps the page as it is, as iOS's are. */
    private fun applyDarkening(webView: WebView, dark: Boolean) {
        try {
            if (androidx.webkit.WebViewFeature.isFeatureSupported(androidx.webkit.WebViewFeature.ALGORITHMIC_DARKENING)) {
                androidx.webkit.WebSettingsCompat.setAlgorithmicDarkeningAllowed(webView.settings, dark)
            }
        } catch (_: Throwable) {
        }
    }

    /** The page's text zoom in percent, kept with the tab so a frozen one comes back at it. */
    fun setZoom(tab: Tab, percent: Int) {
        val kept = percent.coerceIn(50, 200)
        state.update(tab.copy(pageZoom = kept))
        webViews[tab.id]?.settings?.textZoom = kept
    }

    /** Prints the page through Android's own print dialog, as iOS's print does through UIPrintController. */
    fun print(context: android.content.Context, tab: Tab) {
        val webView = webViews[tab.id] ?: return
        val manager = context.getSystemService(android.content.Context.PRINT_SERVICE) as android.print.PrintManager
        manager.print("ilo", webView.createPrintDocumentAdapter("ilo"), android.print.PrintAttributes.Builder().build())
    }

    /** What a desktop's Safari says it is, as iOS's desktopAgent does: a Mac's platform and
     * width, which is what sites sort desktops by. */
    private val desktopAgent =
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) " +
            "Version/17.0 Safari/605.1.15"

    fun onPageShown(tab: Tab) { state.touch(tab.id) }

    /**
     * A picture of the top of the page, in a card's shape, as iOS's WebTab.picture does: the
     * viewport's own width, as tall as the card's shape is to that width, no taller than the
     * page itself, never scaled down, because the deck's card is the width of the screen and
     * a smaller capture would come up blurry in it. Null when the WebView is gone or shows
     * nothing, and never taken for a private tab, whose page no picture is kept of.
     */
    fun picture(tab: Tab): Bitmap? {
        if (tab.isPrivate) return null
        val webView = webViews[tab.id] ?: return null
        if (webView.width <= 0 || webView.height <= 0) return null
        return runCatching {
            val tall = (webView.width / CARD_ASPECT).toInt().coerceAtMost(webView.height)
            val whole = Bitmap.createBitmap(webView.width, tall.coerceAtLeast(1), Bitmap.Config.ARGB_8888)
            webView.draw(android.graphics.Canvas(whole))
            whole
        }.getOrNull()
    }

    companion object {
        private const val PRIVATE_PROFILE = "ilo_private"

        /** The card's width to its height, as the switcher's card holds it. */
        private const val CARD_ASPECT = 0.62f

        /** A CSS color as a 0xRRGGBB long, from a theme-color meta's content: hex forms
         * (3, 6 or 8 digits) and rgb()/rgba() names only, the forms pages give. The null of
         * a page without a meta is the JSON "null", not a color. */
        internal fun parseCssColor(value: String?): Long? {
            val text = value?.trim()?.removeSurrounding("\"") ?: return null
            if (text.equals("null", true) || text.isEmpty()) return null
            val hex = text.removePrefix("#")
            if (hex.length == 3 && hex.all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' }) {
                val r = hex[0].digitToInt(16); val g = hex[1].digitToInt(16); val b = hex[2].digitToInt(16)
                return (((r * 17) shl 16) or ((g * 17) shl 8) or (b * 17)).toLong()
            }
            if (hex.length == 6 && hex.all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' }) {
                return hex.toLong(16)
            }
            if (hex.length == 8) {
                return hex.takeLast(6).toLong(16)
            }
            val rgb = text.lowercase().removePrefix("rgb(").removePrefix("rgba(").removeSuffix(")")
                .split(',').map { it.trim() }
            if (rgb.size >= 3) {
                val parts = rgb.take(3).map { part ->
                    val n = part.toIntOrNull() ?: return null
                    if (n !in 0..255) return null else n
                }
                return ((parts[0] shl 16) or (parts[1] shl 8) or parts[2]).toLong()
            }
            return null
        }
    }

    private inner class Client(private val tab: Tab, private val exceptions: CertificateExceptions) : WebViewClient() {
        /** The address the main frame is on, its own while a load is on its way, set from the
         * engine's own callbacks on the main thread: a load's host comes from it, never from
         * view.url, which still names the page before while the next is asked for. */
        @Volatile
        private var pageUrl: String? = null

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            pageUrl = url
            turnedAway.remove(tab.id)
            listener.loadStarted(tab.id)
            // The login script goes in at every start, as iOS's does: a page brought in from
            // the back-forward cache keeps its own copy, and a fresh document needs a new one.
            // A private tab never gets it: nothing of the user's typing is captured there.
            if (passwords != null && !tab.isPrivate) view.evaluateJavascript(LoginScript.script, null)
            injectHides(view, url)
        }

        /** The hides go in at every page start, main frame only: the styles are the page's
         * own host's, and every choice that can pause them is asked here, once. */
        private fun injectHides(view: WebView, url: String?) {
            val easy = blocker as? EasyListBlocker ?: return
            if (!hidesWanted(
                    url,
                    easy.isOn,
                    easy.hidesOn(),
                    url?.let { easy.siteBlockingOff(it) } ?: false,
                    heldChoices[tab.id]?.let { held ->
                        url?.let { u -> EasyListBlocker.hostOf(u)?.let { held[it] } }
                    },
                    state.tabFaults.value[tab.id] != null,
                )
            ) return
            val styles = easy.hidesFor(url?.let { EasyListBlocker.hostOf(it) }) ?: return
            HideScript.script(styles)?.let { view.evaluateJavascript(it, null) }
        }

        override fun shouldInterceptRequest(
            view: WebView, request: WebResourceRequest
        ): WebResourceResponse? {
            val url = request.url.toString()
            if (!blocker.isOn) return null
            // The tab's own choice for the page's site runs over the site's, as iOS's hold does.
            val held = heldChoices[tab.id]
            val pageSite = pageUrl?.let { app.ilo.browser.android.blocking.EasyListBlocker.hostOf(it) }
            val heldChoice = if (held != null && pageSite != null) held[pageSite] else null
            if (url.startsWith("data:") || url.startsWith("blob:")) return null
            if (blocker.blocks(url, pageUrl, heldChoice)) {
                // WebView calls this on its own threads, so the count needs an atomic update
                state.blockedCount.update { it + 1 }
                return WebResourceResponse("text/plain", "utf-8", java.io.ByteArrayInputStream(ByteArray(0)))
            }
            return null
        }

        override fun shouldOverrideUrlLoading(
            view: WebView, request: WebResourceRequest
        ): Boolean {
            val url = request.url.toString()
            // Only a link that leaves the web is the host's business; http(s) loads in place.
            when (val choice = ExternalRouter.decide(url, openInApps?.invoke() ?: false)) {
                is ExternalDecision.Stay -> return false
                is ExternalDecision.LoadInBrowser -> { view.loadUrl(choice.url); return true }
                is ExternalDecision.OpenInApp -> {
                    listener.externalLinkAsked(tab.id, choice.resolvedIntentUri, choice.appName, choice.fallbackUrl)
                    return true
                }
            }
        }

        override fun onReceivedSslError(
            view: WebView, handler: SslErrorHandler, error: SslError
        ) {
            // The address whose certificate is in question is the one the error itself names; a
            // redirect between start and ask leaves the parked address behind, and view.url
            // still names the page before until this one commits.
            val asked = runCatching { java.net.URI(error.url ?: "") }.getOrNull()
                ?: pageUrl?.let { runCatching { java.net.URI(it) }.getOrNull() }
            val host = asked?.host ?: ""
            val port = asked?.port?.takeIf { it > 0 } ?: 443
            val leaf = error.certificate.x509Certificate ?: run { handler.cancel(); return }
            val exception = CertificateException.of(host, port, leaf)
            if (exceptions.trusts(exception)) { handler.proceed(); return }
            val fault = CertificateFault(exception, facts(error, leaf))
            // The engine was told of this fault at this address, which is what a page over a
            // connection it still holds serves again when the way in was taken back.
            exceptions.tell(fault)
            pendingSsl[tab.id] = handler to fault
            state.tabFaults.value = state.tabFaults.value + (tab.id to fault)
        }

        override fun doUpdateVisitedHistory(view: WebView, url: String?, isReload: Boolean) {
            if (url != null) {
                pageUrl = url
                listener.loaded(tab.id, url, view.title ?: "")
            }
        }

        override fun onPageFinished(view: WebView, url: String?) {
            listener.progress(tab.id, 100)
            listener.loadFinished(tab.id)
            val title = view.title ?: ""
            if (url != null) listener.loaded(tab.id, url, title)
            // What the phone remembers of its pages, never a private one's.
            if (url != null && !tab.isPrivate && url.startsWith("http")) stores?.visit(url, title)
            // The tab's own dark choice rides along to every page it opens, as its zoom does.
            applyContentDark(view, tab)
            // The page is drawn: its preview is taken now, while it is fresh, not only when
            // its tab is left, as the owner asked for a preview always provided.
            listener.pageReady(tab.id, url ?: "")
            // The bar takes the page's own color, as Chrome's does, read after the page is
            // drawn so the meta is in the document.
            readThemeColor(view, tab)
            // A page whose script was stopped early, as a restore's is, gets it again: the
            // offers' word needs the script in the page that ended up drawn. A private tab
            // never gets it: nothing of the user's typing is captured there.
            if (passwords != null && !tab.isPrivate) view.evaluateJavascript(LoginScript.script, null)
            passwords?.of(tab.id)?.let { lane ->
                lane.save.committed(url)
                lane.fill.committed()
            }
            mark(url)
        }

        /** Marks a page that came in past a warning, and puts the warning back over one whose
         * way in the user took back: WebView asks nothing again of a server it still holds a
         * connection to. Port of the iOS app's WebTab.mark(), which reads the trust from the
         * connection itself; Android has no way to, so the fault told at this address stands. */
        private fun mark(url: String?) {
            // A page whose warning was turned away: its failed load's aftermath is not a page
            // to mark, and its warning stays down until another load begins.
            val uri = url?.let { runCatching { java.net.URI(it) }.getOrNull() }
            val host = uri?.host
            val port = uri?.port?.takeIf { it > 0 } ?: 443
            if (host != null && turnedAway[tab.id] == "$host:$port") return
            state.tabExceptions.value = state.tabExceptions.value - tab.id
            if (uri == null || uri.scheme?.lowercase() != "https" || host == null) {
                state.tabFaults.value = state.tabFaults.value - tab.id
                return
            }
            if (!exceptions.knows(host, port)) {
                state.tabFaults.value = state.tabFaults.value - tab.id
                return
            }
            val told = exceptions.faultAt(host, port)
            if (told != null && exceptions.trusts(told.exception)) {
                state.tabExceptions.value = state.tabExceptions.value + (tab.id to told.exception)
                // A warning left from an older attempt at this tab comes down, as iOS's mark
                // clears the stale failure after keeping the exception.
                state.tabFaults.value = state.tabFaults.value - tab.id
            } else if (told != null && exceptions.marks(told.exception)) {
                state.tabFaults.value = state.tabFaults.value + (tab.id to told)
            } else {
                state.tabFaults.value = state.tabFaults.value - tab.id
            }
        }

        private fun facts(error: SslError, leaf: X509Certificate): CertificateFacts {
            val faults = ArrayList<CertificateFacts.Fault>()
            when (error.primaryError) {
                SslError.SSL_EXPIRED -> faults.add(CertificateFacts.Fault.RAN_OUT)
                SslError.SSL_NOTYETVALID -> faults.add(CertificateFacts.Fault.NOT_VALID_UNTIL)
                SslError.SSL_IDMISMATCH -> faults.add(CertificateFacts.Fault.WRONG_NAME)
                SslError.SSL_UNTRUSTED -> faults.add(CertificateFacts.Fault.UNKNOWN_SIGNER)
                else -> faults.add(CertificateFacts.Fault.OTHER)
            }
            val itself = leaf.subjectX500Principal == leaf.issuerX500Principal
            val signer = if (itself) null else leaf.issuerX500Principal.name
            return CertificateFacts(
                madeOutTo = leaf.subjectX500Principal.name,
                signedBy = signer,
                signedItself = itself,
                faults = faults,
            )
        }
    }

    /** Takes the custom view down: the back key's exit, called by the host through the engine. */
    fun exitFullscreen() {
        hideCustomView()
    }

    /** Whether `tabId`'s page has its fullscreen video up, so a switch can take it down first. */
    fun fullscreenFor(tabId: String): Boolean = fullscreen.active && fullscreenTabId.value == tabId

    private fun hideCustomView() {
        val gone = fullscreen.hide() ?: return
        fullscreenTabId.value = null
        // The host's own take-down is idempotent, so a second call with none up does nothing.
        fullscreenHost?.hideFullscreen()
        gone.second.invoke()
    }

    private inner class Chrome(private val tab: Tab) : WebChromeClient() {
        /** A page went fullscreen: the view is put over the whole window, since the WebView
         * shows nothing of its own without the host adding the custom view. */
        override fun onShowCustomView(view: View, callback: CustomViewCallback) {
            // WebView can show a second view without hiding the first: take the old one down
            // so its callback is never left hanging.
            if (fullscreen.active) hideCustomView()
            fullscreen.show(view) {
                fullscreenTabId.value = null
                callback.onCustomViewHidden()
            }
            fullscreenTabId.value = tab.id
            fullscreenHost?.showFullscreen(view)
        }

        /** The page left fullscreen: take the view down once, since WebView calls this twice. */
        override fun onHideCustomView() {
            hideCustomView()
        }

        override fun onProgressChanged(view: WebView, newProgress: Int) {
            listener.progress(tab.id, newProgress)
        }

        /** The login script's word rides the page's own prompt: the engine takes it before
         * the page sees it, answers null so the page goes on as if it were dismissed, and a
         * prompt of any other shape is the page's own, shown as it always was. */
        override fun onJsPrompt(
            view: WebView, url: String?, message: String?, defaultValue: String?,
            result: android.webkit.JsPromptResult,
        ): Boolean {
            if (message?.startsWith(LoginScript.PREFIX) == true) {
                val body = message.substring(LoginScript.PREFIX.length)
                LoginMessage.parse(body)?.let { word -> passwords?.of(tab.id)?.heard(word, url) }
                result.cancel()
                return true
            }
            return false
        }

        /** A page asked for a capability: its host's stored answer decides without asking, and
         * only an unanswered kind reaches the user's own ask, as Chrome's own dialog does. */
        override fun onPermissionRequest(request: android.webkit.PermissionRequest) {
            val host = tab.url?.let { runCatching { java.net.URI(it).host?.lowercase() }.getOrNull() }
            val kind = request.resources.firstNotNullOfOrNull { resourceOf(it) }
            if (host == null || kind == null || sitePermissions == null) {
                request.deny()
                return
            }
            when (sitePermissions?.answer(host, kind)) {
                SiteAnswer.ALLOW -> {
                    // An Allow the OS can no longer back denies, and the stale exception goes
                    // with it, so the next ask runs its chain again instead of failing silently.
                    if (osGrantCheck?.invoke(kind) != false) request.grant(request.resources)
                    else {
                        sitePermissions?.setAnswer(host, kind, null)
                        request.deny()
                    }
                }
                SiteAnswer.BLOCK -> request.deny()
                else -> {
                    val asker = permissionAsker
                    if (asker == null) request.deny() else asker(tab, host, kind, request)
                }
            }
        }

        /** The permission kind a page's own resource name stands for; a name ilo has no answer
         * for stays null, so the ask never shows for what the browser can't grant. */
        private fun resourceOf(resource: String): SitePermission? = when (resource) {
            android.webkit.PermissionRequest.RESOURCE_VIDEO_CAPTURE -> SitePermission.CAMERA
            android.webkit.PermissionRequest.RESOURCE_AUDIO_CAPTURE -> SitePermission.MICROPHONE
            else -> null
        }

        /** Where a page is, as its own prompt: the host's stored answer decides, and only an
         * unanswered host reaches the user's ask. The callback must be answered either way, or
         * the page waits forever, so every path calls it. */
        override fun onGeolocationPermissionsShowPrompt(
            origin: String?,
            callback: android.webkit.GeolocationPermissions.Callback,
        ) {
            val host = origin?.let { runCatching { java.net.URI(it).host?.lowercase() }.getOrNull() }
                ?: origin?.lowercase()
            if (host == null) {
                callback.invoke(origin, false, false)
                return
            }
            when (sitePermissions?.answer(host, SitePermission.LOCATION)) {
                SiteAnswer.ALLOW -> {
                    // As the capture kinds above: an Allow the OS can no longer back denies, and
                    // the stale exception goes with it, so the next ask runs its chain again.
                    if (osGrantCheck?.invoke(SitePermission.LOCATION) != false) callback.invoke(origin, true, false)
                    else {
                        sitePermissions?.setAnswer(host, SitePermission.LOCATION, null)
                        callback.invoke(origin, false, false)
                    }
                }
                SiteAnswer.BLOCK -> callback.invoke(origin, false, false)
                else -> {
                    val asker = permissionAsker
                    if (asker == null) callback.invoke(origin, false, false)
                    else asker(tab, host, SitePermission.LOCATION, GeolocationAsGeolocationRequest(origin, callback))
                }
            }
        }

        override fun onReceivedTitle(view: WebView, title: String?) {
            listener.loaded(tab.id, view.url ?: tab.url ?: "", title ?: "")
        }

        override fun onReceivedIcon(view: WebView, icon: Bitmap) {
            listener.favicon(tab.id, tab.faviconColor)
            // The icon itself is kept only for an ordinary tab, as nothing of a private page is.
            if (!tab.isPrivate) listener.pageIcon(tab.id, view.url ?: tab.url ?: "", icon)
        }

        override fun onCreateWindow(
            view: WebView, isDialog: Boolean, isUserGesture: Boolean, resultMsg: android.os.Message
        ): Boolean {
            // A page's new window becomes a tab: a WebView for it, handed to the page.
            val newTab = state.newTab(url = null, isPrivate = tab.isPrivate)
            val webView = create(view.context, newTab)
            webViews[newTab.id] = webView
            listener.newWindow(tab.id, webView)
            (resultMsg.obj as? android.webkit.WebView.WebViewTransport)?.let {
                it.webView = webView
                resultMsg.sendToTarget()
            }
            return true
        }
    }

    fun clearPrivateData() {
        // Exceptions live in memory alone, so leaving is what clears them.
    }

    /** A geolocation ask dressed as the PermissionRequest the asker takes, so one ask serves
     * both prompts: grant says yes, deny says no, and the origin rides along unchanged. */
    private class GeolocationAsGeolocationRequest(
        private val origin: String?,
        private val callback: android.webkit.GeolocationPermissions.Callback,
    ) : android.webkit.PermissionRequest() {
        override fun getOrigin() = android.net.Uri.parse(origin ?: "")
        override fun getResources() = arrayOf("")
        override fun grant(resources: Array<out String>?) = callback.invoke(origin, true, false)
        override fun deny() = callback.invoke(origin, false, false)
    }

}