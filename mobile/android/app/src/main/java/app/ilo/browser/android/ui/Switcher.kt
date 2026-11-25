package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.Tab
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/**
 * The switcher the bar's deck opens, over the whole screen as Android's own recents are: the
 * tabs stacked in stairs, the front card whole, each one behind it half showing beside it
 * and a step above it, tilted as if pinned to a wheel's rim. A tap on a behind card steps
 * the deck to it. The incognito mark and the archive and the way
 * out sit together at the top right: the hat lifted and tabs come up regular, the hat worn
 * and they come up incognito, the archive is a card away, and the close X leaves the deck. A
 * card opens its page, keeps the
 * picture it last showed, and can be thrown: up and its tab closes, the card flying away; down
 * and its tab goes to the archive, the card dropping out. Which way up goes is the user's own
 * choice in Settings; down archives unless that is turned off. A thrown card's notice waits
 * under the deck for its way back.
 */
@Composable
fun Switcher(
    browser: Browser,
    tab: Tab?,
    look: Look,
    onOpen: (Tab) -> Unit,
    onOpenArchive: () -> Unit,
    onClose: () -> Unit,
    account: app.ilo.browser.android.sync.IloAccount? = null,
) {
    val tabs by browser.state.tabs.collectAsState()
    val archived by browser.stores.archivedFlow.collectAsState()
    // The kind the deck shows is the mode the Space is in, newest first as iOS's Today list is,
    // so the cards behind the front one are the older tabs beneath it and the stack has cards.
    val private by browser.state.privateMode.collectAsState()
    val thrown by browser.thrownFlow.collectAsState()
    val context = androidx.compose.ui.platform.LocalContext.current
    val kind = remember(tabs, private) { tabs.filter { it.isPrivate == private }.reversed() }
    val upAction = browser.settings.swipeUpAction
    val downAction = browser.settings.swipeDownAction

    BackHandler(onBack = onClose)

    Column(
        Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(look.groundTop, look.groundBottom)))
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        KindRow(
            look = look,
            private = private,
            archivedCount = archived.size,
            onEye = {
                val on = !private
                browser.state.privateMode.value = on
                android.widget.Toast.makeText(
                    context,
                    context.getString(if (on) R.string.incognito_on else R.string.incognito_off),
                    if (on) android.widget.Toast.LENGTH_LONG else android.widget.Toast.LENGTH_SHORT,
                ).show()
            },
            onArchive = onOpenArchive,
            onClose = onClose,
        )
        // The deck is a carousel: the front card whole, the ones behind it half showing
        // beside it and a stair above, a tap on a behind card rotating the deck to it, a
        // sideways drag past a share of the screen rotating the whole ring one card over.
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (kind.isEmpty()) {
                Text(
                    stringResource(if (private) R.string.switcher_none_incognito else R.string.switcher_none_regular),
                    color = look.soft,
                    fontSize = 17.sp,
                    modifier = Modifier.align(Alignment.Center),
                )
            } else {
                // The carousel's own ring: the current tab first, the rest behind it newest
                // first, the order the deck showed. A turn rotates the ring, front to back for a
                // left swipe, back to front for a right.
                var ring by remember { mutableStateOf(rotateToCurrent(kind, tab)) }
                // The ring follows the kind: a thrown card leaves it, a new one joins at the back.
                LaunchedEffect(kind) {
                    val ids = kind.map { it.id }
                    // A thrown card leaves the ring in its order, so the next one comes front;
                    // a new one joins at the back. The user's turn is never undone.
                    ring = ring.filter { it in ids }.plus(ids.filter { it !in ring })
                }
                val ordered = remember(ring, kind) { ring.mapNotNull { id -> kind.firstOrNull { it.id == id } } }
                BoxWithConstraints {
                    // The card is a share of the deck wide, so the stack's steps and stairs are
                    // shares of the card the user sees, not of the whole deck. Both shares are
                    // the owner's dials in Settings, read once per open of the deck.
                    val cardShare = browser.settings.switcherCardShare
                    val covered = browser.settings.switcherOverlap
                    // The fan fits the deck at any dial values, and the same pure math a
                    // unit test walks is what runs here.
                    val layout = fanGeometry(maxWidth.value, maxHeight.value, cardShare, covered, ordered.size)
                    val cardWidth = layout.cardWidth.dp
                    val cardHeight = layout.cardHeight.dp
                    // The reach is a share of the screen the deck fills, and the throw flies a
                    // deck's height, both too big for the card to measure on itself.
                    val reachX = with(LocalDensity.current) { maxWidth.toPx() } * Metrics.Switcher.reach
                    val reachY = with(LocalDensity.current) { maxHeight.toPx() } * Metrics.Switcher.reach
                    val flyTo = with(LocalDensity.current) { maxHeight.toPx() }
                    // The deeper cards compose first so each nearer one covers the one
                    // behind it, and their taps reach them.
                    for (depth in layout.shownBehind downTo 1) {
                        val index = depth
                        StackCardBack(
                            browser = browser,
                            tab = ordered[index],
                            look = look,
                            modifier = Modifier
                                .align(Alignment.Center)
                                .offset(x = (layout.step * depth - layout.fanShift).dp, y = (-layout.stair * depth).dp)
                                // Tilted a little more the deeper it stands, as if the
                                // stack curved around a wheel's rim, never past the angle
                                // that keeps the card's corners on screen.
                                .graphicsLayer(rotationZ = layout.tilts[depth - 1])
                                // The same size as the front card, so the stairs line up.
                                .size(width = cardWidth, height = cardHeight)
                                // A tap rotates the carousel straight to the card the
                                // user reached for.
                                .clickable(role = Role.Button) {
                                    ring = ring.drop(index) + ring.take(index)
                                },
                        )
                    }
                    val cardTab = ordered.firstOrNull() ?: kind.first()
                    // One physical drag is one turn even across the card swap a turn
                    // makes, so the lock is the deck's, not the card's.
                    var turnLock by remember { mutableStateOf(false) }
                    // Keyed by the tab: a throw that shortens the deck must not carry the
                    // card's drag state into the next card that takes its place.
                    key(cardTab.id) {
                        SwitcherCard(
                            browser = browser,
                            tab = cardTab,
                            look = look,
                            cardWidth = cardWidth,
                            homeX = (-layout.fanShift).dp,
                            reachX = reachX,
                            reachY = reachY,
                            swipeReach = browser.settings.swipeReach,
                            flyTo = flyTo,
                            upAction = upAction,
                            downAction = downAction,
                            onOpen = { onOpen(cardTab) },
                            onGestureStart = { turnLock = false },
                            // A drag past a share of the screen turns the ring once: left
                            // sends the front card to the back, right brings the last one
                            // forward.
                            onTurn = { right ->
                                if (ring.size > 1 && !turnLock) {
                                    turnLock = true
                                    ring = if (right) listOf(ring.last()) + ring.dropLast(1)
                                    else ring.drop(1) + ring.take(1)
                                }
                            },
                        )
                    }
                }
            }
        }
        thrown?.let { UndoNotice(browser = browser, thrown = it, look = look) }
        if (!private) OtherComputers(account, look) { url ->
            browser.openUrlInNewTab(url)
            onClose()
        }
    }
}

