// SessionEntity's hand-rolled read of Chromium's SessionSpecifics, against payloads the
// test builds itself, since no generated proto exists on the phone.

package app.ilo.browser.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionEntityTest {

    /** An EntitySpecifics whose SESSIONS field holds the message, as an update carries it. */
    private fun sessions(build: ProtoWriter.() -> Unit): ByteArray {
        val writer = ProtoWriter()
        writer.message(DataType.SESSIONS, build)
        return writer.toByteArray()
    }

    private fun header(tag: String, name: String? = null): ByteArray = sessions {
        string(1, tag)
        message(2) {
            if (name != null) string(3, name)
        }
    }

    private fun tab(
        tag: String, id: Long, navigations: List<Pair<String, String>>, current: Int = -1
    ): ByteArray = sessions {
        string(1, tag)
        message(3) {
            varint(1, id.toULong())
            if (current >= 0) int(4, current.toLong())
            for ((url, title) in navigations) {
                message(7) {
                    string(2, url)
                    string(4, title)
                }
            }
        }
    }

    @Test
    fun readsHeaderName() {
        val entity = SessionEntity.parse(header("guid-a", "Alejandro's Mac"))!!
        assertTrue(entity.isHeader)
        assertEquals("guid-a", entity.sessionTag)
        assertEquals("Alejandro's Mac", entity.clientName)
        assertNull(entity.url)
    }

    @Test
    fun readsTabCurrentNavigation() {
        val entity = SessionEntity.parse(
            tab(
                "guid-a", 7,
                listOf(
                    "https://example.com/one" to "One",
                    "https://example.com/two" to "Two"),
                current = 1))!!
        assertEquals("guid-a", entity.sessionTag)
        assertEquals(7L, entity.tabId)
        assertEquals("https://example.com/two", entity.url)
        assertEquals("Two", entity.title)
    }

    @Test
    fun missingIndexTakesLastNavigation() {
        val entity = SessionEntity.parse(
            tab("guid-b", 3, listOf("https://example.com/only" to "Only")))!!
        assertEquals("https://example.com/only", entity.url)
        assertEquals("Only", entity.title)
    }

    @Test
    fun missingOptionalFieldsStayNull() {
        val entity = SessionEntity.parse(
            tab("guid-b", 3, listOf("" to "No URL Yet")))!!
        assertNull(entity.url)
        assertEquals("No URL Yet", entity.title)
        val bare = SessionEntity.parse(sessions { string(1, "guid-c") })!!
        assertTrue(bare.isHeader)
        assertNull(bare.clientName)
    }

    @Test
    fun notSessionsIsRejected() {
        val other = ProtoWriter()
        other.message(DataType.ILO_SPACE) { bytes(1, ByteArray(0)) }
        assertNull(SessionEntity.parse(other.toByteArray()))
        assertNull(SessionEntity.parse(ByteArray(0).let { it }))
    }
}