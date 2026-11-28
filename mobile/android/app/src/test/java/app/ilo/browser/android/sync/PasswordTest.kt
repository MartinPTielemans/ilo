package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PasswordWriteTest {

    private val key = NigoriKey(
        ByteArray(16) { (it * 3).toByte() }, ByteArray(16) { (it * 5 + 1).toByte() })
    private val keys = Keybag(mapOf(key.name to key), key.name)

    /** A sync_pb.PasswordSpecificsData with the fields password_proto_utils.cc's
     * SpecificsDataFromPassword sets, as the iOS test helper writes it. */
    private fun passwordData(
        realm: String, origin: String? = null, username: String, password: String,
        scheme: Long = 0, blocked: Boolean = false, federation: String = "",
        element: String = "username", used: Long = 0
    ): ByteArray {
        val data = ProtoWriter()
        data.int(1, scheme)
        data.string(2, realm)
        data.string(3, origin ?: realm + "login")
        data.string(4, realm + "session")
        data.string(5, element)
        data.string(6, username)
        data.string(7, "password")
        data.string(8, password)
        data.int(11, 13_400_000_000_000_000)
        data.bool(12, blocked)
        data.int(13, 0)
        data.int(14, 3)
        data.string(15, "")
        data.string(16, "")
        data.string(17, federation)
        data.int(18, if (used == 0L) 0 else 13_400_000_000_000_000 + used)
        data.message(19) { }
        data.int(20, 13_400_000_000_000_000)
        data.message(22) { }
        data.string(23, "")
        data.string(24, "")
        data.int(25, 0)
        data.bool(26, false)
        data.string(27, "")
        return data.toByteArray()
    }

    private fun hex(text: String): ByteArray {
        val out = ByteArray(text.length / 2)
        for (i in out.indices) {
            out[i] = text.substring(i * 2, i * 2 + 2).toInt(16).toByte()
        }
        return out
    }

    @Test
    fun escapesAsChromiumsEscapePath() {
        assertEquals("https%3A//example.org/", PasswordTag.escapePath("https://example.org/"))
        assertEquals("http%3A//example.org%3A8080/a%20b", PasswordTag.escapePath("http://example.org:8080/a b"))
        assertEquals(
            "%20%22%23%25%3A%3C%3E%3F%5B%5C%5D%5E%60%7B%7C%7D",
            PasswordTag.escapePath(" \"#%:<>?[\\]^`{|}"))
        assertEquals("%01%1F%7F", PasswordTag.escapePath("\u0001\u001F\u007F"))
        assertEquals("zo%C3%AB", PasswordTag.escapePath("zoë"))
        val plain = "!$&'()*+,-./0129;=@AZ_az~"
        assertEquals(plain, PasswordTag.escapePath(plain))
        assertEquals("", PasswordTag.escapePath(""))
    }

    @Test
    fun namesALoginAsADesktopDid() {
        // The interop run of 2026-11-18: what the desktop's own Chromium made of them.
        val secret = hex("87354b92d368a44c39e400798c31a7fed9856eee6829d48b3f42b99467ffc623")
        assertEquals(
            "G/wMVXw1sne7tBOwjaH6VTDfKII=",
            PasswordTag.hash(
                secret, origin = "http://127.0.0.1:49154/p/login.html", usernameElement = "username",
                username = "interop-user@example.test", passwordElement = "password",
                realm = "http://127.0.0.1:49154/"))
        assertFalse(
            "G/wMVXw1sne7tBOwjaH6VTDfKII=" == PasswordTag.hash(
                ByteArray(32) { 7 }, origin = "http://127.0.0.1:49154/p/login.html",
                usernameElement = "username", username = "interop-user@example.test",
                passwordElement = "password", realm = "http://127.0.0.1:49154/"))
        assertEquals(
            PasswordTag.hash(secret, "https://example.org/", "pat"),
            PasswordTag.hash(
                secret, origin = "https://example.org/", usernameElement = "", username = "pat",
                passwordElement = "", realm = "https://example.org/"))
    }

    @Test
    fun savesOnlyForARealmGURLWritesTheSame() {
        for (realm in listOf(
            "https://example.org/", "http://example.org/", "https://shop.example.org:8443/",
            "http://localhost:8080/", "http://127.0.0.1:49154/", "https://xn--bcher-kva.example/"
        )) {
            assertTrue(realm, PasswordTag.takes(realm))
        }
        for (realm in listOf(
            "https://[::1]/", "https://[::1]:8443/", "https://EXAMPLE.org/", "https://example.org",
            "https:///", "https://example.org:/", "https://example.org:0443/",
            "https://example.org:99999/", "https://a:b:c/", "https://ex ample.org/",
            "https://exa%6dple.org/", "https://bücher.example/", "ftp://example.org/",
            "android://hash@org.example.app/", "https://example.org/Staff", ""
        )) {
            assertFalse(realm, PasswordTag.takes(realm))
        }
    }

    @Test
    fun aNewLoginIsWrittenAsADesktopWritesOne() {
        val now = 1_800_000_000_000L
        val data = PasswordData.new("https://example.org/", "pat", "made-up-pass", now)
        val fields = protoFields(data)!!
        assertEquals(
            (1..8).toList() + (11..20).toList() + (22..29).toList(), fields.map { it.number })
        // Microseconds since 1601.
        assertEquals(11_644_473_600_000_000L, PasswordData.time(0))
        val time = PasswordData.time(now)
        assertEquals(time.toULong(), fields.lastOrNull { it.number == 11 }?.varint)
        assertEquals(time.toULong(), fields.lastOrNull { it.number == 18 }?.varint)
        assertEquals(time.toULong(), fields.lastOrNull { it.number == 20 }?.varint)
        val form = PasswordForm(data)
        assertTrue(form.fills)
        assertEquals("https://example.org/", form.realm)
        assertEquals("pat", form.username)
        assertEquals("made-up-pass", form.password)
        assertArrayEquals(ByteArray(0), PasswordData.notes(data))
    }

    @Test
    fun changingAPasswordKeepsTheRest() {
        var known = ProtoWriter()
        val written = passwordData(
            realm = "https://example.org/", origin = "https://example.org/account/login",
            username = "pat", password = "old-made-up", element = "email", used = 5)
        for (field in protoFields(written)!!) known.field(field)
        // Fields of a Chromium to come, of each kind the wire has.
        known.int(90, -4)
        known.field(ProtoField(91, WireValue.Fixed64(0x0102_0304_0506_0708u)))
        known.field(ProtoField(92, WireValue.Fixed32(0x0a0b_0c0du)))
        known.field(ProtoField(93, WireValue.Group(byteArrayOf(8, 1))))
        known.string(94, "kept")
        val before = protoFields(known.toByteArray())!!
        var data = ProtoWriter()
        for (field in before) {
            if (field.number != 19 && field.number != 22) data.field(field)
        }
        data.message(19) { message(1) { int(1, 5) } }
        data.message(22) { message(1) { string(2, "a made-up note") } }
        val now = 1_800_000_000_000L
        val changed = PasswordData.changed(data.toByteArray(), "new-made-up", now)!!
        val after = protoFields(changed)!!
        assertEquals("new-made-up", after.lastOrNull { it.number == 8 }?.string)
        assertEquals(PasswordData.time(now).toULong(), after.lastOrNull { it.number == 18 }?.varint)
        assertEquals(PasswordData.time(now).toULong(), after.lastOrNull { it.number == 20 }?.varint)
        assertNull(after.lastOrNull { it.number == 19 })
        val rest = { fields: List<ProtoField> ->
            fields.filter { it.number != 8 && it.number != 18 && it.number != 19 && it.number != 20 }
        }
        assertEquals(rest(after), rest(protoFields(data.toByteArray())!!))
        assertEquals(1, after.count { it.number == 8 })
        assertTrue(PasswordData.notes(changed).isNotEmpty())
        assertArrayEquals(PasswordData.notes(changed), PasswordData.notes(data.toByteArray()))
        assertNull(PasswordData.changed(byteArrayOf(-1, -1, -1), "x", now))
    }

    @Test
    fun aPasswordsCommitEntry() {
        // CommitContributionImpl::PopulateCommitProto, after EncryptPasswordSpecificsData.
        val data = PasswordData.new("https://example.org/", "pat", "made-up-pass", 1_700_000_000_000)
        val write = PasswordWrite.of(data, keys, base = 0, saved = 5)!!
        val entry = CommitEntry(
            id = "an-id", hash = "a-hash", version = 0, created = 5, modified = 6,
            encrypted = null, password = write)
        val writer = ProtoWriter()
        entry.write(writer)
        val fields = protoFields(writer.toByteArray())!!
        assertEquals(listOf(1, 4, 5, 6, 7, 18, 22, 21, 23), fields.map { it.number })
        assertEquals("encrypted", fields.lastOrNull { it.number == 7 }?.string)
        assertEquals(0uL, fields.lastOrNull { it.number == 18 }?.varint)
        // The type's own field carries nothing: the password rides in its own inside.
        assertEquals(0uL, fields.lastOrNull { it.number == 22 }?.varint)
        val specifics = protoFields(fields.lastOrNull { it.number == 21 }?.bytes!!)!!
        assertEquals(listOf(DataType.PASSWORDS), specifics.map { it.number })
        val own = protoFields(specifics[0].bytes!!)!!
        assertEquals(listOf(1, 5), own.map { it.number })
        assertArrayEquals(data, keys.decrypt(EncryptedData.parse(own[0].bytes!!)!!))
        assertArrayEquals(ByteArray(0), keys.decrypt(EncryptedData.parse(own[1].bytes!!)!!))
        // The reader takes it for a password.
        val update = SyncUpdate.parse(writer.toByteArray())!!
        assertTrue(update.isPassword)
        assertFalse(update.isSpace)
        assertNull(update.encrypted)
    }
}

