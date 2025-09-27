// Bring in formatting core
use core::fmt::{self, Write};

/// A simple fixed-size buffer you can write formatted strings into.
pub struct FmtBuf<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> FmtBuf<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        FmtBuf { buf, pos: 0 }
    }

    pub fn as_str(&self) -> &str {
        // Safety: we only ever write valid UTF-8 via `write_str`
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.pos]) }
    }
}

impl<'a> Write for FmtBuf<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        if self.pos + bytes.len() > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}

/// Your `format`-like function (returns &str instead of String).
pub fn format_to<'a>(
    buf: &'a mut [u8],
    args: fmt::Arguments<'_>
) -> Result<&'a str, fmt::Error> {
    let mut f = FmtBuf::new(buf);
    f.write_fmt(args)?;
    let s = unsafe { core::str::from_utf8_unchecked(&f.buf[..f.pos]) };
    Ok(s)
}

// Re-export format_to at the crate root if needed
#[macro_export]
macro_rules! format_no_std {
    ($buf:expr, $($arg:tt)*) => {
        $crate::format::format_to($buf, core::format_args!($($arg)*))
    };
}