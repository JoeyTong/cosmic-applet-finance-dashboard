#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-"$HOME/.local"}
python3 "$root/scripts/panel.py" --remove
rm -f "$prefix/bin/cosmic-applet-finance-dashboard" \
    "$prefix/bin/daily-work-cosmic-applet" \
    "$prefix/share/applications/io.github.joeytong.cosmic-applet-finance-dashboard.desktop" \
    "$prefix/share/applications/io.github.cosmic_finance_dashboard.Applet.desktop" \
    "$prefix/share/applications/io.github.dailywork.CosmicApplet.desktop" \
    "$prefix/share/licenses/cosmic-applet-finance-dashboard/flag-icons-LICENSE" \
    "$prefix/share/licenses/cosmic-applet-finance-dashboard/japanese-words-WORDS.md" \
    "$prefix/share/licenses/daily-work-cosmic-applet/flag-icons-LICENSE"
rmdir "$prefix/share/licenses/cosmic-applet-finance-dashboard" 2>/dev/null || true
rmdir "$prefix/share/licenses/daily-work-cosmic-applet" 2>/dev/null || true
echo 'Finance Dashboard uninstalled. Settings and panel backups are retained.'
echo 'Disconnect Google in the app before uninstalling to remove its keyring token.'
