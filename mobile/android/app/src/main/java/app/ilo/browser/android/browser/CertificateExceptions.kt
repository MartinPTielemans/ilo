package app.ilo.browser.android.browser

import java.security.MessageDigest
import java.security.cert.X509Certificate

/** One certificate at one address: all that Allow trusts. Another host, another port's other
 * certificate and a new certificate at the same address are each something else. */
class CertificateException(
    host: String,
    val port: Int,
    /** The SHA-256 of the site's own certificate, as it sent it. */
    val fingerprint: ByteArray,
) {
    /** Host names match without case, so it's held lowercase, as the hashed sets below expect. */
    val host: String = host.lowercase()

    override fun equals(other: Any?): Boolean =
        other is CertificateException && host == other.host && port == other.port &&
            fingerprint.contentEquals(other.fingerprint)

    override fun hashCode(): Int = host.hashCode() * 31 + port * 31 + fingerprint.contentHashCode()

    fun fingerprintHex(): String = fingerprint.joinToString("") { "%02x".format(it) }

    companion object {
        fun of(host: String, port: Int, leaf: X509Certificate): CertificateException =
            CertificateException(host, port, fingerprint(leaf.encoded))

        fun fingerprint(der: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(der)
    }
}

/** A fault the phone finds in a certificate, and how the warning says it. */
data class CertificateFacts(
    val madeOutTo: String,
    val signedBy: String?,
    val signedItself: Boolean,
    val faults: List<Fault>,
) {
    enum class Fault { RAN_OUT, NOT_VALID_UNTIL, WRONG_NAME, UNKNOWN_SIGNER, OTHER }

    /** What's wrong, in a sentence: the fault that says most, where there are several. Mirrors
     * the iOS app's CertificateFacts.sentence. */
    val sentence: String
        get() {
            if (Fault.WRONG_NAME in faults) return "Its certificate is for another site."
            if (Fault.RAN_OUT in faults) return "Its certificate ran out."
            if (Fault.NOT_VALID_UNTIL in faults) return "Its certificate isn't valid yet."
            if (Fault.UNKNOWN_SIGNER in faults) {
                return if (signedItself) "It signed its own certificate, which anyone can do."
                else "Nobody the phone trusts signed its certificate."
            }
            return "The phone doesn't accept its certificate."
        }

    /** One line of the certificate's facts, in the warning's detail: a label and its value, or
     * a sentence that stands alone. Mirrors the iOS app's CertificateFacts.lines. */
    data class Line(val label: String, val value: String?)

    /** The certificate's facts as lines, for the host whose name the wrong-name fault means. */
    fun lines(host: String): List<Line> {
        val lines = mutableListOf(Line("Made out to", madeOutTo))
        if (signedItself) {
            lines.add(Line("Signed by", "itself"))
        } else if (signedBy != null) {
            lines.add(Line("Signed by", signedBy))
        }
        for (fault in faults) when (fault) {
            Fault.WRONG_NAME -> lines.add(Line("The name doesn't match", host))
            Fault.RAN_OUT -> lines.add(Line("Ran out", null))
            Fault.NOT_VALID_UNTIL -> lines.add(Line("Not valid yet", null))
            // One that signed itself has said so on the line above.
            Fault.UNKNOWN_SIGNER -> if (!signedItself) {
                lines.add(
                    Line(
                        if (signedBy == null) "Signed by nobody the phone trusts"
                        else "Nobody the phone trusts signed that",
                        null,
                    )
                )
            }
            Fault.OTHER -> lines.add(Line("The phone doesn't accept it", null))
        }
        return lines
    }
}

/** A certificate the phone doesn't trust, with what going on would trust. */
data class CertificateFault(
    val exception: CertificateException,
    val facts: CertificateFacts,
)

/** The certificates the user went on past, until ilo quits. They're in memory alone: nothing
 * here is written to a file, to a log or to the account, and a private tab has a set of its own.
 * Port of the iOS app's Certificates.swift exception store. */
class CertificateExceptions {
    private val trusted = HashSet<CertificateException>()
    /** Every one trusted since launch, the ones no longer trusted too: a page that still comes
     * over a connection made before is told by it, and gets its warning back. */
    private val marked = HashSet<CertificateException>()
    /** The fault the engine was told of at each address, the last one it stood a warning for:
     * a page that comes in over a connection WebView still holds serves that same fault, for
     * Android has no way to read the connection's certificate as iOS's mark() reads its trust. */
    private val told = HashMap<String, CertificateFault>()

    /** Whether any was made for an address, which costs nothing to ask for a page with none. */
    fun knows(host: String, port: Int): Boolean {
        if (marked.isEmpty()) return false
        val host = host.lowercase()
        return marked.any { it.host == host && it.port == port }
    }

    fun trust(exception: CertificateException) {
        trusted.add(exception)
        marked.add(exception)
    }

    fun stop(exception: CertificateException) { trusted.remove(exception) }

    fun trusts(exception: CertificateException): Boolean = trusted.contains(exception)

    /** Whether a page with this certificate got here past a warning, one since taken back or not. */
    fun marks(exception: CertificateException): Boolean = marked.contains(exception)

    /** Records the fault the engine was told of at an address, the one a held connection serves. */
    fun tell(fault: CertificateFault) {
        told["${fault.exception.host}:${fault.exception.port}"] = fault
    }

    /** The fault the engine was last told of at an address, or null when it never was. */
    fun faultAt(host: String, port: Int): CertificateFault? = told["${host.lowercase()}:$port"]

    fun clear() {
        trusted.clear()
        marked.clear()
        told.clear()
    }
}