# Metra UI fonts

Metra ships local font files so its WebView UI uses the same glyph metrics on macOS and Windows without contacting a font CDN at runtime.

- `InterVariable.woff2`: unmodified Inter 4.1 variable webfont for Latin text and numbers.
- `NotoSans*-UI-*.woff2`: Google Fonts `text=` subsets of Noto Sans SC, JP, and KR 2.004 for Metra's current UI copy.
- `font-manifest.json`: pinned hashes, source revisions, requested characters, CSS ranges, WOFF2 character maps, and the effective browser coverage where both agree.
- `OFL-*.txt`: SIL Open Font License 1.1 notices distributed with the fonts.

Japanese and Korean text uses its regional Noto glyphs first, with the bundled Simplified Chinese subset as a deterministic fallback for Chinese diagnostics. Arbitrary user-entered bubble labels may still use an operating-system fallback when they contain characters outside the committed UI subset. Native tray menus always use the operating system font.

After changing localized or native user-facing copy, regenerate the subsets with:

```bash
node --experimental-strip-types scripts/fetch-ui-fonts.mjs
npm run verify:fonts
```

The generation step needs network access. Normal development, CI, application startup, and release builds are fully offline with respect to fonts.

The generator rejects a different Google Fonts revision and also rejects changed bytes for an unchanged shard. Use `--refresh-lock` only after intentionally reviewing a regenerated font revision; CI always parses the committed WOFF2 character maps directly.
