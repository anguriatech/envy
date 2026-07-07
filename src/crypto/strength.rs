//! Passphrase strength estimation for interactive `encrypt`/`rotate` prompts.
//!
//! This is purely informational: it never rejects or blocks a passphrase.
//! `envy.enc` security ultimately rests on Argon2id + AES-256-GCM (see
//! `crate::crypto::artifact`), not on this heuristic — a low score is a
//! nudge to the user, never an enforced gate.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::db`.

/// Qualitative strength bucket derived from estimated entropy bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrengthLevel {
    VeryWeak,
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl StrengthLevel {
    /// Lowercase label suitable for CLI output (e.g. "weak").
    pub fn label(self) -> &'static str {
        match self {
            StrengthLevel::VeryWeak => "very weak",
            StrengthLevel::Weak => "weak",
            StrengthLevel::Fair => "fair",
            StrengthLevel::Strong => "strong",
            StrengthLevel::VeryStrong => "very strong",
        }
    }

    /// `true` for the two weakest buckets — used to decide whether to print
    /// a hint pointing the user at `envy`'s own Diceware generator.
    pub fn is_weak(self) -> bool {
        matches!(self, StrengthLevel::VeryWeak | StrengthLevel::Weak)
    }
}

/// Estimates the Shannon entropy (in bits) of `passphrase` from the size of
/// the character classes it draws from, multiplied by its length.
///
/// This is a coarse, dependency-free heuristic — not a dictionary/pattern
/// aware estimator like zxcvbn. It cannot detect that "password123" is a
/// common credential; it only measures character-class diversity and
/// length. It intentionally errs conservative (never overstates strength by
/// assuming a larger alphabet than what's observed).
pub fn estimate_entropy_bits(passphrase: &str) -> f64 {
    let len = passphrase.chars().count();
    if len == 0 {
        return 0.0;
    }

    let has_lower = passphrase.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = passphrase.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = passphrase.chars().any(|c| c.is_ascii_digit());
    let has_symbol = passphrase
        .chars()
        .any(|c| c.is_ascii() && !c.is_ascii_alphanumeric() && c != ' ');
    let has_space = passphrase.contains(' ');
    let has_non_ascii = !passphrase.is_ascii();

    let mut charset_size: u32 = 0;
    if has_lower {
        charset_size += 26;
    }
    if has_upper {
        charset_size += 26;
    }
    if has_digit {
        charset_size += 10;
    }
    if has_symbol {
        charset_size += 32;
    }
    if has_space {
        charset_size += 1;
    }
    if has_non_ascii {
        // Conservative floor for the observed non-ASCII usage — not a real
        // alphabet count, just enough to avoid under-crediting Unicode input.
        charset_size += 100;
    }

    if charset_size == 0 {
        return 0.0;
    }

    (len as f64) * (charset_size as f64).log2()
}

/// Classifies `bits` of entropy into a coarse [`StrengthLevel`].
///
/// Thresholds are aligned with common crack-time guidance (zxcvbn-style
/// buckets): below 28 bits is crackable in seconds to minutes on consumer
/// hardware; 80+ bits is comparable to a 6-word Diceware passphrase
/// (log2(7776^6) ≈ 77.5 bits) from `envy`'s own generator.
pub fn classify(bits: f64) -> StrengthLevel {
    if bits < 28.0 {
        StrengthLevel::VeryWeak
    } else if bits < 40.0 {
        StrengthLevel::Weak
    } else if bits < 60.0 {
        StrengthLevel::Fair
    } else if bits < 80.0 {
        StrengthLevel::Strong
    } else {
        StrengthLevel::VeryStrong
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_passphrase_has_zero_entropy() {
        assert_eq!(estimate_entropy_bits(""), 0.0);
        assert_eq!(classify(0.0), StrengthLevel::VeryWeak);
    }

    #[test]
    fn short_lowercase_only_is_weak_or_worse() {
        let bits = estimate_entropy_bits("hunter2".trim_end_matches(char::is_numeric));
        assert!(classify(bits).is_weak(), "bits = {bits}");
    }

    #[test]
    fn long_mixed_charset_is_strong() {
        let bits = estimate_entropy_bits("Tr0ub4dor&3-XyZ!qw");
        assert!(
            matches!(
                classify(bits),
                StrengthLevel::Strong | StrengthLevel::VeryStrong
            ),
            "bits = {bits}"
        );
    }

    #[test]
    fn longer_passphrase_scores_higher_than_shorter_with_same_charset() {
        let short = estimate_entropy_bits("abcabc");
        let long = estimate_entropy_bits("abcabcabcabc");
        assert!(long > short);
    }

    #[test]
    fn classify_boundaries() {
        assert_eq!(classify(27.9), StrengthLevel::VeryWeak);
        assert_eq!(classify(28.0), StrengthLevel::Weak);
        assert_eq!(classify(39.9), StrengthLevel::Weak);
        assert_eq!(classify(40.0), StrengthLevel::Fair);
        assert_eq!(classify(59.9), StrengthLevel::Fair);
        assert_eq!(classify(60.0), StrengthLevel::Strong);
        assert_eq!(classify(79.9), StrengthLevel::Strong);
        assert_eq!(classify(80.0), StrengthLevel::VeryStrong);
    }
}
