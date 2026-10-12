//! `story::interp` — **Form A-7 Tier-1 interpolation** for authored `[note]`
//! bodies (`docs/lore/FORM_A7_OPERATOR_DIAGNOSTIC.md`).
//!
//! The Stewardship Division's diagnostic probe is diegetic: a `system_log` placard can address
//! the operator by their real handle ("operator session: {username}@{hostname}.
//! welcome."). This module is that substitution — a **level-load templating pass**
//! over note bodies, run once per load in [`crate::level_text::bind_level_text`],
//! never per frame.
//!
//! ## The allowlist IS the feature (canonical token table)
//!
//! | Token (pinned aliases) | A-7 § | Source in v0.1 |
//! |---|---|---|
//! | `{username}` (`{whoami}`) | §3.2.1 | `$USER` → `$LOGNAME` → `$USERNAME`; else `anon` |
//! | `{hostname}` | §3.2.2 | `$HOSTNAME` → `$COMPUTERNAME` → `/proc/sys/kernel/hostname` → `/etc/hostname`; else `unknown` |
//! | `{local_time}` (`{time}`) | §3.2.3 | the machine clock, human-readable |
//! | `{tz}` → `tz_offset_min` | §3.2.3 | the local UTC offset when the platform declares one |
//! | `{locale}` | §3.2.3 | `$LC_ALL` → `$LANG` → `$LANGUAGE`; else `C` |
//! | `{terminal}` → `terminal_name` | §3.2.3 | **undeclared in v0.1** — see below |
//! | `{sys_uptime}` (`{uptime}`) | §3.2.4 | `/proc/uptime`; unreadable on Windows |
//!
//! **Nothing else resolves.** Tier-2/3 fields (`{battery}`, `{last_commit}`,
//! `{visible_apps[0]}`, …) are NOT read in v0.1, and this module needs zero
//! per-token code to contain them:
//!
//! ## Structural containment — the one rule that cannot leak
//!
//! After the substitution pass, **any LINE still containing an unresolved brace run
//! (`{…}`) is omitted entirely.** The scan matches ANY run — not just
//! `[a-z_]+` — so `{visible_apps[0]}` and `{ battery }` are contained exactly like
//! `{battery}`. A future token added to a `.lvl` before its Rust read exists costs
//! its line, never a leaked placeholder. (A literal `{}` with nothing inside is not
//! a token run and does not drop its line.)
//!
//! ## Sanitization (A-7 preamble, verbatim contract)
//!
//! Every substituted value is: control codes **stripped** (so no ESC can reach the
//! terminal through a placard), invalid UTF-8 **replaced with U+FFFD** (the
//! `OsString::to_string_lossy` path — every OS read goes through it), template
//! characters **escaped** (`{`/`}` → `(`/`)`, so a hostile `$USER` cannot inject a
//! token or defeat the containment scan), and the result **capped at 64 visible
//! characters**.
//!
//! ## Opt-out (`lore_fourth_wall = false`, A-7 §8)
//!
//! [`crate::story::interp::OperatorProfile::for_fourth_wall`] called with `false` performs **no reads at all** — every
//! declared Tier-1 token substitutes the A-7 §8 fallback constant
//! ([`crate::story::interp::FALLBACK_USERNAME`] / [`crate::story::interp::FALLBACK_UNKNOWN`] / [`crate::story::interp::FALLBACK_LOCALE`]). It is
//! stricter than the form's own wording, which the form now records (its 2026-07-30
//! implementation note).
//!
//! ## `{terminal}`: declared vs UNDECLARED
//!
//! `terminal_name` is the one Tier-1 field with no OS read — A-7 §3.2.3 says it is
//! *supplied by the host layer after the backend initialises*. That hand-off does
//! not exist in v0.1 (`re_present` hands `re_game` no terminal string, and this
//! crate must never touch the terminal itself — `rules/engine-api-contract.md`), so
//! the field is **undeclared**: [`OperatorProfile::terminal_name`] is `None` and
//! `{terminal}` stays UNRESOLVED in *both* opt states, dropping its line. That is
//! deliberate — an undeclared field has no disclosed value to fall back *from*,
//! unlike a declared field whose read merely failed. [`with_terminal_name`] is the
//! hook the day `re_present` hands one over.
//!
//! [`with_terminal_name`]: OperatorProfile::with_terminal_name

