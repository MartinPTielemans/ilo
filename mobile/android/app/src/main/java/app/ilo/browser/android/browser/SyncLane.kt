package app.ilo.browser.android.browser

import app.ilo.browser.android.sync.AccountState
import app.ilo.browser.android.sync.IloAccount
import app.ilo.browser.android.sync.SyncedNode
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * The bits of the browser state the lane moves between the account and. An interface so a plain
 * unit test stands in for the state; the adapter over BrowserState is [BrowserState.host].
 */
interface TabsHost {
    val spaces: StateFlow<List<Space>>
    val tabs: StateFlow<List<Tab>>
    /** The tab whose page is on screen, so the lane notes its showing to the account. */
    val selected: StateFlow<String?>
    /** Replaces the Space list; an empty list falls back to the phone's own. */
    fun setSpaces(list: List<Space>)
    /** Replaces the whole tab list, so the lane's changes are one write. */
    fun writeTabs(list: List<Tab>)
    /** The lane sets this; its echo from a push must reach nobody, or the lane would loop. */
    var spacesChanged: ((List<Space>) -> Unit)?
}

/** The browser's state as the lane sees it. */
fun BrowserState.host(): TabsHost = object : TabsHost {
    override val spaces: StateFlow<List<Space>> get() = this@host.spaces
    override val tabs: StateFlow<List<Tab>> get() = this@host.tabs
    override val selected: StateFlow<String?> get() = this@host.selectedId
    override fun setSpaces(list: List<Space>) = this@host.setSpaces(list)
    override fun writeTabs(list: List<Tab>) { this@host.tabs.value = list }
    override var spacesChanged: ((List<Space>) -> Unit)?
        get() = this@host.spacesChanged
        set(value) { this@host.spacesChanged = value }
}

/**
 * The sync lane: the only thing that moves between the account and the browser state. A tab the
 * account takes is re-keyed to its node id, and the id map carries everything the phone keeps.
 */
