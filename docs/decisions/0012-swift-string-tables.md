# 0012: Strings in Swift tables, not resource bundles

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

The app had English prose in view bodies. Translations should be possible later
without touching views, and the same sources must build through XcodeGen and
through xtool from Linux. The core package runs its tests on Linux, where the
app target cannot be built.

Apple's usual paths are `.lproj`/`.strings` files or a String Catalog. Both are
resources: they must be copied into the bundle by each build system, and the
Linux tests cannot read the app bundle. A missing resource silently leaves text
in the source language.

## Decision

- User-facing text is a `TextKey` case with a per-language table in
  `AbsoluteTouchCore`. Views call `Copy.text(_:)` or `Copy.format(_:_:)` and
  contain no prose.
- `Localization.current` resolves `Locale.preferredLanguages` by primary
  subtag and falls back to English. Formatting takes an explicit `Locale`, so
  decimals follow the device and tests stay deterministic.
- Placeholders are positional (`%1$@`, `%2$d`) so a translation may reorder
  them; `String(format:locale:arguments:)` fills them in.
- The tables are Swift source, so both build systems package no localization
  resources and `swift test` can prove completeness on Linux: every key must
  exist in every `AppLanguage`, with no stray keys, empty values, or surrounding
  whitespace.
- Adding a language is one enum case, one table, and one branch in
  `Localization.table(for:)`. The tests fail until the table is complete.

## Consequences

- No Xcode string extraction, plural rules, or `.stringsdict` support. If a
  language needs plural forms, the plural logic belongs in Swift next to the
  key, with tests, rather than in a resource format.
- Translators edit Swift files. The keys are documented with their placeholders
  and context, which is the information a translator needs.
- A language cannot ship half-translated: the build stays green only when every
  key is present.
- The brand name and numeric-only labels (ratio `16:9`, percentages) are not
  translated.
