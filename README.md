# Finance Dashboard · COSMIC applet

A local, native Rust/libcosmic dashboard for a trader’s working day. Its panel label follows the installed COSMIC **Date, Time & Calendar** applet’s locale, 12/24-hour preference, date, weekday, seconds, custom format, size and orientation.

The dashboard sizes itself to the clocks, currency pairs, and calendar events. World clocks sit across the upper left. The public IP card is on the upper right, and its right edge lines up with forex and commodities. The two-week calendar is lower left. Each day box is at least three times the previous date-only height. The Japanese card sits under the calendar, and its bottom lines up with the commodities card. Forex and commodities occupy the lower right. Fewer items make the window smaller. Added clocks, pairs, and events make it larger at the current type size, until it reaches the monitor measured when the applet starts. Past that size the items shrink so everything stays visible without scrolling. A second clock row or forex column is used only when another row would pass the monitor. Settings opens in that same window and scrolls inside it. Full event details and source/timing information remain available on hover.

A centered, text-only banner tops the dashboard. Its initial message is
**“Thrived Today, Leading Tomorrow”**; your saved message takes precedence.
Weekends and holidays share a subtle full-cell theme tint. Country and currency
flags use flat, full-color SVG artwork with gently rounded corners and native
COSMIC outlines.

## Use

- Left-click the panel clock to open or close the dashboard. Hovering never opens or closes it; there is no separate Pin mode.
- At each login, the dashboard automatically opens once the panel is ready. Panel/applet restarts during that login do not reopen it automatically.
- The dashboard stays open when the pointer or keyboard focus moves away, including with COSMIC’s focus-follows-cursor enabled. Click the clock again or outside the dashboard to dismiss it. Escape dismisses it when the applet has keyboard focus. The dashboard has no × close button.
- The first click in the surrounding desktop/work area only dismisses the dashboard; it does not also activate the window underneath. Click again to interact with that window. This also applies to the automatic login dashboard.
- **Settings** configures the banner, clocks, forex pairs, Japanese vocabulary and Google Calendar. Changes to watchlists and the vocabulary library are saved immediately.
- Under **Settings → Dashboard banner**, edit the message and choose **Save** (or Enter) to apply it; **Cancel** restores the saved message. Use 1–160 characters on one line. Longer messages shrink to fit, then use an ellipsis if necessary; hover shows the complete text. Drafts survive navigation and validation/write failures.
- Under **Settings → World clocks**, click **Edit** beside an existing clock to change its name or IANA time zone. **Save** (or Enter) validates and saves the changes in the same position; **Cancel** discards the draft. Finish the current edit before editing or removing another clock.
- Hover over calendar events for details; click to open their Google Calendar page.
- Select a day for its complete agenda below the two-week grid. Busy days automatically use smaller event labels rather than hiding additional events.
- Hover over a market price for the instrument, delay, retrieval time and last available one-minute bar.
- Each currency pair uses two tightly overlapping rectangular flags, positioned together so they cannot wrap toward the adjacent row. The embedded artwork requires no runtime downloads or emoji font; unknown country codes use a neutral dash badge.
- The refresh button updates clocks, the calendar, markets, and the public IP immediately. It does not change the Japanese word. Background refresh of those data modules runs even while the popup is closed.

## Build and install

The source folder, Cargo package, and installed executable are named
`cosmic-applet-finance-dashboard`. Run the commands below from this folder.

After moving or renaming only the source folder on the same computer, verify
`cargo build --release --locked` from its new location. Cargo manages the generated
build paths in `target/`. The installation scripts locate the project relative to
themselves; the installed launcher and saved settings live outside the source
folder, so a folder-only rename needs no reinstall or settings migration.

Developed against COSMIC 1.11 on CachyOS/Arch, with the matching libcosmic revision pinned. `Cargo.lock` pins the complete dependency graph. Rust 1.93 or newer, a C toolchain, pkg-config, Wayland, libxkbcommon, fontconfig and the native X11/rendering libraries used by COSMIC are required. Google sign-in uses `xdg-open` and **libsecret’s `secret-tool`**, with an unlocked Secret Service keyring.

