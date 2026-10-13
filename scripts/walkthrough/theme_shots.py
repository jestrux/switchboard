#!/usr/bin/env python3
"""Capture the theming screenshots in docs/media/themes/.

Built-in themes (Graphite, Patch Bay, Paper) in light and dark on the board, then the
CityBuild and Kinpay sample projects in their own windows with their custom themes, light
and dark. Needs the installed app and both sample projects; starts one dev server per shot
and stops it again.
"""
import subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))
import record as r   # reuse the driver helpers

OUT = ROOT / "docs" / "media" / "themes"
PROJECTS = Path.home() / "Library/Application Support/Switchboard/Projects"
D = "defaults write dev.waky.switchboard"

def launch(theme, mode):
    r.sh('''osascript -e 'tell application "Switchboard" to quit' ''')
    time.sleep(2.5)
    for key in ["layout.main", "focus.main", "reopenProjectWindows", "layout.project.citybuild", "layout.project.kinpay",
                "focus.project.citybuild", "focus.project.kinpay"]:
        r.sh(f"defaults delete dev.waky.switchboard {key}")
    r.sh(f"{D} appearance.theme {theme}; {D} appearance.mode {mode}; {D} appearance.rail -bool true; {D} rail.main citybuild")
    r.sh(f"{D} homeOffer.dismissed.citybuild -bool true; {D} homeOffer.dismissed.kinpay -bool true")
    r.sh("open /Applications/Switchboard.app")
    time.sleep(4.5)
    r.ui("activate", "resize", 1280, 800, "sleep", 700)

def shot(name):
    r.ui("move", *r.pt(760, 560), "sleep", 500)
    path = OUT / f"{name}.png"
    r.ui("shot", path)
    r.sh(f'sips -Z 1600 "{path}" --out "{path}"')
    print("shot", name)

def set_appearance(slug, mode):
    cfg = PROJECTS / slug / "config.yaml"
    lines = [l for l in cfg.read_text().splitlines() if not l.startswith("appearance:")]
    i = next(i for i, l in enumerate(lines) if l.startswith("commands:") or l.startswith("groups:"))
    lines.insert(i, f"appearance: {mode}")
    cfg.write_text("\n".join(lines) + "\n")
    time.sleep(2.2)

def main():
    r.WORK.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    if not r.DRIVER.exists():
        r.sh(f"swiftc -O -o {r.DRIVER} {r.DRIVER_SRC}", check=True)

    for theme, name in [("graphite", "graphite"), ("patchBay", "patch-bay"), ("paper", "paper")]:
        for mode in ["dark", "light"]:
            launch(theme, mode)
            r.ui("click", *r.pt(150, 178), "sleep", 500)          # CityBuild › Frontend › dev
            r.menu("Command", "Start")
            time.sleep(6)
            r.ui("drag", *r.pt(165, 325), *r.pt(1255, 420), "sleep", 900)   # make:dev beside it (idle card)
            shot(f"{name}-{mode}")

    # Settings, dark.
    launch("graphite", "dark")
    r.ui("activate", "key", 43, "cmd", "sleep", 1500)
    wid = r.sh('''swift -e 'import AppKit; let l = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []; for w in l where (w["kCGWindowOwnerName"] as? String) == "Switchboard" && (w["kCGWindowName"] as? String)?.contains("Settings") == true { print(w["kCGWindowNumber"] as! Int); break }' ''').stdout.strip()
    if wid:
        p = OUT / "settings-dark.png"
        r.sh(f'screencapture -x -o -l {wid} "{p}"; sips -Z 1100 "{p}" --out "{p}"')
        print("shot settings-dark")
    r.ui("key", 13, "cmd", "sleep", 400)

    # Custom themes: each project in its own window, light and dark.
    for slug, tile, row in [("citybuild", (36, 67), (73, 227)), ("kinpay", (36, 128), (73, 197))]:
        launch("graphite", "system")
        r.ui("click", *r.pt(*tile), "sleep", 700, "click", *r.pt(205, 56), "sleep", 2500)   # rail tile, Open in New Window
        r.ui("activate", "resize", 1280, 800, "sleep", 700, "click", *r.pt(*row), "sleep", 600)
        r.menu("Command", "Start")
        time.sleep(7)
        for mode in ["dark", "light"]:
            set_appearance(slug, mode)
            shot(f"{slug}-{mode}")
        set_appearance(slug, "system")
        r.menu("Command", "Stop All in Project")
        time.sleep(2)

    r.sh('''osascript -e 'tell application "Switchboard" to quit' ''')
    time.sleep(2)
    for key in ["homeOffer.dismissed.citybuild", "homeOffer.dismissed.kinpay", "reopenProjectWindows"]:
        r.sh(f"defaults delete dev.waky.switchboard {key}")
    r.sh(f"{D} appearance.theme graphite; {D} appearance.mode system")
    r.sh("open /Applications/Switchboard.app")

if __name__ == "__main__":
    main()
