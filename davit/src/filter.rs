//! Stateful Vivado message filter.
//!
//! Rules (spec `vivado-session` / docs §7):
//! - `INFO:` suppressed with continuation lines.
//! - `ERROR:` and `CRITICAL WARNING:` retained and surfaced as errors.
//! - `WARNING:` suppressed by default; with a valid `elfws.yaml`
//!   suppression file, IDs listed are suppressed and IDs absent are
//!   retained; warnings without an ID are suppressed.
//! - Ordinary output is retained and resets filter state.
//! - Indented lines and `Resolution:` follow the preceding decision.
//! - Suppression file reloaded per command; parse failure falls back to
//!   blanket warning suppression (recorded by the caller in the raw log).

use std::collections::HashSet;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Decision {
    Retain,
    Suppress,
    RetainError,
}

/// Per-command warning suppression policy.
#[derive(Clone, Debug)]
pub enum WarningPolicy {
    /// No suppression file (or parse failure): suppress all warnings.
    SuppressAll,
    /// Valid file: suppress listed IDs, retain unlisted; ID-less warnings
    /// are suppressed.
    ByList(HashSet<String>),
}

/// Load the suppression policy from a workspace `elfws.yaml`.
/// Returns `(policy, parse_error)`; a parse error yields `SuppressAll`
/// plus a message the caller must record in the raw log.
pub fn load_policy(path: &Path) -> (WarningPolicy, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (WarningPolicy::SuppressAll, None), // no file: blanket suppression
    };
    match parse_suppression_yaml(&text) {
        Ok(ids) => (WarningPolicy::ByList(ids), None),
        Err(e) => (
            WarningPolicy::SuppressAll,
            Some(format!("elfws.yaml parse failure, falling back to blanket warning suppression: {e}")),
        ),
    }
}

/// Minimal YAML subset parser: a document containing a list of message
/// IDs, either as top-level `- ID` items or under a `suppress:` key.
/// Anything else is a parse error (safe fallback applies).
pub fn parse_suppression_yaml(text: &str) -> Result<HashSet<String>, String> {
    let mut ids = HashSet::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim_end();
        let t = line.trim_start();
        if t.is_empty() || t == "---" || t == "suppress:" {
            continue;
        }
        if let Some(item) = t.strip_prefix("- ") {
            let id = item.trim().trim_matches('"').trim_matches('\'');
            // Vivado message IDs are "<Facility> <num-num>": exactly one space.
            let valid = matches!(id.split(' ').collect::<Vec<_>>().as_slice(),
                [fac, code] if !fac.is_empty() && code.contains('-'));
            if !valid {
                return Err(format!("line {}: invalid message ID {id:?}", n + 1));
            }
            ids.insert(id.to_string());
        } else {
            return Err(format!("line {}: unsupported YAML construct {t:?}", n + 1));
        }
    }
    Ok(ids)
}

/// Extract a Vivado message ID like `[Synth 8-7080]` from a message line.
fn message_id(line: &str) -> Option<String> {
    let start = line.find('[')?;
    let end = line[start..].find(']')? + start;
    let id = &line[start + 1..end];
    if id.is_empty() || !id.contains(' ') {
        return None; // Vivado IDs are "<facility> <num-num>"
    }
    Some(id.to_string())
}

/// Stateful line filter for one command's output.
pub struct Filter {
    policy: WarningPolicy,
    last: Decision,
    output: Vec<String>,
    errors: Vec<String>,
    current_error: Option<String>,
}

impl Filter {
    pub fn new(policy: WarningPolicy) -> Self {
        Self {
            policy,
            last: Decision::Retain,
            output: Vec::new(),
            errors: Vec::new(),
            current_error: None,
        }
    }

    pub fn feed_line(&mut self, line: &str) {
        let decision = self.classify(line);
        match decision {
            Decision::Retain => self.output.push(line.to_string()),
            Decision::RetainError => {
                self.output.push(line.to_string());
                match &mut self.current_error {
                    Some(e) if self.last == Decision::RetainError && is_continuation(line) => {
                        e.push('\n');
                        e.push_str(line);
                    }
                    _ => {
                        if let Some(e) = self.current_error.take() {
                            self.errors.push(e);
                        }
                        self.current_error = Some(line.to_string());
                    }
                }
            }
            Decision::Suppress => {}
        }
        self.last = decision;
    }

