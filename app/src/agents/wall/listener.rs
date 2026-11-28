//! The one way out of a walled task's network (#4101): a local listener that the assistant's
//! proxy variables name, which opens a tunnel to the assistant's own service and refuses every
//! other host. The wall lets the task reach this port and ilo's MCP socket, and nothing else, so a
//! command can't send the user's file to a host of its own. One listener per walled task, alive
//! while it runs. It keeps no host a command asked for, only two counts for the task's end.
//! A connection that never sends its head keeps one of the 32 places until the task ends, so a
//! command can use them all up and stop its own assistant's network, but never reach another host.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::agents::runner::Cli;

/// Claude Code's own hosts, from its docs' network table (code.claude.com/docs/en/network-config):
/// the API and its sign-in's refresh. claude.ai is its account sign-in, which never happens in a
/// wall. Updates, telemetry and claude.ai's connectors are turned off at a walled start, and its
/// own WebFetch reaches these hosts and no other: a task's web is ilo's tools.
const CLAUDE: &[&str] = &["api.anthropic.com", "platform.claude.com"];
/// Codex's own hosts, from its source at rust-v0.172.0: the backend for a ChatGPT sign-in, and the
/// sign-in's refresh. Its docs list no proxy variables; its source reads them
/// (codex-rs/websocket-client/src/dialer.rs:54, codex-rs/http-client/README.md:12).
const CODEX: &[&str] = &["chatgpt.com", "auth.openai.com"];

/// The hosts an assistant's tunnel may reach: constants, so no setting, agent, page or variable
/// adds one.
pub fn hosts(cli: Cli) -> &'static [&'static str] {
    match cli {
        Cli::Claude => CLAUDE,
        Cli::Codex => CODEX,
    }
}

/// The longest request head read before a refusal.
const MOST_HEAD: usize = 8 * 1024;
/// The most connections a task has open at once, each a thread or two in ilo: one more is refused,
/// so a command can't grow them without end. An assistant keeps a few.
const MOST_OPEN: usize = 32;

/// Why a request was refused. None of them carries the host it asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not one `CONNECT name:port HTTP/1.x` line ending its head.
    Malformed,
    /// A method other than CONNECT.
    NotConnect,
    /// A port other than 443.
    Port,
    /// An address in place of a name.
    Address,
    /// A name not in the assistant's list.
    Host,
}

