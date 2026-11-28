package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.font.FontWeight
import app.ilo.browser.android.ui.Glyph
import app.ilo.browser.android.ui.LocalLook
import app.ilo.browser.android.ui.PlateButton
import app.ilo.browser.android.R
import app.ilo.browser.android.sync.AccountState
import app.ilo.browser.android.sync.IloAccount
import app.ilo.browser.android.sync.PairingState
import app.ilo.browser.android.sync.RecoveryWords
import app.ilo.browser.android.sync.SyncStats
import app.ilo.browser.android.sync.SyncedAccount
import kotlinx.coroutines.launch

/** The words the list offers for the one being typed: the text's last word's completions. */
internal fun recoveryWordSuggestions(text: String): List<String> {
    val typed = text.split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (typed.isEmpty() || typed.size > 24) return emptyList()
    if (text.lastOrNull() == ' ') return emptyList()
    return RecoveryWords.startingWith(typed.last())
}

/** The account's screen, in Settings' place: every [AccountState] has its own face here. */
@Composable
fun AccountScreen(account: IloAccount, onBack: () -> Unit) {
    val look = LocalLook.current
    val state by account.state.collectAsState()
    val problem by account.problem.collectAsState()
    val email by account.email.collectAsState()
    val spaces by account.spaces.collectAsState()
    val nodes by account.nodes.collectAsState()
    val scope = rememberCoroutineScope()
    // The sign-in page takes the whole screen: the scrollable column below measures its
    // children with unbounded height, so the web view's weight(1f) resolved to zero there.
    var signInWebServer by remember { mutableStateOf<String?>(null) }
    var showDisconnected by remember { mutableStateOf(false) }
    val disconnected = { showDisconnected = true }
    val synced = account as? SyncedAccount
    if (signInWebServer != null && synced != null) {
        val server = signInWebServer
        // One pending sign-in per open: a recomposition must not mint a new verifier.
        val pending = remember(server!!) { synced.beginSignIn(server!!) }
        SignInWebScreen(
            account = synced,
            page = pending.first,
            redirect = pending.second,
            scope = scope,
            onClose = { signInWebServer = null },
        )
        return
    }

    // The app draws edge to edge, so content must clear the status bar's window:
    // taps inside it go to SystemUI and never reach the app. The background is
    // painted first so the bar's area keeps the theme's color, not the window's.
    // A Surface, not a bare background: it provides the scheme's content color, so
    // text is readable in dark mode too.
    Surface(
        Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Column(
            Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars)
                .verticalScroll(rememberScrollState()).padding(16.dp)
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                PlateButton(Glyph.BACK, stringResource(R.string.back), look, onClick = onBack)
                Spacer(Modifier.width(12.dp))
                Text(
                    stringResource(R.string.account_title),
                    color = look.text,
                    fontSize = 24.sp,
                    fontWeight = FontWeight.SemiBold,
                )
            }

            when (val phase = state) {
                AccountState.SignedOut -> SignedOut(account) { server -> signInWebServer = server }
                AccountState.NeedsSecret -> NeedsSecret(account)
                AccountState.Starting -> Starting(account, disconnected)
                AccountState.SignedIn -> SignedIn(account, email, spaces, nodes, look, disconnected)
                AccountState.SigningIn -> Text(stringResource(R.string.account_signing_in))
                is AccountState.Failed -> Failed(phase.message) {
                    scope.launch { account.refresh() }
                }
            }

            // The last thing that went wrong stays on the screen until something works again,
            // whatever state the account is in, as the interface's problem flow promises.
            problem?.let { sentence ->
                Text(sentence, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 16.dp))
            }
            if (showDisconnected && state == AccountState.SignedOut) {
                Text(
                    stringResource(R.string.account_disconnect_done),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(top = 16.dp),
                )
            }
        }
    }
}

