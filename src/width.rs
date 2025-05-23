//! Helper functions related to string or grapheme width.

use unicode_segmentation::UnicodeSegmentation;

#[cfg(feature = "fish")]
use crate::widecharwidth::char_width;

/// Returns the width of a str `s`, breaking the string down into multiple [graphemes](https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries).
/// This takes into account some things like [joiners](https://unicode-explorer.com/c/200D) when calculating width.
#[inline]
pub fn str_width(s: &str) -> usize {
    UnicodeSegmentation::graphemes(s, true)
        .map(grapheme_width)
        .sum()
}

/// Returns the width of a single grapheme `g`. This takes into account some things like
/// [joiners](https://unicode-explorer.com/c/200D) when calculating width.
///
/// Note that while you *can* pass in an entire string, this function assumes you are passing in
/// just a single grapheme (e.g. `"a"`, `"💎"`, `"大"`, `"🇨🇦"`), and therefore makes no attempt in
/// splitting the string into its individual graphemes.
#[inline]
pub fn grapheme_width(g: &str) -> usize {
    if g.contains('\u{200d}') {
        2
    } else {
        #[cfg(feature = "fish")]
        {
            use unicode_width::UnicodeWidthChar;
            g.chars()
                .map(|c| {
                    if let Some(w) = char_width(c) {
                        w
                    } else {
                        UnicodeWidthChar::width(c).unwrap_or(0)
                    }
                })
                .sum()
        }

        #[cfg(not(feature = "fish"))]
        {
            use unicode_width::UnicodeWidthStr;
            UnicodeWidthStr::width(g)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_str_width() {
        // cSpell:disable
        assert_eq!(str_width("aaa"), 3);
        assert_eq!(str_width("a"), 1);
        assert_eq!(str_width("💎💎"), 4);
        assert_eq!(str_width("💎"), 2);
        assert_eq!(str_width("大大"), 4);
        assert_eq!(str_width("大"), 2);
        assert_eq!(str_width("🇨🇦🇨🇦"), 4);
        assert_eq!(str_width("🇨🇦"), 2);

        #[cfg(feature = "fish")]
        {
            assert_eq!(str_width("हिन्दी"), 3);
            assert_eq!(str_width("हि"), 1);
        }

        #[cfg(not(feature = "fish"))]
        {
            assert_eq!(str_width("हिन्दी"), 5);
            assert_eq!(str_width("हि"), 2);
        }
        // cSpell:enable;
    }

    #[test]
    fn test_grapheme_width() {
        // cSpell:disable
        assert_eq!(grapheme_width("a"), 1);
        assert_eq!(grapheme_width("💎"), 2);
        assert_eq!(grapheme_width("大"), 2);
        assert_eq!(grapheme_width("🇨🇦"), 2);
        #[cfg(feature = "fish")]
        assert_eq!(grapheme_width("हि"), 1);
        #[cfg(not(feature = "fish"))]
        assert_eq!(grapheme_width("हि"), 2);
        // cSpell:enable;
    }
}
