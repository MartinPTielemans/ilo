// The account's saved passwords as Chromium's PASSWORDS type writes and reads them
// (password_specifics.proto, password_proto_utils.cc), named and keyed as its computers do,
// byte for byte with the iOS app's PasswordWrite.swift and Passwords.swift.

package app.ilo.browser.android.sync

/** A login a page can be filled with, read from PasswordSpecificsData by field number. */
class PasswordForm(bytes: ByteArray) {
    /** PasswordSpecificsData.Scheme: 0 a page's form, 1 and 2 HTTP authentication, 4 a username
     * saved with no password. */
    var scheme: Int = 0
    var realm: String = ""
    var username: String = ""
    var password: String = ""
    /** A site the user said never to save for, which holds no login. */
    var blocked: Boolean = false
    var federation: String = ""
    var created: Long = 0
    var used: Long = 0

    init {
        val fields = try {
            protoFields(bytes)
        } catch (_: Exception) {
            null
        }
        // What isn't a message at all reads as one that fills nothing.
        if (fields != null) {
            for (field in fields) {
                when (field.number) {
                    1 -> scheme = field.varint?.toInt() ?: 0
                    2 -> realm = field.string ?: ""
                    6 -> username = field.string ?: ""
                    8 -> password = field.string ?: ""
                    11 -> created = field.varint?.toLong() ?: 0
                    12 -> blocked = field.varint == 1uL
                    17 -> federation = field.string ?: ""
                    18 -> used = field.varint?.toLong() ?: 0
                }
            }
        } else {
            scheme = -1
        }
    }

    /** Whether a page's form can be filled with this: not a "never save" entry, a federated
     * sign-in, HTTP authentication or an Android app's. */
    val fills: Boolean
        get() = scheme == 0 && !blocked && password.isNotEmpty() && federation.isEmpty() &&
            (realm.startsWith("https://") || realm.startsWith("http://"))

    companion object {
        /** A page's sign-on realm as Chromium writes one for a form: the scheme, the host and a
         * port that isn't the scheme's own, then "/". Null for what isn't a web page. */
        fun realm(page: String): String? {
            // Never the page's own name for itself, and no credentials in what's kept.
            val url = try {
                java.net.URI(page)
            } catch (_: Exception) {
                return null
            }
            val scheme = url.scheme?.lowercase() ?: return null
            if (scheme != "https" && scheme != "http") return null
            var host = url.host?.lowercase() ?: return null
            if (host.isEmpty()) return null
            if (host.contains(":") && !host.startsWith("[")) host = "[$host]"
            val own = if (scheme == "https") 443 else 80
            val port = url.port.takeIf { it > 0 && it != own }?.let { ":$it" } ?: ""
            return "$scheme://$host$port/"
        }
    }
}

/** The name a login goes under: the client tag hash Chromium gives it, from the five things its
 * computers name it by (password_proto_utils.cc's GetClientTag), keyed with the account's secret
 * as core/sync_guard.cc's SyncPasswordTag does. */
object PasswordTag {
    /** base::EscapePath (base/strings/escape.cc): kPathCharmap's bits, one for each byte that's
     * written as "%XX". */
    private val escaped = intArrayOf(
        -1, 0xd400_002d.toInt(), 0x7800_0000.toInt(), 0xb800_0001.toInt(),
        -1, -1, -1, -1)

    fun escapePath(text: String): String {
        val hex = "0123456789ABCDEF".encodeToByteArray()
        val bytes = text.encodeToByteArray()
        val out = ByteArray(bytes.size * 3)
        var at = 0
        for (byte in bytes) {
            val unsigned = byte.toInt() and 0xff
            if (escaped[unsigned shr 5] and (1 shl (unsigned and 31)) != 0) {
                out[at++] = '%'.code.toByte()
                out[at++] = hex[unsigned shr 4]
                out[at++] = hex[unsigned and 15]
            } else {
                out[at++] = byte
            }
        }
        return String(out.copyOf(at), Charsets.UTF_8)
    }

    /** The hash that names a password to the server, from the five things its computers name it by. */
    fun hash(
        secret: ByteArray, origin: String, usernameElement: String, username: String,
        passwordElement: String, realm: String
    ): String {
        val plain = listOf(origin, usernameElement, username, passwordElement, realm)
            .joinToString("|") { escapePath(it) }
        val key = Crypto.hkdf(secret, ByteArray(0), "ilo sync client tags v1".encodeToByteArray(), 32)
        val tag = Crypto.hmacSha256(key, plain.encodeToByteArray()).base64URL()
        return clientTagHash(type = DataType.PASSWORDS, tag = tag)
    }

