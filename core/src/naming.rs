//! Pure functions that turn source names into SP-404-safe output names.

use std::collections::HashSet;
use std::path::Path;

use deunicode::deunicode;

use crate::tags::prepend;

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Sanitizes one path component (a file stem or a folder name): ASCII letters, digits,
/// `-` and `_` only, no repeated or edge underscores, never empty, never a Windows device name.
pub fn sanitize_component(name: &str) -> String {
    let ascii = deunicode(name);
    let mut out = String::with_capacity(ascii.len());
    for ch in ascii.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '-' { ch } else { '_' };
        if mapped == '_' && out.ends_with('_') {
            continue;
        }
        out.push(mapped);
    }
    let trimmed = out.trim_matches('_');
    let mut result = if trimmed.is_empty() {
        String::from("untitled")
    } else {
        trimmed.to_string()
    };
    if WINDOWS_RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&result)) {
        result.push('_');
    }
    result
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// `["Breaks", "Funk"]` + `"kick"` → `"Breaks_Funk_kick"`.
pub fn flatten_stem<S: AsRef<str>>(dirs: &[S], stem: &str) -> String {
    let mut parts: Vec<&str> = dirs.iter().map(|d| d.as_ref()).collect();
    parts.push(stem);
    parts.join("_")
}

/// Hands out unique file names per output folder. Comparison ignores case because the
/// SD card (exFAT), APFS and NTFS all do.
#[derive(Debug, Default)]
pub struct CollisionResolver {
    taken: HashSet<String>,
}

