//! A crate to truncate Unicode strings to a certain width, automatically adding an ellipsis if the string is too long.
//!
//! Additionally contains some helper functions regarding string width.

use std::num::NonZeroUsize;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns the width of a str `s`. This takes into account some things like
/// joiners when calculating width.
#[inline]
pub fn str_width(s: &str) -> usize {
    UnicodeSegmentation::graphemes(s, true)
        .map(|g| {
            if g.contains('\u{200d}') {
                2
            } else {
                UnicodeWidthStr::width(g)
            }
        })
        .sum()
}

/// Returns the width of grapheme `g`. This takes into account some things like
/// joiners when calculating width.
///
/// Note that while you *can* pass in an entire string, the point is to check
/// individual graphemes (e.g. `"a"`, `"💎"`, `"大"`, `"🇨🇦"`).
#[inline]
fn grapheme_width(g: &str) -> usize {
    if g.contains('\u{200d}') {
        2
    } else {
        UnicodeWidthStr::width(g)
    }
}

enum AsciiIterationResult {
    Complete,
    Remaining(usize),
}

/// Greedily add characters to the output until a non-ASCII grapheme is found, or
/// the output is `width` long.
#[inline]
fn greedy_ascii_add(content: &str, width: NonZeroUsize) -> (String, AsciiIterationResult) {
    let width: usize = width.into();

    const SIZE_OF_ELLIPSIS: usize = 3;
    let mut text = Vec::with_capacity(width - 1 + SIZE_OF_ELLIPSIS);

    let s = content.as_bytes();

    let mut current_index = 0;

    while current_index < width - 1 {
        let current_byte = s[current_index];
        if current_byte.is_ascii() {
            text.push(current_byte);
            current_index += 1;
        } else {
            debug_assert!(text.is_ascii());

            let current_index = AsciiIterationResult::Remaining(current_index);

            // SAFETY: This conversion is safe to do unchecked, we only push ASCII characters up to
            // this point.
            let current_text = unsafe { String::from_utf8_unchecked(text) };

            return (current_text, current_index);
        }
    }

    // If we made it all the way through, then we probably hit the width limit.
    debug_assert!(text.is_ascii());

    let current_index = if s[current_index].is_ascii() {
        let mut ellipsis = [0; SIZE_OF_ELLIPSIS];
        '…'.encode_utf8(&mut ellipsis);
        text.extend_from_slice(&ellipsis);
        AsciiIterationResult::Complete
    } else {
        AsciiIterationResult::Remaining(current_index)
    };

    // SAFETY: This conversion is safe to do unchecked, we only push ASCII characters up to
    // this point.
    let current_text = unsafe { String::from_utf8_unchecked(text) };

    (current_text, current_index)
}

/// Truncates a string to the specified width with an ellipsis character.
#[inline]
pub fn truncate_str(content: &str, width: usize) -> String {
    if content.len() <= width {
        // If the entire string fits in the width, then we just
        // need to copy the entire string over.

        content.to_owned()
    } else if let Some(nz_width) = NonZeroUsize::new(width) {
        // What we are essentially doing is optimizing for the case that
        // most, if not all of the string is ASCII. As such:
        // - Step through each byte until (width - 1) is hit or we find a non-ascii
        //   byte.
        // - If the byte is ascii, then add it.
        //
        // If we didn't get a complete truncated string, then continue on treating the rest as graphemes.

        let (mut text, res) = greedy_ascii_add(content, nz_width);
        match res {
            AsciiIterationResult::Complete => text,
            AsciiIterationResult::Remaining(current_index) => {
                let mut curr_width = text.len();
                let mut early_break = false;

                // This tracks the length of the last added string - note this does NOT match the grapheme *width*.
                // Since the previous characters are always ASCII, this is always initialized as 1, unless the string
                // is empty.
                let mut last_added_str_len = if text.is_empty() { 0 } else { 1 };

                // Cases to handle:
                // - Completes adding the entire string.
                // - Adds a character up to the boundary, then fails.
                // - Adds a character not up to the boundary, then fails.
                // Inspired by https://tomdebruijn.com/posts/rust-string-length-width-calculations/
                for g in UnicodeSegmentation::graphemes(&content[current_index..], true) {
                    let g_width = grapheme_width(g);

                    if curr_width + g_width <= width {
                        curr_width += g_width;
                        last_added_str_len = g.len();
                        text.push_str(g);
                    } else {
                        early_break = true;
                        break;
                    }
                }

                if early_break {
                    if curr_width == width {
                        // Remove the last grapheme cluster added.
                        text.truncate(text.len() - last_added_str_len);
                    }
                    text.push('…');
                }
                text
            }
        }
    } else {
        String::default()
    }
}

