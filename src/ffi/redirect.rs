// redirect: capture the process's stdout/stderr for one call.
//
// hvolt writes through println!/eprintln!, so the only way to hand its text
// back to an embedder is to move the fds underneath it. A pipe plus a reader
// thread is used instead of a temp file: the file version came back full of NUL
// bytes on device, while a pipe carries exactly the bytes that were written,
// in order.
#[cfg(unix)]
mod unix {
    use std::io::Read;
    use std::os::unix::io::FromRawFd;

    pub struct Capture {
        saved_out: i32,
        saved_err: i32,
        reader: Option<std::thread::JoinHandle<Vec<u8>>>,
    }

    impl Capture {
        pub fn start() -> Option<Self> {
            // SAFETY: pipe/dup/dup2 on the standard fds; every return is checked.
            #[allow(unsafe_code)]
            unsafe {
                let mut fds = [0i32; 2];
                if libc::pipe(fds.as_mut_ptr()) != 0 {
                    return None;
                }
                let saved_out = libc::dup(1);
                let saved_err = libc::dup(2);
                if saved_out < 0 || saved_err < 0 {
                    return None;
                }
                if libc::dup2(fds[1], 1) < 0 || libc::dup2(fds[1], 2) < 0 {
                    return None;
                }
                libc::close(fds[1]);

                // The read end belongs to the thread; it sees EOF once every write
                // end is closed, which happens when finish() restores the originals.
                let mut file = std::fs::File::from_raw_fd(fds[0]);
                let reader = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    let _ = file.read_to_end(&mut buf);
                    buf
                });
                Some(Self {
                    saved_out,
                    saved_err,
                    reader: Some(reader),
                })
            }
        }

        pub fn finish(mut self) -> String {
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
            // SAFETY: restoring the fds saved in start() and closing the copies.
            #[allow(unsafe_code)]
            unsafe {
                libc::dup2(self.saved_out, 1);
                libc::dup2(self.saved_err, 2);
                libc::close(self.saved_out);
                libc::close(self.saved_err);
            }
            match self.reader.take() {
                Some(h) => h
                    .join()
                    .map(|b| String::from_utf8_lossy(&b).to_string())
                    .unwrap_or_default(),
                None => String::new(),
            }
        }
    }
}

#[cfg(unix)]
pub use unix::Capture;

#[cfg(not(unix))]
pub struct Capture;

#[cfg(not(unix))]
impl Capture {
    pub fn start() -> Option<Self> {
        None
    }
    pub fn finish(self) -> String {
        String::new()
    }
}
