# Finance Dashboard COSMIC Applet — project handoff

This file preserves project guidance and user decisions across machines and AI
sessions. Read it together with `README.md`; check the current source before
changing behavior. Update both documents after relevant user-approved changes.

## Purpose and collaboration

- Build a local, native COSMIC panel applet for a trader's daily reference:
  world clocks, Google Calendar, forex, commodities and public IP.
- **Discuss ambiguities and key decisions with the user through Q&A.** Existing
  decisions below should carry forward; ask before changing their meaning.
- Use native Rust/libcosmic controls and the current COSMIC theme throughout.
- Preserve the user's configured banner, clocks, watchlist, calendars and panel layout.
  Defaults in source are not a replacement for their saved preferences.
- Keep credentials and personal account data outside project documentation and
  source control. Describe configuration locations rather than copying contents.
- Keep this file focused on durable guidance; use `README.md` for detailed setup.

## Architecture and identifiers

- Source checkout folder, Cargo package, and installed binary:
  `cosmic-applet-finance-dashboard`.
- App/desktop-entry ID: `io.github.joeytong.cosmic-applet-finance-dashboard`.
- Panel and desktop name: Finance Dashboard.
- Rust edition 2024; manifest minimum Rust version: 1.93.
- Reference environment: CachyOS/Arch Linux, COSMIC 1.11, Wayland, 1920×1080.
  Recheck the target machine's desktop version and display dimensions.
- GPL-3.0-only; preserve the System76 attribution in source and `THIRD_PARTY.md`.

| File | Responsibility |
|---|---|
| `src/main.rs` | Panel launch, standalone `--preview`, public-service `--diagnose` |
| `src/app.rs` | Dashboard/settings widgets, click-only popup lifecycle, asynchronous jobs and refresh scheduling |
| `src/panel_time.rs` | Formatting compatible with the stock COSMIC clock |
| `src/analog.rs` | Scalable analog clock drawing |
| `src/config.rs` | Defaults, validation, private atomic settings writes, COSMIC preferences |
| `src/flags.rs`, `data/flags/` | Embedded flat flags, lazy SVG handles, artwork and MIT license |
| `src/login.rs` | Once-per-login automatic opening, including concurrent instances |
| `src/calendar.rs` | Google calendar/event fetching, pagination and date handling |
| `src/oauth.rs` | Desktop OAuth, PKCE/state, loopback callback, keyring, browser launch |
| `src/market.rs` | TradingView instrument mapping, parsing, precision and quote status |
| `src/network.rs` | HTTP client, time synchronization and public IP |
| `src/japanese/` | Daily word queue, bundled catalog, Jisho and Wiktionary lookup, British spelling, idle detection, progress and detail cache |
| `data/japanese/` | Embedded word list and CC BY-SA attribution |
| `data/*.desktop`, `scripts/` | Per-user installation, panel integration and removal |

## User-approved behavior

### Panel and dashboard

- Panel date/time comes directly from the OS and matches the stock COSMIC
  Date, Time & Calendar applet's formatting, locale, size and orientation.
  Read its existing `com.system76.CosmicAppletTime` configuration.
- The dashboard has no fixed size and no dashboard scrolling. It hugs the
  clocks, forex pairs, and calendar events at the readability floor: clock
  faces about 100px, centered in the clock card, forex rows about 32px, and the
  current text sizes. The public IP and forex column is only as wide as the
  flags, the longest pair or commodity label, the price, and the daily change.
  The top row is only as tall as the clocks; the public IP card uses that
  height. A shorter calendar grows to meet the markets card, and shorter
  quote rows and commodity tiles grow to meet the Japanese card.
  One clock row and one forex column are used until another row would pass
  the monitor; then a new row or column is added. If the content still does
  not fit, those items shrink until it does. The monitor size is read at
  startup and when the compositor reports a new output. If the panel is not
  on one named output, the smaller connected output is the limit.
  Clocks are upper left. Public IP is upper right and shares the markets
  column width. The two-week calendar is lower left. Forex and both
  commodities are lower right. Each calendar day box is at least three times
  the previous date-only height, and grows further when that day has more
  events. The Japanese card is under the calendar and
  stays bottom-aligned with commodities. Settings uses that same window and
  scrolls inside it.
- The dashboard popup has no title. The panel and desktop-entry name is
  Finance Dashboard.