    /** The name of a login this phone writes: for the site alone, at its realm, with no form's
     * field names. */
    fun hash(secret: ByteArray, realm: String, username: String): String =
        hash(secret, realm, "", username, "", realm)

    /** Whether a realm is one GURL would write just as it stands, so the name worked out here is
     * the one a computer works out: a plain host, with a port or none. */
    fun takes(realm: String): Boolean {
        for (scheme in listOf("https://", "http://")) {
            if (!realm.startsWith(scheme) || !realm.endsWith("/")) continue
            val authority = realm.substring(scheme.length, realm.length - 1)
            val parts = authority.split(":", limit = 2)
            if (parts.size > 2 || parts.isEmpty()) return false
            val host = parts[0]
            if (host.isEmpty()) return false
            for (byte in host.encodeToByteArray()) {
                val c = byte.toInt()
                if (!((c in 0x61..0x7a) || (c in 0x30..0x39) || c == 0x2e || c == 0x2d)) return false
            }
            if (parts.size == 2) {
                val port = parts[1].toIntOrNull() ?: return false
                if (port <= 0 || port >= 65536 || port.toString() != parts[1]) return false
            }
            return true
        }
        return false
    }
}

/** sync_pb.PasswordSpecificsData as this phone writes it. */
object PasswordData {
    /** Chromium's time for a password: microseconds since 1601. */
    fun time(unixMillis: Long): Long = unixMillis * 1000 + 11_644_473_600_000_000L

    /** A login saved from a page's form, for its site: every field password_proto_utils.cc's
     * SpecificsDataFromPassword writes for one a computer saved and hasn't used since. */
    fun new(realm: String, username: String, password: String, unixMillis: Long): ByteArray {
        val data = ProtoWriter()
        data.int(1, 0)
        data.string(2, realm)
        data.string(3, realm)
        data.string(4, "")
        data.string(5, "")
        data.string(6, username)
        data.string(7, "")
        data.string(8, password)
        val time = time(unixMillis)
        data.int(11, time)
        data.bool(12, false)
        data.int(13, 0)
        data.int(14, 0)
        data.string(15, "")
        data.string(16, "")
        data.string(17, "")
        data.int(18, time)
        data.message(19) { }
        data.int(20, time)
        data.message(22) { }
        data.string(23, "")
        data.string(24, "")
        data.int(25, 0)
        data.bool(26, false)
        data.string(27, "")
        data.int(28, 0)
        data.bool(29, false)
        return data.toByteArray()
    }

    /** A login's data with another password: every other field as it was, but for the password
     * and when it was changed and last used, and the issues found with the old one, which a
     * computer clears too (PasswordStore::UpdateLogin). */
    fun changed(data: ByteArray, password: String, unixMillis: Long): ByteArray? {
        val fields = protoFields(data) ?: return null
        val out = ProtoWriter()
        for (field in fields) {
            if (field.number != 8 && field.number != 18 && field.number != 19 && field.number != 20) {
                out.field(field)
            }
        }
        out.string(8, password)
        val time = time(unixMillis)
        out.int(18, time)
        out.int(20, time)
        return out.toByteArray()
    }

    /** The login's notes, which a computer writes a second time beside the data. */
    fun notes(data: ByteArray): ByteArray =
        protoFields(data)?.lastOrNull { it.number == 22 }?.bytes ?: ByteArray(0)
}

/** A password entity as the account has it: the ciphertext of PasswordSpecifics' `encrypted`,
 * untouched. Nothing of a login is kept in the clear. */
class PasswordRecord(
    val version: Long,
    val encrypted: EncryptedData,
    /** The server's id for the entity, which a commit that changes it gives back. */
    val id: String?,
    val created: Long?,
) {
    override fun equals(other: Any?): Boolean = other is PasswordRecord && other.version == version &&
        other.encrypted == encrypted && other.id == id && other.created == created

    override fun hashCode(): Int = (version.hashCode() * 31 + encrypted.hashCode()) * 31 +
        (id?.hashCode() ?: 0) * 31 + (created?.hashCode() ?: 0)
}

/** A login this phone saved or changed and the account hasn't taken yet: its ciphertext only,
 * as the account will have it, so the saved state holds no more of it than of any other. */