@Composable
private fun SignedOut(account: IloAccount, openSignInWeb: (String) -> Unit) {
    var server by remember { mutableStateOf(IloAccount.DEFAULT_SERVER) }
    var code by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    Text(stringResource(R.string.account_title), style = MaterialTheme.typography.titleMedium)
    OutlinedTextField(
        value = server,
        onValueChange = { server = it },
        label = { Text(stringResource(R.string.account_server_address)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
    // One pending sign-in per server address: a new one each recomposition would leave the
    // code the page answers with unmatched. The page and its redirect come from the same
    // call, so the answer lands on the verifier and state the page was made with.
    val pending = remember(server) { (account as? SyncedAccount)?.beginSignIn(server) }
    val synced = account as? SyncedAccount
    Button(
        enabled = pending != null && synced != null,
        onClick = { openSignInWeb(server) },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_open_sign_in_page)) }
    // The manual way, for when the page can't run here: another browser, then the code back.
    pending?.let { address ->
        Text(
            stringResource(R.string.account_sign_in_page, address.first),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
    OutlinedTextField(
        value = code,
        onValueChange = { code = it },
        label = { Text(stringResource(R.string.account_sign_in_code_hint)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
    Button(
        onClick = { scope.launch { account.signIn(signInPayload(code)) } },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_sign_in)) }
}

@Composable
private fun NeedsSecret(account: IloAccount) {
    var words by remember { mutableStateOf("") }
    val pairing by account.pairing.collectAsState()
    val scope = rememberCoroutineScope()
    Text(stringResource(R.string.account_recovery_words_title), style = MaterialTheme.typography.titleMedium)
    OutlinedTextField(
        value = words,
        onValueChange = { words = it },
        label = { Text(stringResource(R.string.account_recovery_words_hint)) },
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
    // The last word's typing is suggested from the list, so a word is finished, not guessed.
    val suggestions = remember(words) { recoveryWordSuggestions(words) }
    if (suggestions.isNotEmpty() && words.lastOrNull() != ' ') {
        Text(
            suggestions.take(5).joinToString("  "),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
    Button(
        onClick = { scope.launch { (account as? SyncedAccount)?.enterRecoveryWords(words) } },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_recovery_words_use)) }

    HorizontalDivider(Modifier.padding(vertical = 16.dp))
    Text(stringResource(R.string.account_pairing_title), style = MaterialTheme.typography.titleMedium)
    when (val phase = pairing) {
        is PairingState.Waiting -> {
            Text(
                stringResource(
                    R.string.account_pairing_code_is,
                    phase.code.ifEmpty { stringResource(R.string.account_pairing_waiting) }),
                modifier = Modifier.padding(top = 8.dp),
            )
            OutlinedButton(
                onClick = { account.cancelPairing() },
                modifier = Modifier.padding(top = 8.dp),
            ) { Text(stringResource(R.string.account_pairing_cancel)) }
        }
        PairingState.Refused -> {
            Text(
                stringResource(R.string.account_pairing_refused),
                modifier = Modifier.padding(top = 8.dp),
            )
            PairingAskButton(account, scope)
        }
        PairingState.None -> PairingAskButton(account, scope)
    }
}

@Composable
private fun PairingAskButton(account: IloAccount, scope: kotlinx.coroutines.CoroutineScope) {
    Button(
        onClick = { scope.launch { account.requestPairing() } },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_pairing_start)) }
}

@Composable
private fun Starting(account: IloAccount, disconnected: () -> Unit) {
    val scope = rememberCoroutineScope()
    val stats by account.stats.collectAsState()
    val syncing by account.syncing.collectAsState()
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        CircularProgressIndicator(Modifier.padding(end = 12.dp))
        Text(stringResource(R.string.account_starting))
    }
    SyncNow(account, stats, syncing)
    DisconnectButton(account, disconnected, Modifier.padding(top = 16.dp))
    OutlinedButton(
        onClick = { scope.launch { account.signOut() } },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_sign_out)) }
}

/** Stops the sync without erasing what the phone already has, the row the user reaches for
 * when the account is the problem, not the data. */
@Composable
private fun DisconnectButton(account: IloAccount, onDone: () -> Unit, modifier: Modifier) {
    val scope = rememberCoroutineScope()
    OutlinedButton(
        onClick = { scope.launch { account.disconnect(); onDone() } },
        modifier = modifier,
    ) { Text(stringResource(R.string.account_disconnect)) }
}

