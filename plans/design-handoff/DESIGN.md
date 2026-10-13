# Switchboard — DESIGN.md

The design system for the Switchboard Mac app (SwiftUI + AppKit, macOS 14+, SwiftTerm panes).
Tokens live in `Theme.swift`; this file explains how to use them. Canvas of record: the
"Switchboard — 01 App shell" design canvas.

## Principles

1. **Native behaviour, designed look.** Keyboard, menus, Finder and VS Code hand-offs, light/dark and
   window management behave like a Mac app. The chrome (sidebar, rows, pane bars, dialogs) is ours.
2. **Status by shape and colour.** Every command state has its own glyph shape so it reads at a glance and
   without colour vision.
3. **The terminal is sacred.** We own its colours, font, padding and surroundings, never its text layout,
   and we never dim terminal text.
4. **One component per idea.** One pane (bar + terminal + footer) everywhere; one dialog card; one status glyph.
5. **Dark-first, light first-class.** Every surface ships in both, in every theme.

## Themes

Three themes, each with a light and a dark palette. Settings › Appearance has two independent controls:
**Theme** (Graphite · Patch Bay · Paper; Graphite is the default) and **Appearance** (System · Light · Dark).
A third control, **Project rail** (on by default), picks rail + one-project sidebar or the full tree.

| | Graphite | Patch Bay | Paper |
|---|---|---|---|
| Feel | calm, neutral, Linear/VS Code | warm hardware, lit LEDs | soft, floating, Raycast/Framer |
| Group labels | caps sans 11/600, +0.06em | caps mono 10.5/500, +0.10em | sentence case 11.5/600 |
| Selected row | accent bar (2 pt inset) | accent bar | white card with hairline |
| Glow | none | status glyphs glow, dark only | none |
| Pane area | flush | flush | floating card, 10 pt inset, 14 pt radius |
| Config warning border | solid | dashed | solid |

Everything else is shared structure. A theme is one `SBTheme` value: an `SBThemeStyle` plus two `SBPalette`s.

## Layout

- **Board (main window):** rail 72 · sidebar 236 (rail on) or 272 (tree) · pane area. A 1 pt `line` divider always
  separates rail and sidebar, in every theme.
- **Project window:** no rail. Sidebar 268 starting with the project header, then groups and commands at the
  top level · pane area.
- **Window minimum** 820 × 480. Sidebar resizes 220–420.

## Sidebar

- **Rail tile** 40 pt, tile radius per theme, badge bottom-right: running count (`running` fill) or `!` for a broken
  config (`error` fill). Selected tile: 2 pt window-colour gap + 2 pt accent ring.
- **Board project header:** name 17/600, Open in New Window (⌘⇧N) icon button, Start all. Sub line "N of M running · K panes".
- **Group row** 28 pt, theme label style, chevron, trailing summary when collapsed ("3 idle").
- **Command row** 30 pt, 22 pt indent: glyph · name · meta (uptime, `↻ 16s`, `exit 0`, `no folder`, mono 11) · pane
  number chip when shown in a pane. Restart-to-apply adds a `↻ apply` chip.
- **Foot:** "1 project in its own window ▾" (only when some are extracted) and a status line "N running · K crashed".
- **Broken project:** in tree mode a warning card (errorSoft, theme border style) with Edit Config; in rail mode a
  dashed tile with `!`.

## Status language

| State | Glyph | Colour | Motion |
|---|---|---|---|
| Stopped | hollow ring | `stopped` | — |
| Starting | ring track + 90° arc | `starting` | arc spins, 0.9 s |
| Running | filled dot + 22% halo | `running` | halo breathes, 2.4 s |
| Done (exit 0) | check in ring | `text2` | — |
| Crashed | red core, track, arc that drains | `crashed` | arc drains over the backoff delay |
| Error | filled triangle | `error` | — |

Glyph 12 pt in an 18 pt box. Patch Bay dark adds `drop-shadow(0 0 4px colour)` to live glyphs.
"Restarting in N s" and "restart to apply" are variants, not states.

## Pane

One component for single and split views.

- **Bar** 36 pt: glyph · `Group /` name ▾ (button: swap command) · chips (restart to apply, maximized) · status text ·
  actions. Focused: `button` background, 2 pt accent rule on top, full-contrast text. Unfocused: `sidebar`
  background, `text3`. Hover lifts to `hover`.
- **Width tiers:** full ≥ 560 · mid 360–559 · min 320–359. Width only removes: group prefix → long status → secondary
  actions into ⋯ → footer.