```sh
rustup component add rustfmt clippy
cargo build --release --locked
sh scripts/install.sh --no-build
```

Install is per-user: `~/.local/bin/cosmic-applet-finance-dashboard` and `~/.local/share/applications/io.github.joeytong.cosmic-applet-finance-dashboard.desktop`. The flag artwork’s MIT notice is installed to `~/.local/share/licenses/cosmic-applet-finance-dashboard/flag-icons-LICENSE`. No root access is needed. The desktop entry uses an absolute executable path, so the panel’s PATH need not include `~/.local/bin`.

Add **Finance Dashboard** in **COSMIC Settings → Desktop → Panel → Applets**, or add it beside the existing center clock. `python3 scripts/panel.py --add` replaces an older Daily Work or Finance Dashboard panel entry in its current position:

```sh
python3 scripts/panel.py --add
```

This backs up the existing panel center configuration and retains other applets. COSMIC normally picks up the change immediately. The applet is then launched by the panel at login and opens its dashboard. A session marker in `$XDG_RUNTIME_DIR/cosmic-applet-finance-dashboard/`, keyed by the boot and login session IDs, ensures this only happens once per login, even across panel restarts. Automatic opening waits for the applet's panel layout. Click and login opening share one popup path with a transparent native layer behind the popup to receive outside clicks, including at login when no user-input serial exists. The layer respects the panel's reserved space and adds no tint or blur. Clicking the panel/dashboard gives native keyboard focus; Settings retains the same popup surface. Standalone `--preview` windows do not consume the login marker or create the dismissal layer.

For a standalone native preview or public-service diagnostics:

```sh
target/release/cosmic-applet-finance-dashboard --preview
target/release/cosmic-applet-finance-dashboard --diagnose
```

The preview is a development convenience; the installed application is a real Wayland COSMIC panel applet.

## Google Calendar setup

