// Chromium's SessionSpecifics (components/sync/protocol/session_specifics.proto at the engine's
// pin), hand-read: a session's header, which names its computer, and one entity a tab, its
// current navigation's URL and title. The rest of the message stays unread.

package app.ilo.browser.android.sync

/** One SESSIONS entity as the phone reads it: a session's header, or one of its tabs. */
class SessionEntity private constructor() {
    /** The session's tag, the originating computer's cache GUID. */
    var sessionTag: String = ""
    /** The computer's human-readable name, on the header entity only. */
    var clientName: String? = null
    /** The tab's id, on a tab entity; null marks the header. */
    var tabId: Long? = null
    var url: String? = null
    var title: String? = null

    val isHeader: Boolean get() = tabId == null

    companion object {
        fun parse(specifics: ByteArray): SessionEntity? {
            // The specifics are an EntitySpecifics whose SESSIONS field holds the message.
            val fields = protoFields(specifics)?.last(DataType.SESSIONS)?.bytes
                ?.let { protoFields(it) } ?: return null
            val entity = SessionEntity()
            var navigationIndex = -1L
            val urls = ArrayList<Pair<String, String>>()
            for (field in fields) {
                when (field.number) {
                    1 -> entity.sessionTag = field.string ?: ""
                    2 -> field.bytes?.let { protoFields(it) }?.forEach { header ->
                        // SessionHeader.client_name is field 3.
                        if (header.number == 3) entity.clientName = header.string
                    }
                    3 -> field.bytes?.let { protoFields(it) }?.forEach { tab ->
                        when (tab.number) {
                            1 -> entity.tabId = tab.varint?.toLong()
                            4 -> navigationIndex = tab.varint?.toLong() ?: -1L
                            // A TabNavigation's virtual_url (2) and title (4), in its order.
                            7 -> tab.bytes?.let { protoFields(it) }?.let { nav ->
                                var url = ""
                                var title = ""
                                for (entry in nav) {
                                    when (entry.number) {
                                        2 -> url = entry.string ?: ""
                                        4 -> title = entry.string ?: ""
                                    }
                                }
                                urls.add(url to title)
                            }
                        }
                    }
                }
            }
            // The current navigation is the indexed one, or the last when the index is absent.
            if (urls.isNotEmpty()) {
                val current = urls.getOrNull(navigationIndex.toInt()) ?: urls.last()
                entity.url = current.first.ifEmpty { null }
                entity.title = current.second
            }
            return entity
        }
    }
}