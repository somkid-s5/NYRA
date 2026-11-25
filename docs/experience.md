# NYRA — Appearance, Themes, Animations and Sound

## Locked decision: all cyberpunk themes
Do **not** choose one final color palette. Ship all four presets and a user-configurable theme engine. Color definitions are data and must not be scattered as hardcoded per-screen constants.

| Theme ID | Mood | Background | Accent 1 | Accent 2 | Accent 3 |
|---|---|---|---|---|---|
| cyber-cyan | Original futuristic cyan/magenta | #071B22 | #00E5FF | #FF39C6 | #0A3342 |
| neo-tokyo | Urban purple/pink/orange | #1A082C | #A855F7 | #FF35A7 | #FF942E |
| acid-grid | Hackery lime/cyan/yellow | #0B1712 | #B5FF00 | #00E5E5 | #F5E634 |
| crimson-noir | Red/violet/dark | #190810 | #FF285D | #BB6BD9 | #58203A |

These are starting palette tokens from discussion, subject to contrast testing. Error/warning/success should remain discernible by words/icons as well as color.

## Semantic theme tokens
background, surface, foreground, muted, primary/accent, secondary/accent, border, focus, selection, success, warning, error, disabled, progress. Validate custom hex values and contrast; handle terminals without truecolor using a documented fallback.

## Appearance settings
- Change theme at runtime without restart. Ctrl+T cycles themes; F9 opens the keyboard-driven appearance screen to preview and apply a built-in or valid loaded custom theme.
- Preview built-in and custom themes in F9; validate, import, and export custom TOML through the CLI.
- Persist choice with portable config or OS user config.
- Disable terminal colors with `NO_COLOR` or `[appearance].no_color = true`.
- F9 controls `animations`, `reduced_motion`, and `animation_speed` for the running-state activity indicator. Avoid decorative rendering on tiny terminals.
- Fall back to a usable neutral palette when the selected custom theme is missing or invalid; do not persist that temporary fallback automatically.

### Implemented scope (updated 2026-11-23)

NYRA loads `[appearance].theme` at TUI startup and saves the selected theme after a Ctrl+T or F9 appearance-screen change. TOML comments and unrelated settings are retained, including edits made while NYRA is open. A `portable.flag` beside the executable or one directory above it selects `config/config.toml` and `themes/custom.toml` under that same portable root. Otherwise Windows uses `%APPDATA%/NYRA`, Linux uses `$XDG_CONFIG_HOME/nyra` or `$HOME/.config/nyra`, and macOS uses `~/Library/Application Support/NYRA`; each location stores `config.toml` and `themes/custom.toml`. A malformed or unreadable settings file falls back to Cyber Cyan and is not overwritten. If the selected custom theme is missing or invalid, the TUI uses the session-only Safe Fallback palette until the user selects and saves another theme. If saving fails, the current session still works and the TUI reports that the choice was not saved.

To use a custom palette, validate and install it with `nyra theme validate examples/custom-theme.toml` and `nyra theme import examples/custom-theme.toml`, then open F9 and apply the custom theme. Import writes to the active config root's `themes/custom.toml` and refuses to replace an existing file unless `--force` is supplied. Export an installed theme with `nyra theme export <destination>`; it also refuses to overwrite unless `--force` is supplied. The v1 format requires `version = 1`, a 1–48 character name, and all 15 semantic colors as `#RRGGBB`. If `custom` is selected and its file is missing or invalid, NYRA uses Safe Fallback with a status message. The custom TOML example is executable input for the current parser; other fields in `examples/config.toml` remain planned unless stated above.

