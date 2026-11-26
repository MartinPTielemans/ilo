# A stand-in Google on loopback, for check/sync-google.sh: the authorization page and the token
# endpoint an ilo-server in DEV_MODE reaches through GOOGLE_TEST_URL, never the real Google.
#   python3 -I check/google-standin.py <client id> <client secret> <mode file> <email> <sub>
# Says "standin is up on <port>" once it listens on 127.0.0.1. Each authorization request is
# checked as Google's page would read the server's: the code flow with PKCE, the address alone,
# the account picker, no refresh token. The mode file says what the person on the page does:
# "hold" leaves the page up, as someone who hasn't chosen yet, and "pass" chooses <email> and
# sends the tab back with a code. The token endpoint gives the code's ID token once, for the
# client with its secret and the verifier the request committed to. Its lines name what happened,
# never a code, a token or a nonce, so the run can look for those in every other log.
import base64
import hashlib
import json
import secrets
import sys
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

client_id, client_secret, mode_file, email, subject = sys.argv[1:6]
issued = {}


def part(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def say(line):
    print("standin: " + line, flush=True)


class Google(BaseHTTPRequestHandler):
    # The server's own lines carry the query, and so the code.
    def log_message(self, *args):
        pass

    def answer(self, status, body, kind="text/plain", where=None):
        body = body.encode()
        self.send_response(status)
        if where:
            self.send_header("Location", where)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urllib.parse.urlsplit(self.path)
        if url.path != "/auth":
            return self.answer(404, "not here")
        query = dict(urllib.parse.parse_qsl(url.query))
        wrong = [
            name
            for name, right in [
                ("response_type", "code"),
                ("client_id", client_id),
                ("scope", "openid email"),
                ("prompt", "select_account"),
                ("code_challenge_method", "S256"),
            ]
            if query.get(name) != right
        ]
        wrong += [name for name in ["redirect_uri", "state", "nonce", "code_challenge"] if not query.get(name)]
        if "access_type" in query:
            wrong.append("access_type (a refresh token)")
        if wrong:
            say("auth refused, wrong " + ", ".join(wrong))
            return self.answer(400, "refused")
        with open(mode_file) as held:
            mode = held.read().strip()
        say("auth " + mode)
        if mode != "pass":
            return self.answer(
                200,
                "<!doctype html><title>Stand-in Google</title><p>Choose an account",
                "text/html",
            )
        code = "4/standin-" + secrets.token_urlsafe(24)
        now = int(time.time())
        issuer = "http://%s:%d" % self.server.server_address[:2]
        issued[code] = {
            "claims": {
                "iss": issuer,
                "aud": client_id,
                "azp": client_id,
                "iat": now,
                "exp": now + 3600,
                "nonce": query["nonce"],
                "sub": subject,
                "email": email,
                "email_verified": True,
                # The address's domain is a Workspace domain, so Google holds its mailbox, and
                # the server links it without a mailed code.
                "hd": email.split("@")[-1],
            },
            "challenge": query["code_challenge"],
            "redirect_uri": query["redirect_uri"],
        }
        back = urllib.parse.urlsplit(query["redirect_uri"])
        sent = urllib.parse.urlencode({"code": code, "state": query["state"]})
        back = back._replace(query=back.query + "&" + sent if back.query else sent)
        self.answer(302, "", where=urllib.parse.urlunsplit(back))

    def do_POST(self):
        if urllib.parse.urlsplit(self.path).path != "/token":
            return self.answer(404, "not here")
        length = int(self.headers.get("Content-Length") or 0)
        form = dict(urllib.parse.parse_qsl(self.rfile.read(length).decode()))
        grant = issued.pop(form.get("code", ""), None)
        verifier = form.get("code_verifier", "").encode()
        why = None
        if grant is None:
            why = "an unknown or used code"
        elif form.get("grant_type") != "authorization_code":
            why = "the grant type"
        elif form.get("client_id") != client_id or form.get("client_secret") != client_secret:
            why = "the client"
        elif form.get("redirect_uri") != grant["redirect_uri"]:
            why = "the redirect"
        elif part(hashlib.sha256(verifier).digest()) != grant["challenge"]:
            why = "the verifier"
        if why:
            say("token refused, " + why)
            return self.answer(400, json.dumps({"error": "invalid_grant"}), "application/json")
        say("token given")
        token = ".".join(
            [
                part(json.dumps({"alg": "RS256", "kid": "standin"}).encode()),
                part(json.dumps(grant["claims"]).encode()),
                part(b"standin-signature"),
            ]
        )
        self.answer(
            200,
            json.dumps(
                {
                    "access_token": "ya29.standin-" + secrets.token_urlsafe(16),
                    "expires_in": 3599,
                    "scope": "openid https://www.googleapis.com/auth/userinfo.email",
                    "token_type": "Bearer",
                    "id_token": token,
                }
            ),
            "application/json",
        )


google = ThreadingHTTPServer(("127.0.0.1", 0), Google)
say("is up on %d" % google.server_address[1])
google.serve_forever()
