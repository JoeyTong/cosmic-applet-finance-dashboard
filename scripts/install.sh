#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-"$HOME/.local"}
case "${1:-}" in
    --no-build) ;;
    "") cargo build --release --locked --manifest-path "$root/Cargo.toml" ;;
    *) echo 'Usage: scripts/install.sh [--no-build]' >&2; exit 2 ;;
esac
install -d "$prefix/bin" "$prefix/share/applications" "$prefix/share/licenses/cosmic-applet-finance-dashboard"
install -m 755 "$root/target/release/cosmic-applet-finance-dashboard" "$prefix/bin/cosmic-applet-finance-dashboard.new"
mv -f "$prefix/bin/cosmic-applet-finance-dashboard.new" "$prefix/bin/cosmic-applet-finance-dashboard"
install -m 644 "$root/data/flags/LICENSE" "$prefix/share/licenses/cosmic-applet-finance-dashboard/flag-icons-LICENSE"
rm -f "$prefix/bin/daily-work-cosmic-applet" \
    "$prefix/share/applications/io.github.dailywork.CosmicApplet.desktop" \
    "$prefix/share/applications/io.github.cosmic_finance_dashboard.Applet.desktop" \
    "$prefix/share/licenses/daily-work-cosmic-applet/flag-icons-LICENSE"
rmdir "$prefix/share/licenses/daily-work-cosmic-applet" 2>/dev/null || true
python3 - "$root" "$prefix" <<'PY'
import pathlib, sys
root, prefix = map(pathlib.Path, sys.argv[1:])
binary = str((prefix / 'bin/cosmic-applet-finance-dashboard').resolve())
escaped = binary.replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
desktop = (root / 'data/io.github.joeytong.cosmic-applet-finance-dashboard.desktop').read_text()
desktop = desktop.replace('Exec=cosmic-applet-finance-dashboard', f'Exec="{escaped}"')
(prefix / 'share/applications/io.github.joeytong.cosmic-applet-finance-dashboard.desktop').write_text(desktop)
PY
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$prefix/share/applications"
fi
echo "Installed: $prefix/bin/cosmic-applet-finance-dashboard"
echo 'Add Finance Dashboard through COSMIC Settings → Desktop → Panel → Applets,'
echo 'or run: python3 scripts/panel.py --add'
