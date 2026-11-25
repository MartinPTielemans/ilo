// The server's HTTP: the sign-in's token endpoints and sync's one command, as core/sync.cc and
// Chromium's sync engine call them. HttpURLConnection on Dispatchers.IO, keeping nothing: no
// cookies, no cache, and no redirect followed, since the phone's tokens are for this server
// alone, never for wherever an answer sends them on to.

package app.ilo.browser.android.sync

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/** What a sign-in or a refresh gives. */
class Tokens(
    val access: String,
    val refresh: String,
    val expires: Long,
    val account: String,
    val device: String,
    val email: String,
)

sealed class ServerFailure(message: String) : Exception(message) {
    /** No answer: the network's own words for why. */
    class Unreachable(val why: String) : ServerFailure(why)
    /** The sign-in's code or the refresh token was refused, as it is once the device is removed
     * or the token was used: /token's 400 with invalid_grant, and nothing else. */
    class Refused : ServerFailure("refused")
    /** The access token was refused. */
    class Unauthorized : ServerFailure("unauthorized")
    class Unexpected : ServerFailure("unexpected")
}

/** A test's hand on the wire: it gets each request and the call that sends it, and gives the
 * answer the phone sees, so it can lose an answer the server gave or answer in its place. */
typealias Wire = suspend (request: Request, send: suspend () -> Answer) -> Answer

class Request(val url: String, val method: String, val headers: Map<String, String>,
    val body: ByteArray?)

class Answer(val body: ByteArray, val status: Int)

class Server(val base: String, var wire: Wire? = null) {
    /** POST /token, for a sign-in's code or a refresh token. */
    suspend fun token(form: List<Pair<String, String>>): Tokens {
        val answer = send(url("/token"), form)
        val json = jsonObject(String(answer.body)) ?: emptyMap()
        // Only this answer is the server saying the code or token isn't one it takes. A 401, a
        // 403 or a 500, its own or a proxy's on the way, is something to try again after.
        if (answer.status == 400 && json.string("error") == "invalid_grant") {
            throw ServerFailure.Refused()
        }
        if (answer.status != 200) throw ServerFailure.Unexpected()
        val access = json.string("access_token") ?: throw ServerFailure.Unexpected()
        val refresh = json.string("refresh_token") ?: throw ServerFailure.Unexpected()
        val lifetime = json.long("expires_in") ?: throw ServerFailure.Unexpected()
        val claims = claims(json.string("id_token") ?: throw ServerFailure.Unexpected())
            ?: throw ServerFailure.Unexpected()
        val account = claims.string("sub") ?: throw ServerFailure.Unexpected()
        val device = claims.string("sid") ?: throw ServerFailure.Unexpected()
        val email = claims.string("email") ?: throw ServerFailure.Unexpected()
        return Tokens(
            access, refresh, System.currentTimeMillis() / 1000 + lifetime, account, device, email)
    }

    /** What an ID token says. Its signature isn't checked: it came from the server itself, over
     * TLS, in answer to this request. */
    private fun claims(token: String): Map<String, Any?>? {
        val parts = token.split(".")
        if (parts.size != 3) return null
        val payload = base64URL(parts[1]) ?: return null
        return jsonObject(String(payload))
    }

    /** Signs the device out at the server, which is told once and not asked again. */
    suspend fun revoke(refresh: String) {
        try {
            send(url("/revoke"), listOf("token" to refresh))
        } catch (_: Exception) {
        }
    }

    /** One of sync's messages, and its answer. */
    suspend fun command(message: ByteArray, access: String, cacheGUID: String): SyncResponse {
        val answer = exchange(message, access, cacheGUID)
        return SyncResponse(answer) ?: throw ServerFailure.Unexpected()
    }

    /** A message's answer as the server sent it. */
    suspend fun exchange(message: ByteArray, access: String, cacheGUID: String): ByteArray {
        val answer = send(
            url("/command/", listOf("client" to "Chromium", "client_id" to cacheGUID)),
            ByteArray(0), "POST", mapOf(
                "Authorization" to "Bearer $access",
                "Content-Type" to "application/octet-stream"),
            message)
        if (answer.status == 401) throw ServerFailure.Unauthorized()
        if (answer.status != 200) throw ServerFailure.Unexpected()
        return answer.body
    }

    /** `path` from the server's root, whatever path its own address has. */
    internal fun url(path: String, query: List<Pair<String, String>> = emptyList()): String {
        val queryText = if (query.isEmpty()) "" else "?" + form(query)
        val root = base.trimEnd('/')
        // The path is from the server's root, whatever path its own address has.
        val cut = root.indexOf('/', root.indexOf("://") + 3)
        return (if (cut == -1) root else root.substring(0, cut)) + path + queryText
    }

    private suspend fun send(path: String, form: List<Pair<String, String>>?): Answer =
        if (form != null) {
            send(path, form(form).encodeToByteArray(), "POST",
                mapOf("Content-Type" to "application/x-www-form-urlencoded"), null)
        } else {
            send(path, ByteArray(0), "GET", emptyMap(), null)
        }

    private suspend fun send(
        url: String, body: ByteArray, method: String,
        headers: Map<String, String>, payload: ByteArray?
    ): Answer {
        val request = Request(url, method, headers, payload ?: body)
        val transport: suspend () -> Answer = { transport(request) }
        return wire?.invoke(request, transport) ?: transport()
    }

    private suspend fun transport(request: Request): Answer = withContext(Dispatchers.IO) {
        try {
            val connection = URL(request.url).openConnection() as HttpURLConnection
            connection.requestMethod = request.method
            connection.connectTimeout = 30_000
            connection.readTimeout = 30_000
            connection.instanceFollowRedirects = false
            connection.useCaches = false
            for ((name, value) in request.headers) connection.setRequestProperty(name, value)
            request.body?.let { body ->
                connection.doOutput = true
                connection.setFixedLengthStreamingMode(body.size)
                connection.outputStream.use { it.write(body) }
            }
            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            val body = stream?.use { it.readBytes() } ?: ByteArray(0)
            connection.disconnect()
            Answer(body, status)
        } catch (error: IOException) {
            throw ServerFailure.Unreachable(error.message ?: "no answer")
        }
    }

    companion object {
        private fun escape(text: String): String =
            URLEncoder.encode(text, "UTF-8").replace("+", "%20")

        fun form(fields: List<Pair<String, String>>): String =
            fields.joinToString("&") { escape(it.first) + "=" + escape(it.second) }
    }
}