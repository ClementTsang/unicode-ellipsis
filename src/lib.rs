//! A crate to truncate Unicode strings to a certain width, automatically adding an ellipsis if the string is too long.
//!
//! Additionally contains some helper functions regarding string and grapheme width.

mod width;
pub use width::*;

use std::{borrow::Cow, num::NonZeroUsize};

use unicode_segmentation::UnicodeSegmentation;

enum AsciiIterationResult {
    Complete(String),
    Remaining(usize),
}

macro_rules! add_ellipsis {
    ($text:expr) => {{
        const SIZE_OF_ELLIPSIS: usize = 3;
        let mut ret = String::with_capacity($text.len() + SIZE_OF_ELLIPSIS);

        if REVERSE {
            ret.push('…');
        }

        ret.push_str($text);

        if !REVERSE {
            ret.push('…');
        }

        ret
    }};
}

/// Greedily add characters to the output until a non-ASCII grapheme is found, or
/// the output is `width` long.
#[inline]
fn greedy_ascii_add<const REVERSE: bool>(
    content: &str,
    width: NonZeroUsize,
) -> AsciiIterationResult {
    let width: usize = width.into();
    debug_assert!(width < content.len());

    let mut bytes_consumed = 0;

    macro_rules! current_byte {
        () => {
            if REVERSE {
                content.as_bytes()[content.len() - 1 - bytes_consumed]
            } else {
                content.as_bytes()[bytes_consumed]
            }
        };
    }

    macro_rules! consumed_slice {
        () => {
            // SAFETY: The use of `get_unchecked` is safe here because
            // (`bytes_consumed` < `width`) && (`width` < `content.len()`)
            // and `bytes_consumed` is at an ascii boundary.
            unsafe {
                if REVERSE {
                    content.get_unchecked(content.len() - bytes_consumed..)
                } else {
                    content.get_unchecked(..bytes_consumed)
                }
            }
        };
    }

    while bytes_consumed < width - 1 {
        let current_byte = current_byte!();
        if current_byte.is_ascii() {
            bytes_consumed += 1;
        } else {
            debug_assert!(consumed_slice!().is_ascii());
            return AsciiIterationResult::Remaining(bytes_consumed);
        }
    }

    // If we made it all the way through, then we probably hit the width limit.
    debug_assert!(consumed_slice!().is_ascii());

    if current_byte!().is_ascii() {
        AsciiIterationResult::Complete(add_ellipsis!(consumed_slice!()))
    } else {
        AsciiIterationResult::Remaining(bytes_consumed)
    }
}

/// Handle the remaining characters in a [`&str`].
#[inline]
fn handle_remaining<const REVERSE: bool>(
    content: &str,
    mut bytes_consumed: usize,
    width: usize,
) -> Cow<'_, str> {
    // SAFETY: The use of `get_unchecked` is safe here because
    // (`bytes_consumed` < `width`) && (`width` < `content.len()`)
    // and `bytes_consumed` is at an ASCII boundary.
    let content_remaining = unsafe {
        if REVERSE {
            content.get_unchecked(..=content.len() - 1 - bytes_consumed)
        } else {
            content.get_unchecked(bytes_consumed..)
        }
    };

    let mut curr_width = bytes_consumed;
    let mut exceeded_width = false;

    // This tracks the length of the last added string - note this does NOT match the grapheme *width*.
    // Since the previous characters are always ASCII, this is always initialized as 1, unless the string
    // is empty.
    let mut last_grapheme_len = if curr_width == 0 { 0 } else { 1 };

    // Cases to handle:
    // - Completes adding the entire string.
    // - Adds a character up to the boundary, then fails.
    // - Adds a character not up to the boundary, then fails.
    // Inspired by https://tomdebruijn.com/posts/rust-string-length-width-calculations/
    macro_rules! measure_graphemes {
        ($graphemes:expr) => {
            for g in $graphemes {
                let g_width = grapheme_width(g);

                if curr_width + g_width <= width {
                    curr_width += g_width;
                    last_grapheme_len = g.len();
                    bytes_consumed += last_grapheme_len;
                } else {
                    exceeded_width = true;
                    break;
                }
            }
        };
    }

    let graphemes = UnicodeSegmentation::graphemes(content_remaining, true);

    if REVERSE {
        measure_graphemes!(graphemes.rev())
    } else {
        measure_graphemes!(graphemes)
    }

    macro_rules! consumed_slice {
        () => {
            // SAFETY: The use of `get_unchecked` is safe here because
            // `bytes_consumed` is tracking the lengths of graphemes contained
            // within `content` and `bytes_consumed` is at a grapheme boundary.
            unsafe {
                if REVERSE {
                    content.get_unchecked(content.len() - bytes_consumed..)
                } else {
                    content.get_unchecked(..bytes_consumed)
                }
            }
        };
    }

    if exceeded_width {
        if curr_width == width {
            // Remove the last consumed grapheme cluster.
            bytes_consumed -= last_grapheme_len;
        }

        add_ellipsis!(consumed_slice!()).into()
    } else {
        consumed_slice!().into()
    }
}

