# Third-party attribution

- **System76 COSMIC applets 1.11** — panel formatting, locale-selection behavior,
  configuration defaults and panel sizing/anchoring patterns are adapted from
  `cosmic-applet-time/src/window.rs` and `cosmic-applets-config/src/time.rs`.
  Copyright 2023 System76, GPL-3.0-only.
  <https://github.com/pop-os/cosmic-applets/tree/epoch-1.11.0>
- **libcosmic** — native COSMIC widgets, configuration, theming and applet
  framework. Upstream licenses apply to each dependency.
  <https://github.com/pop-os/libcosmic>
- **TradingView**, **Google Calendar**, **TimeAPI.io**, **WorldTimeAPI**,
  **ipwho.is** and **ipapi.co** are remote data/API providers, not bundled data.
  The app identifies the actual source and does not claim affiliation.
- **flag-icons 7.6.0** — bundled full-color SVG flags, copyright 2013 Panayiotis
  Lipiridis, MIT. Pinned commit: `0d8909b00aa8418802a5abf2afa82356b129a7ef`.
  <https://github.com/lipis/flag-icons/tree/v7.6.0>
  The original artwork is in `data/flags/4x3/`; see `data/flags/LICENSE` for the
  full license and `data/flags/README.md` for provenance. The app applies rounded
  clipping and a theme-native outline. The installer copies the MIT notice into
  the per-user `share/licenses/cosmic-applet-finance-dashboard/` directory.
- The panel icon uses the installed COSMIC clock icon.
- **Japanese vocabulary** — queue, catalog, Jisho and Wiktionary lookup, kana
  pronunciation, British spelling, and idle detection are adapted from
  `cosmic-applet-japanese-words` and remain MPL-2.0. The dashboard integration
  in `src/japanese/mod.rs` and `src/japanese/store.rs` is GPL-3.0-only, like the
  rest of this applet. Jisho and English Wiktionary are remote services. Their
  responses are cached locally for the current word and the next 10 queued
  words and are not committed in this repository.
- **`data/japanese/words.json`** — bundled JLPT lists and JMdict glosses,
  CC BY-SA 4.0. Copyright the Electronic Dictionary Research and Development
  Group for the JMdict portions. See `data/japanese/WORDS.md`. The file is not
  relicensed. The installer copies that attribution beside the flag-icons notice.