- **Primary action** is always first: running/starting Stop · crashed Start now (accent-soft) + Stop (cancels the
  restart) · done/never run Start · error Start disabled with the reason as tooltip.
- **Backoff:** 2 pt `crashedSoft` bar under the bar, `crashed` fill drains.
- **Terminal:** SwiftTerm, SF Mono 12.5, insets 10/14, theme ANSI palette. "started HH:MM:SS" separators stay.
- **Footer** 22 pt mono 10.5: full = folder · pid · uptime (or exit · ran); mid drops folder; min has none.
- **Project icon in the bar** only when the pane's command belongs to a different project than the window's.
- **Never run:** inline card with the run line and Start. **Error:** inline errorSoft card with the folder and Edit Config.
- **Empty pane:** picker ("Show a command"), filter focused, ↩ to show, Esc closes a fresh pane.

## Splits

- 1–4 panes as a split tree. ⌘D split right, ⌘⇧D split down, split button (⌥-click = down), View menu.
- Divider: 1 pt `line`, 7 pt hit area; hover accent; drag shows columns × rows per side; snaps at 50% and at the
  minimum; double-click evens out.
- Focus: click, ⌘[ / ⌘], ⌘⌥ + arrows. Keystrokes go only to the pane with the accent rule.
- ⌘⇧↩ maximizes (others hide but keep running; "Maximized · N panes" chip). ⌘⇧W closes a pane; the neighbour
  takes its space; the command keeps running. The last pane can't close. ⌘W closes the window.
- Minimum pane 320 × 180 pt (~40 × 8 cells); max 4 panes. Refused splits dim the button with the reason and toast on ⌘D.
- One view per command: picking a command shown in another pane swaps the two panes.
- Layout (tree, ratios, assignments) is remembered per project in app state, not config.yaml. View › Reset Layout.

## Windows (extraction)

- The main window is the board. Open in New Window pulls the project off the board until its window closes or
  Move to Board is chosen. A project is on the board or in its own window, never both.
