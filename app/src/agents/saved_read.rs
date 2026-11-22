//! Saved reads (#3819): the core's read of a saved address on a profile's own session, with no tab
//! (bc_profile_read), and what it answers. The tools that propose and use a read (#3822) build
//! on these.

use std::ffi::{CStr, c_char};

use crate::bc;

/// The most of a body a read keeps, as the core caps it.
pub const MAX_BYTES: usize = 4 << 20;
/// The longest a read waits, in milliseconds, as the core caps it.
pub const MAX_MILLISECONDS: i64 = 30_000;

/// How a read ended (bc_read_answer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    /// The HTTP status, or 0 if none came.
    pub status: u16,
    /// The address that answered, after redirects on the read's own origin.
    pub url: String,
    /// A redirect to another origin, which the core told and didn't follow, such as to a sign-in.
    pub redirect: Option<String>,
    /// The body's MIME type, or "".
    pub kind: String,
    pub body: Vec<u8>,
    /// Whether the body is only its first part.
    pub cut: bool,
    /// Why it wasn't sent or broke off.
    pub error: Option<String>,
}

impl Answer {
    /// The answer the core gave, copied, as it lasts only for profile_read_done.
    ///
    /// # Safety
    /// `answer`'s pointers are the core's: each null or a NUL-terminated string, and `body` null or
    /// `size` bytes.
    pub unsafe fn from_core(answer: &bc::bc_read_answer) -> Self {
        let text = |text: *const c_char| {
            (!text.is_null()).then(|| {
                unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .into_owned()
            })
        };
        let body = match (answer.body.is_null(), answer.size) {
            (true, _) | (_, 0) => Vec::new(),
            _ => unsafe { std::slice::from_raw_parts(answer.body.cast::<u8>(), answer.size) }
                .to_vec(),
        };
        Self {
            status: u16::try_from(answer.status).unwrap_or(0),
            url: text(answer.url).unwrap_or_default(),
            redirect: text(answer.redirect),
            kind: text(answer.type_).unwrap_or_default(),
            body,
            cut: answer.cut,
            error: text(answer.error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn an_answer_is_copied_whole_with_its_body_not_ended_at_a_nul() {
        let (url, kind) = (c"https://www.example.test/inbox", c"application/json");
        let body = b"{\"rows\":[1,\0 2]}";
        let answer = bc::bc_read_answer {
            status: 200,
            url: url.as_ptr(),
            redirect: std::ptr::null(),
            type_: kind.as_ptr(),
            body: body.as_ptr().cast(),
            size: body.len(),
            cut: true,
            error: std::ptr::null(),
        };
        let copied = unsafe { Answer::from_core(&answer) };
        assert_eq!(
            copied,
            Answer {
                status: 200,
                url: "https://www.example.test/inbox".into(),
                redirect: None,
                kind: "application/json".into(),
                body: body.to_vec(),
                cut: true,
                error: None,
            }
        );
    }

    #[test]
    fn a_redirect_to_another_origin_and_an_error_come_through_as_told() {
        let url = c"https://www.example.test/inbox";
        let redirect = CString::new("https://login.example.test/?next=%2Finbox").unwrap();
        let told = bc::bc_read_answer {
            status: 302,
            url: url.as_ptr(),
            redirect: redirect.as_ptr(),
            type_: c"".as_ptr(),
            body: std::ptr::null(),
            size: 0,
            cut: false,
            error: std::ptr::null(),
        };
        let told = unsafe { Answer::from_core(&told) };
        assert_eq!(told.status, 302);
        assert_eq!(
            told.redirect.as_deref(),
            Some("https://login.example.test/?next=%2Finbox")
        );
        assert!(told.body.is_empty());

        let error = c"No answer came: ERR_TIMED_OUT";
        let failed = bc::bc_read_answer {
            status: 0,
            url: c"".as_ptr(),
            redirect: std::ptr::null(),
            type_: c"".as_ptr(),
            body: std::ptr::null(),
            size: 0,
            cut: false,
            error: error.as_ptr(),
        };
        let failed = unsafe { Answer::from_core(&failed) };
        assert_eq!(failed.status, 0);
        assert_eq!(
            failed.error.as_deref(),
            Some("No answer came: ERR_TIMED_OUT")
        );
        assert_eq!(failed.redirect, None);
    }
}