/** The tabs the account's other computers hold open, from SESSIONS, grouped by the computer
 * each came from; a tap opens one here as a new tab. Shown only signed in and never in private. */
@Composable
private fun OtherComputers(
    account: app.ilo.browser.android.sync.IloAccount?,
    look: Look,
    onOpen: (String) -> Unit,
) {
    val others by (account?.otherComputers
        ?: kotlinx.coroutines.flow.MutableStateFlow(emptyList()))
        .collectAsState()
    if (others.isEmpty()) return
    val grouped = remember(others) { others.groupBy { it.computerName ?: it.sessionTag } }
    LazyColumn(
        verticalArrangement = Arrangement.spacedBy(2.dp),
        contentPadding = PaddingValues(start = Metrics.Column.side, end = Metrics.Column.side, bottom = 12.dp),
        modifier = Modifier.fillMaxWidth().heightIn(max = 220.dp),
    ) {
        // The group names itself, so a machine's tag is read as what it is: one of the
        // account's other computers, not a heading the user must decode.
        item(key = "caption") {
            Text(
                stringResource(R.string.other_computers),
                color = look.soft,
                fontSize = 13.sp,
                modifier = Modifier.padding(top = 8.dp, start = 4.dp),
            )
        }
        for ((name, tabs) in grouped) {
            item(key = "head-$name") {
                Text(
                    name,
                    color = look.soft,
                    fontSize = 13.sp,
                    modifier = Modifier.padding(top = 8.dp, start = 4.dp),
                )
            }
            items(tabs, key = { "${it.sessionTag}/${it.tabId}" }) { other ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(min = 52.dp)
                        .background(look.plate, RoundedCornerShape(12.dp))
                        .clickable(role = Role.Button) { onOpen(other.url) }
                        .padding(horizontal = 12.dp)
                        .semantics { contentDescription = other.title },
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            other.title.ifEmpty { other.url },
                            color = look.text,
                            fontSize = Type.bar,
                            maxLines = 1,
                        )
                        Text(hostOf(other.url), color = look.soft, fontSize = 13.sp, maxLines = 1)
                    }
                }
            }
        }
    }
}