use re_level::Level;

/// A-7 §8 fallback for an unreadable `username`.
pub const FALLBACK_USERNAME: &str = "anon";
/// A-7 §8 fallback for any unreadable non-locale field (`hostname`, `local_time`,
/// `tz`, `sys_uptime`).
pub const FALLBACK_UNKNOWN: &str = "unknown";
/// A-7 §8 fallback for an unreadable `locale` (the POSIX default locale).
pub const FALLBACK_LOCALE: &str = "C";

/// The A-7 preamble's visible-character cap on every substituted value.
pub const VISIBLE_CAP: usize = 64;

/// The Tier-1 field values one interpolation pass substitutes.
///
/// Construct with [`for_fourth_wall`](Self::for_fourth_wall) (the shipped path,
/// driven by [`crate::settings::Settings::lore_fourth_wall`]),
/// [`probe`](Self::probe) (real reads), or [`fallbacks`](Self::fallbacks) (the
/// A-7 §8 constants). Every stored value is ALREADY sanitized, so substitution is
/// a plain string splice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorProfile {
    /// §3.2.1 local entity handle.
    pub username: String,
    /// §3.2.2 machine handle (short hostname; never an FQDN — no resolver call).
    pub hostname: String,
    /// §3.2.3 human-readable machine clock reading.
    pub local_time: String,
    /// §3.2.3 UTC offset of the local zone, rendered `+HH:MM`, or the fallback.
    pub tz: String,
    /// §3.2.3 locale name.
    pub locale: String,
    /// §3.2.3 terminal emulator name — `None` while UNDECLARED (see the module
    /// docs); `{terminal}` then stays unresolved and drops its line.
    pub terminal_name: Option<String>,
    /// §3.2.4 system uptime, human-readable, or the fallback.
    pub sys_uptime: String,
}

impl Default for OperatorProfile {
    /// The A-7 §8 fallback profile — the safe, read-free default.
    fn default() -> Self {
        Self::fallbacks()
    }
}

impl OperatorProfile {
    /// The profile the shipped game uses: real Tier-1 reads when the operator has
    /// left the fourth wall UP (`lore_fourth_wall = true`, the default), and the
    /// A-7 §8 fallback constants — with **no reads performed** — when they opted
    /// out.
    pub fn for_fourth_wall(fourth_wall: bool) -> Self {
        if fourth_wall {
            Self::probe()
        } else {
            Self::fallbacks()
        }
    }

    /// The A-7 §8 fallback profile: `anon` / `unknown` / `C`, no reads, no clock.
    /// `terminal_name` stays `None` (undeclared, not "failed").
    pub fn fallbacks() -> Self {
        OperatorProfile {
            username: FALLBACK_USERNAME.to_string(),
            hostname: FALLBACK_UNKNOWN.to_string(),
            local_time: FALLBACK_UNKNOWN.to_string(),
            tz: FALLBACK_UNKNOWN.to_string(),
            locale: FALLBACK_LOCALE.to_string(),
            terminal_name: None,
            sys_uptime: FALLBACK_UNKNOWN.to_string(),
        }
    }

    /// Perform the real Tier-1 reads. Every field falls back to its A-7 §8
    /// constant when its source is absent or unreadable, so this never fails and
    /// never yields an empty value.
    pub fn probe() -> Self {
        let tz_min = read_tz_offset_min();
        OperatorProfile {
            username: sanitize_or(read_username(), FALLBACK_USERNAME),
            hostname: sanitize_or(read_hostname(), FALLBACK_UNKNOWN),
            local_time: sanitize_or(read_local_time(tz_min), FALLBACK_UNKNOWN),
            tz: sanitize_or(tz_min.map(format_tz_offset), FALLBACK_UNKNOWN),
            locale: sanitize_or(read_locale(), FALLBACK_LOCALE),
            // UNDECLARED in v0.1 — see the module docs.
            terminal_name: None,
            sys_uptime: sanitize_or(read_sys_uptime(), FALLBACK_UNKNOWN),
        }
    }