#[cfg(test)]
mod tests {
    // TODO: Testing against Fish's script [here](https://github.com/ridiculousfish/widecharwidth) might be useful.

    use super::*;

    #[test]
    fn test_truncate_str() {
        let cpu_header = "CPU(c)▲";

        assert_eq!(
            truncate_str(cpu_header, 8),
            cpu_header,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str(cpu_header, 7),
            cpu_header,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str(cpu_header, 6), "CPU(c…");
        assert_eq!(truncate_str(cpu_header, 5), "CPU(…");
        assert_eq!(truncate_str(cpu_header, 4), "CPU…");
        assert_eq!(truncate_str(cpu_header, 1), "…");
        assert_eq!(truncate_str(cpu_header, 0), "");
    }

    #[test]
    fn test_truncate_ascii() {
        let content = "0123456";

        assert_eq!(
            truncate_str(content, 8),
            content,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str(content, 7),
            content,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str(content, 6), "01234…");
        assert_eq!(truncate_str(content, 5), "0123…");
        assert_eq!(truncate_str(content, 4), "012…");
        assert_eq!(truncate_str(content, 1), "…");
        assert_eq!(truncate_str(content, 0), "");
    }

    #[test]
    fn test_truncate_cjk() {
        let cjk = "施氏食獅史";

        assert_eq!(
            truncate_str(cjk, 11),
            cjk,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str(cjk, 10),
            cjk,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str(cjk, 9), "施氏食獅…");
        assert_eq!(truncate_str(cjk, 8), "施氏食…");
        assert_eq!(truncate_str(cjk, 2), "…");
        assert_eq!(truncate_str(cjk, 1), "…");
        assert_eq!(truncate_str(cjk, 0), "");

        let cjk_2 = "你好嗎";
        assert_eq!(truncate_str(cjk_2, 5), "你好…");
        assert_eq!(truncate_str(cjk_2, 4), "你…");
        assert_eq!(truncate_str(cjk_2, 3), "你…");
        assert_eq!(truncate_str(cjk_2, 2), "…");
        assert_eq!(truncate_str(cjk_2, 1), "…");
        assert_eq!(truncate_str(cjk_2, 0), "");
    }

    #[test]
    fn test_truncate_mixed_one() {
        let test = "Test (施氏食獅史) Test";

        assert_eq!(
            truncate_str(test, 30),
            test,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str(test, 22),
            test,
            "should match base string as there is just enough room"
        );

        assert_eq!(
            truncate_str(test, 21),
            "Test (施氏食獅史) Te…",
            "should truncate the t and replace the s with ellipsis"
        );

        assert_eq!(truncate_str(test, 20), "Test (施氏食獅史) T…");
        assert_eq!(truncate_str(test, 19), "Test (施氏食獅史) …");
        assert_eq!(truncate_str(test, 18), "Test (施氏食獅史)…");
        assert_eq!(truncate_str(test, 17), "Test (施氏食獅史…");
        assert_eq!(truncate_str(test, 16), "Test (施氏食獅…");
        assert_eq!(truncate_str(test, 15), "Test (施氏食獅…");
        assert_eq!(truncate_str(test, 14), "Test (施氏食…");
        assert_eq!(truncate_str(test, 13), "Test (施氏食…");
        assert_eq!(truncate_str(test, 8), "Test (…");
        assert_eq!(truncate_str(test, 7), "Test (…");
        assert_eq!(truncate_str(test, 6), "Test …");
        assert_eq!(truncate_str(test, 5), "Test…");
        assert_eq!(truncate_str(test, 4), "Tes…");
    }

    #[test]
    fn test_truncate_mixed_two() {
        let test = "Test (施氏abc食abc獅史) Test";

        assert_eq!(
            truncate_str(test, 30),
            test,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str(test, 28),
            test,
            "should match base string as there is just enough room"
        );

        assert_eq!(truncate_str(test, 26), "Test (施氏abc食abc獅史) T…");
        assert_eq!(truncate_str(test, 21), "Test (施氏abc食abc獅…");
        assert_eq!(truncate_str(test, 20), "Test (施氏abc食abc…");
        assert_eq!(truncate_str(test, 16), "Test (施氏abc食…");
        assert_eq!(truncate_str(test, 15), "Test (施氏abc…");
        assert_eq!(truncate_str(test, 14), "Test (施氏abc…");
        assert_eq!(truncate_str(test, 11), "Test (施氏…");
        assert_eq!(truncate_str(test, 10), "Test (施…");
    }