class PasswordCodecTest {

    private val key = NigoriKey(
        ByteArray(16) { (it * 3).toByte() }, ByteArray(16) { (it * 5 + 1).toByte() })
    private val keys = Keybag(mapOf(key.name to key), key.name)

    @Test
    fun readsAPasswordEntityAsChromiumWritesOne() {
        // An entity as GetUpdates gives one: the ciphertext in PasswordSpecifics.encrypted
        // (EntitySpecifics' field 45873, then 1).
        val data = ProtoWriter()
        data.int(1, 0)
        data.string(2, "https://example.org/")
        data.string(6, "pat@example.org")
        data.string(8, "correct horse")
        data.int(11, 13_400_000_000_000_000)
        data.int(18, 13_400_000_000_000_007)
        val entity = ProtoWriter()
        entity.string(1, "server-id")
        entity.int(4, 12)
        entity.string(7, "encrypted")
        entity.message(21) {
            message(DataType.PASSWORDS) {
                message(1) {
                    EncryptedData(key.name, key.encrypt(data.toByteArray())).write(this)
                }
                message(5) {
                    EncryptedData(key.name, key.encrypt(ByteArray(0))).write(this)
                }
            }
        }
        entity.string(23, "hash")
        val update = SyncUpdate.parse(entity.toByteArray())!!
        assertTrue(update.isPassword)
        assertFalse(update.isSpace)
        assertNull(update.encrypted)
        val encrypted = update.password!!
        assertEquals(key.name, encrypted.keyName)
        val form = PasswordForm(keys.decrypt(EncryptedData(encrypted.keyName, encrypted.blob))!!)
        assertEquals("https://example.org/", form.realm)
        assertEquals("pat@example.org", form.username)
        assertEquals("correct horse", form.password)
        assertEquals(0, form.scheme)
        assertEquals(13_400_000_000_000_007L, form.used)
        assertFalse(form.blocked)
        assertTrue(form.fills)
        // Another key reads nothing of it.
        val other = Keybag(mapOf("other" to NigoriKey(ByteArray(16) { 9 }, ByteArray(16) { 8 })), "other")
        assertNull(other.decrypt(EncryptedData(encrypted.keyName, encrypted.blob)))
    }

