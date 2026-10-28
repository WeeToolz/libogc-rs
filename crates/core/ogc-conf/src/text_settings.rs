//! Checked setting.txt field extraction used by the production cache.
use crate::{CONF_ENOENT, CONF_ETOOBIG};

pub(super) fn get_txt_from_data(name: &str, data: &[u8], buf: &mut [u8]) -> Result<usize, i32> {
    let mut line_start = 0usize;
    while line_start < data.len() {
        let line = &data[line_start..];
        // C uses strchr for '=' and line terminators.  Those searches stop at
        // the first NUL, even though the later newline-skip loop walks across
        // embedded NUL bytes.  Restrict the string-style searches to the same
        // prefix so malformed setting.txt input has the same result.
        let c_string_len = line
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(line.len());
        let c_line = &line[..c_string_len];
        let Some(eq_rel) = c_line.iter().position(|byte| *byte == b'=') else {
            line_start = line
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|idx| line_start + idx + 1)
                .unwrap_or(data.len());
            continue;
        };
        if eq_rel == name.len() && &c_line[..eq_rel] == name.as_bytes() {
            // C searches the complete line for CR first. It checks for LF
            // only when no CR exists, even when that LF appears earlier.
            let end_idx = c_line
                .iter()
                .position(|byte| *byte == b'\r')
                .or_else(|| c_line.iter().position(|byte| *byte == b'\n'));
            let Some(end_idx) = end_idx else {
                line_start = line
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map(|idx| line_start + idx + 1)
                    .unwrap_or(data.len());
                continue;
            };
            let value_start = eq_rel + 1;
            let end_rel = end_idx.saturating_sub(value_start);
            if end_rel < buf.len() {
                buf[..end_rel].copy_from_slice(&c_line[value_start..value_start + end_rel]);
                buf[end_rel] = 0;
                return Ok(end_rel);
            }
            return Err(CONF_ETOOBIG);
        }

        let next = line
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|idx| line_start + idx + 1)
            .unwrap_or(data.len());
        line_start = next;
    }

    Err(CONF_ENOENT)
}
