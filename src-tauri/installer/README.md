# Windows installer artwork

NSIS uses the committed 24-bit `sidebar.bmp` (328 × 628) on its welcome/finish
pages and `header.bmp` (300 × 114) on the other pages. These are twice the logical
NSIS dimensions for sharper display scaling. The source is
`tools/prepare-installer-artwork.py`, using the existing application icon.
To regenerate on Linux, install Pillow, fontconfig and Noto Sans, then run:

```sh
python3 tools/prepare-installer-artwork.py
```

Normal builds need no image tools. Keep native accessible controls and Tauri's
default installer template, DPI handling, upgrade/uninstall and integrity checks.
Welcome/finish copy is in `appearance.nsh`; it is included by the existing hook
before Tauri expands the MUI pages. English and Russian follow Windows' language
(English fallback), without a separate language-selection popup.

Keep generated preview executables and screenshots in ignored `artifacts/`.