1. Create/select a project in [Google Cloud Console](https://console.cloud.google.com/).
2. Enable **Google Calendar API**.
3. Configure the OAuth consent screen. If the app is in **Testing**, add your Google email as a test user.
4. Create an OAuth client with application type **Desktop app**, then download its JSON file.
5. Open Finance Dashboard → Settings. Select **Choose JSON…** to open the native file chooser, or enter the JSON path (including `~/` if desired). Optionally enter your Google email.
6. Select **Sign in with Google**, then complete consent in the default browser. Sign-in is enabled after a JSON path is provided; invalid/missing files show a setup error before opening a browser. An ephemeral listener bound only to `127.0.0.1` receives the redirect.
7. Select which of your calendars to display. Add country holidays in Google Calendar → Settings → Add calendar → Browse calendars of interest, then refresh Finance Dashboard.

The scope is `calendar.readonly`. OAuth uses PKCE and a random state value. Refresh tokens are stored only in the system Secret Service, never in settings or logs. A private copy of the desktop OAuth client JSON is stored in the app’s configuration directory. Access tokens remain in memory. **Disconnect** deletes the local refresh token; access can also be revoked in your Google account’s third-party connections.

Google Testing-mode refresh tokens may expire after seven days. Reconnect if the app reports revoked/expired authorization. An email address alone is not sufficient to authorize calendar access. **Set up Google OAuth in browser** opens Google Cloud even before credentials are configured. Browser links, sign-in and the file chooser explicitly dismiss the dashboard to hand off to their external window. Click the clock again to return to Settings with your selected path or sign-in result.

Events from all visible, enabled calendars are fetched, including recurring instances, all-day and multi-day events. Calendar pagination is followed. All-day end dates are exclusive; timed events are displayed in the computer’s local time zone. Day and week boundaries are recalculated after midnight and after COSMIC’s first-weekday setting changes. Different country holiday calendars receive distinct colors and a diamond marker; ordinary events have circle markers and Google colors. Saturdays, Sundays and days with enabled holiday events share the same subtle full-cell COSMIC-theme tint. **Google Tasks are not included**, as selected during requirements Q&A.

## Modules and sources

| Module | Defaults / meaning | Refresh |
|---|---|---|
| Panel date/time | OS clock, COSMIC’s existing clock settings and ICU locale formatting | 1 second; settings checked every 30 seconds |
| World clocks | Singapore (`Asia/Singapore`), GMT (`Etc/UTC`), New York (`America/New_York`, with DST), Tokyo (`Asia/Tokyo`) | OS sampled every 30 seconds; public sync at startup and hourly |
| Calendar | This week + next week, starting on COSMIC’s configured weekday | Startup when connected, every 10 minutes, and on date/week changes |
| Forex | USDCNH, USDJPY, USDHKD, JPYCNY, SGDCNY, GBPCNH, EURCNH, GBPUSD, GBPJPY, GBPSGD, GBPEUR | Startup and every 10 minutes |
| BRN1 | `ICEEUR:BRN1!`, nearest continuous ICE Brent crude futures, USD/barrel | Startup and every 10 minutes |
| GOLD | `TVC:GOLD`, spot gold / XAUUSD, USD/troy ounce | Startup and every 10 minutes |
| Public IP | Public egress address, country flag, country and city | Startup and every minute |
| Japanese | Headword, reading, pronunciation, British gloss, senses, and examples | One new word on the next local day |

The Japanese card under the calendar has two centered lines. The first line is the headword and the reading at 45px. The second line is the broad Tokyo IPA pronunciation and the British English gloss. **Next Word** stays at the right. Hovering the lines opens the pronunciation source, each sense under its part of speech, each example with the Japanese sentence above the English, and the dictionary source. There is no JLPT label, countdown, or separate Settings button. At the next local midnight, or on the next launch after one or more missed midnights, the active library takes one step. The step marks the word learned only after 30 minutes of active session time; otherwise the word is replaced and stays unlearned. Active time pauses while logind reports the session idle, locked, or inactive. Libraries that are not selected keep their current word until you select them. Dashboard **Refresh** does not change the word. Library choice and reset are in Settings. Reset asks for confirmation and clears only the selected library. Progress starts empty in this applet and is not read from Japanese Words.

Senses come from Jisho, with Wiktionary as the backup. Example sentences and dictionary pronunciation come from English Wiktionary. Pronunciation is shown as broad Tokyo IPA: long vowels and small-tsu consonants use ː, and the voiced velar stop is ɡ. Pitch accent and narrow vowel marks are removed. If neither source has pronunciation, it is generated from the kana and labeled. Missing examples stay blank.

World clocks use [TimeAPI.io](https://timeapi.io/) with [WorldTimeAPI](https://worldtimeapi.org/) as fallback. The midpoint of each HTTPS request estimates the offset. This is an application-level correction: the OS clock is never changed, and the panel always uses uncorrected OS time. On sync failure the analog clocks fall back to OS time with a visible status. A detected system-clock jump invalidates the old offset and triggers another sync. The analog faces have only hour and minute hands; AM/PM and the local date distinguish days and half-days.

Markets use TradingView’s public scanner snapshots. This public endpoint is undocumented and availability/coverage may change. The app shows source and timing rather than presenting snapshots as an execution-grade streaming feed. The feed currently marks Brent as 10-minute delayed. The daily absolute and percentage changes come from the provider’s previous-session comparison, not from the previous app refresh. CNH and CNY are separate instruments and are never substituted. Unsupported pairs display **Unavailable**, and failed refreshes retain clearly marked **last known** prices. An old bar is labeled **Market closed / old quote** because a public snapshot cannot reliably distinguish a closure from a stalled feed. A bar time is labeled as a bar time; absent trade timestamps are not fabricated.

Public IP uses [ipwho.is](https://ipwho.is/) with [ipapi.co](https://ipapi.co/) fallback. It reflects the route used for the HTTP request, including a VPN or proxy. Location is the provider’s IP geolocation.

Requests have connection/overall timeouts and run asynchronously. Module failures are independent and visible. A partial Google refresh retains events from failed calendars and displays its errors. No fabricated/demo events or prices are shown.

## Screenshot field analysis

The supplied TradingView image contains eleven forex rows. Each has two currency-country flags, a base/quote ticker, a human-readable pair description, a right-aligned price, a signed absolute daily change and a signed percentage daily change. Negative changes are red; positive changes are green. Some prices show the final fractional digit at smaller size. Finance Dashboard implements these fields, preserves provider precision (up to eight places), adds timing/source information, and uses COSMIC’s theme for surfaces, controls and market direction colors. The unrelated stock and Crypto/navigation portions of the screenshot are not modules in this app.

## Configuration and development

Preferences: `$XDG_CONFIG_HOME/cosmic-applet-finance-dashboard/settings.json` (normally `~/.config/cosmic-applet-finance-dashboard/settings.json`). These include `banner_message`, saved clocks/watchlists, and Google configuration. Japanese progress is separate, in `$XDG_DATA_HOME/cosmic-applet-finance-dashboard/japanese-study.json` (normally `~/.local/share/cosmic-applet-finance-dashboard/japanese-study.json`). Dictionary details for the current word and the next 10 queued words are cached beside it in `japanese-details.json`. Older settings without a banner gain the initial message while retaining their existing preferences. A previous `~/.config/daily-work-cosmic-applet` directory is adopted on startup, and a stored Google token from either previous application id is moved to `io.github.joeytong.cosmic-applet-finance-dashboard`. Writes are atomic and private. Invalid configuration is reported without overwriting the file. Backups from panel edits are in `panel-backups/` under the same directory.

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run -- --diagnose
```

The tests cover calendar DST boundaries, exclusive all-day endings, zero-duration events, week/year rollover, financial response parsing, CNH/CNY identity, OAuth callback/client validation, click toggling and same-surface Settings, ignored pointer/focus changes, outside-click layer cleanup, stale surface events, external-window handoff, banner validation/drafts/legacy settings, flag coverage/fallbacks, clock-edit validation/cancellation/order preservation, and the Japanese daily word. Once-per-login tests cover layout readiness, preview isolation, restarts and concurrent panel instances using temporary session markers. Verify actual panel clicks, anchoring and the next login on the desktop. Browser launches remove the panel-only inherited Wayland socket. Live Google integration requires your credentials and browser consent; the diagnostics command checks only public services.

Main files:

- `src/app.rs`: native panel, dashboard, settings and refresh scheduling
- `src/analog.rs`, `src/panel_time.rs`: analog drawing and stock COSMIC date/time formatting
- `src/calendar.rs`, `src/oauth.rs`: read-only Google access and secure sign-in
- `src/market.rs`, `src/network.rs`: public data providers and time/IP checks
- `src/config.rs`, `src/login.rs`: preferences and once-per-login opening
- `src/flags.rs`, `data/flags/`: embedded flag rendering, artwork and license
- `src/japanese/`, `data/japanese/`: daily vocabulary, bundled lists and study progress

To remove the panel entry without uninstalling, run `python3 scripts/panel.py --remove`. To uninstall, disconnect Google in Settings first, then run `sh scripts/uninstall.sh`. Preferences and panel backups are retained.

## OpenCode context and session history

Start OpenCode in this project folder with `opencode .`. Its `AGENTS.md` loads
automatically as project guidance. Use `/sessions` to resume an existing
conversation; a new conversation does not automatically include older transcripts.

OpenCode stores history separately from the source, normally in
`~/.local/share/opencode/opencode.db`, and associates sessions with their working
directory/project. Copying or moving this folder does not move that association.
If history disappears after a folder move, ask OpenCode to locate the old session,
export a private backup, and move the session to the current directory with its
supported session-move tool/API. This preserves the conversation and lets it find
the current `AGENTS.md`. On another computer, transfer sessions using OpenCode's
session export/import commands as well as transferring the project files.

## License

GPL-3.0-only. COSMIC panel date/time formatting and defaults are adapted from System76’s `cosmic-applet-time` / `cosmic-applets-config` 1.11 (Copyright 2023 System76, GPL-3.0-only). Bundled flag-icons artwork is MIT-licensed; see `data/flags/LICENSE` and `data/flags/README.md`. The Japanese queue, catalog, spelling and idle-detection sources keep their MPL-2.0 headers. `data/japanese/words.json` stays CC BY-SA 4.0; see `data/japanese/WORDS.md`. Native widget/runtime dependencies retain their respective licenses. See `LICENSE` and `THIRD_PARTY.md`.
