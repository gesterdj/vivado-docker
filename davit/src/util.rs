//! Small shared helpers: time formatting, atomic writes, misc.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as ISO 8601 UTC with `Z` suffix, second resolution.
pub fn iso8601_now() -> String {
    iso8601(SystemTime::now())
}

pub fn iso8601(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Compact timestamp for filenames: 20260716-152233.
pub fn stamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Days-based civil calendar conversion (Howard Hinnant's algorithm).
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (h, mi, s) = ((rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, h, mi, s)
}

/// Unix seconds now.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write `data` to `path` via temp file + atomic rename in the same dir.
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(
        ".{}.tmp.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("f"),
        std::process::id()
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Human duration like `12m02s` from whole seconds.
pub fn human_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m{:02}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_epoch() {
        assert_eq!(iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso8601_known() {
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_752_684_082);
        assert_eq!(iso8601(t), "2025-07-16T16:41:22Z");
    }

    #[test]
    fn atomic_write_replaces() {
        let dir = std::env::temp_dir().join(format!("davit-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.json");
        atomic_write(&p, b"one").unwrap();
        atomic_write(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn durations() {
        assert_eq!(human_duration(722), "12m02s");
        assert_eq!(human_duration(5), "5s");
        assert_eq!(human_duration(3723), "1h02m03s");
    }
}
