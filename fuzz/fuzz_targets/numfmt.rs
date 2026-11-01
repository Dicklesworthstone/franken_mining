//! Number-format codes and displayed values (`fmine_io::numfmt`).
//!
//! Input: a selector byte, then text: the format code, a newline, and the stored cell text. The code is parsed on
//! its own, then used to display the cell text as a custom format; the selector also picks a built-in format id to
//! display it with.
#![no_main]

use fmine_io::numfmt::{display, parse_format};
use fmine_io::styles::NumberFormat;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let (code, raw) = text.split_once('\n').unwrap_or((text, "1234.5"));
    let _ = parse_format(code);
    let custom = NumberFormat::Custom {
        id: 164,
        code: code.to_owned(),
    };
    for fmt in [custom, NumberFormat::Builtin(u32::from(selector % 64))] {
        if let Ok(d) = display(raw, &fmt) {
            let _ = d.text();
        }
    }
});