- The centered, text-only banner initially reads **“Thrived Today, Leading
  Tomorrow”**. Use native COSMIC typography; the decorative gold dash was rejected.
  Settings provides **Save/Enter and Cancel**. Accept 1–160 single-line characters,
  preserve drafts on validation/write failure, and change saved settings only on
  Save/Enter. Long messages shrink and then ellipsize; a tooltip shows the full text.
  Existing `banner_message` values take precedence over the default.
- **Click-only opening:** left-click the panel clock to open or close the dashboard.
  Hover opening/closing, pointer tracking, delayed timers and separate Pin mode
  were deliberately removed, including `src/popup.rs`. Informational tooltips remain.
  Settings retains the existing popup surface.
- The dashboard stays open when the pointer or keyboard focus moves away.
  COSMIC's **focus-follows-cursor** is enabled on the reference machine; losing
  keyboard focus alone must never dismiss the dashboard.
- Outside clicks dismiss the dashboard, including the initial login
  dashboard. The user approved **dismiss only**: the first click in the surrounding
  desktop/work area is consumed; a second click interacts with the window beneath.
  Escape dismisses the dashboard when the applet has keyboard focus. The user
  also removed the dashboard/Settings × close button; keep those explicit
  dismissal methods instead.
- Automatically open **once per login**, after the panel is ready.
  Reopening on every panel/applet restart was explicitly rejected.
- Flags are flat full-color rectangles with subtle rounded corners and native
  outlines. Currency flags are a compact, overlapping pair with explicit positions;
  the public-IP flag is prominent. Embed the pinned flag-icons artwork and preserve
  its MIT notice/provenance in `data/flags/`; unknown codes use a neutral dash badge.
  Do not return to emoji flags or wrapping combined flag text runs.
- Saturdays, Sundays and days with enabled holiday events use the same subtle
  full-cell COSMIC-theme tint; retain event colors, markers and day selection.

### Data semantics and refresh

| Module | Required behavior |
|---|---|
| World clocks | Analog hour/minute hands only; add/edit/remove clocks in Settings. Sample OS every 30 s; public time sync at startup and hourly. |
| Calendar | This week plus next week, using COSMIC's first weekday; fetch at startup when connected and every 10 minutes, refresh on date/week changes. |
| Forex | Configurable pairs; fetch at startup and every 10 minutes. Show flags, symbol, description, price, signed daily absolute and percentage change. |
| Commodities | Fetch at startup and every 10 minutes; retain exact instrument meanings below. |
| Public IP | Fetch at startup and every minute; show egress IP, country, city and prominent flag. |
| Japanese | One word per local day for the selected library. No countdown. See the vocabulary section below. |

- Default clocks: Singapore (`Asia/Singapore`), GMT (`Etc/UTC`), New York
  (`America/New_York`, including DST), Tokyo (`Asia/Tokyo`). “GTM” was clarified
  as GMT; “EST” was clarified as New York with DST.
  The user-approved first-clock change to Singapore has already been applied to
  saved settings; do not repeat that migration or replace their five-clock layout.
- Existing clocks have **Edit → Save/Cancel** for both name and IANA time zone.
  Validate before saving, replace the clock in its existing position, and retain
  the draft on validation/write failure. Editing must not change saved settings
  until Save/Enter. Finish or cancel one edit before editing/removing another row.
- Default forex order: `USDCNH`, `USDJPY`, `USDHKD`, `JPYCNY`, `SGDCNY`, `GBPCNH`,
  `EURCNH`, `GBPUSD`, `GBPJPY`, `GBPSGD`, `GBPEUR`.
- Keep offshore **CNH distinct from onshore CNY**. Preserve provider precision,
  smaller final fractional digits where applicable, and red/green daily changes.
- `BRN1` means `ICEEUR:BRN1!`: front-month continuous ICE Brent futures,
  USD/barrel. `GOLD` means `TVC:GOLD`: spot XAU/USD, USD/troy ounce, not gold futures.

### Japanese vocabulary

- The card under the calendar has two centered lines. The first line shows the
  headword and reading at 45px. The second line shows the pronunciation and the
  British English gloss. Hovering shows the pronunciation source, each sense
  under its part of speech, and each example with the Japanese sentence above
  the English. No JLPT label, section title, separate Settings button, or
  countdown. **Next Word** is at the right of the card. The card's bottom lines
  up with the commodities card.
- Pronunciation on the main line is broad Tokyo IPA in slashes. Long vowels
  and small-tsu consonants use ː. The voiced velar stop is ɡ. Pitch accent and
  narrow vowel marks are removed, and Wiktionary text is normalized to this
  style. Senses and examples come from Jisho and English Wiktionary.
  Generated kana pronunciation is labeled. Do not invent examples.
