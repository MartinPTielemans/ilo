package app.ilo.browser.android.browser

import app.ilo.browser.android.sync.AccountState
import app.ilo.browser.android.sync.IloAccount
import app.ilo.browser.android.sync.OtherComputerTab
import app.ilo.browser.android.sync.PairingState
import app.ilo.browser.android.sync.SyncStats
import app.ilo.browser.android.sync.SyncedNode
import app.ilo.browser.android.sync.SyncedSpace
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** A stand-in account that records what the lane asked of it. */
private class FakeAccount : IloAccount {
    val stateFlow = MutableStateFlow<AccountState>(AccountState.SignedOut)
    val spacesFlow = MutableStateFlow<List<SyncedSpace>>(emptyList())
    val nodesFlow = MutableStateFlow<List<SyncedNode>>(emptyList())

    override val state: StateFlow<AccountState> = stateFlow.asStateFlow()
    override val spaces: StateFlow<List<SyncedSpace>> = spacesFlow.asStateFlow()
    override val nodes: StateFlow<List<SyncedNode>> = nodesFlow.asStateFlow()
    override val problem = MutableStateFlow<String?>(null).asStateFlow()
    override val email = MutableStateFlow("").asStateFlow()
    override val pairing = MutableStateFlow<PairingState>(PairingState.None).asStateFlow()
    override val stats = MutableStateFlow<SyncStats?>(null).asStateFlow()
    override val syncing = MutableStateFlow(false).asStateFlow()
    override val otherComputers =
        MutableStateFlow<List<OtherComputerTab>>(emptyList()).asStateFlow()

    val added = ArrayList<Pair<String, String>>()
    val removed = ArrayList<String>()
    val saws = ArrayList<String>()
    private var nextId = 0

    override suspend fun signIn(code: String): Result<Unit> = Result.success(Unit)
    override suspend fun signOut() {}
    override suspend fun disconnect() {}
    override suspend fun refresh() {}
    override suspend fun addTodayTab(space: String, url: String, title: String): String? {
        // A stand-in id the lane is expected to adopt as the tab's new id.
        val id = "today:fake${nextId++}"
        added.add(space to url)
        nodesFlow.value = nodesFlow.value + SyncedNode(
            id = id, kind = SyncedNode.Kind.TODAY, space = space, url = url, title = title)
        return id
    }
    override suspend fun removeNode(id: String) { removed.add(id) }
    override suspend fun sawToday(id: String) { saws.add(id) }
    override suspend fun requestPairing() {}
    override fun cancelPairing() {}
    override suspend fun recoveryWords(): List<String>? = null
}

/** A stand-in state: it records setSpaces without a file or any Android underneath. */
private class FakeState : TabsHost {
    val spacesFlow = MutableStateFlow(listOf(Space(Space.DEFAULT, "Personal")))
    val tabsFlow = MutableStateFlow<List<Tab>>(emptyList())
    val selectedFlow = MutableStateFlow<String?>(null)
    override val spaces: StateFlow<List<Space>> get() = spacesFlow
    override val tabs: StateFlow<List<Tab>> get() = tabsFlow
    override val selected: StateFlow<String?> get() = selectedFlow
    val setSpacesCalls = ArrayList<List<Space>>()
    override var spacesChanged: ((List<Space>) -> Unit)? = null

    override fun setSpaces(list: List<Space>) {
        spacesFlow.value = if (list.isEmpty()) listOf(Space(Space.DEFAULT, "Personal")) else list
        setSpacesCalls.add(list)
        spacesChanged?.invoke(spacesFlow.value)
    }
    override fun writeTabs(list: List<Tab>) { tabsFlow.value = list }
}

