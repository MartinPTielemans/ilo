//! The agents' server on Windows: a named pipe, owned by the user and open to them alone, as the
//! Unix socket elsewhere is a file only they can open (server.rs). No other user or service can
//! connect to it or make an instance of it, no other machine can reach it, and a client checks
//! the pipe it reached is the user's before it hands over its token, so a pipe another user made
//! first under the name gets nothing. Its handles are overlapped, as a synchronous pipe handle
//! lets one thread's write wait for another's read, and each wait also watches the connection's
//! closed event, so closing it ends what another thread waits in. Threads wait in the kernel
//! between messages, as on a socket, so an idle server wakes nothing.

use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_OPERATION_ABORTED,
    ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED, GENERIC_READ, GENERIC_WRITE,
    GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX, ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    WaitNamedPipeW,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenProcessToken, SetEvent, WaitForMultipleObjects,
    WaitForSingleObject, INFINITE,
};

/// What each end buffers.
const BUFFER: u32 = 64 * 1024;
/// How long a client waits for the server to have an instance free, in ms, and how often it tries.
const BUSY_WAIT: u32 = 2000;
const TRIES: usize = 5;

/// The pipe of the profile in `folder`: one of its own, so each profile's browser has its own, and
/// the same at every launch, as the clients' configs name it.
pub fn address(folder: &Path) -> PathBuf {
    // FNV-1a, which no Rust release changes, as `Hash` might.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in folder.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
    }
    PathBuf::from(format!(r"\\.\pipe\{}-agents-{hash:016x}", env!("APP_NAME")))
}

/// Waits for `thread` to end, as join does, but without join's panic for a thread that the
/// process's exit ended before it let go of its result: the last drop of the server can come as
/// the process exits, after its wake let the thread go on.
pub fn wait_for(thread: std::thread::JoinHandle<()>) {
    use std::os::windows::io::AsRawHandle;
    unsafe { WaitForSingleObject(thread.as_raw_handle(), INFINITE) };
}

/// A handle, closed when it's dropped.
struct Handle(HANDLE);

// A kernel handle may be used and closed from any thread.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// A manual-reset event, not set.
fn event() -> io::Result<Handle> {
    let made = unsafe { CreateEventW(null(), 1, 0, null()) };
    match made.is_null() {
        true => Err(io::Error::last_os_error()),
        false => Ok(Handle(made)),
    }
}

/// What every clone of a connection shares.
struct Connection {
    pipe: Handle,
    /// Set once either side of the browser's or the bridge's closes it.
    closed: Handle,
    /// Whether this is the server's end, which closing disconnects.
    server: bool,
}

/// One end of a connection, which a thread reads or writes while another does the other.
pub struct Pipe {
    connection: Arc<Connection>,
    /// This clone's own, for the operation it waits on.
    event: Handle,
}

impl Pipe {
    fn new(pipe: Handle, server: bool) -> io::Result<Pipe> {
        let connection = Connection { pipe, closed: event()?, server };
        Ok(Pipe { connection: Arc::new(connection), event: event()? })
    }

