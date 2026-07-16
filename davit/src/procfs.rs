//! Procfs helpers: process-tree discovery, CPU/RSS sampling, and the
//! read-only diagnostic probes (ps/wchan/fdtable/fionread). Races with
//! exiting processes are tolerated — missing /proc entries are skipped.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;

/// All live PIDs whose ancestry includes `root` (root included).
pub fn descendants(root: i32) -> Vec<i32> {
    let mut ppid_map: HashMap<i32, i32> = HashMap::new();
    if let Ok(rd) = fs::read_dir("/proc") {
        for e in rd.flatten() {
            if let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() {
                if let Some(ppid) = stat_field(pid, StatField::Ppid) {
                    ppid_map.insert(pid, ppid as i32);
                }
            }
        }
    }
    let mut out = vec![];
    for (&pid, _) in &ppid_map {
        let mut cur = pid;
        let mut hops = 0;
        while hops < 128 {
            if cur == root {
                out.push(pid);
                break;
            }
            match ppid_map.get(&cur) {
                Some(&p) if p != cur && p != 0 => cur = p,
                _ => break,
            }
            hops += 1;
        }
    }
    out.sort_unstable();
    out
}

enum StatField {
    Ppid,
    State,
    Utime,
    Stime,
    RssPages,
}

/// Parse a field out of /proc/<pid>/stat, robust to comm containing
/// spaces/parens (split after the last ')').
fn stat_field(pid: i32, field: StatField) -> Option<u64> {
    let s = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &s[s.rfind(')')? + 2..];
    let parts: Vec<&str> = after.split_whitespace().collect();
    // after-comm index 0 = field 3 (state)
    match field {
        StatField::State => parts.first().map(|c| c.bytes().next().unwrap_or(b'?') as u64),
        StatField::Ppid => parts.get(1)?.parse().ok(),
        StatField::Utime => parts.get(11)?.parse().ok(),
        StatField::Stime => parts.get(12)?.parse().ok(),
        StatField::RssPages => parts.get(21)?.parse().ok(),
    }
}

fn proc_state(pid: i32) -> char {
    stat_field(pid, StatField::State).map(|b| b as u8 as char).unwrap_or('?')
}

fn cpu_ticks(pid: i32) -> u64 {
    stat_field(pid, StatField::Utime).unwrap_or(0) + stat_field(pid, StatField::Stime).unwrap_or(0)
}

fn rss_kib(pid: i32) -> u64 {
    let page_kib = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64 / 1024;
    stat_field(pid, StatField::RssPages).unwrap_or(0) * page_kib
}

fn ticks_per_sec() -> f64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) as f64 }
}

/// One aggregated process-tree sample. CPU is sampled over `sample_ms`;
/// 100.0 means one full core.
pub struct TreeSample {
    pub live_non_zombie: u32,
    pub cpu_percent: f64,
    pub rss_kib: u64,
}

pub fn sample_tree(root: i32, sample_ms: u64) -> TreeSample {
    let pids = descendants(root);
    let before: HashMap<i32, u64> = pids.iter().map(|&p| (p, cpu_ticks(p))).collect();
    std::thread::sleep(std::time::Duration::from_millis(sample_ms));
    let pids = descendants(root);
    let mut delta = 0u64;
    let mut rss = 0u64;
    let mut live = 0u32;
    for &p in &pids {
        let st = proc_state(p);
        if st != 'Z' && st != '?' {
            live += 1;
            rss += rss_kib(p);
            let now = cpu_ticks(p);
            delta += now.saturating_sub(*before.get(&p).unwrap_or(&now));
        }
    }
    let cpu = 100.0 * (delta as f64 / ticks_per_sec()) / (sample_ms as f64 / 1000.0);
    TreeSample { live_non_zombie: live, cpu_percent: (cpu * 10.0).round() / 10.0, rss_kib: rss }
}

#[derive(Serialize)]
pub struct PsEntry {
    pub pid: i32,
    pub state: String,
    pub cpu_percent: f64,
    pub rss_kib: u64,
    pub comm: String,
}

/// `diagnose ps`: per-process listing with a short CPU sample.
pub fn ps_tree(root: i32, sample_ms: u64) -> Vec<PsEntry> {
    let pids = descendants(root);
    let before: HashMap<i32, u64> = pids.iter().map(|&p| (p, cpu_ticks(p))).collect();
    std::thread::sleep(std::time::Duration::from_millis(sample_ms));
    descendants(root)
        .into_iter()
        .map(|p| {
            let now = cpu_ticks(p);
            let delta = now.saturating_sub(*before.get(&p).unwrap_or(&now));
            let cpu =
                100.0 * (delta as f64 / ticks_per_sec()) / (sample_ms as f64 / 1000.0);
            PsEntry {
                pid: p,
                state: proc_state(p).to_string(),
                cpu_percent: (cpu * 10.0).round() / 10.0,
                rss_kib: rss_kib(p),
                comm: fs::read_to_string(format!("/proc/{p}/comm"))
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct WchanEntry {
    pub pid: i32,
    pub comm: String,
    pub wchan: String,
}

/// `diagnose wchan`: kernel wait channels for the given PIDs.
pub fn wchan(pids: &[i32]) -> Vec<WchanEntry> {
    pids.iter()
        .filter_map(|&p| {
            Some(WchanEntry {
                pid: p,
                comm: fs::read_to_string(format!("/proc/{p}/comm")).ok()?.trim().to_string(),
                wchan: fs::read_to_string(format!("/proc/{p}/wchan"))
                    .unwrap_or_else(|_| "?".into()),
            })
        })
        .collect()
}

#[derive(Serialize)]
pub struct FdEntry {
    pub fd: i32,
    pub target: String,
}

/// `diagnose fdtable`: fd numbers and resolved targets for a PID.
pub fn fdtable(pid: i32) -> Vec<FdEntry> {
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(format!("/proc/{pid}/fd")) {
        for e in rd.flatten() {
            if let Ok(fd) = e.file_name().to_string_lossy().parse::<i32>() {
                let target = fs::read_link(e.path())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "?".into());
                out.push(FdEntry { fd, target });
            }
        }
    }
    out.sort_by_key(|e| e.fd);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_is_own_descendant() {
        let me = std::process::id() as i32;
        assert!(descendants(me).contains(&me));
    }

    #[test]
    fn sample_tolerates_missing_pids() {
        // A PID that certainly doesn't exist: sample must not panic.
        let s = sample_tree(-1, 10);
        assert_eq!(s.live_non_zombie, 0);
        assert_eq!(s.rss_kib, 0);
    }

    #[test]
    fn fdtable_of_self_has_stdio() {
        let me = std::process::id() as i32;
        let fds = fdtable(me);
        assert!(fds.iter().any(|e| e.fd == 0));
    }
}
