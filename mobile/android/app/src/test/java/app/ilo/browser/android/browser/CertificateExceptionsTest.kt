package app.ilo.browser.android.browser

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CertificateExceptionsTest {
    private fun exception(host: String = "example.com", port: Int = 443, fp: ByteArray = byteArrayOf(1)) =
        CertificateException(host, port, fp)

    @Test
    fun nothingIsTrustedUntilAllowed() {
        val exceptions = CertificateExceptions()
        val e = exception()
        assertFalse(exceptions.trusts(e))
        exceptions.trust(e)
        assertTrue(exceptions.trusts(e))
    }

    @Test
    fun aNewCertificateAtTheSameAddressWarnsAgain() {
        val exceptions = CertificateExceptions()
        exceptions.trust(exception(fp = byteArrayOf(1)))
        assertFalse(exceptions.trusts(exception(fp = byteArrayOf(2))))
    }

    @Test
    fun anotherHostIsSomethingElse() {
        val exceptions = CertificateExceptions()
        exceptions.trust(exception(host = "example.com"))
        assertFalse(exceptions.trusts(exception(host = "other.example")))
    }

    @Test
    fun anotherPortIsSomethingElse() {
        val exceptions = CertificateExceptions()
        exceptions.trust(exception(port = 443))
        assertFalse(exceptions.trusts(exception(port = 8443)))
    }

    @Test
    fun hostCaseDoesNotMatter() {
        val exceptions = CertificateExceptions()
        exceptions.trust(exception(host = "EXAMPLE.com"))
        assertTrue(exceptions.trusts(exception(host = "example.COM")))
    }

    @Test
    fun aTakenBackExceptionIsMarkedStill() {
        val exceptions = CertificateExceptions()
        val e = exception()
        exceptions.trust(e)
        exceptions.stop(e)
        assertFalse(exceptions.trusts(e))
        assertTrue(exceptions.marks(e))
        assertTrue(exceptions.knows(e.host, e.port))
    }

    @Test
    fun knowsCostsNothingWhenEmpty() {
        val exceptions = CertificateExceptions()
        assertFalse(exceptions.knows("example.com", 443))
    }

    @Test
    fun aToldFaultIsTheOneServedBackAtThatAddress() {
        val exceptions = CertificateExceptions()
        val e = exception()
        val fault = CertificateFault(e, CertificateFacts("CN=example.com", null, true, listOf(CertificateFacts.Fault.UNKNOWN_SIGNER)))
        assertNull(exceptions.faultAt(e.host, e.port))
        exceptions.tell(fault)
        assertEquals(fault, exceptions.faultAt(e.host, e.port))
        assertEquals(fault, exceptions.faultAt("EXAMPLE.com", e.port))
        assertNull(exceptions.faultAt(e.host, 8443))
    }

    @Test
    fun aNewFaultAtAnAddressReplacesTheOneToldBefore() {
        val exceptions = CertificateExceptions()
        val e = exception()
        val first = CertificateFault(e, CertificateFacts("CN=first", null, true, listOf(CertificateFacts.Fault.OTHER)))
        val second = CertificateFault(e, CertificateFacts("CN=second", null, true, listOf(CertificateFacts.Fault.OTHER)))
        exceptions.tell(first)
        exceptions.tell(second)
        assertEquals(second, exceptions.faultAt(e.host, e.port))
    }

    @Test
    fun clearRemovesTheToldFaultsToo() {
        val exceptions = CertificateExceptions()
        val e = exception()
        exceptions.tell(CertificateFault(e, CertificateFacts("CN=example.com", null, true, listOf(CertificateFacts.Fault.OTHER))))
        exceptions.clear()
        assertNull(exceptions.faultAt(e.host, e.port))
        assertFalse(exceptions.marks(e))
    }

    @Test
    fun fingerprintHexReadsAsLowercaseHex() {
        assertEquals("00ff0f", exception(fp = byteArrayOf(0, 255.toByte(), 15)).fingerprintHex())
    }

    @Test
    fun theSentenceNamesTheFault() {
        assertTrue(fault(CertificateFacts.Fault.WRONG_NAME).contains("another site"))
        assertTrue(fault(CertificateFacts.Fault.RAN_OUT).contains("ran out"))
        assertTrue(fault(CertificateFacts.Fault.NOT_VALID_UNTIL).contains("valid yet"))
        assertTrue(fault(CertificateFacts.Fault.UNKNOWN_SIGNER).contains("It signed its own certificate"))
        assertTrue(
            fault(CertificateFacts.Fault.UNKNOWN_SIGNER, signedItself = false)
                .contains("Nobody the phone trusts")
        )
        assertEquals("The phone doesn't accept its certificate.", fault(CertificateFacts.Fault.OTHER))
    }

    private fun fault(fault: CertificateFacts.Fault, signedItself: Boolean = true) =
        CertificateFacts("CN=example.com", null, signedItself, listOf(fault)).sentence

    @Test
    fun theLinesNameTheCertificate() {
        val facts = CertificateFacts("CN=example.com", null, true, listOf(CertificateFacts.Fault.WRONG_NAME))
        val lines = facts.lines("example.org")
        assertEquals("Made out to", lines[0].label)
        assertEquals("CN=example.com", lines[0].value)
        assertEquals("Signed by", lines[1].label)
        assertEquals("itself", lines[1].value)
        assertEquals("The name doesn't match", lines[2].label)
        assertEquals("example.org", lines[2].value)
    }

    @Test
    fun aSelfSignedCertificateSaysSoOnlyOnce() {
        val facts = CertificateFacts("CN=example.com", null, true, listOf(CertificateFacts.Fault.UNKNOWN_SIGNER))
        val labels = facts.lines("example.org").map { it.label }
        assertEquals(listOf("Made out to", "Signed by"), labels)
    }

    @Test
    fun anUnknownSignerStandsAloneWhenNotSignedByAnyone() {
        val facts = CertificateFacts("CN=example.com", null, false, listOf(CertificateFacts.Fault.UNKNOWN_SIGNER))
        val lines = facts.lines("example.org")
        assertEquals("Signed by nobody the phone trusts", lines[1].label)
        assertNull(lines[1].value)
    }
}