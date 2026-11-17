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
- Change theme at runtime via Settings (proposed F9) without restart.
- Preview built-in and custom themes; import/export custom TOML.
- Persist choice with portable config or OS user config.
- animation_enabled, animation_speed, reduced_motion; avoid decorative rendering on tiny terminals.
- Fallback to usable neutral palette on invalid custom theme.

## Audio feature requirements
Events: startup, navigation, select, run start, success, warning, error, workflow complete. Controls: enabled/mute, volume 0..100, toggles per event, optional sound pack. Event sounds must never block async executor/TUI, interfere with output or prevent diagnostics.

**Important platform constraint:** audio engine is optional/capability-checked. SSH/headless/noninteractive CLI must default silent. Some audio backends require native OS libraries; do not assume all Linux distros are dependency-free. Never bundle unlicensed audio from Capri, games or creators.

## Visual effects
Compact startup animation; optional short typing/boot cue; selection highlight, scan progress, success flash and error pulse. No GPU shader or imagined glassmorphism: these are terminal-cell/text/ANSI effects. Pause/skip animations on input; disable by reduced motion flag. Never print glitch effects into JSON CLI output.

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
~~~

## Test requirements
- Switching all themes affects every screen and preserves legibility.
- Custom themes load, reject malformed colors, save and reload.
- Reduced-motion and no-color modes usable.
- Sound controls work, and disabled audio devices never crash program.
- Color/animation do not change diagnostics, CLI JSON or exit status.
