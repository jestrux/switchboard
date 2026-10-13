#!/usr/bin/env python3
"""Record the narrated Switchboard walkthrough.

  scripts/walkthrough/record.py --dry      # run the beats, screenshot each, no recording
  scripts/walkthrough/record.py            # record video + Kokoro narration -> docs/media/

Needs: /Applications/Switchboard.app (scripts/package.sh), the CityBuild and Kinpay sample
projects installed, ffmpeg, and Kokoro (`pip install kokoro-onnx soundfile`, model files in
~/.kokoro). Screen Recording and Accessibility permission for the terminal running it.
The app window is recorded as a screen region, so keep it unobstructed while this runs.
"""
import json, os, signal, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MEDIA = ROOT / "docs" / "media"
WORK = ROOT / "target" / "walkthrough"
DRIVER_SRC = ROOT / "scripts" / "dev" / "uidrive.swift"
DRIVER = WORK / "uidrive"
ORIGIN = (116, 60)          # where `uidrive resize 1280 800` puts the window on a 1512 pt wide screen
SIZE = (1280, 800)
DRY = "--dry" in sys.argv
VOICE = "am_michael"

def sh(cmd, check=False):
    return subprocess.run(cmd, shell=True, check=check, capture_output=True, text=True)

def ui(*args):
    subprocess.run([str(DRIVER), *map(str, args)], check=False)

def pt(x, y):
    """Window point (0..1280, 0..800) -> screen point."""
    return ORIGIN[0] + x, ORIGIN[1] + y

def menu(menu_name, item):
    sh(f'''osascript -e 'tell application "System Events" to tell process "Switchboard" to click menu item "{item}" of menu "{menu_name}" of menu bar 1' ''')

def keystroke(text, ret=True):
    sh(f'''osascript -e 'tell application "System Events" to keystroke "{text}"' ''')
    if ret:
        sh('''osascript -e 'tell application "System Events" to key code 36' ''')

def glide(x, y, steps=18, dur=0.5):
    """Move the pointer smoothly to a window point so the eye can follow it."""
    global _pos
    sx, sy = _pos
    tx, ty = pt(x, y)
    args = []
    for i in range(1, steps + 1):
        t = i / steps
        t = t * t * (3 - 2 * t)
        # One driver process for the whole path; `move` itself waits ~60 ms per step.
        args += ["move", round(sx + (tx - sx) * t), round(sy + (ty - sy) * t)]
    ui(*args)
    _pos = (tx, ty)

def click(x, y):
    glide(x, y)
    ui("click", *pt(x, y))

_pos = pt(700, 420)

# ---- beats: (id, narration, action) ---------------------------------------------------

def b_intro():
    glide(110, 150); time.sleep(0.6)
    glide(140, 330); time.sleep(0.6)
    glide(140, 500); time.sleep(0.4)

def b_start():
    click(258, 56)                       # Start all
    time.sleep(1.2)
    click(150, 178)                      # Frontend / dev
    time.sleep(4.5)
    click(165, 325)                      # Backend / make:dev
    time.sleep(2.5)
    click(150, 178)

def b_links():
    glide(620, 150); time.sleep(0.4)
    x, y = LINK
    sh('''osascript -e 'tell application "System Events" to key down command' ''')
    time.sleep(0.3)
    ui("cmdmove", *pt(x, y)); ui("cmdmove", *pt(x + 2, y)); ui("cmdmove", *pt(x + 3, y))
    time.sleep(3.0)
    sh('''osascript -e 'tell application "System Events" to key up command' ''')
    ui("move", *pt(x + 4, y + 60))

def b_split():
    glide(165, 325)
    ui("drag", *pt(165, 325), *pt(1255, 420))
    time.sleep(2.5)
    glide(900, 300)

def b_terminal():
    ui("key", 17, "cmd")                 # ⌘T
    time.sleep(2.5)
    # Harmless commands only: a directory listing here would show private filenames.
    keystroke("echo ready in $(basename $PWD)")
    time.sleep(2.0)
    keystroke("date")
    time.sleep(1.5)

def b_window():
    click(205, 56)                       # Open in New Window
    time.sleep(3.0)
    click(73, 227)                       # Frontend / dev, in the project window
    time.sleep(3.0)

def b_stop():
    click(115, 115)                      # Stop all, in the project window's header
    time.sleep(3.5)

LINK = (512, 114)   # the "http://localhost:3006/" text in the dev pane; checked in --dry

BEATS = [
    ("intro", "This is Switchboard. One window that starts, watches and stops every dev command for a project, even when that project is spread across several folders.", b_intro),
    ("start", "One click starts the whole project. Every command runs in a real terminal, and its marker shows what is running, what finished, and what crashed.", b_start),
    ("links", "Links in the output are live. Hold Command, and they underline. Click, and they open in your browser.", b_links),
    ("split", "To watch two things at once, drag a command onto the edge of a pane. It opens right beside it.", b_split),
    ("terminal", "Need a quick shell? Command T opens a terminal in that group's folder. It will still be here the next time you open the app.", b_terminal),
    ("window", "Give a project its own window, and it can wear its own colours. Here, CityBuild uses its brand theme, while the main board keeps yours.", b_window),
    ("stop", "When you are done, stop everything. Nothing is left running in the background.", b_stop),
]

# ---- setup ----------------------------------------------------------------------------