class SyncLane(
    private val account: IloAccount,
    private val state: TabsHost,
    scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
) {
    private val jobs = scope
    /** A tab's id in the state and its node id, in both directions. */
    private val toNode = HashMap<String, String>()
    private val toTab = HashSet<String>()
    /** What the lane last pushed, so an unchanged account list is never pushed back. */
    private var pushed: List<Space> = emptyList()
    /** The account Space ids the tabs were open in, kept for the sign-out rehome: at sign-out the
     * account clears its Spaces after it says SignedOut, so the live list is no use then. */
    private var accountIds = HashSet<String>()
    /** Whether the account's Spaces have been open here, so a sign-out keeps the tabs. */
    private var joinedOnce = false
    /** Tabs whose push to the account is done, so an address set after a new tab pushes once. */
    private val pushedTabs = HashSet<String>()
    /** The ids of the tabs the last pass saw, so a gone id is a close. */
    private var known = HashSet<String>()
    /** Tabs that were here before the account's nodes first arrived, so may be restored at their
     * node ids: only these wait for the nodes before being pushed, never a newly made tab. */
    private val carried = HashSet<String>()
    private var sawFirstTabs = false
    /** The tab ids present when the lane was made, so join() can tell a tab that may still be a
     * node of this account from one made since: after a sign-out they sit rehomed at their old
     * node ids, and pushing them before the nodes are down would duplicate them server-side. */
    private val joinCarried = state.tabs.value.map { it.id }.toHashSet()
    /** Whether the account's nodes have come down for the current sign-in, so join() can stop
     * waiting for them and the waiting tabs be resolved. */
    private var nodesArrived = false

    init {
        state.spacesChanged = null
        jobs.launch { account.state.collect { phase -> onPhase(phase) } }
        jobs.launch { account.spaces.collect { spaces -> onSpaces(spaces) } }
        jobs.launch { account.nodes.collect { nodes -> onNodes(nodes) } }
        jobs.launch { state.tabs.collect { tabs -> onTabs(tabs) } }
        jobs.launch { state.selected.collect { id -> id?.let { saw(it) } } }
    }

    /** A Space color moves between the account's Int and the state's Long. */
    private fun toSpaces(list: List<app.ilo.browser.android.sync.SyncedSpace>): List<Space> =
        list.map { Space(it.id, it.name, it.color?.toLong()?.and(0xFFFFFF)) }

    /** Equal in every field the state shows, so an unchanged account never triggers a push. */
    private fun same(list: List<Space>): Boolean =
        list.size == pushed.size && list.zip(pushed).all { (one, other) ->
            one.id == other.id && one.name == other.name && one.color == other.color
        }

    private fun showing(): Boolean = account.spaces.value.isNotEmpty() &&
        account.state.value != AccountState.SignedOut

    private suspend fun onPhase(phase: AccountState) {
        if (phase is AccountState.SignedIn) account.refresh()
    }

    private suspend fun onSpaces(list: List<app.ilo.browser.android.sync.SyncedSpace>) {
        val spaces = toSpaces(list)
        if (same(spaces)) return
        pushed = spaces
        state.setSpaces(spaces)
        if (list.isNotEmpty()) {
            if (!joinedOnce) joinedOnce = true
            accountIds = list.map { it.id }.toHashSet()
            join()
        } else if (joinedOnce) {
            // The account's Spaces went empty after they'd been open, which its sign-out does
            // before its state says so: what synced stays on the device and nothing more syncs, so
            // the tabs stay and those whose Space went with the account come back to the phone's own.
            joinedOnce = false
            toNode.clear()
            toTab.clear()
            pushedTabs.clear()
            nodesArrived = false
            val tabs = state.tabs.value
            if (tabs.any { it.space in accountIds }) {
                writeTabs(tabs.map {
                    if (it.space in accountIds) it.copy(space = Space.DEFAULT) else it })
            }
            // The rehomed tabs still sit at their old node ids, which this account kept, so a
            // re-sign-in must adopt them, not push copies: init can't have seen these ids.
            joinCarried.addAll(state.tabs.value.map { it.id })
        }
    }

    /**
     * The tabs held in the phone's own Space become the account's, each pushed as a Today tab of
     * its first Space and re-keyed to the node the account made. What the account won't take
     * moves Space without becoming the account's. One bulk write, so it is one session persist.
     */
    private suspend fun join() {
        val target = account.spaces.value.firstOrNull()?.id ?: return
        val local = state.tabs.value
        var changed = false
        val joinedList = ArrayList<Tab>(local.size)
        for (tab in local) {
            if (tab.id in pushedTabs) { joinedList.add(tab); continue }
            if (tab.isPrivate) {
                // A private tab follows its Space, as iOS rehomes its private tabs, but is never
                // the account's.
                joinedList.add(if (tab.space == Space.DEFAULT) tab.copy(space = target) else tab)
                continue
            }
            if (tab.space != Space.DEFAULT) { joinedList.add(tab); continue }
            // A carried tab rehomed by a sign-out still sits at its old node id: the account kept
            // that node, so it is adopted back, and pushing a copy would duplicate it server-side.
            if (tab.id in joinCarried && toNode[tab.id] == null && tab.url != null) {
                if (account.nodes.value.isEmpty()) { joinedList.add(tab); continue }
                if (account.nodes.value.any { it.id == tab.id }) {
                    changed = true
                    joinedList.add(tab.copy(space = target))
                    map(tab.id, tab.id)
                    pushedTabs.add(tab.id)
                    continue
                }
            }
            changed = true
            val url = tab.url
            if (url == null) {
                joinedList.add(tab.copy(space = target))
                continue
            }
            val node = account.addTodayTab(target, url, tab.title)
            if (node == null) {
                joinedList.add(tab.copy(space = target))
                continue
            }
            joinedList.add(tab.copy(id = node, space = target))
            map(node, node)
            pushedTabs.add(node)
        }
        if (changed) writeTabs(joinedList)
    }

    private fun map(tabId: String, nodeId: String) {
        toNode[tabId] = nodeId
        toTab.add(nodeId)
    }

    private fun forget(id: String) {
        val node = toNode.remove(id)
        if (node != null) toTab.remove(node)
        pushedTabs.remove(id)
    }

    /**
     * The account's Today tabs are the truth of what's open: this brings in what arrived from
     * another device, drops what one closed there, and never touches a private or unmapped tab.
     */
    private suspend fun onNodes(nodes: List<SyncedNode>) {
        if (nodes.isNotEmpty() && !nodesArrived) {
            nodesArrived = true
            // Join waited on these nodes for tabs that may still be this account's, so they
            // resolve now that the account's word is down.
            if (joinedOnce) join()
        }
        if (!showing()) return
        val today = nodes.filter { it.kind == SyncedNode.Kind.TODAY }
        val tabs = state.tabs.value
        var changed = false
        val kept = ArrayList<Tab>(tabs.size)
        for (tab in tabs) {
            if (tab.isPrivate) { kept.add(tab); continue }
            val nodeId = toNode[tab.id] ?: run { kept.add(tab); continue }
            val node = today.firstOrNull { it.id == nodeId }
                ?: run { changed = true; forget(tab.id); continue }
            if (tab.url != node.url || tab.title != node.title || tab.space != node.space) {
                changed = true
                kept.add(tab.copy(url = node.url, title = node.title, space = node.space))
            } else {
                kept.add(tab)
            }
        }
        val present = kept.filter { !it.isPrivate }.mapNotNull { toNode[it.id] }.toSet()
        for (node in today) {
            // A node with no url is one the account can't open; the phone that made it holds it.
            if (node.id in present || node.id in toTab || node.url == null) continue
            changed = true
            // A tab restored from the session already sits at its node's id: it is adopted, not
            // added beside itself.
            val held = kept.indexOfFirst { it.id == node.id && !it.isPrivate }
            if (held >= 0) {
                kept[held] = kept[held].copy(url = node.url, title = node.title, space = node.space)
            } else {
                kept.add(Tab(id = node.id, url = node.url, title = node.title, space = node.space))
            }
            map(node.id, node.id)
            pushedTabs.add(node.id)
        }
        if (changed) writeTabs(kept)
    }

    /** A tab the state lost is a close: the account hears of it only for a mapped, non-private tab. */
    private suspend fun onTabs(tabs: List<Tab>) {
        val ids = tabs.map { it.id }.toSet()
        if (!sawFirstTabs) {
            sawFirstTabs = true
            carried.addAll(ids)
        }
        if (showing()) {
            for (id in known) {
                if (id !in ids) {
                    // Only tabs the lane mapped are the account's; a private one never is.
                    val nodeId = toNode[id]
                    if (nodeId != null) account.removeNode(nodeId)
                    forget(id)
                }
            }
            // A tab that got its address after it was made is pushed once, so a Go after an empty
            // new tab is one Today tab, not two.
            for (tab in tabs) {
                if (tab.isPrivate || tab.url == null) continue
                if (tab.id in pushedTabs || toNode[tab.id] != null) continue
                if (account.spaces.value.none { it.id == tab.space }) continue
                // A carried tab may be a restored one sitting at its node's id, and the account's
                // nodes may not have come down yet: pushing it before they have would open it
                // twice, so it waits, and the nodes pass adopts it. A newly made tab pushes now.
                if (tab.id in carried && account.nodes.value.isEmpty()) continue
                // A tab restored from the session already sits at its node's id: it is the
                // account's, and pushing it would open it twice.
                if (account.nodes.value.any { it.id == tab.id }) {
                    map(tab.id, tab.id)
                    pushedTabs.add(tab.id)
                    continue
                }
                val nodeId = account.addTodayTab(tab.space, tab.url!!, tab.title)
                pushedTabs.add(tab.id)
                if (nodeId != null) {
                    map(nodeId, nodeId)
                    pushedTabs.add(nodeId)
                    // The node may have arrived as a tab while the push was answering, so the
                    // old id goes and nothing is doubled.
                    val current = state.tabs.value
                    val hasNode = current.any { it.id == nodeId }
                    state.writeTabs(
                        if (hasNode) current.filterNot { it.id == tab.id }
                        else current.map { if (it.id == tab.id) it.copy(id = nodeId) else it })
                }
            }
        }
        known = state.tabs.value.map { it.id }.toHashSet()
    }

    private fun writeTabs(list: List<Tab>) {
        state.writeTabs(list)
        known = list.map { it.id }.toHashSet()
    }

    /** Called on a tab being shown; the account's seen-hour rule holds the writes back. */
    fun saw(tabId: String) {
        val nodeId = toNode[tabId] ?: return
        jobs.launch { account.sawToday(nodeId) }
    }
}