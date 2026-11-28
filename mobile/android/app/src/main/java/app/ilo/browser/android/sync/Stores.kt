// Where an account keeps what outlives the app: what must stay secret behind SecretStore, and
// what it has of the account's data behind SyncStateStore.

package app.ilo.browser.android.sync

import java.io.File

/** Holds the account's secret, the sign-in's tokens and the keys made from them, each under a
 * name. A read of a name never written gives null, and writing null removes it. */
interface SecretStore {
    fun read(name: String): ByteArray?
    fun write(name: String, value: ByteArray?)
}

/** Secrets for as long as the process runs: for tests. */
class MemorySecretStore : SecretStore {
    private val lock = Object()
    private var values = HashMap<String, ByteArray>()

    override fun read(name: String): ByteArray? = synchronized(lock) { values[name] }

    override fun write(name: String, value: ByteArray?) {
        synchronized(lock) {
            if (value == null) values.remove(name) else values[name] = value
            Unit
        }
    }
}

/**
 * Secrets in a private directory. Android's EncryptedSharedPreferences
 * (androidx.security:security-crypto) is not a dependency of this app, and the Keystore-backed
 * KeyChain of the iOS side has no equivalent here without adding one, so the files are only as
 * private as the app's own directory. That is a real limitation: a backup or a rooted device
 * reads the tokens and the account's secret. The browser UI lane should surface it before ship.
 */
class FileSecretStore(private val directory: File) : SecretStore {
    override fun read(name: String): ByteArray? {
        val file = File(directory, name)
        return if (file.exists()) file.readBytes() else null
    }

    override fun write(name: String, value: ByteArray?) {
        directory.mkdirs()
        val file = File(directory, name)
        if (value == null) {
            file.delete()
        } else {
            file.writeBytes(value)
        }
    }
}

/** Holds what the phone has of the account's data and the changes it hasn't sent yet, as one
 * value that is replaced whole, and a mark that this install of the app signed in. The mark goes
 * only with the app's own files: `erase` leaves it. */
interface SyncStateStore {
    fun load(): ByteArray?
    fun save(state: ByteArray)
    fun erase()
    val isMarked: Boolean
    fun mark()
}

/** The state as a file in a directory of the app's, which is made when first written. */
class FileStateStore(directory: File) : SyncStateStore {
    private val file = File(directory, "spaces.json")
    private val marker = File(directory, "installed")

    override val isMarked: Boolean get() = marker.exists()

    override fun mark() {
        if (isMarked) return
        marker.parentFile?.mkdirs()
        marker.writeBytes(ByteArray(0))
    }

    override fun load(): ByteArray? = if (file.exists()) file.readBytes() else null

    override fun save(state: ByteArray) {
        file.parentFile?.mkdirs()
        // Atomic, so a crash while writing leaves the last state whole.
        val temp = File(file.parentFile, file.name + ".tmp")
        temp.writeBytes(state)
        if (!temp.renameTo(file)) {
            file.writeBytes(state)
            temp.delete()
        }
    }

    override fun erase() {
        file.delete()
    }
}

/** The state for as long as the process runs: for tests. */
class MemoryStateStore : SyncStateStore {
    private val lock = Object()
    private var state: ByteArray? = null
    private var marked = false

    override val isMarked: Boolean get() = synchronized(lock) { marked }

    override fun mark() = synchronized(lock) { marked = true }

    override fun load(): ByteArray? = synchronized(lock) { state }

    override fun save(state: ByteArray) = synchronized(lock) { this.state = state }

    override fun erase() = synchronized(lock) { state = null }
}