    /// Declare the terminal emulator name (the A-7 §3.2.3 host hand-off). Call
    /// this when a backend-detected string becomes available as plain data; the
    /// value is sanitized like any other field.
    #[must_use]
    pub fn with_terminal_name(mut self, name: impl Into<String>) -> Self {
        let s = sanitize(&name.into());
        self.terminal_name = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Resolve one token NAME (the text between the braces, already trimmed of
    /// nothing — an exact match is required) to its value, honouring the pinned
    /// aliases. `None` ⇒ the token is not on the allowlist (or is undeclared), so
    /// its line drops.
    pub fn value_for(&self, token: &str) -> Option<&str> {
        match token {
            "username" | "whoami" => Some(&self.username),
            "hostname" => Some(&self.hostname),
            "local_time" | "time" => Some(&self.local_time),
            "tz" | "tz_offset_min" => Some(&self.tz),
            "locale" => Some(&self.locale),
            "terminal" | "terminal_name" => self.terminal_name.as_deref(),
            "sys_uptime" | "uptime" => Some(&self.sys_uptime),
            _ => None,
        }
    }
}

/// Run the pass over one note body: substitute every allowlisted token, then OMIT
/// every line that still carries an unresolved `{…}` run.
///
/// Pure: same body + same profile ⇒ same output, always.
pub fn interpolate(body: &str, profile: &OperatorProfile) -> String {
    // Cheap exit for the overwhelmingly common case (no braces at all).
    if !body.contains('{') {
        return body.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    let substituted: Vec<String> = body
        .split('\n')
        .map(|l| substitute_line(l, profile))
        .collect();
    for line in &substituted {
        if !has_unresolved_token(line) {
            out.push(line.as_str());
        }
    }
    out.join("\n")
}

/// Apply [`interpolate`] to every `[note]` body of `level`, in place. Returns how
/// many bodies actually changed (diagnostics only).
pub fn interpolate_level_notes(level: &mut Level, profile: &OperatorProfile) -> usize {
    let mut changed = 0;
    for note in &mut level.notes {
        let next = interpolate(&note.text, profile);
        if next != note.text {
            note.text = next;
            changed += 1;
        }
    }
    changed
}

// ─────────────────────────── the substitution core ──────────────────────────

/// Substitute every allowlisted token in ONE line. An unknown / undeclared token
/// is left verbatim so [`has_unresolved_token`] can drop the line.
fn substitute_line(line: &str, profile: &OperatorProfile) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '{' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // A candidate token: `{` … `}` with no nested `{` and no `}` inside.
        match chars[i + 1..].iter().position(|&c| c == '}' || c == '{') {
            Some(rel) if chars[i + 1 + rel] == '}' && rel > 0 => {
                let name: String = chars[i + 1..i + 1 + rel].iter().collect();
                match profile.value_for(&name) {
                    Some(v) => out.push_str(v),
                    // Not on the allowlist: keep it verbatim (the line will drop).
                    None => {
                        out.push('{');
                        out.push_str(&name);
                        out.push('}');
                    }
                }
                i += rel + 2;
            }
            // `{}`, `{` with no close, or a nested `{` — never a token; emit the
            // brace and continue from the next char (the nested run gets its own
            // chance to match).
            _ => {
                out.push('{');
                i += 1;
            }
        }
    }
    out
}

/// Does this line still contain an unresolved brace run — the regex `\{[^}]+\}`,
/// hand-scanned (no regex dependency)?
///
/// `[^}]` deliberately INCLUDES `{`, so `{a{b}` is a run and drops its line; `{}`
/// (nothing between the braces) is not.
fn has_unresolved_token(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != '{' {
            continue;
        }
        // The nearest `}` strictly after `i + 1` closes a non-empty run.
        if let Some(rel) = chars[i + 1..].iter().position(|&d| d == '}') {
            if rel > 0 {
                return true;
            }
        }
    }
    false
}