class SyncLaneTest {
    @Test
    fun equalSpacesNeverPushBack() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        val spaces = listOf(SyncedSpace("s1", "Work"))
        account.spacesFlow.value = spaces
        assertEquals(1, state.setSpacesCalls.size)
        // The same list again, as a rebuild would send it, must not push a second time.
        account.spacesFlow.value = spaces
        assertEquals(1, state.setSpacesCalls.size)
        scope.cancel()
    }

    @Test
    fun joinRehomesPhoneTabsIntoFirstAccountSpace() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "local1", url = "https://one.example/", title = "One", space = Space.DEFAULT),
            Tab(id = "local2", url = "https://two.example/", title = "Two", space = Space.DEFAULT),
            Tab(id = "priv", url = "https://three.example/", space = Space.DEFAULT, isPrivate = true),
        ))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        // The account's word is down, so join doesn't wait for it before pushing.
        account.nodesFlow.value = listOf(
            SyncedNode("today:other", SyncedNode.Kind.TODAY, "acct", url = "https://other.example/"))
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        assertEquals("acct", state.setSpacesCalls.single().single().id)
        // The private tab is never pushed; the two ordinary ones are, in the account's Space.
        assertEquals(
            listOf("acct" to "https://one.example/", "acct" to "https://two.example/"), account.added)
        val ids = state.tabs.value.map { it.id }
        assertTrue(ids.containsAll(listOf("today:fake0", "today:fake1")))
        assertTrue(state.tabs.value.all { it.space == "acct" })
        scope.cancel()
    }

    @Test
    fun closeRoutesOnlyMappedTabsAndNeverPrivate() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "today:kept", url = "https://kept.example/", space = "acct"),
            Tab(id = "priv", url = "https://gone.example/", space = "acct", isPrivate = true),
            // No address yet, so the lane never pushes this tab.
            Tab(id = "local", url = null, space = "acct"),
        ))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        // The account has the mapped tab; the lane adopts it, and the other two stay local.
        account.nodesFlow.value = listOf(
            SyncedNode("today:kept", SyncedNode.Kind.TODAY, "acct", url = "https://kept.example/"),
        )
        // The user closes the private tab and the local one together with the mapped one.
        state.writeTabs(listOf(
            Tab(id = "priv", url = "https://gone.example/", space = "acct", isPrivate = true),
            Tab(id = "local", url = null, space = "acct"),
        ))
        // Only the account's own tab reached removeNode; the private and the local one never do.
        assertEquals(listOf("today:kept"), account.removed)
        assertTrue(account.added.isEmpty())
        scope.cancel()
    }

    @Test
    fun pushHappensOnceWhenUrlArrivesAfterCreation() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        // The new tab is made empty, as the + tile does, then Go sets its address.
        state.writeTabs(listOf(Tab(id = "fresh", space = "acct")))
        state.writeTabs(listOf(Tab(id = "fresh", url = "https://once.example/", space = "acct")))
        // The lane renames the tab to the node id, which is another emission the lane must not
        // answer with a second push.
        assertTrue(state.tabs.value.none { it.id == "fresh" })
        assertEquals(listOf("acct" to "https://once.example/"), account.added)
        scope.cancel()
    }

    @Test
    fun selectingAMappedTabNotesItsShowing() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "today:shown", url = "https://shown.example/", space = "acct"),
            Tab(id = "local", url = "https://mine.example/", space = "acct"),
            Tab(id = "priv", url = "https://priv.example/", space = "acct", isPrivate = true),
        ))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        account.nodesFlow.value = listOf(
            SyncedNode("today:shown", SyncedNode.Kind.TODAY, "acct", url = "https://shown.example/"))
        state.selectedFlow.value = "local"
        state.selectedFlow.value = "priv"
        assertTrue(account.saws.isEmpty())
        // Selecting the account's tab tells the account it was shown; the local and the private
        // one are never the account's.
        state.selectedFlow.value = "today:shown"
        assertEquals(listOf("today:shown"), account.saws)
        scope.cancel()
    }

    @Test
    fun restoredTabIsNotPushedBeforeNodesArrive() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "today:kept", url = "https://kept.example/", space = "acct")))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        // The tabs pass ran before any node came down, so the push waits for the account's word.
        assertTrue(account.added.isEmpty())
        account.nodesFlow.value = listOf(
            SyncedNode("today:kept", SyncedNode.Kind.TODAY, "acct", url = "https://kept.example/"))
        assertTrue(account.added.isEmpty())
        scope.cancel()
    }

    @Test
    fun reSignInToSameAccountAdoptsOldNodesWithoutDuplicates() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "local1", url = "https://one.example/", space = Space.DEFAULT)))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        account.nodesFlow.value = listOf(
            SyncedNode("today:fake0", SyncedNode.Kind.TODAY, "acct", url = "https://one.example/"))
        assertEquals(1, account.added.size)
        // The account says SignedOut before it clears its Spaces and nodes, as production does.
        account.stateFlow.value = AccountState.SignedOut
        account.spacesFlow.value = emptyList()
        account.nodesFlow.value = emptyList()
        assertTrue(state.tabs.value.all { it.space == Space.DEFAULT })
        // A sign-in again to the same account, its Spaces arriving before its nodes.
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        // The rehomed tab still sits at its node id, so join waits for the account's word.
        assertEquals(1, account.added.size)
        account.nodesFlow.value = listOf(
            SyncedNode("today:fake0", SyncedNode.Kind.TODAY, "acct", url = "https://one.example/"))
        // The old node is adopted, not pushed again: one push ever, one tab, back in the Space.
        assertEquals(1, account.added.size)
        assertEquals(listOf("today:fake0"), state.tabs.value.map { it.id })
        assertTrue(state.tabs.value.all { it.space == "acct" })
        scope.cancel()
    }

    @Test
    fun reSignInToDifferentAccountPushesFresh() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "local1", url = "https://one.example/", space = Space.DEFAULT)))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        account.nodesFlow.value = listOf(
            SyncedNode("today:fake0", SyncedNode.Kind.TODAY, "acct", url = "https://one.example/"))
        account.stateFlow.value = AccountState.SignedOut
        account.spacesFlow.value = emptyList()
        account.nodesFlow.value = emptyList()
        // A sign-in to a different account, whose nodes know nothing of the old ids.
        account.stateFlow.value = AccountState.SignedIn
        account.spacesFlow.value = listOf(SyncedSpace("other", "Home"))
        account.nodesFlow.value = listOf(
            SyncedNode("today:theirs", SyncedNode.Kind.TODAY, "other", url = "https://theirs.example/"))
        // The old node id matches nothing in the new account, so the tab is pushed fresh to it;
        // the first account's push from the earlier sign-in stays in the log.
        assertEquals(
            listOf("acct" to "https://one.example/", "other" to "https://one.example/"),
            account.added)
        // Their own node came down too, and the old id is gone.
        assertTrue(state.tabs.value.any { it.id == "today:theirs" })
        assertTrue(state.tabs.value.none { it.id == "local1" })
        scope.cancel()
    }

    @Test
    fun signOutKeepsTabsAndRehomesOrphans() = runTest {
        val account = FakeAccount()
        val state = FakeState()
        state.writeTabs(listOf(
            Tab(id = "today:gone", url = "https://kept.example/", space = "acct"),
            // No address yet, so the join moves it without making it the account's.
            Tab(id = "local", space = Space.DEFAULT),
        ))
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        SyncLane(account, state, scope)
        account.stateFlow.value = AccountState.SignedIn
        account.nodesFlow.value = listOf(
            SyncedNode("today:gone", SyncedNode.Kind.TODAY, "acct", url = "https://kept.example/"))
        account.spacesFlow.value = listOf(SyncedSpace("acct", "Work"))
        // The account says SignedOut before it clears its Spaces, as production does.
        account.stateFlow.value = AccountState.SignedOut
        account.spacesFlow.value = emptyList()
        // Both tabs stay, and the one whose Space went with the account is back in the phone's own.
        assertEquals(listOf("today:gone", "local").sorted(), state.tabs.value.map { it.id }.sorted())
        assertTrue(state.tabs.value.all { it.space == Space.DEFAULT })
        assertTrue(account.removed.isEmpty())
        assertTrue(account.added.isEmpty())
        scope.cancel()
    }
}