- One automatic word per local date for the library currently selected. Several
  missed midnights still take one step, on the next launch or while the applet
  is running. The same word survives restarts that happen on the same local date.
- The step marks the word learned only after 30 minutes of active session time
  on that word. Active time pauses when logind reports idle, locked, or a
  non-active session. If the probe fails, time still counts. Below 30 minutes,
  the word is replaced and stays unlearned. **Next Word** marks it learned immediately
  and does not affect other libraries. Dashboard Refresh does not change the word.
- A library that is not selected does not accumulate time and does not rotate
  until it is selected. Each library has its own word, learned set, and day.
- Reset asks for confirmation and clears only the selected library. A completed
  library shows a completion line and does not rotate.
- Progress is stored in `$XDG_DATA_HOME/cosmic-applet-finance-dashboard/japanese-study.json`.
  Dictionary details are cached in `japanese-details.json` in the same directory.
  Do not import the Japanese Words applet's files. Leave that applet in place.
- The bundled gloss is shown with dictionary senses in British spelling.
  Jisho is the primary source for senses. Wiktionary supplies examples,
  backup senses, and dictionary pronunciation.
- Moved study sources keep their MPL-2.0 headers. `data/japanese/words.json`
  stays CC BY-SA 4.0. The dashboard package stays GPL-3.0-only.

- Free public snapshots were approved. Show delays, stale/last-known data,
  unavailable instruments and sources honestly. Bar timestamps are not trade
  timestamps; daily change is not change since the previous app refresh.
- Current sources: TradingView public scanner (`FX_IDC:` forex symbols);
  TimeAPI.io with WorldTimeAPI fallback; ipwho.is with ipapi.co fallback.
  See `README.md` for provider limitations and diagnostics.
- Public time correction affects world clocks only; never set the OS clock.
- Google scope is `calendar.readonly`. Include events and subscribed holiday
  calendars; **Google Tasks were explicitly excluded**. Holidays use distinct
  colors/diamond markers. Hover shows details; clicking opens Google Calendar.
- Keep requests asynchronous and module failures independent; refresh while
  the popup is closed. Retain clearly labeled last-known data on failure.

## Build and verification

Required tools/libraries include Rust/Cargo, a C toolchain, pkg-config, Wayland,
libxkbcommon, fontconfig and COSMIC's native rendering/X11 dependencies.
Installation scripts also use Python 3. Google integration needs `xdg-open`,
libsecret's `secret-tool`, an unlocked Secret Service keyring and the desktop
file-chooser portal. Package names vary by distribution.

Keep `Cargo.lock` and the pinned Git revisions/source URL spellings in
`Cargo.toml`. The patch setup matches the working COSMIC stack; moving upstream
HEADs previously caused incompatible dependency resolution. Dependency upgrades
are a deliberate task, not incidental cleanup.

Run from the project root for Rust changes:

```sh
rustup component add rustfmt clippy
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
```

- Use `cargo fmt --all` to apply formatting when needed.
- Add meaningful regression tests for logic changes, particularly popup races,
  session markers, OAuth validation, quote semantics and calendar boundaries.
  Documentation-only edits do not require rebuilding Rust.
- `target/release/cosmic-applet-finance-dashboard --diagnose` checks public services;
  it makes real network requests and prints the public IP. It does not test Google.
- `target/release/cosmic-applet-finance-dashboard --preview` opens the native dashboard
  as a regular window. Verify click toggling, dismissal and anchoring in the actual
  panel, and that hovering does not open or close the dashboard.
- When pointer automation is unavailable, ask the user to check interactions.
  On COSMIC, `cosmic-screenshot --interactive=false --notify=false --save-dir <dir>`
  can capture the current layout; use an existing temporary directory.

## Migration to another directory or computer

For a source-folder-only rename on the same machine, verify
`cargo build --release --locked` from the new directory. Cargo's generated
`target/` files can contain absolute build paths; let Cargo regenerate them.
The install/uninstall scripts locate the checkout relative to themselves, and
the installed launcher uses the per-user binary outside the checkout. Such a
rename alone needs no reinstall or settings migration.

OpenCode sessions have a separate working-directory/project association. After
moving the source, start OpenCode in the destination so `AGENTS.md` is discovered.
If earlier conversations are missing from `/sessions`, locate them across
projects, export a private backup, and use OpenCode's supported session-move
tool/API to associate them with the destination. Do not edit its database to
repair paths. A new session loads project guidance, not all previous transcripts;
resume the existing session for conversational continuity. See `README.md` for
OpenCode history transfer guidance.