/**
 * The switcher's top row, grouped at the right as the owner asked: the incognito mark that
 * turns the mode first, the hat worn while it is on, the archive beside it with its count, and
 * the close X that leaves the deck, all one control size with one gap between them.
 */
@Composable
private fun KindRow(
    look: Look,
    private: Boolean,
    archivedCount: Int,
    onEye: () -> Unit,
    onArchive: () -> Unit,
    onClose: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Metrics.Switcher.controlGap),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = Metrics.Switcher.controlSide, vertical = 12.dp),
    ) {
        Spacer(Modifier.weight(1f))
        val incognito = stringResource(R.string.incognito)
        Box(
            modifier = Modifier
                .size(Metrics.Switcher.control)
                .background(if (private) look.accent else look.plate, CircleShape)
                .clickable(role = Role.Button, onClick = onEye)
                .semantics { contentDescription = incognito },
            contentAlignment = Alignment.Center,
        ) {
            Canvas(Modifier.size(18.dp)) {
                if (private) drawIncognitoGlyph(Look.Paper, size.width, size.height)
                else drawIncognitoOffGlyph(look.text, size.width, size.height)
            }
        }
        // The archive is a way in from wherever the deck is, its count saying what it holds.
        val archivedWord = stringResource(R.string.archived)
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .height(Metrics.Switcher.control)
                .background(look.plate, RoundedCornerShape(Metrics.Switcher.control / 2))
                .clickable(role = Role.Button, onClick = onArchive)
                .padding(horizontal = 12.dp)
                .semantics { contentDescription = "$archivedWord, $archivedCount" },
        ) {
            Text(archivedWord, color = look.text, fontSize = 14.sp)
            Spacer(Modifier.width(6.dp))
            Text("$archivedCount", color = look.soft, fontSize = 14.sp)
        }
        // The way out the owner asked for, beside the archive: the X closes the whole deck.
        val closeWord = stringResource(R.string.close)
        Box(
            modifier = Modifier
                .size(Metrics.Switcher.control)
                .background(look.plate, CircleShape)
                .clickable(role = Role.Button, onClick = onClose)
                .semantics { contentDescription = closeWord },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                Icons.Filled.Close,
                contentDescription = null,
                tint = look.text,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

/**
 * The notice a thrown card leaves: its move named, its way back one press, and the seconds the
 * move waits before it stands, read fresh for each throw as the user set them.
 */
@Composable
private fun UndoNotice(browser: Browser, thrown: Browser.Thrown, look: Look) {
    // The remaining seconds come from the throw's own moment, so the count and the window
    // Browser holds can never disagree.
    var seconds by remember(thrown) {
        mutableStateOf(
            browser.settings.undoSeconds - ((System.currentTimeMillis() - thrown.at) / 1000L).toInt()
        )
    }
    LaunchedEffect(thrown) {
        while (seconds > 0) {
            kotlinx.coroutines.delay(1000L)
            seconds = browser.settings.undoSeconds -
                ((System.currentTimeMillis() - thrown.at) / 1000L).toInt()
        }
    }
    val title = thrown.tab.title.ifEmpty { thrown.tab.url ?: "" }
    val words = stringResource(if (thrown.kind == "archived") R.string.tab_archived else R.string.tab_closed)
    val undo = stringResource(R.string.undo)
    Box(Modifier.fillMaxWidth().padding(bottom = 10.dp), contentAlignment = Alignment.Center) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier
                .background(look.plate, RoundedCornerShape(18.dp))
                .clickable(role = Role.Button, onClick = { browser.undoThrown() })
                .semantics { contentDescription = undo }
                // The notice keeps Android's own smallest touch target, so the press that
                // brings the tab back lands anywhere on it.
                .heightIn(min = 48.dp)
                .padding(horizontal = 14.dp, vertical = 9.dp),
        ) {
            Icon(
                Icons.Filled.ArrowBack,
                contentDescription = null,
                tint = look.text,
                modifier = Modifier.size(16.dp),
            )
            Text(
                text = "$words \u00b7 $title",
                color = look.text,
                fontSize = 14.sp,
                maxLines = 1,
            )
            Text("$seconds", color = look.soft, fontSize = 14.sp)
        }
    }
}

