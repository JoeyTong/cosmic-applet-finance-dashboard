#!/usr/bin/env python3
"""Add/remove only Finance Dashboard in the COSMIC panel center, with an automatic backup."""
import argparse
import datetime
import json
import os
import pathlib
import re
import shutil

APP_ID = "io.github.joeytong.cosmic-applet-finance-dashboard"
PREVIOUS_APP_IDS = (
    "io.github.cosmic_finance_dashboard.Applet",
    "io.github.dailywork.CosmicApplet",
)
NAME = "Finance Dashboard"
parser = argparse.ArgumentParser(description=__doc__)
action = parser.add_mutually_exclusive_group(required=True)
action.add_argument("--add", action="store_true")
action.add_argument("--remove", action="store_true")
arguments = parser.parse_args()

config = pathlib.Path(os.environ.get("XDG_CONFIG_HOME", pathlib.Path.home() / ".config"))
path = config / "cosmic/com.system76.CosmicPanel.Panel/v1/plugins_center"
if not path.is_file():
    raise SystemExit(f"Panel center configuration not found. Add {NAME} through COSMIC Settings.")
original = path.read_text()
match = re.fullmatch(r"\s*Some\(\s*\[(.*?)\]\s*\)\s*", original, re.S)
if match:
    # A RON list of strings is JSON-compatible once its optional trailing comma is removed.
    try:
        entries = json.loads("[" + re.sub(r",\s*$", "", match[1]) + "]")
    except json.JSONDecodeError as error:
        raise SystemExit("Unrecognized panel configuration; use COSMIC Settings instead.") from error
elif original.strip() == "None":
    entries = []
else:
    raise SystemExit("Unrecognized panel configuration; use COSMIC Settings instead.")
if not isinstance(entries, list) or not all(isinstance(item, str) for item in entries):
    raise SystemExit("Panel configuration is not a list of applet IDs.")

if arguments.add:
    if APP_ID in entries and not any(item in PREVIOUS_APP_IDS for item in entries):
        print(f"{NAME} is already on the panel.")
        raise SystemExit(0)
    updated = [item for item in entries if item not in PREVIOUS_APP_IDS and item != APP_ID]
    replaced = next((index for index, item in enumerate(entries) if item in PREVIOUS_APP_IDS), None)
    if replaced is None:
        insert_at = (
            updated.index("com.system76.CosmicAppletTime") + 1
            if "com.system76.CosmicAppletTime" in updated
            else len(updated)
        )
    else:
        insert_at = sum(
            1
            for item in entries[:replaced]
            if item not in PREVIOUS_APP_IDS and item != APP_ID
        )
    updated.insert(insert_at, APP_ID)
    entries = updated
else:
    if APP_ID not in entries and not any(item in PREVIOUS_APP_IDS for item in entries):
        print(f"{NAME} is not in the panel center.")
        raise SystemExit(0)
    entries = [item for item in entries if item != APP_ID and item not in PREVIOUS_APP_IDS]

backups = config / "cosmic-applet-finance-dashboard/panel-backups"
backups.mkdir(parents=True, exist_ok=True, mode=0o700)
backup = backups / (datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f") + "-plugins_center.ron")
shutil.copy2(path, backup)
result = "Some([\n" + "".join("    " + json.dumps(item) + ",\n" for item in entries) + "])\n"
temporary = path.with_name(".finance-dashboard-plugins-center.tmp")
temporary.write_text(result)
os.chmod(temporary, path.stat().st_mode & 0o777)
temporary.replace(path)
print(f"{NAME} " + ("added to the panel center." if arguments.add else "removed from the panel center."))
print(f"Backup: {backup}")