// ───────────────────────────── sanitization ─────────────────────────────────

/// The A-7 preamble's sanitization, in its stated order: strip control codes,
/// escape template characters, cap at [`VISIBLE_CAP`] visible characters.
///
/// (Invalid UTF-8 became U+FFFD upstream: every OS read in this module goes
/// through [`std::ffi::OsStr::to_string_lossy`], which is exactly that
/// replacement. U+FFFD is a printable char and survives here, so the operator
/// sees the honest replacement glyph rather than a silently dropped name.)
pub fn sanitize(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '{' => '(',
            '}' => ')',
            other => other,
        })
        .take(VISIBLE_CAP)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Sanitize a read value, substituting `fallback` when the read was absent or
/// sanitized down to nothing.
fn sanitize_or(raw: Option<String>, fallback: &str) -> String {
    match raw {
        Some(v) => {
            let s = sanitize(&v);
            if s.is_empty() {
                fallback.to_string()
            } else {
                s
            }
        }
        None => fallback.to_string(),
    }
}

// ──────────────────────────── the Tier-1 reads ──────────────────────────────
//
// Every read is LOCAL and bounded: environment variables, two `/proc`/`/etc`
// text files, and the process clock. No network, no subprocess, no account
// database — A-7 §6's prohibitions hold by construction because there is no code
// here that could violate them.

/// Read the first environment variable of `names` that is present and non-empty.
/// `to_string_lossy` is the U+FFFD path for a non-UTF-8 value.
fn first_env(names: &[&str]) -> Option<String> {
    for n in names {
        if let Some(v) = std::env::var_os(n) {
            let s = v.to_string_lossy().to_string();
            if !s.trim().is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// §3.2.1 — `$USER` → `$LOGNAME` → `$USERNAME` (the Windows equivalent).
fn read_username() -> Option<String> {
    first_env(&["USER", "LOGNAME", "USERNAME"])
}

/// §3.2.2 — the SHORT hostname. `$HOSTNAME` / `$COMPUTERNAME` first (the latter
/// is always set on Windows), then Linux's own two canonical text files. No FQDN
/// lookup, so no resolver and no network.
fn read_hostname() -> Option<String> {
    if let Some(v) = first_env(&["HOSTNAME", "COMPUTERNAME"]) {
        return Some(v);
    }
    for path in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let first = text.lines().next().unwrap_or("").trim().to_string();
            if !first.is_empty() {
                return Some(first);
            }
        }
    }
    None
}

/// §3.2.3 — `$LC_ALL` → `$LANG` → `$LANGUAGE`.
fn read_locale() -> Option<String> {
    first_env(&["LC_ALL", "LANG", "LANGUAGE"])
}

/// §3.2.3 — the local UTC offset in MINUTES, when the platform declares one.
///
/// v0.1 LIMITATION (recorded, not hidden): the real sources — `localtime_r` /
/// `GetTimeZoneInformation` — are unreachable from safe `std` without a new
/// dependency, and `unsafe_code` is forbidden workspace-wide. We therefore accept
/// an explicit numeric offset in `$TZ` (`Z`, `UTC`, `+03:00`, `-0430`, `+05`) and
/// otherwise report the field as unreadable ⇒ its A-7 fallback. A zone NAME in
/// `$TZ` (`Europe/Moscow`) is NOT guessed at — a wrong offset would be worse than
/// an honest `unknown`.
fn read_tz_offset_min() -> Option<i32> {
    parse_tz_offset_min(&first_env(&["TZ"])?)
}

/// Parse an explicit numeric UTC offset. Accepts `Z` / `UTC` / `GMT` (⇒ 0),
/// `+HH`, `+HH:MM`, `+HHMM` (and the `-` forms). Anything else ⇒ `None`.
fn parse_tz_offset_min(raw: &str) -> Option<i32> {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("z") || t.eq_ignore_ascii_case("utc") || t.eq_ignore_ascii_case("gmt")
    {
        return Some(0);
    }
    let (sign, rest) = match t.as_bytes().first()? {
        b'+' => (1, &t[1..]),
        b'-' => (-1, &t[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    if rest.chars().any(|c| !c.is_ascii_digit() && c != ':') {
        return None;
    }
    let (h, m) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().ok()?, 0),
        3 => (
            digits[..1].parse::<i32>().ok()?,
            digits[1..].parse::<i32>().ok()?,
        ),
        4 => (
            digits[..2].parse::<i32>().ok()?,
            digits[2..].parse::<i32>().ok()?,
        ),
        _ => return None,
    };
    if h > 14 || m > 59 {
        return None;
    }
    Some(sign * (h * 60 + m))
}

/// Render an offset in minutes as `+HH:MM` / `-HH:MM` (`+00:00` for UTC).
fn format_tz_offset(min: i32) -> String {
    let sign = if min < 0 { '-' } else { '+' };
    let a = min.abs();
    format!("{sign}{:02}:{:02}", a / 60, a % 60)
}

/// §3.2.3 — a human-readable reading of the machine clock.
///
/// `YYYY-MM-DD HH:MM` in the local zone when the offset is declared (suffixed
/// with the offset), else the SAME instant in UTC, explicitly labelled. The
/// instant is always a real read; only its zone labelling degrades.
fn read_local_time(tz_offset_min: Option<i32>) -> Option<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    let shifted = secs + i64::from(tz_offset_min.unwrap_or(0)) * 60;
    let (y, mo, d, h, mi) = civil_from_unix(shifted);
    Some(match tz_offset_min {
        Some(off) => format!(
            "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02} {}",
            format_tz_offset(off)
        ),
        None => format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02} UTC"),
    })
}

