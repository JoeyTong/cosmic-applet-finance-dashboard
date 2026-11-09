# Flat flag artwork

The 256 SVG files in `4x3/` are unmodified two-letter flag assets from
[flag-icons 7.6.0](https://github.com/lipis/flag-icons/tree/v7.6.0), pinned to commit
`0d8909b00aa8418802a5abf2afa82356b129a7ef`.

- Upstream directory: `flags/4x3/`
- License: MIT, copyright 2013 Panayiotis Lipiridis; see `LICENSE`.
- Source archive: `https://codeload.github.com/lipis/flag-icons/tar.gz/0d8909b00aa8418802a5abf2afa82356b129a7ef`
- Archive SHA-256: `6db6e6aac7c266cb35ff94dd86f34f4359f959e551e53bfd39ffb6fc51fb516d`
- The upstream `xx.svg` placeholder is excluded. Unknown codes use a neutral,
  theme-native dash badge instead.

`src/flags.rs` embeds these files in the executable, adds a rounded clipping mask
and a subtle COSMIC-colored outline, and caches each SVG handle on first use.
The original full-color artwork is retained. No runtime file lookup or flag
download is needed. Keep the asset registry in sync when deliberately updating
this bundle; a test checks that every bundled SVG has a registry entry.

The installer includes the MIT notice in
`$PREFIX/share/licenses/cosmic-applet-finance-dashboard/flag-icons-LICENSE` (the default
prefix is `~/.local`).
