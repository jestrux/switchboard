# 4. Design handoff gaps

**Status: Ready**, independent of the other items. Next step: dim the refused split button, the smallest item and already backed by `WindowContext.splitRefusal`. Then the divider readout, then directional focus.

The redesign in [`design-handoff/`](design-handoff) is implemented except for the items
below. `DESIGN.md` is the spec; `HANDOFF.md` has the build order and acceptance criteria;
the mockups named here are in `design-handoff/mockups/` and need their markup read, not
opened in a browser.

## Not done

| Item | Spec | Notes |
|---|---|---|
| Extraction and return animations | `DESIGN.md` "Windows", `X-Extract.dc.html` | Tile lifts 180 ms, window zooms from the tile, rail settles 220 ms; on return a 600 ms accent halo. Reduce Motion: 120 ms crossfade. Today a project just appears and disappears. Durations are in `SBMotion`. |
| ⌘⌥ + arrows to move pane focus | `DESIGN.md` "Splits", "Keyboard" | ⌘[ and ⌘] cycle. Directional focus needs pane frames, which `WindowContext.paneSizes` only half provides (sizes, not origins). |
| Divider drag readout | `X-SplitCtl.dc.html` | Show columns × rows per side while dragging, and snap at the minimum pane size. 50 % snap and double-click-to-even exist. |
| Refused split dims the button | `DESIGN.md` "Splits" | Only the toast exists. Use `WindowContext.splitRefusal` to disable the bar button and put the reason in its tooltip. |
| Invalid config in a project window | `X-WinStates.dc.html` | Should keep the last good tree and running sessions with the problems under the title. Today the tree is replaced by an error card. Needs the engine to keep the last good `effective` config beside the error. |

## Partial

- **Hover actions should reserve their slot** so rows do not shift. Rows currently swap meta
  text for buttons in the same trailing area.
- **Board header's Open in New Window** is a borderless icon; the mockup has a 28 pt bordered
  button (`Win.dc.html`).
- **Rail tile selection ring** approximates the 2 pt gap plus 2 pt ring (`Shell.dc.html`).
- **Contrast.** Acceptance asks for 4.5:1 on text. The built-in palettes came from the
  handoff as given. The CityBuild and Kinpay custom palettes were derived by hand and never
  measured.

## Acceptance criteria still to verify

From `HANDOFF.md`: every spec board reproducible in all three themes in light and dark;
every hover action has a menu item; every icon button has a label and a tooltip with its
shortcut. These were checked by eye on the main flows, not audited board by board.

## Method that worked

Take a window screenshot with `scripts/dev/uidrive.swift`, put it beside the mockup's
markup, and compare spacing values directly. The top-padding bug was found that way: the
mockups give exact numbers (`padding: 44px 14px 12px`, `height: 52px`).