/**
 * One card standing behind the front one: the same face as the deck's card, dimmed so the
 * front one stays the figure, and never interactive beyond the tap that steps the deck to it.
 */
@Composable
private fun StackCardBack(
    browser: Browser,
    tab: Tab,
    look: Look,
    modifier: Modifier = Modifier,
) {
    val pictures by browser.pictures.version.collectAsState()
    val picture = remember(tab.id, pictures) { browser.pictures.picture(of = tab.id) }
    LaunchedEffect(tab.id) { browser.pictures.load(of = tab.id) }
    val cardShape = RoundedCornerShape(Metrics.Card.radius)
    Column(
        modifier
            .shadow(8.dp, cardShape, spotColor = look.cardShadow, ambientColor = look.cardShadow)
            .background(if (tab.isPrivate) Look.Ink else look.overlay, cardShape)
            .then(
                if (tab.isPrivate) Modifier.border(0.5.dp, Look.Paper.copy(alpha = Metrics.PrivateMark.cardRim), cardShape)
                else Modifier.border(1.dp, look.rim, cardShape)
            )
            .clip(cardShape)
            // The dim over the whole card keeps a behind card behind the front one; drawn here
            // so the tap on it still reaches the step-to-front handler.
            .drawWithContent {
                drawContent()
                drawRect(Color.Black.copy(alpha = 0.14f))
            }
            .semantics { contentDescription = tab.title.ifEmpty { tab.url ?: "" } },
    ) {
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (picture != null) {
                Image(
                    bitmap = picture.asImageBitmap(),
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                // The same always-present fallback as the front card, so a behind card never
                // shows a bare plate.
                FaviconFallback(browser = browser, tab = tab, look = look)
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .background(if (tab.isPrivate) Look.Ink else look.overlay)
                .padding(horizontal = 14.dp, vertical = 10.dp),
        ) {
            Text(
                text = tab.title.ifEmpty { tab.url ?: "" },
                color = look.text,
                fontSize = Type.card,
                maxLines = 1,
            )
        }
    }
}

/**
 * The card's answer when no picture exists yet, as the owner asked that a preview always be
 * given: the site's favicon or its host's letter, and the tab's title under it. FaviconMark
 * only reads, so a private tab keeps falling back to its host's letter, never a kept icon.
 */
@Composable
private fun FaviconFallback(browser: Browser, tab: Tab, look: Look) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
        modifier = Modifier.fillMaxSize(),
    ) {
        if (tab.isPrivate) {
            // The private deck keeps its hat as the mark: a host letter would name a
            // site a private page never names.
            Box(
                Modifier
                    .size(Metrics.PrivateMark.plate)
                    .background(
                        Look.Paper.copy(alpha = Metrics.PrivateMark.plateFill),
                        RoundedCornerShape(Metrics.PrivateMark.plateRadius),
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Canvas(Modifier.size(Metrics.PrivateMark.onCard)) {
                    drawIncognitoGlyph(Look.Paper.copy(alpha = Metrics.PrivateMark.cardGlyph), size.width, size.height)
                }
            }
        } else {
            FaviconMark(
                url = tab.url ?: "",
                favicons = browser.favicons,
                look = look,
                modifier = Modifier.size(Metrics.PrivateMark.plate),
            )
        }
        Spacer(Modifier.height(10.dp))
        Text(
            text = tab.title.ifEmpty { tab.url ?: stringResource(R.string.private_tab) },
            color = look.text,
            fontSize = Type.card,
            maxLines = 2,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
    }
}

/**
 * The card in the carousel's front place: the page's picture where the phone kept one, else
 * its favicon or host letter with its title. The finger owns the card on its own two axes,
 * picked at the drag's first move and locked there: a vertical drag previews the move its
 * direction would make, red for close and yellow for archive over the picture, and a
 * release past the reach throws; a horizontal drag past the reach turns the carousel once,
 * the card thinning as it goes so the one behind reads through it. Up and its tab closes,
 * down it goes to the archive, as the user set each.
 */
@Composable
private fun SwitcherCard(
    browser: Browser,
    tab: Tab,
    look: Look,
    cardWidth: Dp,
    homeX: Dp,
    reachX: Float,
    reachY: Float,
    flyTo: Float,
    // The owner's dial on how much travel a release needs, 1 the old full reach.
    swipeReach: Float = 0.5f,
    upAction: String,
    downAction: String,
    onOpen: () -> Unit,
    onGestureStart: () -> Unit,
    onTurn: (right: Boolean) -> Unit,
) {
    // The version is read so a capture landing after the card first showed reaches it.
    val pictures by browser.pictures.version.collectAsState()
    val picture = remember(tab.id, pictures) { browser.pictures.picture(of = tab.id) }
    // The disk read runs outside composition, so a decode never lands mid-frame.
    LaunchedEffect(tab.id) { browser.pictures.load(of = tab.id) }
    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val cardShape = RoundedCornerShape(Metrics.Card.radius)

    // The finger's own two axes, one drag never turning and throwing the same card.
    val dragX = remember(tab.id) { Animatable(0f) }
    val dragY = remember(tab.id) { Animatable(0f) }
    // Armed when the pull is past and the direction has a move to make, so the preview and
    // the release both answer the same state.
    var armed by remember(tab.id) { mutableStateOf(false) }
    var armedUp by remember(tab.id) { mutableStateOf(false) }
    // The drag's own axis, picked at its first move: a card goes either sideways or up and
    // down, never both, so a throw never bleeds into a turn.
    var axis by remember(tab.id) { mutableStateOf(0) }

    fun moveFor(up: Boolean): (() -> Unit)? = when {
        up && upAction == "archive" -> { { browser.thrown("archived", tab) { browser.archiveTab(tab) } } }
        up && upAction == "none" -> null
        up -> { { browser.thrown("closed", tab) { browser.engine.destroy(tab); browser.state.close(tab.id) } } }
        !up && downAction == "archive" -> { { browser.thrown("archived", tab) { browser.archiveTab(tab) } } }
        !up && downAction == "close" -> { { browser.thrown("closed", tab) { browser.engine.destroy(tab); browser.state.close(tab.id) } } }
        else -> null
    }

    // The throw runs inline in the release's own coroutine, so the spring back can never
    // race the card off it and cancel the move.
    suspend fun throwAndMove(up: Boolean, flyTo: Float) {
        val move = moveFor(up) ?: return
        dragY.animateTo(if (up) -flyTo else flyTo, animationSpec = tween(280))
        move()
    }

    fun settleHome() {
        scope.launch { dragX.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = 380f)) }
        scope.launch { dragY.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = 380f)) }
        armed = false
        // The axis is the drag's own; without this reset the thinned card would stay thin.
        axis = 0
    }

    // The card's own name for what it is, hoisted so the semantics block stays plain.
    val privateName = stringResource(R.string.private_tab)
    // The hint names what each direction does, so up and down can't be mixed up.
    val actionWord = mapOf(
        "close" to stringResource(R.string.card_action_close),
        "archive" to stringResource(R.string.card_action_archive),
        "none" to stringResource(R.string.card_action_none),
    )
    val hint = stringResource(R.string.card_drag_hint, actionWord[upAction].orEmpty(), actionWord[downAction].orEmpty())
    val closeWord = stringResource(R.string.close)
    val archivedWord = stringResource(R.string.archived)
    Box(Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Box(
            Modifier
                .align(Alignment.Center)
                .size(width = cardWidth, height = cardWidth / Metrics.Switcher.aspect)
                .graphicsLayer {
                    // 30% transparent while a switch swipe drags the card, so the card
                    // behind it reads through it as it moves.
                    alpha = if (axis == 1) 1f - Metrics.Switcher.frontFade else 1f
                }
                .offset(
                    // homeX centers the whole fan: the front card stands as far left of
                    // the deck's middle as the deepest stands right of it.
                    x = homeX + with(density) { (dragX.value).toDp() },
                    y = with(density) { (dragY.value).toDp() },
                )
        ) {
            Column(
                Modifier
                    .fillMaxSize()
                    .shadow(8.dp, cardShape, spotColor = look.cardShadow, ambientColor = look.cardShadow)
                    .background(if (tab.isPrivate) Look.Ink else look.overlay, cardShape)
                    .then(
                        if (tab.isPrivate) Modifier.border(0.5.dp, Look.Paper.copy(alpha = Metrics.PrivateMark.cardRim), cardShape)
                        else Modifier.border(1.dp, look.rim, cardShape)
                    )
                    .clip(cardShape)
                    // The dim is drawn inside the clip, so the preview never bleeds past the card:
                    // red says close, yellow says archive, the move the release would make.
                    .drawWithContent {
                        drawContent()
                        if (armed) {
                            val archive =
                                (armedUp && upAction == "archive") || (!armedUp && downAction == "archive")
                            drawRect(
                                if (archive) Color(0xFFE0A600).copy(alpha = 0.45f)
                                else Color(0xFFC9414B).copy(alpha = 0.45f),
                            )
                        }
                    }
                // Keyed by the tab: a new card in this place starts its drag from
                // zero, never from the card before it.
                .pointerInput(tab.id) {
                    // The drag's running position lives here and not in the Animatable: a
                    // snap runs in its own coroutine and a fast swipe would read stale zeros.
                    var accX = 0f
                    var accY = 0f
                    // detectDragGestures hands no release speed, so the drag feeds this
                    // tracker and the release reads it.
                    val tracker = VelocityTracker()
                    detectDragGestures(
                        onDragStart = {
                            axis = 0
                            accX = 0f
                            accY = 0f
                            tracker.resetTracking()
                            onGestureStart()
                        },
                        onDrag = { change, amount ->
                            change.consume()
                            tracker.addPosition(change.uptimeMillis, change.position)
                            // The first move picks the axis and locks it: the card goes one
                            // way, sideways or up and down, as the owner asked.
                            if (axis == 0) {
                                axis = if (abs(amount.x) >= abs(amount.y)) 1 else 2
                            }
                            if (axis == 1) accX += amount.x
                            if (axis == 2) accY += amount.y
                            scope.launch {
                                dragX.snapTo(accX)
                                dragY.snapTo(accY)
                            }
                            // The drag only follows; the verdict waits for the release's speed.
                            val up = accY < 0f
                            // The preview arms early, at the floor a flick would need, so the
                            // user sees the move before the slow drag pays the full travel.
                            val floorY = reachY * swipeReach / 3f
                            armed = axis == 2 && abs(accY) >= floorY && moveFor(up) != null
                            armedUp = up
                        },
                        onDragEnd = {
                            val upward = accY < 0f
                            val velocity = tracker.calculateVelocity()
                            // The deck's own size backs the formula's speed term.
                            val deckW = reachX / Metrics.Switcher.reach
                            val deckH = reachY / Metrics.Switcher.reach
                            val travelX = swipeCommitTravel(deckW, velocity.x, reachX * swipeReach)
                            val travelY = swipeCommitTravel(deckH, velocity.y, reachY * swipeReach)
                            scope.launch {
                                when {
                                    // A release past its travel turns the ring once; the deck's
                                    // own lock keeps one swipe to one turn.
                                    axis == 1 && abs(accX) >= travelX -> onTurn(accX > 0f)
                                    // A vertical release past its travel throws the card the
                                    // way it was held; the distance clears the card whole.
                                    axis == 2 && abs(accY) >= travelY && moveFor(upward) != null -> {
                                        throwAndMove(upward, flyTo)
                                        // The card leaves with its tab; no spring back may chase it.
                                        return@launch
                                    }
                                }
                                // Short of the reach, or after a turn, the card springs
                                // home, the cancelled throw the ease-off was for.
                                dragX.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = 380f))
                                dragY.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = 380f))
                                armed = false
                                axis = 0
                            }
                        },
                        onDragCancel = { settleHome() },
                    )
                }
                .clickable(role = Role.Button, onClick = onOpen)
                .semantics {
                    contentDescription = tab.title.ifEmpty { tab.url ?: privateName } + ", " + hint
                },
        ) {
            Box(Modifier.weight(1f).fillMaxWidth()) {
                if (picture != null) {
                    Image(
                        bitmap = picture.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    FaviconFallback(browser = browser, tab = tab, look = look)
                }
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(if (tab.isPrivate) Look.Ink else look.overlay)
                    .padding(horizontal = 14.dp, vertical = 10.dp),
            ) {
                if (tab.isPrivate) {
                    Canvas(Modifier.size(16.dp)) { drawIncognitoGlyph(look.soft, size.width, size.height) }
                    Spacer(Modifier.width(7.dp))
                } else {
                    Box(Modifier.size(16.dp).background(Color(0xFF000000 + tab.faviconColor), CircleShape))
                }
                Text(
                    text = tab.title.ifEmpty { tab.url ?: stringResource(R.string.private_tab) },
                    color = look.text,
                    fontSize = Type.card,
                    maxLines = 1,
                    modifier = Modifier.padding(start = 7.dp),
                )
            }
            }
            // The label names the move a release past the pull would make, over its color.
            if (armed) {
                val archive =
                    (armedUp && upAction == "archive") || (!armedUp && downAction == "archive")
                Text(
                    text = if (archive) archivedWord else closeWord,
                    color = Look.Paper,
                    fontSize = 22.sp,
                    modifier = Modifier.align(Alignment.Center),
                )
            }
        }
    }
}

