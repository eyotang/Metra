# Native font assets

`InterVariable.ttf` comes from the official rsms/inter distribution and is used by the legacy WebView. The GPUI desktop embeds the `MetraInter-*.ttf` static instances. Both are covered by `OFL-Inter.txt`. Windows uses a heavier body weight and an additional 850 weight for small badges and bubble percentages. CJK glyphs use system fallbacks selected for the interface language on Windows. `OFL-Noto-Sans-CJK.txt` is retained for previously distributed font assets.

Regenerate the native fonts with `uv run --no-project --with fonttools==4.64.0 python assets/fonts/build-native.py`.