1. Transfer the project source, `Cargo.toml`, **`Cargo.lock`**, `data/`, `scripts/`,
   `.gitignore`, `README.md`, this `AGENTS.md`, `LICENSE` and `THIRD_PARTY.md`.
   Rebuild `target/` locally; it is generated output. Source code uses portable
   home/XDG paths rather than the original developer's project directory.
   The original forex reference image, `unnamed.png`, was supplied outside this
   project. `README.md` records its data fields; carry the image separately if
   future visual comparison is needed. It is not a runtime dependency.
2. Install the target machine's toolchain/native dependencies, then run the
   build checks above. Confirm a working COSMIC Wayland session for panel use.
3. If migrating preferences, transfer the external settings described below.
   Reconcile old absolute OAuth client paths with the destination home directory.
4. Install for the destination user:

   ```sh
   sh scripts/install.sh --no-build
   ```

   This writes `~/.local/bin/cosmic-applet-finance-dashboard` and
   `~/.local/share/applications/io.github.joeytong.cosmic-applet-finance-dashboard.desktop` by
   default, plus `~/.local/share/licenses/cosmic-applet-finance-dashboard/flag-icons-LICENSE`.
   Rerun the installer instead of copying the old installed desktop
   entry: its `Exec` contains an absolute path for that user. `PREFIX` can override
   the install prefix; confirm the desktop can discover a custom location.
5. Add **Finance Dashboard** through COSMIC Settings → Desktop → Panel → Applets.
   Alternatively, `python3 scripts/panel.py --add` inserts it in the standard
   panel center, after the stock clock when present. A previous
   `io.github.cosmic_finance_dashboard.Applet` or `io.github.dailywork.CosmicApplet`
   entry is replaced in its current position.
   The helper edits only
   `com.system76.CosmicPanel.Panel/v1/plugins_center` and makes a backup.
   Use Settings for other panel layouts. Preserve the destination's other applets.
6. Reconnect Google in applet Settings using a Desktop app OAuth JSON. The target
   keyring must have its own connection; copying settings alone does not authorize
   Google access. Enable Calendar API and add the signing-in account as a test
   user in the same Cloud project when the OAuth app is in Testing.
7. Check the live dashboard, browser links, flags, prices, calendar, and the
   once-per-login behavior. The panel already launches the applet at login;
   a second standalone autostart entry is unnecessary.

### State outside the project

`$XDG_CONFIG_HOME` defaults to `~/.config`; `$XDG_RUNTIME_DIR` is session runtime
storage. Check their values on the destination.

| State | Location / migration handling |
|---|---|
| User preferences | `$XDG_CONFIG_HOME/cosmic-applet-finance-dashboard/settings.json`: banner message, clocks, pairs, Google email/client path and disabled calendars. Transfer if retaining these choices. |
| OAuth client copy | Same directory, `google-client.json`. Transfer privately or select the original downloaded JSON on the target and reconnect. Keep it outside the source tree. |
| Google refresh token | System Secret Service; attributes `application=io.github.joeytong.cosmic-applet-finance-dashboard`, `account=google-calendar`. Reconnect on the target; token is not in settings JSON. |
| Stock clock preferences | `$XDG_CONFIG_HOME/cosmic/com.system76.CosmicAppletTime/v1/`. By default follow the destination's clock/week preferences. |
| Panel placement/backups | COSMIC panel configuration; helper backups in the app config directory under `panel-backups/`. Recreate placement on the destination rather than overwriting its entire panel config. |
| Login marker | `$XDG_RUNTIME_DIR/cosmic-applet-finance-dashboard/login-*.shown`. Ephemeral; do not transfer. New logins create their own markers. |
| Japanese progress | `$XDG_DATA_HOME/cosmic-applet-finance-dashboard/japanese-study.json`, with dictionary details in `japanese-details.json` beside it. Transfer only to keep this dashboard's own vocabulary progress. Do not copy the Japanese Words applet's study file into it. |
| OpenCode history | Normally `~/.local/share/opencode/opencode.db`, separate from this project. Move session associations after directory changes; use OpenCode session export/import when transferring history to another computer. This guide supplies portable context, not a conversation-history export. |