    fn classify(&self, line: &str) -> Decision {
        let t = line.trim_start();
        if t.starts_with("ERROR:") || t.starts_with("CRITICAL WARNING:") {
            return Decision::RetainError;
        }
        if t.starts_with("INFO:") {
            return Decision::Suppress;
        }
        if t.starts_with("WARNING:") {
            return match &self.policy {
                WarningPolicy::SuppressAll => Decision::Suppress,
                WarningPolicy::ByList(ids) => match message_id(t) {
                    None => Decision::Suppress, // ID-less warnings suppressed
                    Some(id) => {
                        if ids.contains(&id) {
                            Decision::Suppress
                        } else {
                            Decision::Retain
                        }
                    }
                },
            };
        }
        // Continuations and Resolution: follow the preceding decision.
        if is_continuation(line) {
            return self.last;
        }
        // Ordinary output retains and resets state.
        Decision::Retain
    }

    /// Finish: returns (filtered_output, errors).
    pub fn finish(mut self) -> (String, Vec<String>) {
        if let Some(e) = self.current_error.take() {
            self.errors.push(e);
        }
        (self.output.join("\n"), self.errors)
    }
}

fn is_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t') || line.trim_start().starts_with("Resolution:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(policy: WarningPolicy, lines: &[&str]) -> (String, Vec<String>) {
        let mut f = Filter::new(policy);
        for l in lines {
            f.feed_line(l);
        }
        f.finish()
    }

    #[test]
    fn info_suppressed_with_continuations() {
        let (out, errs) = run(
            WarningPolicy::SuppressAll,
            &["INFO: [Common 17-206] exiting", "    continuation of info", "real output"],
        );
        assert_eq!(out, "real output");
        assert!(errs.is_empty());
    }

    #[test]
    fn errors_retained_and_surfaced() {
        let (out, errs) = run(
            WarningPolicy::SuppressAll,
            &[
                "ERROR: [Synth 8-439] module not found",
                "  Resolution: check the module name",
                "ok line",
            ],
        );
        assert!(out.contains("ERROR:"));
        assert!(out.contains("Resolution:"));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("Resolution: check the module name"));
    }

    #[test]
    fn critical_warning_is_error() {
        let (_, errs) =
            run(WarningPolicy::SuppressAll, &["CRITICAL WARNING: [Vivado 12-1] bad"]);
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn warnings_suppressed_by_default() {
        let (out, _) = run(
            WarningPolicy::SuppressAll,
            &["WARNING: [Synth 8-7080] parallel synthesis", "  more warning detail", "data"],
        );
        assert_eq!(out, "data");
    }

    #[test]
    fn suppression_list_retains_unlisted_ids() {
        let ids: HashSet<String> = ["Synth 8-7080".to_string()].into();
        let (out, _) = run(
            WarningPolicy::ByList(ids),
            &[
                "WARNING: [Synth 8-7080] listed -> suppressed",
                "WARNING: [Synth 8-9999] unlisted -> retained",
                "WARNING: no id here -> suppressed",
            ],
        );
        assert_eq!(out, "WARNING: [Synth 8-9999] unlisted -> retained");
    }

    #[test]
    fn ordinary_output_resets_state() {
        let (out, _) = run(
            WarningPolicy::SuppressAll,
            &["INFO: suppressed", "plain", "  indented follows plain -> retained"],
        );
        assert_eq!(out, "plain\n  indented follows plain -> retained");
    }

    #[test]
    fn yaml_list_parses() {
        let ids = parse_suppression_yaml("# ids\n- Synth 8-7080\n- \"Vivado 12-1\"\n").unwrap();
        assert!(ids.contains("Synth 8-7080") && ids.contains("Vivado 12-1"));
    }

    #[test]
    fn yaml_with_suppress_key() {
        let ids = parse_suppression_yaml("suppress:\n  - Synth 8-7080\n").unwrap();
        assert!(ids.contains("Synth 8-7080"));
    }

    #[test]
    fn yaml_garbage_fails_safely() {
        assert!(parse_suppression_yaml("foo: {bar}").is_err());
        let dir = std::env::temp_dir().join(format!("davit-flt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("elfws.yaml");
        std::fs::write(&p, "foo: {bar}").unwrap();
        let (policy, err) = load_policy(&p);
        assert!(matches!(policy, WarningPolicy::SuppressAll));
        assert!(err.unwrap().contains("falling back"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_means_blanket_suppression_no_error() {
        let (policy, err) = load_policy(Path::new("/nonexistent/elfws.yaml"));
        assert!(matches!(policy, WarningPolicy::SuppressAll));
        assert!(err.is_none());
    }
}