    @Test
    fun onlyAFormsLoginFills() {
        fun fills(data: ByteArray) = PasswordForm(data).fills
        val realm = "https://example.org/"
        fun data(
            username: String, password: String, scheme: Long = 0, blocked: Boolean = false,
            federation: String = "", realm: String = "https://example.org/"
        ): ByteArray {
            val writer = ProtoWriter()
            writer.int(1, scheme)
            writer.string(2, realm)
            writer.string(6, username)
            writer.string(8, password)
            writer.bool(12, blocked)
            writer.string(17, federation)
            return writer.toByteArray()
        }
        assertTrue(fills(data("", "p")))
        assertTrue(fills(data("u", "p", realm = "http://example.org/")))
        assertFalse(fills(data("", "", blocked = true)))
        assertFalse(fills(data("u", "p", scheme = 1, realm = realm + "Staff")))
        assertFalse(fills(data("u", "p", scheme = 2)))
        assertFalse(fills(data("u", "")))
        assertFalse(
            fills(data("u", "", federation = "https://accounts.example.com",
                realm = "federation://example.org/accounts.example.com")))
        assertFalse(fills(data("u", "p", federation = "https://accounts.example.com")))
        assertFalse(fills(data("u", "p", realm = "android://hash@org.example.app/")))
    }

    @Test
    fun aPagesRealm() {
        fun realm(page: String) = PasswordForm.realm(page)
        assertEquals("https://example.org/", realm("https://example.org/account/login?next=%2F#form"))
        assertEquals("https://example.org/", realm("https://EXAMPLE.org"))
        // A port only when it isn't the scheme's own, as GURL writes an origin.
        assertEquals("https://example.org/", realm("https://example.org:443/"))
        assertEquals("https://example.org:8443/", realm("https://example.org:8443/"))
        assertEquals("http://example.org/", realm("http://example.org:80/"))
        assertEquals("http://localhost:8080/", realm("http://localhost:8080/login"))
        assertEquals("https://example.org/", realm("https://user:secret@example.org/"))
        assertEquals("https://login.example.org/", realm("https://login.example.org/"))
        for (other in listOf(
            "about:blank", "file:///etc/hosts", "data:text/html,x", "ilo://settings",
            "ftp://example.org/"
        )) {
            assertNull(other, realm(other))
        }
    }
}