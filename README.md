# unicode-ellipsis

[<img src="https://img.shields.io/crates/v/unicode-ellipsis.svg?style=flat-square" alt="crates.io link">](https://crates.io/crates/unicode-ellipsis)
[<img src="https://docs.rs/unicode-ellipsis/badge.svg">](https://docs.rs/unicode-ellipsis)

A crate to truncate Unicode strings to a certain width, automatically adding an ellipsis if the string is too long. Also
has some helper functions around Unicode grapheme and string width.

## Usage

An example of usage:

```rust
fn main() {
    let content = "0123456";

    assert_eq!(truncate_str(content, 6), "01234…");
    assert_eq!(truncate_str(content, 5), "0123…");
    assert_eq!(truncate_str(content, 4), "012…");
    assert_eq!(truncate_str(content, 1), "…");
    assert_eq!(truncate_str(content, 0), "");
}
```

## Licensing

Dual-licensed under Apache 2.0 and MIT.
