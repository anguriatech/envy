//! Diceware passphrase generator using the EFF Large Wordlist.
//!
//! Uses the OS CSPRNG exclusively (Constitution Principle I — secrets must
//! never rely on a seeded or predictable RNG). The word list is embedded at
//! compile time via `include_str!`; no file I/O occurs at runtime.
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`, `crate::core`, or `crate::db`.

use rand::rngs::OsRng;
use rand::seq::SliceRandom as _;
use std::sync::OnceLock;

/// Raw EFF Large Wordlist embedded at compile time.
///
/// Format: one entry per line — `DDDDD\tword` (5-digit dice roll, tab, word).
const WORDLIST_RAW: &str = include_str!("../../data/eff-wordlist.txt");

/// Returns a slice of all words parsed from [`WORDLIST_RAW`].
///
/// Parsing is performed once on first call and the result is cached. The
/// parsed slice is `'static` because it borrows from the embedded constant.
fn words() -> &'static [&'static str] {
    static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        WORDLIST_RAW
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1))
            .collect()
    })
}

/// Generates a Diceware passphrase of `word_count` words separated by spaces.
///
/// Uses [`OsRng`] (OS CSPRNG) — never a seeded or deterministic RNG.
/// [`SliceRandom::choose`] uses rejection sampling internally, eliminating
/// modulo bias for non-power-of-2 wordlist sizes.
///
/// # Panics
/// Panics if the embedded word list is empty (structurally impossible).
pub fn suggest_passphrase(word_count: usize) -> String {
    let w = words();
    let mut rng = OsRng;
    let chosen: Vec<&str> = (0..word_count)
        .map(|_| *w.choose(&mut rng).expect("word list must not be empty"))
        .collect();
    chosen.join(" ")
}

// ---------------------------------------------------------------------------
// Tests (T005)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordlist_has_7776_entries() {
        assert_eq!(words().len(), 7776);
    }

    #[test]
    fn suggest_4_words_has_3_spaces() {
        assert_eq!(suggest_passphrase(4).matches(' ').count(), 3);
    }

    #[test]
    fn suggest_is_non_empty() {
        assert!(!suggest_passphrase(4).is_empty());
    }

    #[test]
    fn two_suggestions_differ() {
        // Probabilistic: P(collision for 4 words) = 1/7776^4 ≈ 2.8e-16
        assert_ne!(suggest_passphrase(4), suggest_passphrase(4));
    }
}
