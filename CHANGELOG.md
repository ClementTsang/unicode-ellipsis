# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.0

### Breaking Changes

- [#6](https://github.com/ClementTsang/unicode-ellipsis/pull/6): Update unicode dependencies,
  and modify how we calculate some widths to be based on [fish's approach.](https://github.com/ridiculousfish/widecharwidth).
  - This can be disabled by disabling the `fish` feature.

## 0.2.0

### Feature

- [#1](https://github.com/ClementTsang/unicode-ellipsis/pull/1): Add `truncate_str_leading`, return `Cow` instead of `String`.

## 0.1.4

### Other

- Documentation

## 0.1.3

### Other

- Expose grapheme width function.
- Documentation

### 0.1.0

Initial release.