    #[test]
    fn test_truncate_flags() {
        let flag = "🇨🇦";
        assert_eq!(truncate_str(flag, 3), flag);
        assert_eq!(truncate_str(flag, 2), flag);
        assert_eq!(truncate_str(flag, 1), "…");
        assert_eq!(truncate_str(flag, 0), "");

        let flag_text = "oh 🇨🇦";
        assert_eq!(truncate_str(flag_text, 6), flag_text);
        assert_eq!(truncate_str(flag_text, 5), flag_text);
        assert_eq!(truncate_str(flag_text, 4), "oh …");

        let flag_text_wrap = "!🇨🇦!";
        assert_eq!(truncate_str(flag_text_wrap, 6), flag_text_wrap);
        assert_eq!(truncate_str(flag_text_wrap, 4), flag_text_wrap);
        assert_eq!(truncate_str(flag_text_wrap, 3), "!…");
        assert_eq!(truncate_str(flag_text_wrap, 2), "!…");
        assert_eq!(truncate_str(flag_text_wrap, 1), "…");

        let flag_cjk = "加拿大🇨🇦";
        assert_eq!(truncate_str(flag_cjk, 9), flag_cjk);
        assert_eq!(truncate_str(flag_cjk, 8), flag_cjk);
        assert_eq!(truncate_str(flag_cjk, 7), "加拿大…");
        assert_eq!(truncate_str(flag_cjk, 6), "加拿…");
        assert_eq!(truncate_str(flag_cjk, 5), "加拿…");
        assert_eq!(truncate_str(flag_cjk, 4), "加…");

        let flag_mix = "🇨🇦加gaa拿naa大daai🇨🇦";
        assert_eq!(truncate_str(flag_mix, 20), flag_mix);
        assert_eq!(truncate_str(flag_mix, 19), "🇨🇦加gaa拿naa大daai…");
        assert_eq!(truncate_str(flag_mix, 18), "🇨🇦加gaa拿naa大daa…");
        assert_eq!(truncate_str(flag_mix, 17), "🇨🇦加gaa拿naa大da…");
        assert_eq!(truncate_str(flag_mix, 15), "🇨🇦加gaa拿naa大…");
        assert_eq!(truncate_str(flag_mix, 14), "🇨🇦加gaa拿naa…");
        assert_eq!(truncate_str(flag_mix, 13), "🇨🇦加gaa拿naa…");
        assert_eq!(truncate_str(flag_mix, 3), "🇨🇦…");
        assert_eq!(truncate_str(flag_mix, 2), "…");
        assert_eq!(truncate_str(flag_mix, 1), "…");
        assert_eq!(truncate_str(flag_mix, 0), "");
    }

    /// This might not be the best way to handle it, but this at least tests that it doesn't crash...
    #[test]
    fn test_truncate_hindi() {
        // cSpell:disable
        let test = "हिन्दी";
        assert_eq!(truncate_str(test, 10), test);
        assert_eq!(truncate_str(test, 6), "हिन्दी");
        assert_eq!(truncate_str(test, 5), "हिन्दी");
        assert_eq!(truncate_str(test, 4), "हिन्…");
        assert_eq!(truncate_str(test, 3), "हि…");
        assert_eq!(truncate_str(test, 2), "…");
        assert_eq!(truncate_str(test, 1), "…");
        assert_eq!(truncate_str(test, 0), "");
        // cSpell:enable
    }

    #[test]
    fn truncate_emoji() {
        let heart_1 = "♥";
        assert_eq!(truncate_str(heart_1, 2), heart_1);
        assert_eq!(truncate_str(heart_1, 1), heart_1);
        assert_eq!(truncate_str(heart_1, 0), "");

        let heart_2 = "❤";
        assert_eq!(truncate_str(heart_2, 2), heart_2);
        assert_eq!(truncate_str(heart_2, 1), heart_2);
        assert_eq!(truncate_str(heart_2, 0), "");

        // This one has a U+FE0F modifier at the end, and is thus considered "emoji-presentation",
        // see https://github.com/fish-shell/fish-shell/issues/10461#issuecomment-2079624670.
        // This shouldn't really be a common issue in a terminal but eh.
        let heart_emoji_pres = "❤️";
        assert_eq!(truncate_str(heart_emoji_pres, 2), heart_emoji_pres);
        assert_eq!(truncate_str(heart_emoji_pres, 1), "…");
        assert_eq!(truncate_str(heart_emoji_pres, 0), "");

        let emote = "💎";
        assert_eq!(truncate_str(emote, 2), emote);
        assert_eq!(truncate_str(emote, 1), "…");
        assert_eq!(truncate_str(emote, 0), "");

        let family = "👨‍👨‍👧‍👦";
        assert_eq!(truncate_str(family, 2), family);
        assert_eq!(truncate_str(family, 1), "…");
        assert_eq!(truncate_str(family, 0), "");

        let scientist = "👩‍🔬";
        assert_eq!(truncate_str(scientist, 2), scientist);
        assert_eq!(truncate_str(scientist, 1), "…");
        assert_eq!(truncate_str(scientist, 0), "");
    }
}