- **Project window header** (the window's title): 36 pt icon, name 20/650, "N running of M", Start all · Stop all ·
  Restart all · ⋯ (Start/Stop/Restart All, Edit Config, Reveal in Finder, Move to Board, Delete Project…). Actions rest
  borderless in `text3`; on hover the header lifts onto a `hover` card and the buttons get borders.
- **Extraction:** tile lift 180 ms (scale 1 → 1.06, fade), window zooms from the tile, rail settles 220 ms, board
  selects the project above. **Return:** tile slides back to its slot with a 600 ms accent halo; selection doesn't move.
  Reduce Motion: 120 ms crossfade.
- **Board states:** first paint after relaunch has no animation; extracted projects are simply absent. All extracted:
  "The board is clear" with each window (Show Window · Bring Back) and Add Project.
- **Project window states:** deleted (Close Window) · invalid config (keeps the last good tree and running sessions,
  problems under the title) · nothing running (Start all becomes primary).
- **Placeholder** "Showing in the X window · Show Here": one muted line, outlined button. Transitional only.

## Dialogs

Custom card presented as a sheet (invalid folder, name conflict, delete) or app-modal at launch (leftover sessions).
460 pt wide, radius 14, `pane` fill, 1 pt `line`, app icon 48 (default icon in dark, light icon in light), title 15/650,
one line of consequence in `text2`, facts in a `sidebar` box, buttons right-aligned. ↩ triggers the default; Esc cancels.

| Dialog | Buttons (default last) |
|---|---|
| Invalid folder | Reveal in Finder (left) · **OK** |
| Name conflict | Cancel · Replace (red outline) · **Keep Both** |
| Delete project | **Cancel** (focus ring) · Delete Project (red fill) |
| Leftover sessions | Leave Running · **Stop It / Stop Them** |

Destructive buttons are never the default.

## Keyboard

⌘O add project · ⌘⇧N open project in new window · ⌘R restart · ⌘. stop · ⌘K clear · ⌘E edit config · ⌘D / ⌘⇧D split ·
⌘⇧↩ maximize · ⌘⇧W close pane · ⌘[ ⌘] next/previous pane · ⌘⌥ arrows move focus · ⌘, settings.
Every hover action has a menu item.

## App icon

Patch bay: six jacks, one accent cable (top-left to bottom-right), two lit LEDs. 1024 canvas, 824 body at 100 inset,
185 radius. Files: `switchboard-icon-1024.png` (default), `-light-1024.png`, `-tinted-1024.png`, SVG sources, and
`switchboard-menubarTemplate.png` / `@2x` (18 / 36 px, black on transparent) for the future menu-bar extra.

## Accessibility

Status never relies on colour alone (shape per state). Text meets 4.5:1 on its surface; `text3` is for meta only.
Every icon button has a label and a tooltip with its shortcut. Reduce Motion replaces all motion with crossfades.

## Tokens

All values sRGB hex. Type is SF Pro and SF Mono (`SBType`); metrics in `SBMetrics`; motion in `SBMotion`.

| Type role | Spec |
|---|---|
| Sidebar title | 17 / semibold |
| Project window title | 20 / semibold, −0.02em |
| Pane title, rows, body | 13 / regular (selected 600) |
| Secondary | 12 / regular |
| Dialog title | 15 / semibold |
| Meta (uptime, pid) | SF Mono 11, monospaced digits |
| Footer | SF Mono 10.5 |
| Terminal | SF Mono 12.5 |

Spacing scale: 2 · 4 · 6 · 8 · 10 · 12 · 14 · 16 · 20 · 24. Radii: button 8 · chip pill · row/card/tile per theme
(Graphite 7/10/10 · Patch Bay 6/12/9 · Paper 8/14/11) · dialog 14 · Paper pane 14.

### Colour

| Token | Graphite dark | Graphite light | Patch Bay dark | Patch Bay light | Paper dark | Paper light |
|---|---|---|---|---|---|---|
| `window` | `#0B0C0E` | `#ECEEF1` | `#12100D` | `#E6DECF` | `#161618` | `#EFEEEA` |
| `rail` | `#0B0C0E` | `#ECEEF1` | `#12100D` | `#E6DECF` | `#161618` | `#EFEEEA` |
| `sidebar` | `#0E0F11` | `#F4F5F7` | `#181511` | `#EFE8DB` | `#161618` | `#EFEEEA` |
| `line` | `#1E2025` | `#E1E3E8` | `#2A251E` | `#DDD2BF` | `#2C2C30` | `#E2E0DA` |
| `pane` | `#0E0F11` | `#FFFFFF` | `#1C1813` | `#F7F2E9` | `#1F1F22` | `#FFFFFF` |
| `header` | `#0E0F11` | `#FFFFFF` | `#1C1813` | `#F7F2E9` | `#1F1F22` | `#FFFFFF` |
| `terminal` | `#111214` | `#FCFCFD` | `#0F0D0B` | `#FBF8F2` | `#1B1B1E` | `#FBFBF9` |
| `terminalText` | `#D5D7DC` | `#24262B` | `#D8D0C2` | `#2B241A` | `#E2E1DD` | `#1F1F21` |
| `terminalDim` | `#6B7079` | `#8A9099` | `#7A705F` | `#8F836E` | `#7C7B77` | `#8C8A84` |
| `text` | `#E6E7EA` | `#17181B` | `#EDE6DA` | `#241E16` | `#EDEDEB` | `#1A1A1C` |
| `text2` | `#A9ACB4` | `#4F545C` | `#A99F8E` | `#5F5545` | `#A3A29E` | `#4A4946` |
| `text3` | `#7D828B` | `#7D838C` | `#8A7F6C` | `#857965` | `#85847F` | `#7A7873` |
| `text4` | `#4D525B` | `#B3B8C0` | `#5A5246` | `#B8AC96` | `#55544F` | `#B8B6B0` |
| `hover` | `#16171A` | `#E9EBEF` | `#221E18` | `#E6DCCB` | `#232326` | `#E6E4DF` |
| `selection` | `#1D2233` | `#E3E9FF` | `#2E1F17` | `#F5DDB0` | `#2A2A2F` | `#FFFFFF` |
| `selectionText` | `#FFFFFF` | `#17181B` | `#FFF8EC` | `#241E16` | `#FFFFFF` | `#1A1A1C` |
| `accent` | `#7C9CFF` | `#4A6CF7` | `#FFB224` | `#C47A00` | `#8B8BFF` | `#3B3BD9` |
| `onAccent` | `#0B0C0E` | `#FFFFFF` | `#1A1208` | `#1A1208` | `#121214` | `#FFFFFF` |
| `accentSoft` | `#1D2233` | `#E3E9FF` | `#33250E` | `#F5DDB0` | `#26264A` | `#E7E7FB` |
| `accentSoftText` | `#A3B8FF` | `#2F4FD6` | `#FFCB66` | `#7A4C00` | `#B4B4FF` | `#3030B8` |
| `button` | `#16171A` | `#FFFFFF` | `#1F1B16` | `#FBF8F2` | `#262629` | `#FFFFFF` |
| `buttonLine` | `#2A2D33` | `#DADDE3` | `#3A332A` | `#D6CAB4` | `#35353A` | `#E2E0DA` |
| `running` | `#3DD68C` | `#14965A` | `#9ACD6B` | `#4E8A1A` | `#3FCB7E` | `#1F8A4C` |
| `runningSoft` | `#12301F` | `#DDF4E8` | `#22301A` | `#E1EDCF` | `#16301F` | `#DFF2E6` |
| `starting` | `#F5B83D` | `#B07800` | `#FFB224` | `#A86800` | `#F2B33D` | `#A6720A` |
| `startingSoft` | `#33280F` | `#FBF0D6` | `#33250E` | `#F7E4BE` | `#33280F` | `#F8EDD4` |
| `crashed` | `#FF5C5C` | `#D93A3F` | `#FF6B5A` | `#C2412E` | `#FF6B6B` | `#D13434` |
| `crashedSoft` | `#3A1A1A` | `#FCE3E3` | `#3A1E18` | `#F6DDD6` | `#3A1C1C` | `#FBE2E2` |
| `error` | `#FF8A3D` | `#C9580B` | `#FF8A3D` | `#B8560B` | `#FF9447` | `#B9520E` |
| `errorSoft` | `#2A1A0F` | `#FDEEE2` | `#2C1C0E` | `#F8E6D3` | `#2E1D10` | `#FCEDE1` |
| `stopped` | `#5A5F68` | `#A8ADB6` | `#5A5246` | `#A89B84` | `#5F5E5A` | `#A5A39D` |
| `stoppedSoft` | `#2A2D33` | `#DADDE3` | `#3A332A` | `#D6CAB4` | `#35353A` | `#D6D4CE` |

### Terminal ANSI palettes

| ANSI | Graphite dark | Graphite light | Patch Bay dark | Patch Bay light | Paper dark | Paper light |
|---|---|---|---|---|---|---|
| 0 black | `#1E2025` | `#1A1B1E` | `#2A251E` | `#2B241A` | `#2A2A2D` | `#1A1A1C` |
| 1 red | `#FF5C5C` | `#D93A3F` | `#FF6B5A` | `#C2412E` | `#FF6B6B` | `#CC2F2F` |
| 2 green | `#3DD68C` | `#128A4A` | `#9ACD6B` | `#4E8A1A` | `#3FCB7E` | `#1F8A4C` |
| 3 yellow | `#F5B83D` | `#A86F00` | `#FFB224` | `#A86800` | `#F2B33D` | `#9A6A00` |
| 4 blue | `#7C9CFF` | `#3B5BDB` | `#7FA7D9` | `#2F64A8` | `#8B8BFF` | `#3B3BD9` |
| 5 magenta | `#C792EA` | `#8E44AD` | `#D08BB8` | `#A0457D` | `#C99BFF` | `#8B3FC1` |
| 6 cyan | `#5FD7E8` | `#0E8399` | `#6CC4B0` | `#1F8573` | `#5CC9D6` | `#0F7F8F` |
| 7 white | `#C9CBD1` | `#D9DCE1` | `#D8D0C2` | `#E0D6C4` | `#D6D5D1` | `#D6D4CE` |
| 8 bright black | `#4D525B` | `#5F646D` | `#5A5246` | `#6F6453` | `#6E6D69` | `#6E6D69` |
| 9 bright red | `#FF7B7B` | `#E5484D` | `#FF8C7D` | `#D2402E` | `#FF8A8A` | `#D93A3A` |
| 10 bright green | `#6BE5A8` | `#18A058` | `#B5E08C` | `#5FA02A` | `#6BDDA0` | `#1F9D55` |
| 11 bright yellow | `#FFD06B` | `#C98A0B` | `#FFCB66` | `#C27F00` | `#FFCC6B` | `#B07800` |
| 12 bright blue | `#A3B8FF` | `#4A6CF7` | `#A3C2E8` | `#3F78C2` | `#A9A9FF` | `#5555E8` |
| 13 bright magenta | `#DDB3F5` | `#A660C8` | `#E3A9CE` | `#B85A93` | `#DAB8FF` | `#9F55D6` |
| 14 bright cyan | `#8AE6F2` | `#12A3BD` | `#92DCC9` | `#2A9C88` | `#86DCE6` | `#14A0B3` |
| 15 bright white | `#FFFFFF` | `#F4F5F7` | `#FFF8EC` | `#FBF8F2` | `#FFFFFF` | `#FBFBF9` |