class PasswordWrite(
    /** PasswordSpecifics' `encrypted`: the login's PasswordSpecificsData. */
    val encrypted: EncryptedData,
    /** Its `encrypted_notes_backup`. */
    val notes: EncryptedData,
    /** The version of the account's entity this was made on: 0 for a login it hasn't got. */
    val base: Long,
    /** When the user saved it, in sync's time. */
    val saved: Long,
) {
    override fun equals(other: Any?): Boolean = other is PasswordWrite && other.encrypted == encrypted &&
        other.notes == notes && other.base == base && other.saved == saved

    override fun hashCode(): Int = encrypted.hashCode() * 31 + notes.hashCode()

    companion object {
        fun of(data: ByteArray, keys: Keybag, base: Long, saved: Long): PasswordWrite? {
            val encrypted = keys.encrypt(data) ?: return null
            val notes = keys.encrypt(PasswordData.notes(data)) ?: return null
            return PasswordWrite(encrypted, notes, base, saved)
        }
    }
}

/** A login the UI fills with: its name to the account, and whose it is. */
data class SavedLogin(val id: String, val username: String, val realm: String)

/** What saving a page's sign-in would do. */
enum class LoginSaving { NONE, SAVE, UPDATE }

/** The account's saved passwords as the app reads them: usernames only until one is asked for. */
class Passwords(private val account: SyncedAccount) {

    /** A login as this phone has it now: what it wrote and hasn't sent, over what the account has. */
    private fun login(hash: String): EncryptedData? = account.passwordCiphertext(hash)

    private fun form(hash: String): PasswordForm? =
        login(hash)?.let { account.keysForPasswords()?.decrypt(it) }?.let(::PasswordForm)

    /** The logins by realm, most recently used first and one for each username, and the realms
     * the user said never to save a password for. */
    data class IndexedLogin(val id: String, val username: String, val used: Long)

    data class LoginIndex(
        val logins: Map<String, List<IndexedLogin>>,
        val never: Set<String>,
    )

    fun index(): LoginIndex {
        val logins = HashMap<String, MutableList<IndexedLogin>>()
        val never = HashSet<String>()
        for (hash in account.passwordHashes()) {
            val form = form(hash) ?: continue
            if (form.scheme == 0 && form.blocked) never.add(form.realm)
            if (!form.fills) continue
            logins.getOrPut(form.realm) { ArrayList() }
                .add(IndexedLogin(hash, form.username, maxOf(form.used, form.created)))
        }
        val sorted = HashMap<String, List<IndexedLogin>>()
        for ((realm, each) in logins) {
            // The one for a username that was used last: the username stands for its oldest use.
            val seen = HashSet<String>()
            sorted[realm] = each.sortedWith(
                compareByDescending<IndexedLogin> { it.used }.thenByDescending { it.id })
                .filter { seen.add(it.username) }
        }
        return LoginIndex(sorted, never)
    }

    /** The saved logins for the page at `page`, newest use first. */
    fun logins(page: String): List<SavedLogin> {
        if (!account.passwordsReady()) return emptyList()
        val realm = PasswordForm.realm(page) ?: return emptyList()
        return (index().logins[realm] ?: emptyList()).map { SavedLogin(it.id, it.username, realm) }
    }

    /** The password of `login`, decrypted for this call alone. */
    fun password(login: SavedLogin): String? {
        if (!account.passwordsReady()) return null
        val form = form(login.id) ?: return null
        if (!form.fills || form.realm != login.realm || form.username != login.username) return null
        return form.password
    }

    /** What saving `username`/`password` on the page at `page` would do, as the offer says. */
    fun saving(username: String, password: String, page: String): LoginSaving {
        val realm = account.savableRealm(page, password) ?: return LoginSaving.NONE
        val index = index()
        if (realm in index.never) return LoginSaving.NONE
        val known = index.logins[realm]?.firstOrNull { it.username == username }
        if (known != null) {
            val form = form(known.id) ?: return LoginSaving.NONE
            return if (form.password == password) LoginSaving.NONE else LoginSaving.UPDATE
        }
        return if (account.unusedPasswordName(realm, username) == null) LoginSaving.NONE
            else LoginSaving.SAVE
    }

    /** Saves the login, or changes the one the account has under the same name. Nothing is kept
     * of it in the clear, and it goes with the next commit. */
    fun save(username: String, password: String, page: String): Boolean {
        val realm = account.savableRealm(page, password) ?: return false
        if (realm in index().never) return false
        val known = index().logins[realm]?.firstOrNull { it.username == username }
        val now = System.currentTimeMillis()
        if (known != null) {
            val plain = login(known.id)?.let { account.keysForPasswords()?.decrypt(it) } ?: return false
            if (PasswordForm(plain).password == password) return true
            val changed = PasswordData.changed(plain, password, now) ?: return false
            return account.savePassword(known.id, changed, now)
        }
        val hash = account.unusedPasswordName(realm, username) ?: return false
        return account.savePassword(hash, PasswordData.new(realm, username, password, now), now)
    }
}