/// Truncates a string to the specified width with a trailing ellipsis character.
#[inline]
pub fn truncate_str(content: &str, width: usize) -> Cow<'_, str> {
    truncate_str_inner::<false>(content, width)
}

/// Truncates a string to the specified width with a leading ellipsis character.
#[inline]
pub fn truncate_str_leading(content: &str, width: usize) -> Cow<'_, str> {
    truncate_str_inner::<true>(content, width)
}

/// A const-generic function to actually handle the
#[inline]
fn truncate_str_inner<const REVERSE: bool>(content: &str, width: usize) -> Cow<'_, str> {
    if content.len() <= width {
        // If the entire string fits in the width, then we just
        // need to copy the entire string over.

        content.into()
    } else if let Some(nz_width) = NonZeroUsize::new(width) {
        // What we are essentially doing is optimizing for the case that
        // most, if not all of the string is ASCII. As such:
        // - Step through each byte until (width - 1) is hit or we find a non-ASCII
        //   byte.
        // - If the byte is ASCII, then add it.
        //
        // If we didn't get a complete truncated string, then continue on treating the rest as graphemes.

        match greedy_ascii_add::<REVERSE>(content, nz_width) {
            AsciiIterationResult::Complete(text) => text.into(),
            AsciiIterationResult::Remaining(bytes_consumed) => {
                handle_remaining::<REVERSE>(content, bytes_consumed, width)
            }
        }
    } else {
        "".into()
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
    fn test_truncate_str_leading() {
        let cpu_header = "▲CPU(c)";

        assert_eq!(
            truncate_str_leading(cpu_header, 8),
            cpu_header,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str_leading(cpu_header, 7),
            cpu_header,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str_leading(cpu_header, 6), "…PU(c)");
        assert_eq!(truncate_str_leading(cpu_header, 5), "…U(c)");
        assert_eq!(truncate_str_leading(cpu_header, 4), "…(c)");
        assert_eq!(truncate_str_leading(cpu_header, 1), "…");
        assert_eq!(truncate_str_leading(cpu_header, 0), "");
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
    fn test_truncate_ascii_leading() {
        let content = "0123456";

        assert_eq!(
            truncate_str_leading(content, 8),
            content,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str_leading(content, 7),
            content,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str_leading(content, 6), "…23456");
        assert_eq!(truncate_str_leading(content, 5), "…3456");
        assert_eq!(truncate_str_leading(content, 4), "…456");
        assert_eq!(truncate_str_leading(content, 1), "…");
        assert_eq!(truncate_str_leading(content, 0), "");
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
    fn test_truncate_cjk_leading() {
        let cjk = "施氏食獅史";

        assert_eq!(
            truncate_str_leading(cjk, 11),
            cjk,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str_leading(cjk, 10),
            cjk,
            "should match base string as there is enough room"
        );

        assert_eq!(truncate_str_leading(cjk, 9), "…氏食獅史");
        assert_eq!(truncate_str_leading(cjk, 8), "…食獅史");
        assert_eq!(truncate_str_leading(cjk, 2), "…");
        assert_eq!(truncate_str_leading(cjk, 1), "…");
        assert_eq!(truncate_str_leading(cjk, 0), "");

        let cjk_2 = "你好嗎";
        assert_eq!(truncate_str_leading(cjk_2, 5), "…好嗎");
        assert_eq!(truncate_str_leading(cjk_2, 4), "…嗎");
        assert_eq!(truncate_str_leading(cjk_2, 3), "…嗎");
        assert_eq!(truncate_str_leading(cjk_2, 2), "…");
        assert_eq!(truncate_str_leading(cjk_2, 1), "…");
        assert_eq!(truncate_str_leading(cjk_2, 0), "");
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
    fn test_truncate_mixed_one_leading() {
        let test = "Test (施氏食獅史) Test";

        assert_eq!(
            truncate_str_leading(test, 30),
            test,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str_leading(test, 22),
            test,
            "should match base string as there is just enough room"
        );

        assert_eq!(
            truncate_str_leading(test, 21),
            "…st (施氏食獅史) Test",
            "should truncate the T and replace the e with ellipsis"
        );

        assert_eq!(truncate_str_leading(test, 20), "…t (施氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 19), "… (施氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 18), "…(施氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 17), "…施氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 16), "…氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 15), "…氏食獅史) Test");
        assert_eq!(truncate_str_leading(test, 14), "…食獅史) Test");
        assert_eq!(truncate_str_leading(test, 13), "…食獅史) Test");
        assert_eq!(truncate_str_leading(test, 8), "…) Test");
        assert_eq!(truncate_str_leading(test, 7), "…) Test");
        assert_eq!(truncate_str_leading(test, 6), "… Test");
        assert_eq!(truncate_str_leading(test, 5), "…Test");
        assert_eq!(truncate_str_leading(test, 4), "…est");
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
    fn test_truncate_mixed_two_leading() {
        let test = "Test (施氏abc食abc獅史) Test";

        assert_eq!(
            truncate_str_leading(test, 30),
            test,
            "should match base string as there is extra room"
        );

        assert_eq!(
            truncate_str_leading(test, 28),
            test,
            "should match base string as there is just enough room"
        );

        assert_eq!(truncate_str_leading(test, 26), "…t (施氏abc食abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 21), "…氏abc食abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 20), "…abc食abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 16), "…食abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 15), "…abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 14), "…abc獅史) Test");
        assert_eq!(truncate_str_leading(test, 11), "…獅史) Test");
        assert_eq!(truncate_str_leading(test, 10), "…史) Test");
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

    #[test]
    fn test_truncate_flags_leading() {
        let flag = "🇨🇦";
        assert_eq!(truncate_str_leading(flag, 3), flag);
        assert_eq!(truncate_str_leading(flag, 2), flag);
        assert_eq!(truncate_str_leading(flag, 1), "…");
        assert_eq!(truncate_str_leading(flag, 0), "");

        let flag_text = "🇨🇦 oh";
        assert_eq!(truncate_str_leading(flag_text, 6), flag_text);
        assert_eq!(truncate_str_leading(flag_text, 5), flag_text);
        assert_eq!(truncate_str_leading(flag_text, 4), "… oh");

        let flag_text_wrap = "!🇨🇦!";
        assert_eq!(truncate_str_leading(flag_text_wrap, 6), flag_text_wrap);
        assert_eq!(truncate_str_leading(flag_text_wrap, 4), flag_text_wrap);
        assert_eq!(truncate_str_leading(flag_text_wrap, 3), "…!");
        assert_eq!(truncate_str_leading(flag_text_wrap, 2), "…!");
        assert_eq!(truncate_str_leading(flag_text_wrap, 1), "…");

        let flag_cjk = "🇨🇦加拿大";
        assert_eq!(truncate_str_leading(flag_cjk, 9), flag_cjk);
        assert_eq!(truncate_str_leading(flag_cjk, 8), flag_cjk);
        assert_eq!(truncate_str_leading(flag_cjk, 7), "…加拿大");
        assert_eq!(truncate_str_leading(flag_cjk, 6), "…拿大");
        assert_eq!(truncate_str_leading(flag_cjk, 5), "…拿大");
        assert_eq!(truncate_str_leading(flag_cjk, 4), "…大");

        let flag_mix = "🇨🇦加gaa拿naa大daai🇨🇦";
        assert_eq!(truncate_str_leading(flag_mix, 20), flag_mix);
        assert_eq!(truncate_str_leading(flag_mix, 19), "…加gaa拿naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 18), "…gaa拿naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 17), "…gaa拿naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 15), "…a拿naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 14), "…拿naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 13), "…naa大daai🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 3), "…🇨🇦");
        assert_eq!(truncate_str_leading(flag_mix, 2), "…");
        assert_eq!(truncate_str_leading(flag_mix, 1), "…");
        assert_eq!(truncate_str_leading(flag_mix, 0), "");
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
    fn test_truncate_hindi_leading() {
        // cSpell:disable
        let test = "हिन्दी";
        assert_eq!(truncate_str_leading(test, 10), test);
        assert_eq!(truncate_str_leading(test, 6), "हिन्दी");
        assert_eq!(truncate_str_leading(test, 5), "हिन्दी");
        assert_eq!(truncate_str_leading(test, 4), "…न्दी");
        assert_eq!(truncate_str_leading(test, 3), "…दी");
        assert_eq!(truncate_str_leading(test, 2), "…");
        assert_eq!(truncate_str_leading(test, 1), "…");
        assert_eq!(truncate_str_leading(test, 0), "");
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

    #[test]
    fn truncate_emoji_leading() {
        let heart_1 = "♥";
        assert_eq!(truncate_str_leading(heart_1, 2), heart_1);
        assert_eq!(truncate_str_leading(heart_1, 1), heart_1);
        assert_eq!(truncate_str_leading(heart_1, 0), "");

        let heart_2 = "❤";
        assert_eq!(truncate_str_leading(heart_2, 2), heart_2);
        assert_eq!(truncate_str_leading(heart_2, 1), heart_2);
        assert_eq!(truncate_str_leading(heart_2, 0), "");

        // This one has a U+FE0F modifier at the end, and is thus considered "emoji-presentation",
        // see https://github.com/fish-shell/fish-shell/issues/10461#issuecomment-2079624670.
        // This shouldn't really be a common issue in a terminal but eh.
        let heart_emoji_pres = "❤️";
        assert_eq!(truncate_str_leading(heart_emoji_pres, 2), heart_emoji_pres);
        assert_eq!(truncate_str_leading(heart_emoji_pres, 1), "…");
        assert_eq!(truncate_str_leading(heart_emoji_pres, 0), "");

        let emote = "💎";
        assert_eq!(truncate_str_leading(emote, 2), emote);
        assert_eq!(truncate_str_leading(emote, 1), "…");
        assert_eq!(truncate_str_leading(emote, 0), "");

        let family = "👨‍👨‍👧‍👦";
        assert_eq!(truncate_str_leading(family, 2), family);
        assert_eq!(truncate_str_leading(family, 1), "…");
        assert_eq!(truncate_str_leading(family, 0), "");

        let scientist = "👩‍🔬";
        assert_eq!(truncate_str_leading(scientist, 2), scientist);
        assert_eq!(truncate_str_leading(scientist, 1), "…");
        assert_eq!(truncate_str_leading(scientist, 0), "");
    }
}