/**
 * The ring's opening order: the selected tab at the front and the rest behind it in the
 * kind's own order, so the carousel opens on the page the user left.
 */
private fun rotateToCurrent(kind: List<Tab>, tab: Tab?): List<String> {
    val ids = kind.map { it.id }
    val at = ids.indexOf(tab?.id).takeIf { it >= 0 } ?: return ids
    return ids.drop(at) + ids.take(at)
}

/** The fan's fitted shape: sizes in the deck's own units, the tilt list one per behind
 * card, deepest first, already clamped to the angle that keeps its corners on screen. */
internal data class FanGeometry(
    val cardWidth: Float,
    val cardHeight: Float,
    val step: Float,
    val stair: Float,
    val fanShift: Float,
    val shownBehind: Int,
    val tilts: List<Float>,
)

/**
 * The whole fan's layout as pure math, so the same numbers a unit test walks are the
 * numbers the deck draws. The card is the dial's share of the deck's width, capped by
 * its own height so a tall fan stays on screen; the fan shows behind cards while both the
 * horizontal span and the stair's lift keep inside, and each shown card's tilt is
 * clamped to the steepest angle whose rotated corners still fit the deck.
 */
internal fun fanGeometry(
    deckW: Float,
    deckH: Float,
    cardShare: Float,
    covered: Float,
    tabCount: Int,
): FanGeometry {
    val behindWanted = min(Metrics.Switcher.behind, (tabCount - 1).coerceAtLeast(0))
    val rawWidth = deckW * cardShare
    // The card must fit the deck's height too: the stair lifts each behind card, so the
    // card plus the deepest card's lift stays within the deck. This cap is what makes
    // the stair fit for every shown card, since shownBehind never passes behindWanted.
    val maxByHeight = deckH * Metrics.Switcher.aspect / (1f + Metrics.Switcher.stair * behindWanted)
    val cardWidth = min(rawWidth, maxByHeight)
    val cardHeight = cardWidth / Metrics.Switcher.aspect
    val step = cardWidth * (1f - covered)
    // The span fits while the front card's left edge and the deepest shown card's right
    // edge both keep inside; a lone card centers by showing none behind.
    val behindBySpan = ((deckW - cardWidth) / step).toInt()
    val shownBehind = min(behindWanted, behindBySpan)
    val stair = cardHeight * Metrics.Switcher.stair
    val fanShift = step * shownBehind / 2f
    val tilts = (1..shownBehind).map { depth ->
        val center = deckW / 2f + step * depth - fanShift
        tiltFits(deckW, center, cardWidth, cardHeight, Metrics.Switcher.tilt * depth)
    }
    return FanGeometry(cardWidth, cardHeight, step, stair, fanShift, shownBehind, tilts)
}

/** The tilt clamped to the steepest angle whose rotated corners keep inside the deck;
 * a flat card fits whenever a behind card is shown at all, so the search always stops. */
private fun tiltFits(deckW: Float, centerX: Float, cardWidth: Float, cardHeight: Float, want: Float): Float {
    var angle = want
    while (!cornersInside(deckW, centerX, cardWidth, cardHeight, angle)) angle -= 0.25f
    return angle
}

/** The four corners of a card rotated about its center, all inside the deck's width. The
 * test file asserts with this same function, so what it proves and what the deck guards
 * with cannot drift apart. */
internal fun cornersInside(deckW: Float, centerX: Float, cardWidth: Float, cardHeight: Float, tilt: Float): Boolean {
    val hw = cardWidth / 2f
    val hh = cardHeight / 2f
    val rad = Math.toRadians(tilt.toDouble())
    val cos = cos(rad).toFloat()
    val sin = sin(rad).toFloat()
    for (dx in listOf(-hw, hw)) {
        for (dy in listOf(-hh, hh)) {
            val x = centerX + dx * cos - dy * sin
            if (x < 0f || x > deckW) return false
        }
    }
    return true
}