def setup():
    WORK.mkdir(parents=True, exist_ok=True)
    MEDIA.mkdir(parents=True, exist_ok=True)
    if not DRIVER.exists() or DRIVER.stat().st_mtime < DRIVER_SRC.stat().st_mtime:
        sh(f"swiftc -O -o {DRIVER} {DRIVER_SRC}", check=True)
    sh('''osascript -e 'tell application "Switchboard" to quit' ''')
    time.sleep(2.5)
    proj = Path.home() / "Library/Application Support/Switchboard/Projects"
    (proj / "citybuild" / "terminals.yaml").unlink(missing_ok=True)
    d = "defaults write dev.waky.switchboard"
    for key in ["layout.main", "focus.main", "layout.project.citybuild", "focus.project.citybuild", "reopenProjectWindows"]:
        sh(f"defaults delete dev.waky.switchboard {key}")
    sh(f"{d} rail.main citybuild; {d} appearance.rail -bool true; {d} appearance.theme graphite; {d} appearance.mode system")
    sh(f"{d} homeOffer.dismissed.citybuild -bool true; {d} homeOffer.dismissed.kinpay -bool true")
    sh("open /Applications/Switchboard.app")
    time.sleep(4.5)
    ui("activate", "resize", *SIZE, "sleep", 800)
    ui("move", *_pos)

def teardown():
    menu("Command", "Stop Everything")
    for key in ["homeOffer.dismissed.citybuild", "homeOffer.dismissed.kinpay"]:
        sh(f"defaults delete dev.waky.switchboard {key}")

# ---- narration ------------------------------------------------------------------------

def synthesize():
    clips = []
    todo = []
    for bid, text, _ in BEATS:
        path = WORK / f"{bid}.wav"
        stamp = WORK / f"{bid}.txt"
        if not path.exists() or not stamp.exists() or stamp.read_text() != text:
            todo.append((text, str(path), str(stamp)))
        clips.append(path)
    if todo:
        py = f'''
import soundfile as sf, os, json
from kokoro_onnx import Kokoro
home = os.path.expanduser("~/.kokoro")
k = Kokoro(home + "/kokoro-v1.0.onnx", home + "/voices-v1.0.bin")
for text, path, stamp in json.loads({json.dumps(json.dumps(todo))}):
    samples, sr = k.create(text, voice="{VOICE}", speed=1.0, lang="en-us")
    sf.write(path, samples, sr)
    open(stamp, "w").write(text)
'''
        subprocess.run(["python3", "-c", py], check=True)
    durs = [float(sh(f'ffprobe -v quiet -show_entries format=duration -of csv=p=0 "{c}"').stdout.strip()) for c in clips]
    return clips, durs

def srt_time(t):
    ms = int(round(t * 1000))
    return f"{ms // 3600000:02d}:{ms // 60000 % 60:02d}:{ms // 1000 % 60:02d},{ms % 1000:03d}"

# ---- main -----------------------------------------------------------------------------

def main():
    setup()
    if DRY:
        for bid, _, action in BEATS:
            action()
            time.sleep(0.5)
            ui("shotapp", WORK / f"dry-{bid}.png")
            print("dry", bid)
        teardown()
        return

    clips, durs = synthesize()
    raw = WORK / "raw.mov"
    raw.unlink(missing_ok=True)
    rec = subprocess.Popen(["screencapture", "-v", "-x", "-R", f"{ORIGIN[0]},{ORIGIN[1]},{SIZE[0]},{SIZE[1]}", str(raw)],
                           stdin=subprocess.PIPE)
    time.sleep(1.5)                      # let the recorder settle
    t0 = time.time()
    starts = []
    stills = {"start": "board-running", "split": "split-panes", "terminal": "terminal", "window": "project-window"}
    for (bid, _, action), dur in zip(BEATS, durs):
        begin = time.time()
        starts.append(begin - t0)
        action()
        # Hold until the line has been spoken, plus a breath before the next one.
        remaining = dur + 0.7 - (time.time() - begin)
        if remaining > 0:
            time.sleep(remaining)
        if bid in stills:
            ui("shotapp", MEDIA / f"{stills[bid]}.png")
    time.sleep(1.0)
    total = time.time() - t0
    rec.send_signal(signal.SIGINT)
    try:
        rec.wait(timeout=20)
    except subprocess.TimeoutExpired:
        rec.kill()
    teardown()

    # Subtitles.
    with open(MEDIA / "walkthrough.srt", "w") as f:
        for i, ((_, text, _), s, d) in enumerate(zip(BEATS, starts, durs), 1):
            f.write(f"{i}\n{srt_time(s)} --> {srt_time(s + d)}\n{text}\n\n")

    # Mux: the recorder started 1.5 s before t0, so narration is offset by that much.
    lead = 1.5
    inputs = "".join(f' -i "{c}"' for c in clips)
    delays = ";".join(f"[{i + 1}:a]adelay={int((s + lead) * 1000)}:all=1[a{i}]" for i, s in enumerate(starts))
    mix = "".join(f"[a{i}]" for i in range(len(clips))) + f"amix=inputs={len(clips)}:normalize=0[aout]"
    out = MEDIA / "walkthrough.mp4"
    cmd = (f'ffmpeg -v error -y -i "{raw}"{inputs} -filter_complex "{delays};{mix};[0:v]fps=30,scale=1440:-2[v]" '
           f'-map "[v]" -map "[aout]" -t {total + lead:.2f} -c:v libx264 -preset slow -crf 26 -pix_fmt yuv420p '
           f'-c:a aac -b:a 96k -movflags +faststart "{out}"')
    subprocess.run(cmd, shell=True, check=True)
    print(f"wrote {out} ({total:.1f}s), {MEDIA / 'walkthrough.srt'}")
    for (bid, _, _), s, d in zip(BEATS, starts, durs):
        print(f"  {bid:<9} starts {s:5.1f}s  line {d:4.1f}s")

if __name__ == "__main__":
    main()