/// Unix seconds → `(year, month, day, hour, minute)` in the same zone as the
/// input. Pure integer arithmetic (Hinnant's civil-from-days), so it is exact,
/// dependency-free, and testable against known instants.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, (tod / 3600) as u32, ((tod % 3600) / 60) as u32)
}

/// §3.2.4 — system uptime, human-readable.
///
/// Linux: `/proc/uptime`'s first token. Windows: `GetTickCount64` is a Win32 call
/// this crate cannot make (no `unsafe`, no platform dependency), so the field
/// reports as unreadable ⇒ its A-7 fallback. Recorded, not silently faked.
fn read_sys_uptime() -> Option<String> {
    let text = std::fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = text.split_whitespace().next()?.parse().ok()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    Some(format_duration_secs(secs as u64))
}

/// `90061 → "1d 1h 1m"`, `3720 → "1h 2m"`, `45 → "45s"`. Coarse on purpose: the
/// lore line wants "a long time", not a stopwatch.
fn format_duration_secs(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fully-DECLARED profile (including the terminal hand-off) with obvious
    /// values, so a substitution is unmistakable in an assertion.
    fn declared() -> OperatorProfile {
        OperatorProfile {
            username: "aleks".into(),
            hostname: "mnemos".into(),
            local_time: "2026-07-30 14:03 +03:00".into(),
            tz: "+03:00".into(),
            locale: "ru_RU.UTF-8".into(),
            terminal_name: Some("ms-terminal".into()),
            sys_uptime: "3d 4h 5m".into(),
        }
    }

    // ── the allowlist + the pinned aliases ───────────────────────────────────

    #[test]
    fn every_allowlisted_token_and_alias_resolves() {
        let p = declared();
        for (token, expect) in [
            ("username", "aleks"),
            ("whoami", "aleks"),
            ("hostname", "mnemos"),
            ("local_time", "2026-07-30 14:03 +03:00"),
            ("time", "2026-07-30 14:03 +03:00"),
            ("tz", "+03:00"),
            ("tz_offset_min", "+03:00"),
            ("locale", "ru_RU.UTF-8"),
            ("terminal", "ms-terminal"),
            ("terminal_name", "ms-terminal"),
            ("sys_uptime", "3d 4h 5m"),
            ("uptime", "3d 4h 5m"),
        ] {
            assert_eq!(p.value_for(token), Some(expect), "token {{{token}}}");
            assert_eq!(
                interpolate(&format!("x {{{token}}} y"), &p),
                format!("x {expect} y"),
                "substituting {{{token}}}"
            );
        }
    }

    #[test]
    fn a_line_with_only_resolved_tokens_survives_intact() {
        let p = declared();
        let body = "> [boot.07] operator session: {username}@{hostname}. welcome.";
        assert_eq!(
            interpolate(body, &p),
            "> [boot.07] operator session: aleks@mnemos. welcome."
        );
    }

    #[test]
    fn a_body_with_no_braces_is_returned_verbatim() {
        let body = "Day 41 of the grow trial.\nWater, light, seed.";
        assert_eq!(interpolate(body, &declared()), body);
    }

    // ── STRUCTURAL containment ───────────────────────────────────────────────

    #[test]
    fn a_line_with_an_unresolved_token_is_omitted_entirely() {
        let p = declared();
        let body = "keep one\nbattery at {battery} percent\nkeep two";
        assert_eq!(interpolate(body, &p), "keep one\nkeep two");
    }

    /// The scan matches ANY brace run — the whole point of B3. Every one of these
    /// Tier-2/3 shapes present in the authored corpus must cost its line.
    #[test]
    fn tier_two_and_three_token_shapes_all_drop_their_lines() {
        let p = declared();
        for token in [
            "{battery}",
            "{last_commit}",
            "{last_file}",
            "{visible_apps[0]}",
            "{build_date}",
            "{ battery }",
            "{BATTERY}",
            "{git.author}",
            "{a{b}",
            "{prev}",
        ] {
            let body = format!("alive\ndoomed {token} tail\nalive too");
            assert_eq!(
                interpolate(&body, &p),
                "alive\nalive too",
                "line carrying {token} must be omitted"
            );
        }
    }

    #[test]
    fn a_mixed_line_drops_even_though_some_tokens_resolved() {
        let p = declared();
        // {username} resolves, {battery} does not — the LINE is the unit.
        let body = "{username} sees {battery}\nsafe";
        assert_eq!(interpolate(body, &p), "safe");
    }

    #[test]
    fn empty_braces_are_not_a_token_and_keep_their_line() {
        let p = declared();
        assert_eq!(interpolate("a {} b", &p), "a {} b");
        // An unterminated brace is not a run either.
        assert_eq!(interpolate("a { b", &p), "a { b");
    }

    #[test]
    fn a_body_whose_every_line_drops_becomes_empty() {
        let p = declared();
        assert_eq!(interpolate("{battery}\n{last_commit}", &p), "");
    }

    // ── {terminal}: declared vs undeclared ───────────────────────────────────

    #[test]
    fn an_undeclared_terminal_drops_its_line_in_both_opt_states() {
        let body = "your terminal is {terminal}. that one's new.\nM. used putty.";
        for mut p in [OperatorProfile::probe(), OperatorProfile::fallbacks()] {
            p.terminal_name = None;
            assert_eq!(
                interpolate(body, &p),
                "M. used putty.",
                "an UNDECLARED terminal_name has no A-7 value to fall back from"
            );
        }
        // …and the hook resolves it the day the host layer hands one over.
        let p = OperatorProfile::fallbacks().with_terminal_name("xterm-256color");
        assert_eq!(
            interpolate(body, &p),
            "your terminal is xterm-256color. that one's new.\nM. used putty."
        );
    }

    // ── sanitization ─────────────────────────────────────────────────────────

    #[test]
    fn control_codes_are_stripped_from_a_value() {
        // An ESC sequence in $USER must never reach the framebuffer/terminal.
        let p = OperatorProfile::fallbacks().with_terminal_name("a\u{1b}[31mb\u{7}c");
        assert_eq!(p.terminal_name.as_deref(), Some("a[31mbc"));
    }

    #[test]
    fn template_characters_in_a_value_are_escaped_and_cannot_inject_a_token() {
        let p = OperatorProfile::fallbacks().with_terminal_name("evil{battery}end");
        assert_eq!(p.terminal_name.as_deref(), Some("evil(battery)end"));
        // Substituting it must NOT drop the line (no brace run survives).
        assert_eq!(
            interpolate("term {terminal} ok", &p),
            "term evil(battery)end ok"
        );
    }

    #[test]
    fn a_value_is_capped_at_64_visible_characters() {
        let long = "x".repeat(200);
        let p = OperatorProfile::fallbacks().with_terminal_name(long);
        assert_eq!(
            p.terminal_name.as_deref().unwrap().chars().count(),
            VISIBLE_CAP
        );
    }

    #[test]
    fn the_replacement_character_survives_sanitization() {
        // Invalid UTF-8 became U+FFFD in `to_string_lossy`; it is printable and
        // must be shown honestly, not dropped.
        assert_eq!(sanitize("na\u{fffd}me"), "na\u{fffd}me");
    }

    #[test]
    fn a_value_sanitized_to_nothing_falls_back() {
        assert_eq!(sanitize_or(Some("\u{1b}\u{7}".into()), "anon"), "anon");
        assert_eq!(sanitize_or(None, "unknown"), "unknown");
    }

    // ── the opt-out, with a POPULATED environment ────────────────────────────
    //
    // These two tests are the acceptance pair: the SAME populated environment,
    // read both ways. They mutate process env, so they share one lock and are
    // written as a single test each side of a helper.

    /// Populate the A-7 Tier-1 environment, run `f`, then restore. Process env is
    /// global, so the env-touching tests serialize on one mutex.
    fn with_populated_env<T>(f: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let keys = [
            "USER",
            "LOGNAME",
            "USERNAME",
            "HOSTNAME",
            "COMPUTERNAME",
            "LC_ALL",
            "LANG",
            "TZ",
        ];
        let saved: Vec<(&str, Option<std::ffi::OsString>)> =
            keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        for k in keys {
            std::env::remove_var(k);
        }
        std::env::set_var("USER", "k.hoffmann");
        std::env::set_var("HOSTNAME", "mnemos-07");
        std::env::set_var("LC_ALL", "de_DE.UTF-8");
        std::env::set_var("TZ", "+02:00");
        let out = f();
        for (k, v) in saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        out
    }

    #[test]
    fn opt_in_with_a_populated_env_shows_real_values() {
        let p = with_populated_env(|| OperatorProfile::for_fourth_wall(true));
        assert_eq!(p.username, "k.hoffmann", "real $USER");
        assert_eq!(p.hostname, "mnemos-07", "real $HOSTNAME");
        assert_eq!(p.locale, "de_DE.UTF-8", "real $LC_ALL");
        assert_eq!(p.tz, "+02:00", "real numeric $TZ");
        // The clock is a real read: a formatted instant, never the fallback.
        assert_ne!(p.local_time, FALLBACK_UNKNOWN);
        assert!(
            p.local_time.starts_with("20") && p.local_time.contains(':'),
            "local_time reads as a real timestamp: {}",
            p.local_time
        );
        let body = "session: {username}@{hostname} · {locale} · {tz}\nbattery {battery}";
        assert_eq!(
            interpolate(body, &p),
            "session: k.hoffmann@mnemos-07 · de_DE.UTF-8 · +02:00",
            "Tier-1 resolves; the Tier-2 line still drops"
        );
    }

    #[test]
    fn opt_out_with_the_same_populated_env_shows_only_fallbacks() {
        let p = with_populated_env(|| OperatorProfile::for_fourth_wall(false));
        assert_eq!(p.username, FALLBACK_USERNAME, "NO real read when opted out");
        assert_eq!(p.hostname, FALLBACK_UNKNOWN);
        assert_eq!(p.locale, FALLBACK_LOCALE);
        assert_eq!(p.tz, FALLBACK_UNKNOWN);
        assert_eq!(p.local_time, FALLBACK_UNKNOWN, "not even the clock is read");
        assert_eq!(p.sys_uptime, FALLBACK_UNKNOWN);
        let body = "session: {username}@{hostname} · {locale}\nbattery {battery}";
        assert_eq!(
            interpolate(body, &p),
            "session: anon@unknown · C",
            "Tier-1 substitutes the A-7 §8 constants; Tier-2/3 lines still drop"
        );
    }

    #[test]
    fn a_scrubbed_env_falls_back_without_blanks() {
        // Every Tier-1 field must be non-empty even with nothing to read.
        let p = with_populated_env(|| {
            for k in [
                "USER",
                "LOGNAME",
                "USERNAME",
                "HOSTNAME",
                "COMPUTERNAME",
                "LC_ALL",
                "LANG",
                "TZ",
            ] {
                std::env::remove_var(k);
            }
            OperatorProfile::probe()
        });
        for v in [
            &p.username,
            &p.hostname,
            &p.local_time,
            &p.tz,
            &p.locale,
            &p.sys_uptime,
        ] {
            assert!(!v.is_empty(), "no Tier-1 field may render blank");
        }
        assert_eq!(p.username, FALLBACK_USERNAME);
        assert_eq!(p.tz, FALLBACK_UNKNOWN, "a zone NAME is never guessed at");
    }

    // ── the level-load pass ──────────────────────────────────────────────────

    #[test]
    fn interpolating_a_level_rewrites_only_the_bodies_that_change() {
        use re_level::{LevelNote, NoteKind};
        let mut level = Level {
            notes: vec![
                LevelNote {
                    kind: NoteKind::SystemLog,
                    x: 1,
                    y: 2,
                    z: 3,
                    facing: 0,
                    text: "> welcome {username}\n> battery {battery}".into(),
                },
                LevelNote {
                    kind: NoteKind::Researcher,
                    x: 4,
                    y: 5,
                    z: 6,
                    facing: 0,
                    text: "Day 41. No tokens here.".into(),
                },
            ],
            ..Default::default()
        };
        let changed = interpolate_level_notes(&mut level, &declared());
        assert_eq!(changed, 1, "only the templated body changed");
        assert_eq!(level.notes[0].text, "> welcome aleks");
        assert_eq!(level.notes[1].text, "Day 41. No tokens here.");
    }

    // ── the helpers ──────────────────────────────────────────────────────────

    #[test]
    fn tz_offsets_parse_only_when_explicitly_numeric() {
        assert_eq!(parse_tz_offset_min("Z"), Some(0));
        assert_eq!(parse_tz_offset_min("utc"), Some(0));
        assert_eq!(parse_tz_offset_min("+03:00"), Some(180));
        assert_eq!(parse_tz_offset_min("-0430"), Some(-270));
        assert_eq!(parse_tz_offset_min("+05"), Some(300));
        // A zone NAME is never guessed at, nor is an out-of-range offset.
        assert_eq!(parse_tz_offset_min("Europe/Moscow"), None);
        assert_eq!(parse_tz_offset_min("EST5EDT"), None);
        assert_eq!(parse_tz_offset_min("+99:00"), None);
        assert_eq!(parse_tz_offset_min(""), None);
    }

    #[test]
    fn tz_offsets_format_with_a_sign_and_padding() {
        assert_eq!(format_tz_offset(0), "+00:00");
        assert_eq!(format_tz_offset(180), "+03:00");
        assert_eq!(format_tz_offset(-270), "-04:30");
    }

    #[test]
    fn civil_time_matches_known_instants() {
        // Epoch.
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0));
        // 2026-07-30 12:14:00 UTC = 1785413640 (= 20664 whole days + 44040 s).
        assert_eq!(civil_from_unix(1_785_413_640), (2026, 7, 30, 12, 14));
        // A leap day.
        assert_eq!(civil_from_unix(1_709_164_800), (2024, 2, 29, 0, 0));
    }

    #[test]
    fn durations_format_coarsely() {
        assert_eq!(format_duration_secs(45), "45s");
        assert_eq!(format_duration_secs(3_720), "1h 2m");
        assert_eq!(format_duration_secs(90_061), "1d 1h 1m");
        assert_eq!(format_duration_secs(600), "10m");
    }
}