    /// Connects to the server's pipe at `path`, if it's the user's.
    pub fn connect(path: &Path) -> io::Result<Pipe> {
        let name = wide(path);
        for _ in 0..TRIES {
            // Identification only: a server can tell who the client is, and never act as them.
            let flags = FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION;
            let access = GENERIC_READ | GENERIC_WRITE;
            let opened = unsafe {
                CreateFileW(name.as_ptr(), access, 0, null(), OPEN_EXISTING, flags, null_mut())
            };
            if opened != INVALID_HANDLE_VALUE {
                let pipe = Handle(opened);
                if !owned_by_user(&pipe)? {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "the pipe isn't the user's",
                    ));
                }
                return Pipe::new(pipe, false);
            }
            let error = unsafe { GetLastError() };
            if error != ERROR_PIPE_BUSY {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            if unsafe { WaitNamedPipeW(name.as_ptr(), BUSY_WAIT) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Err(io::Error::from_raw_os_error(ERROR_PIPE_BUSY as i32))
    }

    pub fn try_clone(&self) -> io::Result<Pipe> {
        Ok(Pipe { connection: self.connection.clone(), event: event()? })
    }

    /// Ends the connection: what waits on it here returns, and the server's end disconnects the
    /// client's, as a socket's shutdown does.
    pub fn close(&self) {
        unsafe { SetEvent(self.connection.closed.0) };
        if self.connection.server {
            unsafe { DisconnectNamedPipe(self.connection.pipe.0) };
        }
    }

    fn is_closed(&self) -> bool {
        unsafe { WaitForSingleObject(self.connection.closed.0, 0) == WAIT_OBJECT_0 }
    }

    /// An OVERLAPPED for an operation that sets this clone's event once it's done.
    fn overlapped(&self) -> OVERLAPPED {
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = self.event.0;
        overlapped
    }

    /// Waits for the operation `started` began, with `overlapped`, to finish, or for the
    /// connection to close, which cancels it. Gives how many bytes it moved.
    fn finish(&self, overlapped: &mut OVERLAPPED, started: i32) -> io::Result<usize> {
        if started == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
        }
        let waits = [self.event.0, self.connection.closed.0];
        let woke = unsafe { WaitForMultipleObjects(2, waits.as_ptr(), 0, INFINITE) };
        let pipe = self.connection.pipe.0;
        if woke != WAIT_OBJECT_0 {
            unsafe { CancelIoEx(pipe, &*overlapped) };
        }
        // Waits for the cancel too, as `overlapped` is the kernel's until the operation ends.
        let mut moved = 0;
        if unsafe { GetOverlappedResult(pipe, &*overlapped, &mut moved, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(moved as usize)
    }
}

/// Whether the error is the connection's end: the other side closed it, or this one did.
fn ended(error: &io::Error) -> bool {
    let ends = [ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, ERROR_NO_DATA, ERROR_OPERATION_ABORTED];
    error.raw_os_error().is_some_and(|code| ends.contains(&(code as u32)))
}

impl Read for Pipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.is_closed() {
            return Ok(0);
        }
        let mut overlapped = self.overlapped();
        let length = buffer.len().min(u32::MAX as usize) as u32;
        let pipe = self.connection.pipe.0;
        let started =
            unsafe { ReadFile(pipe, buffer.as_mut_ptr(), length, null_mut(), &mut overlapped) };
        match self.finish(&mut overlapped, started) {
            Err(error) if ended(&error) => Ok(0),
            read => read,
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.is_closed() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let mut overlapped = self.overlapped();
        let length = bytes.len().min(u32::MAX as usize) as u32;
        let pipe = self.connection.pipe.0;
        let started =
            unsafe { WriteFile(pipe, bytes.as_ptr(), length, null_mut(), &mut overlapped) };
        match self.finish(&mut overlapped, started) {
            Err(error) if ended(&error) => Err(io::ErrorKind::BrokenPipe.into()),
            wrote => wrote,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The server's pipe, with an instance always waiting for the next client.
pub struct Listener {
    name: Vec<u16>,
    security: Descriptor,
    next: Pipe,
}

impl Listener {
    /// Makes the pipe at `path`, which fails if it's there already, whoever made it.
    pub fn bind(path: &Path) -> io::Result<Listener> {
        let name = wide(path);
        let security = Descriptor::users_alone()?;
        let next = instance(&name, &security, true)?;
        Ok(Listener { name, security, next })
    }

    /// The next client's connection, once one connects. An error means no more can.
    pub fn accept(&mut self) -> io::Result<Pipe> {
        for _ in 0..TRIES {
            let mut overlapped = self.next.overlapped();
            let pipe = self.next.connection.pipe.0;
            let started = unsafe { ConnectNamedPipe(pipe, &mut overlapped) };
            // A client that connected between the instance's making and now is connected.
            let connected = (started == 0 && unsafe { GetLastError() } == ERROR_PIPE_CONNECTED)
                || self.next.finish(&mut overlapped, started).is_ok();
            let made = instance(&self.name, &self.security, false)?;
            let taken = std::mem::replace(&mut self.next, made);
            // A client that went before it was taken leaves its instance behind.
            if connected {
                return Ok(taken);
            }
        }
        Err(io::Error::other("clients kept going before they were taken"))
    }
}

/// An instance of the pipe, which the first must be, so that it's the browser's own.
fn instance(name: &[u16], security: &Descriptor, first: bool) -> io::Result<Pipe> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let mut open = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    if first {
        open |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
    let made = unsafe {
        let most = PIPE_UNLIMITED_INSTANCES;
        CreateNamedPipeW(name.as_ptr(), open, mode, most, BUFFER, BUFFER, 0, &attributes)
    };
    match made == INVALID_HANDLE_VALUE {
        true => Err(io::Error::last_os_error()),
        false => Pipe::new(Handle(made), true),
    }
}

/// A security descriptor, freed when it's dropped.
struct Descriptor(PSECURITY_DESCRIPTOR);

// Read only once it's made.
unsafe impl Send for Descriptor {}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}

impl Descriptor {
    /// Owned by the user and open to them alone, with no access inherited: no other user, no
    /// service and no administrator but by taking ownership.
    fn users_alone() -> io::Result<Descriptor> {
        let user = User::current()?.text()?;
        let text: Vec<u16> = format!("O:{user}D:P(A;;GA;;;{user})").encode_utf16().chain([0]).collect();
        let mut made = null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut made,
                null_mut(),
            )
        };
        match converted == 0 {
            true => Err(io::Error::last_os_error()),
            false => Ok(Descriptor(made)),
        }
    }
}

/// The user this process runs as: its token's TOKEN_USER, in a buffer aligned for it.
struct User(Vec<u64>);

impl User {
    fn current() -> io::Result<User> {
        let mut token = null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Handle(token);
        let mut length = 0;
        unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut length) };
        let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
        let read = unsafe {
            GetTokenInformation(token.0, TokenUser, buffer.as_mut_ptr().cast(), length, &mut length)
        };
        match read == 0 {
            true => Err(io::Error::last_os_error()),
            false => Ok(User(buffer)),
        }
    }

    fn sid(&self) -> PSID {
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    /// Its SID as text, such as "S-1-5-21-…", for a security descriptor.
    fn text(&self) -> io::Result<String> {
        let mut text = null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let length = (0..).take_while(|&at| unsafe { *text.add(at) } != 0).count();
        let read = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
        unsafe { LocalFree(text.cast()) };
        Ok(read)
    }
}

/// Whether the user this process runs as owns the pipe `pipe` is a handle to.
fn owned_by_user(pipe: &Handle) -> io::Result<bool> {
    let user = User::current()?;
    let (mut owner, mut descriptor): (PSID, PSECURITY_DESCRIPTOR) = (null_mut(), null_mut());
    let error = unsafe {
        GetSecurityInfo(
            pipe.0,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    let descriptor = Descriptor(descriptor);
    let same = unsafe { EqualSid(owner, user.sid()) } != 0;
    drop(descriptor);
    Ok(same)
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain([0]).collect()
}