The appearance screen, theme preview/apply, CLI theme validate/import/export, session-only neutral fallback, and no-color mode are implemented. NYRA honors the standard `NO_COLOR` environment variable and `[appearance].no_color = true`. The Running screen shows a bounded indeterminate `Working` marker while a diagnostic is active; it never invents a completion percentage. The event loop checks input every 50 ms; F9 keys a/r/s toggle animation, toggle reduced motion, and cycle slow (300 ms), normal (150 ms), and fast (75 ms) cadence. Each change saves asynchronously and applies immediately for the current session. `[appearance].animations = false` or `[appearance].reduced_motion = true` makes it static. With both fields absent, motion is enabled in a local TUI and disabled in an SSH session; explicit configuration can opt into motion over SSH. When motion is enabled, startup shows a three-frame header spinner for at most 900 ms; any keypress skips it while still processing that input. Selection effects and audio remain planned.

### Command Knowledge TUI (2026-11-19)

F6 opens the offline command-card screen from the normal TUI screens. A tool search query is carried into the card search; all other entry points start with the complete local catalog. Results use deterministic relevance ranking over curated English metadata and tolerate small token typos; this is not AI or unrestricted natural-language search. Up/Down selects a card, PageUp/PageDown scrolls its full safety and usage details, Tab filters saved favorites, Ctrl+F toggles the selected card favorite, and Ctrl+Y sends the static command text with an OSC 52 clipboard request. Ctrl+K returns to global tool search. OSC 52 availability depends on the terminal, and the UI reports that limitation. Esc returns to the screen that opened the card catalog. Card commands are displayed or copied only and are never executed by NYRA. Knowledge-card favorite IDs are persisted asynchronously in `[knowledge].favorites` in the active portable or user config; if that write fails, the selection remains available for the current session and the UI reports it.

### Tool Favorites and Recents TUI (2026-11-19)

Ctrl+F toggles the selected tool's favorite in the Tools screen. F5 opens the saved-tools screen, where Tab switches between Favorites and Recent; Enter opens the selected tool form. Ctrl+R disables recent tracking, clears existing recent IDs, and saves `[discovery].record_recent = false`; enabling it again starts with an empty list. A tool enters Recent after a valid run starts while tracking is enabled. NYRA stores only stable tool IDs in `[discovery].favorite_tools` and `[discovery].recent_tools`, keeps at most ten recent IDs, and never records targets or input values there. Tracking defaults to enabled, including for older config files without the setting. Settings writes run asynchronously; write failures leave the current session usable and are shown in the status line. Esc returns to the screen that opened saved tools.

## Audio feature requirements
Events: startup, navigation, select, run start, success, warning, error, workflow complete. Controls: enabled/mute, volume 0..100, toggles per event, optional sound pack. Event sounds must never block async executor/TUI, interfere with output or prevent diagnostics.

**Important platform constraint:** audio engine is optional/capability-checked. SSH/headless/noninteractive CLI must default silent. Some audio backends require native OS libraries; do not assume all Linux distros are dependency-free. Never bundle unlicensed audio from Capri, games or creators.

## Visual effects
A three-frame header spinner follows the user motion settings and skips on any key; optional typing/boot cues and selection highlights, scan progress, success flashes, and error pulses remain planned. No GPU shader or imagined glassmorphism: these are terminal-cell/text/ANSI effects. Pause/skip animations on input; disable by reduced motion flag. Never print glitch effects into JSON CLI output.

## Example settings (proposed schema)
~~~toml
[appearance]
theme = "neo-tokyo"
animations = true
animation_speed = "normal"
reduced_motion = false

[sound]
enabled = true
volume = 40
startup = true
navigation = false
success = true
warning = true
error = true
workflow_complete = true

[knowledge]
favorites = []

[discovery]
favorite_tools = []
recent_tools = []
record_recent = true
~~~

`[knowledge].favorites` and `[discovery].favorite_tools`, `[discovery].recent_tools`, and `[discovery].record_recent` are currently read and written from this example; the other settings remain planned unless described in the implemented scope above.

## Test requirements
- Switching all themes affects every screen and preserves legibility.
- Custom themes load, reject malformed colors, save and reload.
- Reduced-motion and no-color modes usable.
- Sound controls work, and disabled audio devices never crash program.
- Color/animation do not change diagnostics, CLI JSON or exit status.
