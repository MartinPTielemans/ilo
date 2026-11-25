package app.ilo.browser.android.ui

import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import app.ilo.browser.android.R
import app.ilo.browser.android.sync.SyncedAccount
import java.net.URI
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** Whether [hit] is the sign-in's redirect itself: the scheme, host, port and path of the
 * address [redirect] beginSignIn answered with, so the page's answer never loads. Scheme and
 * host compare without case: a keyboard that capitalizes `HTTP://LOCALHOST:8530` is still
 * the same address the server named. */
internal fun isSignInRedirect(hit: String, redirect: String): Boolean {
    val hitUri = runCatching { URI(hit) }.getOrNull() ?: return false
    val redirectUri = runCatching { URI(redirect) }.getOrNull() ?: return false
    return hitUri.scheme?.lowercase() == redirectUri.scheme?.lowercase() &&
        hitUri.host?.lowercase() == redirectUri.host?.lowercase() &&
        hitUri.port == redirectUri.port &&
        hitUri.path == redirectUri.path
}

/** The query a sign-in answer's URL carries, which is what the account's signIn takes: a
 * full URL would parse as one broken pair and reach the server as the code itself. */
internal fun signInPayload(url: String): String =
    runCatching { URI(url).rawQuery }.getOrNull() ?: url

/** The host the user's password goes to, for the label that says so. */
internal fun hostOf(address: String): String =
    runCatching { URI(address).host }.getOrNull() ?: address

/** The server's sign-in page, in a web view that keeps nothing of the visit: the password
 * never enters the user's tab session, and the answer is taken, not loaded. */
@Composable
fun SignInWebScreen(
    account: SyncedAccount,
    page: String,
    redirect: String,
    scope: CoroutineScope,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    // The password is typed here, so nothing of the visit stays on the phone.
    val container = remember(context) { SignInWebView(context, page, redirect) }
    var captured by remember { mutableStateOf(false) }

    fun capture(url: String) {
        if (captured || !isSignInRedirect(url, redirect)) return
        captured = true
        scope.launch { account.signIn(signInPayload(url)) }
        onClose()
    }
    container.capture = ::capture
    // The page is gone when the screen is, so nothing of the visit outlives it.
    DisposableEffect(container) { onDispose { container.destroy() } }

    BackHandler { onClose() }
    Column(
        Modifier
            .fillMaxSize()
            .background(LocalLook.current.overlay)
    ) {
        Box(Modifier.windowInsetsPadding(WindowInsets.statusBars)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
            ) {
                CloseButton(onClose)
                // Where the password is going, for the user to check.
                Text(
                    stringResource(R.string.account_sign_in_host, hostOf(page)),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
                )
            }
        }
        AndroidView(
            factory = { container },
            modifier = Modifier.weight(1f),
        )
    }
}

/** The web view that loads the sign-in page and hands over its answer without loading it. */
private class SignInWebView(
    context: android.content.Context,
    page: String,
    redirect: String,
) : WebView(context) {
    var capture: (String) -> Unit = {}

    init {
        // A second guard on the same one every way out takes: the first would fall to a page
        // that came back after the answer left.
        webViewClient = object : WebViewClient() {
            // The answer is taken here, and false lets the page's own navigations go on.
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val url = request.url.toString()
                if (isSignInRedirect(url, redirect)) {
                    capture(url)
                    return true
                }
                return false
            }

            // Android does not call shouldOverrideUrlLoading for every 30x, so the page's
            // start is the catch-all that still sees the answer.
            override fun onPageStarted(view: WebView, url: String, favicon: android.graphics.Bitmap?) {
                if (isSignInRedirect(url, redirect)) {
                    view.stopLoading()
                    capture(url)
                }
            }
        }
        settings.javaScriptEnabled = true
        loadUrl(page)
    }
}

@Composable
private fun CloseButton(onClose: () -> Unit) {
    TextButton(onClick = onClose) { Text(stringResource(R.string.close)) }
}