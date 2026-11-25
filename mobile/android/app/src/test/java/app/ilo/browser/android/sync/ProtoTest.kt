package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class ProtoTest {

    @Test
    fun varintRoundTrip() {
        val writer = ProtoWriter()
        writer.varint(1, 150uL)
        val bytes = writer.toByteArray()
        assertArrayEquals(byteArrayOf(0x08.toByte(), 0x96.toByte(), 0x01.toByte()), bytes)
        val fields = protoFields(bytes)!!
        assertEquals(1, fields.size)
        val value = fields.last(1)?.value
        assertNotNull(value)
        assertEquals(150uL, (value as WireValue.Varint).value)
    }

    @Test
    fun longVarint() {
        val writer = ProtoWriter()
        // 2^32 as a varint is five bytes, so the field is six in all.
        writer.varint(1, 0x1_0000_0000uL)
        val bytes = writer.toByteArray()
        assertEquals(6, bytes.size)
        val fields = protoFields(bytes)!!
        assertEquals(0x1_0000_0000uL, (fields.last(1)!!.value as WireValue.Varint).value)
    }

    @Test
    fun bytesRoundTrip() {
        val writer = ProtoWriter()
        val payload = byteArrayOf(1, 2, 3, 4, 5)
        writer.bytes(2, payload)
        val fields = protoFields(writer.toByteArray())!!
        assertArrayEquals(payload, (fields.last(2)!!.value as WireValue.Bytes).value)
    }

    @Test
    fun stringRoundTrip() {
        val text = "testing"
        val writer = ProtoWriter()
        writer.string(3, text)
        val fields = protoFields(writer.toByteArray())!!
        assertEquals(text, (fields.last(3)!!.value as WireValue.Bytes).value.decodeToString())
    }

    @Test
    fun messageNesting() {
        val writer = ProtoWriter()
        writer.message(4) {
            varint(1, 1uL)
            string(2, "inner")
        }
        val fields = protoFields(writer.toByteArray())!!
        val inner = protoFields((fields.last(4)!!.value as WireValue.Bytes).value)!!
        assertEquals(1uL, (inner.last(1)!!.value as WireValue.Varint).value)
        assertEquals("inner", (inner.last(2)!!.value as WireValue.Bytes).value.decodeToString())
    }

    @Test
    fun fixed64RoundTrip() {
        val bytes = byteArrayOf(
            ((1 shl 3) or 1).toByte(),
            0xbe.toByte(), 0xba.toByte(), 0xfe.toByte(), 0xca.toByte(),
            0xef.toByte(), 0xbe.toByte(), 0xad.toByte(), 0xde.toByte())
        val fields = protoFields(bytes)!!
        assertEquals(0xdeadbeefcafebabeuL, (fields.last(1)!!.value as WireValue.Fixed64).value)
    }

    @Test
    fun fieldReplay() {
        // A read field writes back to the same bytes, so a pass-through entry survives unchanged.
        val writer = ProtoWriter()
        writer.message(1) {
            string(2, "payload")
            bool(3, true)
        }
        val original = writer.toByteArray()
        val fields = protoFields(original)!!
        val again = ProtoWriter()
        for (field in fields) again.field(field)
        assertArrayEquals(original, again.toByteArray())
    }

    @Test
    fun truncationRejected() {
        val writer = ProtoWriter()
        writer.string(1, "hello")
        val bytes = writer.toByteArray()
        for (cut in 1 until bytes.size) {
            assertNull(protoFields(bytes.copyOf(bytes.size - cut)))
        }
    }

    @Test
    fun emptyInput() {
        assertEquals(emptyList<ProtoField>(), protoFields(ByteArray(0)))
    }

    @Test
    fun negativeIntRoundTrip() {
        val writer = ProtoWriter()
        writer.int(1, -1)
        val fields = protoFields(writer.toByteArray())!!
        // -1 is ten bytes of varint after the tag.
        assertEquals(11, writer.toByteArray().size)
        assertEquals((-1).toULong(), (fields.last(1)!!.value as WireValue.Varint).value)
    }
}