impl CollisionResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn claim(&mut self, dir: &Path, stem: &str, ext: &str, max_len: Option<usize>) -> String {
        self.claim_prefixed(dir, None, stem, ext, max_len)
    }

    /// Like `claim` for `<prefix>_<stem>`, but the length limit only ever shortens `stem`: the
    /// prefix (key/BPM) is kept whole, even when that makes the name longer than `max_len`.
    pub fn claim_prefixed(&mut self, dir: &Path, prefix: Option<&str>, stem: &str, ext: &str, max_len: Option<usize>) -> String {
        let mut n = 1usize;
        loop {
            let suffix = if n == 1 { String::new() } else { format!("_{n}") };
            let room = max_len.map(|max| max.saturating_sub(suffix.chars().count()));
            let stem_part = match (prefix, room) {
                (None, Some(room)) => truncate_chars(stem, room),
                (None, None) => stem.to_string(),
                (Some(p), Some(room)) => {
                    let rest = truncate_chars(stem, room.saturating_sub(p.chars().count() + 1));
                    prepend(Some(p), &rest)
                }
                (Some(p), None) => prepend(Some(p), stem),
            };
            let file_name = if ext.is_empty() {
                format!("{stem_part}{suffix}")
            } else {
                format!("{stem_part}{suffix}.{ext}")
            };
            let key = dir.join(&file_name).to_string_lossy().to_lowercase();
            if self.taken.insert(key) {
                return file_name;
            }
            n += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn transliterates_accents_and_replaces_specials() {
        assert_eq!(sanitize_component("Batida Água #1"), "Batida_Agua_1");
    }

    #[test]
    fn decomposed_accents_from_macos_are_removed() {
        // macOS often stores "Água" decomposed: "A" + U+0301 (combining acute accent).
        assert_eq!(sanitize_component("A\u{301}gua"), "Agua");
    }

    #[test]
    fn keeps_safe_chars() {
        assert_eq!(sanitize_component("kick-01_hard"), "kick-01_hard");
    }

    #[test]
    fn collapses_and_trims_underscores() {
        assert_eq!(sanitize_component("  __snare!!  (v2) "), "snare_v2");
    }

    #[test]
    fn dots_become_underscores() {
        assert_eq!(sanitize_component("kick.v2"), "kick_v2");
    }

    #[test]
    fn empty_or_symbol_only_becomes_untitled() {
        assert_eq!(sanitize_component("!!!"), "untitled");
        assert_eq!(sanitize_component(""), "untitled");
    }

    #[test]
    fn windows_reserved_names_get_suffix() {
        assert_eq!(sanitize_component("con"), "con_");
        assert_eq!(sanitize_component("LPT1"), "LPT1_");
        assert_eq!(sanitize_component("console"), "console");
    }

    #[test]
    fn emoji_and_non_latin_become_ascii() {
        for input in ["🔥 808", "ドラム", "Привет"] {
            let out = sanitize_component(input);
            assert!(!out.is_empty());
            assert!(
                out.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{input} -> {out}"
            );
        }
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        assert_eq!(truncate_chars("abcdef", 3), "abc");
        assert_eq!(truncate_chars("ab", 5), "ab");
        assert_eq!(truncate_chars("ááá", 2), "áá");
    }

    #[test]
    fn flatten_joins_dirs_and_stem() {
        assert_eq!(flatten_stem(&["Breaks", "Funk"], "kick"), "Breaks_Funk_kick");
        assert_eq!(flatten_stem::<&str>(&[], "kick"), "kick");
    }

    #[test]
    fn resolver_returns_name_unchanged_when_free() {
        let mut r = CollisionResolver::new();
        assert_eq!(r.claim(Path::new("Breaks"), "kick", "wav", None), "kick.wav");
    }

    #[test]
    fn resolver_numbers_duplicates() {
        let mut r = CollisionResolver::new();
        let dir = Path::new("");
        assert_eq!(r.claim(dir, "kick", "wav", None), "kick.wav");
        assert_eq!(r.claim(dir, "kick", "wav", None), "kick_2.wav");
        assert_eq!(r.claim(dir, "kick", "wav", None), "kick_3.wav");
    }

    #[test]
    fn resolver_is_case_insensitive() {
        // Review Focus 1: APFS/NTFS/exFAT treat these as the same file.
        let mut r = CollisionResolver::new();
        let dir = Path::new("Drums");
        assert_eq!(r.claim(dir, "Kick", "wav", None), "Kick.wav");
        assert_eq!(r.claim(dir, "kick", "wav", None), "kick_2.wav");
    }

    #[test]
    fn resolver_scopes_by_directory() {
        let mut r = CollisionResolver::new();
        assert_eq!(r.claim(Path::new("A"), "kick", "wav", None), "kick.wav");
        assert_eq!(r.claim(Path::new("B"), "kick", "wav", None), "kick.wav");
    }

    #[test]
    fn resolver_respects_max_len_with_suffix() {
        let mut r = CollisionResolver::new();
        let dir = Path::new("");
        assert_eq!(r.claim(dir, "snare", "wav", Some(4)), "snar.wav");
        assert_eq!(r.claim(dir, "snarex", "wav", Some(4)), "sn_2.wav");
    }

    #[test]
    fn max_len_never_cuts_the_prefix() {
        let mut r = CollisionResolver::new();
        let dir = Path::new("");
        assert_eq!(r.claim_prefixed(dir, Some("Gbm_140"), "Loop", "wav", Some(6)), "Gbm_140.wav");
        assert_eq!(r.claim_prefixed(dir, Some("Gbm_140"), "Loop", "wav", Some(6)), "Gbm_140_2.wav");
        assert_eq!(r.claim_prefixed(dir, Some("Am_90"), "Melody", "wav", Some(9)), "Am_90_Mel.wav");
        assert_eq!(r.claim_prefixed(dir, Some("Am_90"), "Melody", "wav", Some(9)), "Am_90_M_2.wav");
        assert_eq!(r.claim_prefixed(dir, Some("Am_90"), "", "wav", Some(3)), "Am_90.wav");
    }

    #[test]
    fn claim_prefixed_without_prefix_is_claim() {
        let (mut a, mut b) = (CollisionResolver::new(), CollisionResolver::new());
        let dir = Path::new("x");
        for stem in ["snare", "snarex", "kick"] {
            assert_eq!(a.claim_prefixed(dir, None, stem, "wav", Some(4)), b.claim(dir, stem, "wav", Some(4)));
        }
    }

    #[test]
    fn resolver_handles_empty_extension() {
        let mut r = CollisionResolver::new();
        assert_eq!(r.claim(Path::new(""), "README", "", None), "README");
    }
}
