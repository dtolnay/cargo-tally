use indicatif::ProgressBar;

use std::io::{self, Read};

pub struct ProgressRead<'a, R> {
    pb: &'a ProgressBar,
    inner: R,
}

impl<'a, R> ProgressRead<'a, R> {
    pub fn new(pb: &'a ProgressBar, read: R) -> Self {
        ProgressRead { pb, inner: read }
    }
}

impl<'a, R> Read for ProgressRead<'a, R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.inner.read(buf);
        if let Ok(n) = result {
            self.pb.inc(n as u64);
        }
        result
    }
}
