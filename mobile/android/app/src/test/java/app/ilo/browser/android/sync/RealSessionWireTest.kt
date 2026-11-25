// The b14 desktop session's entities as the server sends them, exercising the whole read
// path an update of SESSIONS takes: parse, decrypt with the account's key, decode.

package app.ilo.browser.android.sync

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RealSessionWireTest {
    // The header entity, the computer REV-HM7Q39HVJX, and its first tab, from the local
    // DEV_MODE server's database as its get_updates hands them out.
    private val header =
        "CiI1MDExOV9GUHZBbUExMHNrN0dlY2NONGl3L2JXNVVTWGc9ICYoiNHln6E0MMvS1p+hNDoJZW5jcnlwdGVkkAEAmgEYS3NVNVlu" +
            "UDlncnViSFVhcmY4RHh3dz09qgGkAgqdAgpYUk1wYjEzQkFxdVY4TlhTMEpLb1haK3h4cTdqZ2ovSE9MclgwSWFjbW9EUU5acVF5" +
            "TStlU2dkYWs0UWtZSGZhOVhtOU9HOVJlNDMrZHhrcTBWU3Z3bHc9PRLAATBVMzVZc1JTK1poUUJWQXFtKzVHMjVoTk9CMXZ6TXA3" +
            "M3VJM0RWcXBLU3R2cGZBazhqVXVLRlg1ays0N2lQYWtvUi9XcUNHeklqKzh5MEJ1VHRFVElQdFRZanN1TksxZW0xbWxDeDV1WE9u" +
            "bGlxMWNuVVJweXZLQkNmL290Q24rZHNISFdkcWQ1L0cwZ09Dc2NwU2ZHdUxkcEtEWi9iY0g3bENUZjRoL3diWktpdUtGWkxkK0dW" +
            "U2tGVnhaNWtRNrq8GAC6ARxGUHZBbUExMHNrN0dlY2NONGl3L2JXNVVTWGc9"
    private val tab =
        "CiI1MDExOV9CVEowNkU1azMxNzEwcG96N1h1VENKWU9iS1k9ICcoidHln6E0MMvS1p+hNDoJZW5jcnlwdGVkkAEAmgEYS3NVNVlu" +
            "UDlncnViSFVhcmY4RHh3dz09qgGQAwqJAwpYUk1wYjEzQkFxdVY4TlhTMEpLb1haK3h4cTdqZ2ovSE9MclgwSWFjbW9EUU5acVF5" +
            "TStlU2dkYWs0UWtZSGZhOVhtOU9HOVJlNDMrZHhrcTBWU3Z3bHc9PRKsAkp5WFF2bTJpNk91TG1iSlFocXlEZEZBcHNscUNiSE5p" +
            "YWhJNDgvMlpTRWQ2bnpaY295aUUweUlKRW5SVFVkS3FwRnAvT2g1cGNPSjJFRlpSL1Y2cnQrbWxXK1lDOTBSNEo1a211L2tpWXNN" +
            "TVFTQ052UHYwQ1VyVjUxVEl1aFlMQ1BtbFhicFdRSGxrVVl4OFJRNktFR0F4K3I1QzZML0VsQW1EcmpOWWp5QXBQUm1xZStIWlYy" +
            "NjhnbGlYYmgzVkYrU0lkQ29aU3F4NlBUOS9pcHFJNDJVeENmWHFveWRYdWZPQWxaYzcyY1h3dU1DaGt2dlBRWU93VVRGZHRDcVEr" +
            "OS9NT04wYTBSQk5sRFA0SHo3ajBTZnhWeUtscjhtNkQyeG96MGxYYmFRPbq8GAC6ARxCVEowNkU1azMxNzEwcG96N1h1VENKWU9i" +
            "S1k9"

    // The account's key from its keybag: encryption 16 bytes, mac 16 bytes.
    private val key = NigoriKey(
        java.util.Base64.getDecoder().decode("oV94ku1dUfYmVNHWjML+wg=="),
        java.util.Base64.getDecoder().decode("t4UutZKtnnn10TMa49JAsw=="))

    private fun decode(blob: String): SessionEntity? {
        val update = SyncUpdate.parse(java.util.Base64.getDecoder().decode(blob))
        assertNotNull(update)
        assertTrue(update!!.isSessions)
        val encrypted = update.encrypted
        assertNotNull(encrypted)
        val plain = key.decrypt(encrypted!!.blob)
        assertNotNull(plain)
        return SessionEntity.parse(plain!!)
    }

    @Test
    fun theWholeReadPathDecodesTheHeader() {
        val parsed = decode(header)
        assertNotNull(parsed)
        assertTrue(parsed!!.isHeader)
        assertEquals("REV-HM7Q39HVJX", parsed.clientName)
    }

    @Test
    fun theWholeReadPathDecodesATabWithItsUrl() {
        val parsed = decode(tab)
        assertNotNull(parsed)
        assertEquals("data:text/html,<title>b14 first client</title>", parsed!!.url)
        assertEquals("b14 first client", parsed.title)
    }
}