/** The last sync's word and the button that asks for one now. A press that lands while a
 * cycle runs joins it (refresh waits for the running one) instead of stacking another. */
@Composable
private fun SyncNow(account: IloAccount, stats: SyncStats?, syncing: Boolean) {
    val scope = rememberCoroutineScope()
    if (stats != null) {
        Text(
            stringResource(
                R.string.account_last_sync,
                agoSentence(stats.at),
                stats.downloaded, stats.uploaded),
            style = MaterialTheme.typography.bodyMedium,
        )
    } else {
        Text(stringResource(R.string.account_never_synced), style = MaterialTheme.typography.bodyMedium)
    }
    OutlinedButton(
        enabled = !syncing,
        onClick = { scope.launch { account.refresh() } },
        modifier = Modifier.padding(top = 8.dp),
    ) {
        Text(
            if (syncing) stringResource(R.string.account_syncing_now)
            else stringResource(R.string.account_sync_now))
    }
}

/** How long ago a time was, in words the phone's own language says. */
@Composable
private fun agoSentence(at: Long): String {
    val seconds = (System.currentTimeMillis() - at) / 1000
    return when {
        seconds < 45 -> stringResource(R.string.account_ago_now)
        seconds < 2700 -> stringResource(R.string.account_ago_minutes, (seconds + 30) / 60)
        else -> stringResource(R.string.account_ago_hours, (seconds + 1800) / 3600)
    }
}

@Composable
private fun SignedIn(
    account: IloAccount,
    email: String,
    spaces: List<app.ilo.browser.android.sync.SyncedSpace>,
    nodes: List<app.ilo.browser.android.sync.SyncedNode>,
    look: app.ilo.browser.android.ui.Look,
    disconnected: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var ownWords by remember { mutableStateOf<List<String>?>(null) }
    val stats by account.stats.collectAsState()
    val syncing by account.syncing.collectAsState()
    Text(
        if (email.isEmpty()) stringResource(R.string.account_title)
        else stringResource(R.string.account_signed_in_as, email),
        style = MaterialTheme.typography.titleMedium,
    )
    SyncNow(account, stats, syncing)
    OutlinedButton(
        onClick = { scope.launch { account.signOut() } },
        modifier = Modifier.padding(top = 8.dp),
    ) { Text(stringResource(R.string.account_sign_out)) }
    DisconnectButton(account, disconnected, Modifier.padding(top = 8.dp))
    // The words hide again as soon as they leave the screen, so a look away is all it takes
    // to close them, and the same row both asks and answers.
    DisposableEffect(Unit) { onDispose { ownWords = null } }
    OutlinedButton(
        onClick = {
            if (ownWords == null) scope.launch { ownWords = account.recoveryWords() }
            else ownWords = null
        },
        modifier = Modifier.padding(top = 8.dp),
    ) {
        Text(
            if (ownWords == null) stringResource(R.string.account_show_words)
            else stringResource(R.string.account_hide_words))
    }
    ownWords?.let {
        Text(
            stringResource(R.string.account_words_warning),
            color = look.soft,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(top = 8.dp),
        )
        Text(
            it.joinToString(" "),
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(top = 8.dp),
        )
    }

    Text(
        stringResource(R.string.account_spaces_title),
        style = MaterialTheme.typography.titleMedium,
        modifier = Modifier.padding(top = 16.dp, bottom = 8.dp),
    )
    if (spaces.isEmpty()) {
        Text(stringResource(R.string.account_no_spaces))
        return
    }
    for (space in spaces) {
        val count = nodes.count { it.space == space.id }
        Text(
            stringResource(R.string.account_space_tabs, space.name,
                stringResource(R.string.account_tabs_count, count)),
            modifier = Modifier.padding(vertical = 4.dp),
        )
    }
}

@Composable
private fun Failed(message: String, onRetry: () -> Unit) {
    Text(message, style = MaterialTheme.typography.bodyMedium)
    Button(onClick = onRetry, modifier = Modifier.padding(top = 8.dp)) {
        Text(stringResource(R.string.account_retry))
    }
}