Settings writes use a private directory and atomic replacement. Keep that behavior
when changing persistence. On startup, a previous `daily-work-cosmic-applet`
configuration directory is renamed to `cosmic-applet-finance-dashboard` when the
new directory has no `settings.json`. If the new directory already exists,
`settings.json` and `google-client.json` move across and panel backups already
in the new directory stay there. `google_client_path` is rewritten when it
points at the previous directory's `google-client.json`. The previous
login-marker directory is renamed when the new runtime directory is absent. A
Google token stored with `application=io.github.cosmic_finance_dashboard.Applet`
or `application=io.github.dailywork.CosmicApplet` is saved under the new
application id and then removed. The newer of those two ids is checked first.
Install and uninstall also remove the previous per-user launcher, desktop
entries, and license file.
`python3 scripts/panel.py --remove` removes the helper-managed panel entry;
`sh scripts/uninstall.sh` removes installed files and retains preferences/backups.
Disconnect Google before uninstalling if removing its saved keyring token is
intended. For an installed binary update, reload the applet after installation so
the panel uses the new executable. Preserve the panel list and order on reload.

## Integration details to preserve

- **Popup lifecycle:** click and automatic login opening share one `open_popup()`
  path. Track popup/dismissal surface IDs; pointer motion and focus changes do not
  produce lifecycle messages. Keep the state simple, without hover timers or Pin mode.
- **Login:** wait for a valid panel layout. Atomically claim a marker keyed by
  boot ID and `XDG_SESSION_ID`. Preview mode must not consume the marker.
- **Dismissal:** login opening has no input serial for an xdg popup grab. Use the
  same ungrabbed popup with a fully transparent Top-layer dismissal surface for
  both login and click opening. COSMIC places panel popups above Top-layer surfaces.
  Use the panel's output, neutral exclusive zone
  (respect panel reservations), on-demand keyboard focus, and explicitly disabled
  blur/corners/padding. Destroy the companion surface on every dismissal, including
  compositor-driven closure. Identify events by surface ID. Do not use keyboard
  focus loss (even delayed) as a dismissal signal: focus-follows-cursor makes it
  indistinguishable from ordinary pointer motion. Outside-clicks use the backdrop;
  browser links, OAuth sign-in and file-chooser actions explicitly dismiss both
  surfaces before opening their external UI.
  Native keyboard focus follows clicks on the panel/dashboard. Settings keeps the
  same surface. Preview must never create the layer.
- **Browser launch:** remove inherited `WAYLAND_SOCKET`; honor
  `HOST_WAYLAND_DISPLAY` when set. External browsers must use the desktop
  compositor rather than the applet's embedded panel connection.
- **OAuth:** retain PKCE, state validation, localhost-only callback, read-only
  access and Secret Service storage. Missing client JSON should explain setup
  before launching sign-in. Testing-mode tokens can expire after seven days;
  `403 access_denied` for a testing app usually requires adding the exact account
  as a test user in the project that issued the client JSON.
- **Calendar:** follow pagination, expand recurring instances, handle all-day
  exclusive end dates and zero-duration events, and calculate local midnights
  independently across DST. Preserve partial-refresh errors/last-known events.

## Last verified handoff

2026-11-09 click-only revision, including removal of the × button: formatting,
Clippy, script syntax checks, the
release build and **27 tests** passed. Installed the release and flag license,
verified the new panel process uses that executable, and restored the exact
original panel configuration/order. Saved preferences and the existing login
marker were verified unchanged. COSMIC 1.11 and 1920×1080 were rechecked.

The source in this checkout was restored to the latest approved banner,
weekend/holiday highlights and flat flags before simplifying the popup lifecycle.
The previous removable-drive checkout was unavailable; keep the complete restored
source and embedded artwork together when transferring this project.

Once-per-login behavior is covered by an isolated app-level test, including layout
readiness, preview isolation and restart suppression, plus marker concurrency tests.
These tests do not simulate the compositor or a real desktop login. The user
accepted the revised behavior/design and additionally requested removal of the
× close button. That follow-up passed the Rust checks, was installed and reloaded
in the original panel position. The next real-login check remains pending.

2026-11-09 project rename: the package, binary, application id, desktop entry,
configuration paths, and project docs now use Finance Dashboard /
`cosmic-applet-finance-dashboard`. Formatting, Clippy, script syntax checks,
the release build, and **30 tests** passed. The installed `~/.local` launcher
and panel entry were left in place; switch them with `sh scripts/install.sh
--no-build` and `python3 scripts/panel.py --add`. The first launch of the new
binary adopts the previous configuration directory, login marker, and Google
token.