/// What a request's head asks for: the listed name it may reach, or why not. The name returned is
/// the list's own, so nothing of the request's bytes is what's dialed.
pub fn asked(head: &[u8], hosts: &[&'static str]) -> Result<&'static str, Refused> {
    let head = std::str::from_utf8(head).map_err(|_| Refused::Malformed)?;
    let line = head.split("\r\n").next().ok_or(Refused::Malformed)?;
    let mut words = line.split(' ');
    let (Some(method), Some(authority), Some(version), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err(Refused::Malformed);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(Refused::Malformed);
    }
    if method != "CONNECT" {
        return Err(Refused::NotConnect);
    }
    if authority.starts_with('[') {
        return Err(Refused::Address);
    }
    let (name, port) = authority.rsplit_once(':').ok_or(Refused::Malformed)?;
    if port != "443" {
        return Err(Refused::Port);
    }
    if name.parse::<IpAddr>().is_ok() {
        return Err(Refused::Address);
    }
    hosts
        .iter()
        .find(|host| host.eq_ignore_ascii_case(name))
        .copied()
        .ok_or(Refused::Host)
}

/// How a connection to a listed host is opened: ilo resolves the name and connects, so nothing
/// inside the wall needs DNS.
type Dial = fn(&'static str) -> io::Result<TcpStream>;

fn dial(host: &'static str) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no address");
    for address in (host, 443).to_socket_addrs()? {
        match TcpStream::connect(address) {
            Ok(stream) => return Ok(stream),
            Err(why) => last = why,
        }
    }
    Err(last)
}

/// How the listener's own thread is woken to stop: a connection to its port.
type Wake = fn(u16) -> io::Result<TcpStream>;

fn wake(port: u16) -> io::Result<TcpStream> {
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(1))
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    passed: AtomicUsize,
    refused: AtomicUsize,
    next: AtomicU64,
    /// The task's open connections, closed with the listener.
    open: Mutex<HashMap<u64, TcpStream>>,
}

/// A walled task's listener, bound to 127.0.0.1 on a port the system picks. Dropping it stops it
/// and closes every tunnel it opened.
pub struct Listener {
    port: u16,
    shared: Arc<Shared>,
    accepting: Option<JoinHandle<()>>,
    wake: Wake,
}

/// What a task's end may say of its network: counts, never hosts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub passed: usize,
    pub refused: usize,
}

impl Listener {
    /// Starts the listener for a task of `cli`'s.
    pub fn start(cli: Cli) -> io::Result<Listener> {
        Listener::start_with(hosts(cli), dial)
    }

    fn start_with(hosts: &'static [&'static str], dial: Dial) -> io::Result<Listener> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared::default());
        let accepting = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("wall listener".into())
                .spawn(move || accept(listener, hosts, dial, shared))?
        };
        Ok(Listener {
            port,
            shared,
            accepting: Some(accepting),
            wake,
        })
    }

    /// The port the task's proxy variables name.
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn counts(&self) -> Counts {
        Counts {
            passed: self.shared.passed.load(Ordering::Relaxed),
            refused: self.shared.refused.load(Ordering::Relaxed),
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        // The accept blocks with no timer, so a connection of its own is what wakes it to stop.
        // Without one the join would never return, so the thread is left to stop at the next
        // connection that comes, which it then closes unserved.
        let woken = (self.wake)(self.port).is_ok();
        if let Some(accepting) = self.accepting.take()
            && woken
        {
            _ = accepting.join();
        }
        let open = std::mem::take(&mut *self.shared.open.lock().unwrap());
        for stream in open.values() {
            _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn accept(listener: TcpListener, hosts: &'static [&'static str], dial: Dial, shared: Arc<Shared>) {
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(mut stream) = stream else { continue };
        let Ok(kept) = stream.try_clone() else {
            continue;
        };
        let id = shared.next.fetch_add(1, Ordering::Relaxed);
        {
            // Counted here, before its thread, so the cap holds however fast they come.
            let mut open = shared.open.lock().unwrap();
            if open.len() >= MOST_OPEN {
                drop(open);
                // Its request is left unread, so the client may see a reset in place of the 503:
                // either is the refusal. Reading it would need the thread or the timer the cap is
                // there to spare.
                refuse(&mut stream, &shared, "503 Service Unavailable");
                continue;
            }
            open.insert(id, kept);
        }
        let started = std::thread::Builder::new()
            .name("wall tunnel".into())
            .spawn({
                let shared = shared.clone();
                move || serve(stream, id, hosts, dial, &shared)
            });
        if started.is_err() {
            shared.open.lock().unwrap().remove(&id);
        }
    }
}

/// Reads up to the head's blank line: the head, and any bytes the client sent past it.
fn head(stream: &mut TcpStream) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut read = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let got = stream.read(&mut buffer).ok().filter(|&got| got > 0)?;
        read.extend_from_slice(&buffer[..got]);
        if let Some(at) = read.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = read.split_off(at + 4);
            return Some((read, rest));
        }
        if read.len() > MOST_HEAD {
            return None;
        }
    }
}

fn serve(
    mut client: TcpStream,
    id: u64,
    hosts: &'static [&'static str],
    dial: Dial,
    shared: &Shared,
) {
    tunnel(&mut client, hosts, dial, shared);
    shared.open.lock().unwrap().remove(&id);
}

