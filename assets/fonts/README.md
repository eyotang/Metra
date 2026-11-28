# Native font assets

`InterVariable.ttf` comes from the official rsms/inter distribution and is used by the legacy WebView. The GPUI desktop embeds the `MetraInter-*.ttf` static instances so its native text backends can select the same weights. Both are covered by `OFL-Inter.txt`. CJK glyphs use system fallbacks. `OFL-Noto-Sans-CJK.txt` is retained for previously distributed font assets.

Regenerate the native fonts with `uv run --no-project --with fonttools==4.64.0 python assets/fonts/build-native.py`.