fn refuse(client: &mut TcpStream, shared: &Shared, status: &str) {
    shared.refused.fetch_add(1, Ordering::Relaxed);
    _ = write!(
        client,
        "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

fn tunnel(client: &mut TcpStream, hosts: &'static [&'static str], dial: Dial, shared: &Shared) {
    let Some((head, rest)) = head(client) else {
        return refuse(client, shared, "403 Forbidden");
    };
    let Ok(host) = asked(&head, hosts) else {
        return refuse(client, shared, "403 Forbidden");
    };
    let Ok(mut upstream) = dial(host) else {
        _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    shared.passed.fetch_add(1, Ordering::Relaxed);
    if client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .is_err()
        || upstream.write_all(&rest).is_err()
    {
        return;
    }
    // Past the 200 the bytes are the client's and the host's alone: nothing more is read as a
    // request, so a second CONNECT goes to the listed host as any other bytes.
    let (Ok(mut client_in), Ok(mut upstream_out)) = (client.try_clone(), upstream.try_clone())
    else {
        return;
    };
    let up = std::thread::Builder::new()
        .name("wall tunnel up".into())
        .spawn(move || {
            _ = io::copy(&mut client_in, &mut upstream_out);
            _ = upstream_out.shutdown(Shutdown::Write);
        });
    _ = io::copy(&mut upstream, client);
    _ = client.shutdown(Shutdown::Write);
    if let Ok(up) = up {
        _ = up.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use std::sync::mpsc;

    const LISTED: &[&str] = &["api.example.org"];

    #[test]
    fn only_a_connect_to_a_listed_name_on_443_passes() {
        let ask = |head: &str| asked(head.as_bytes(), LISTED);
        let line = |line: &str| format!("{line}\r\nHost: x\r\n\r\n");
        assert_eq!(
            ask(&line("CONNECT api.example.org:443 HTTP/1.1")),
            Ok("api.example.org")
        );
        assert_eq!(
            ask(&line("CONNECT API.Example.ORG:443 HTTP/1.1")),
            Ok("api.example.org")
        );
        let refused = [
            ("CONNECT other.example.org:443 HTTP/1.1", Refused::Host),
            ("CONNECT api.example.org.:443 HTTP/1.1", Refused::Host),
            (
                "CONNECT api.example.org.evil.example:443 HTTP/1.1",
                Refused::Host,
            ),
            ("CONNECT api.example.org:80 HTTP/1.1", Refused::Port),
            ("CONNECT api.example.org:0443 HTTP/1.1", Refused::Port),
            ("CONNECT 93.184.216.34:443 HTTP/1.1", Refused::Address),
            ("CONNECT [2606:2800::1]:443 HTTP/1.1", Refused::Address),
            ("GET https://api.example.org/ HTTP/1.1", Refused::NotConnect),
            ("POST api.example.org:443 HTTP/1.1", Refused::NotConnect),
            ("CONNECT api.example.org HTTP/1.1", Refused::Malformed),
            ("CONNECT api.example.org:443", Refused::Malformed),
            ("CONNECT  api.example.org:443 HTTP/1.1", Refused::Malformed),
            ("CONNECT api.example.org:443 SPDY/3", Refused::Malformed),
        ];
        for (request, why) in refused {
            assert_eq!(ask(&line(request)), Err(why), "{request}");
        }
    }

    #[test]
    fn each_assistant_has_its_own_hosts_on_no_other_port() {
        for cli in Cli::ALL {
            for host in hosts(cli) {
                let head = format!("CONNECT {host}:443 HTTP/1.1\r\n\r\n");
                assert_eq!(asked(head.as_bytes(), hosts(cli)), Ok(*host));
            }
            let other = Cli::ALL.into_iter().find(|&other| other != cli).unwrap();
            assert!(hosts(cli).iter().all(|host| !hosts(other).contains(host)));
        }
    }

    /// The test's stand-in for a listed host: a local server that echoes what it's sent.
    fn echo() -> &'static str {
        static AT: OnceLock<String> = OnceLock::new();
        AT.get_or_init(|| {
            let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let at = server.local_addr().unwrap().to_string();
            std::thread::spawn(move || {
                for stream in server.incoming().flatten() {
                    std::thread::spawn(move || {
                        let mut back = stream.try_clone().unwrap();
                        let mut stream = stream;
                        _ = io::copy(&mut stream, &mut back);
                    });
                }
            });
            at
        })
    }

    fn to_echo(_: &'static str) -> io::Result<TcpStream> {
        TcpStream::connect(echo())
    }

    fn listener() -> Listener {
        Listener::start_with(LISTED, to_echo).unwrap()
    }

    /// The listener's reply to `request`, empty when it closed the connection with a reset: a
    /// refusal that leaves the request unread (over the cap, or past the longest head) can end so,
    /// and is the refusal as a clean close is.
    fn ask(listener: &Listener, request: &[u8]) -> (String, TcpStream) {
        let mut stream = TcpStream::connect(("127.0.0.1", listener.port())).unwrap();
        // The listener may have reset it before the whole request is sent.
        _ = stream.write_all(request);
        let mut reply = Vec::new();
        let mut byte = [0u8];
        while !reply.ends_with(b"\r\n\r\n") {
            match stream.read(&mut byte) {
                Ok(1) => reply.push(byte[0]),
                Ok(_) => break,
                Err(why) if why.kind() == io::ErrorKind::ConnectionReset => {
                    reply.clear();
                    break;
                }
                Err(why) => panic!("{why}"),
            }
        }
        (String::from_utf8(reply).unwrap(), stream)
    }

    /// A refusal's reply: its status, or nothing when the connection was reset.
    fn refused_with(reply: &str, status: &str) -> bool {
        reply.is_empty() || reply.starts_with(&format!("HTTP/1.1 {status}"))
    }

    #[test]
    fn a_listed_host_gets_a_tunnel_that_carries_its_bytes_as_they_are() {
        let listener = listener();
        let (reply, mut stream) = ask(
            &listener,
            b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\nfirst ",
        );
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        // A second CONNECT is past the head, so it's only bytes to the host, not a request.
        stream
            .write_all(b"CONNECT other.example.org:443 HTTP/1.1\r\n\r\n")
            .unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut back = String::new();
        stream.read_to_string(&mut back).unwrap();
        assert_eq!(back, "first CONNECT other.example.org:443 HTTP/1.1\r\n\r\n");
        assert_eq!(
            listener.counts(),
            Counts {
                passed: 1,
                refused: 0
            }
        );
    }

    #[test]
    fn any_other_request_is_refused_and_closed() {
        let listener = listener();
        let requests: [&[u8]; 4] = [
            b"CONNECT other.example.org:443 HTTP/1.1\r\n\r\n",
            b"CONNECT 127.0.0.1:443 HTTP/1.1\r\n\r\n",
            b"GET http://api.example.org/ HTTP/1.1\r\n\r\n",
            b"CONNECT api.example.org:22 HTTP/1.1\r\n\r\n",
        ];
        for request in requests {
            let (reply, mut stream) = ask(&listener, request);
            assert!(reply.starts_with("HTTP/1.1 403"), "{reply}");
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).unwrap();
            assert!(rest.is_empty());
        }
        let long = [
            b"CONNECT api.example.org:443 HTTP/1.1\r\nX: ".as_slice(),
            &[b'a'; 9000],
        ]
        .concat();
        let (reply, _) = ask(&listener, &long);
        assert!(refused_with(&reply, "403"), "{reply}");
        assert_eq!(
            listener.counts(),
            Counts {
                passed: 0,
                refused: 5
            }
        );
    }

    #[test]
    fn no_more_than_the_cap_are_open_at_once() {
        let listener = listener();
        let open: Vec<TcpStream> = (0..MOST_OPEN)
            .map(|_| ask(&listener, b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\n").1)
            .collect();
        let (reply, _) = ask(&listener, b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\n");
        assert!(refused_with(&reply, "503"), "{reply}");
        assert_eq!(
            listener.counts(),
            Counts {
                passed: MOST_OPEN,
                refused: 1
            }
        );
        drop(open);
    }

    #[test]
    fn the_listener_stops_and_closes_its_tunnels_when_the_task_ends() {
        let listener = listener();
        let port = listener.port();
        let (reply, mut stream) = ask(&listener, b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\n");
        assert!(reply.starts_with("HTTP/1.1 200"));
        drop(listener);
        let mut rest = Vec::new();
        assert_eq!(stream.read_to_end(&mut rest).ok(), Some(0));
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    fn a_listener_whose_wake_fails_still_ends_and_closes_its_tunnels() {
        fn no_wake(_: u16) -> io::Result<TcpStream> {
            Err(io::ErrorKind::ConnectionRefused.into())
        }
        let mut listener = listener();
        listener.wake = no_wake;
        let port = listener.port();
        let (reply, mut stream) = ask(&listener, b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\n");
        assert!(reply.starts_with("HTTP/1.1 200"));
        let (dropped, ended) = mpsc::channel();
        std::thread::spawn(move || {
            drop(listener);
            _ = dropped.send(());
        });
        assert!(
            ended.recv_timeout(Duration::from_secs(5)).is_ok(),
            "the drop waited on a thread nothing woke"
        );
        let mut rest = Vec::new();
        assert_eq!(stream.read_to_end(&mut rest).ok(), Some(0));
        // The thread left behind stops at the next connection and serves it nothing.
        let mut late = TcpStream::connect(("127.0.0.1", port)).unwrap();
        _ = late.write_all(b"CONNECT api.example.org:443 HTTP/1.1\r\n\r\n");
        let mut back = Vec::new();
        _ = late.read_to_end(&mut back);
        assert!(back.is_empty());
    }
}
