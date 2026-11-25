# NYRA — Interactive TUI UX Specification

## Purpose and target style
Create a **real terminal application** with Rust/Ratatui, not a web dashboard rendered inside terminal. Inspired by Capri's cyberpunk energy, neon borders, short glitch/boot cues and optional sound, while remaining practical and keyboard-first. All screen layouts must be implementable with terminal cells, Unicode characters and truecolor/fallback palettes.

## Mode entry and journeys
- Launch \`nyra\` to open interactive TUI.
- First-use: available local tools shown immediately, search focused or quick actions visible; do not require account/login/network.
- Main journey: search "check port 443" → TCP Port Check → form (host, port, timeout) → validate → run → progress → result/status/timing/copy/export.
- Alternative journey: choose Network category → DNS Lookup → hostname → run → output.
- Frequent-user journey: Favorites → select saved tool/target → run.
- Workflow journey: F2 or menu → choose Web Unreachable → target URL → ordered progress/summary/report.
- Knowledge journey: search "disk full" → Linux Disk Health and related command reference.
- Advanced: Direct CLI commands remain separately scriptable with same semantics.

## Screen composition
**Header**: NYRA logo/name/version, platform, offline/core status, sound indicator, running task indicator. Avoid false "offline" promises if optional network diagnostic actively connects to targets; offline-first refers to no backend requirement.

**Global Search**: searchable input, keyboard hint, fuzzy matches/ranked intent tags, no full language model required.

**Wide**: left category navigation, center tool list/search results, right selected tool detail + dynamic input fields; lower expandable result viewer; persistent footer shortcuts.

**Compact**: categories/tools on left, detail/results on right; collapsed categories toggle.

**Minimal**: one-column list/detail/results sequential screen navigation; designed to remain usable around 80x24 terminal cells. Show overflow scrollbars/indicators and never clip critical errors silently.

**Primary screens**: Dashboard/Home, Tool Explorer, Tool Detail, Dynamic Form, Running Task/Results, Workflows/Templates, Workflow Runner, Command Knowledge, Favorites/Recent, Profiles, Reports, Settings/Appearance/Sound, Help.

## Interaction model
| Key | Proposed behavior |
|---|---|
| Up/Down | Navigate current list |
| Tab / Shift+Tab | Move form or pane focus |
| Enter | Open item, confirm or submit when valid (depends on focus) |
| Ctrl+K | Search globally, from normal navigation state |
| / | Search current tool library when not editing input |
| F1 | Contextual help |
| F2 | Workflow list |
| F5 | Open saved tools (Favorites / Recent tabs); Recent stores at most 10 tool IDs only |
| F6 | Command Knowledge |
| Ctrl+F | Toggle the selected tool or knowledge-card favorite, depending on screen |
| Ctrl+R | In Saved Tools, disable recent tracking and clear stored recent IDs; press again to enable a fresh list |
| Ctrl+Y | Send the selected knowledge command to the terminal clipboard with OSC 52; support varies |
| F9 | Appearance settings (theme and motion controls; sound planned) |
| a / r / s in F9 | Toggle animation, toggle reduced motion, and cycle indicator speed |
| Esc | Back/close overlay |
| Ctrl+C | Cancel running operation (first), then exit when safe |
| Ctrl+E | Export currently displayed result if available |
| ? | Help shortcut when not editing a form |

Avoid hijacking typing in inputs; test key behavior across Windows Terminal and Linux shells. Offer alternative menu navigation when F-keys are intercepted by host terminal.

In the current TUI, Ctrl+K opens global tool search, F5 opens the saved-tools screen, and F6 opens Command Knowledge. Ctrl+F only acts in the Tools, Saved Tools, or Command Knowledge screens; it does not consume characters in editable form fields. In Saved Tools, Ctrl+R toggles `[discovery].record_recent`; disabling it clears saved recent IDs, while enabling it again starts with an empty list. The setting defaults to `true` when absent. Knowledge-card commands are shown or copied, never executed. Tool favorites and up to ten recent tool IDs are stored in the active config without target or input values.

## Tool form requirements
- Generated from typed registry metadata; show required fields, default values, validation and tool support.
- Inputs: hostname/IP, port 1..65535, URL (https), path, timeout, enum, booleans.
- Never pre-fill secrets from persistent plaintext files.
- Inline validation and helpful error text, not generic panic.
- Visible primary action and cancel/escape path.
- Show tool backend and readiness (built-in, system, external requires setup).
- Surface authorization and risk before any non-read-only invocation.

## Results viewer
- Structured summary with status, target, elapsed time, warnings, next steps and raw details/JSON view.
- Distinguish "connection refused" as diagnostic outcome vs tool execution bug.
- Scrolling, selection, copy, export; avoid logging secrets.
- Optional colored status lines supplemented by icons/words (not color alone).
- Async task progress and cancellation; do not freeze event/render loop.
- While a diagnostic runs, show an indeterminate `Working` marker without a fabricated percentage. The marker advances at a bounded cadence, remains text-readable, and becomes static when animations are disabled, reduced motion is enabled, or SSH defaults apply. F9 provides animation, reduced-motion, and speed controls for diagnostic runs in the current TUI session. Cancellation text remains visible.
- When motion is enabled, show a three-frame header startup spinner; end it within 900 ms and let any key skip it while keeping the same input action. Disabled animation, reduced motion, and default SSH settings keep startup static.
- Show actual, measured timings only. Mockups use example-only metrics.

## Design principles
- Hierarchy beats decoration: high contrast, clear focus, compact spacing, fast scan.
- Animations may enhance feedback but must be short/nonblocking; respect reduced motion.
- Sounds must never play in headless/non-interactive CLI by default.
- Themes are semantic tokens; errors/success must stay distinguishable.
- Use Unicode-aware cell sizing and test combined characters and fallback fonts.
- Accessibility: no reliance on color alone, focus visible, keyboard available, hints consistent.
- Never imply truly clickable terminal buttons if only keyboard support is implemented.

## UX acceptance walkthroughs
1. User starts tool and runs TCP test in under ~5 interactions after target entry; keyboard-only.
2. Search "check port" and "test TCP connection" yields the same tool in top results.
3. Bad port/input produces actionable inline error.
4. Connection timeout responds and returns UI control, with cancel available.
5. Entering/editing field does not trigger global shortcuts incorrectly.
6. Terminal resize wide→compact→80x24 preserves controls and context.
7. Theme switching works without process restart; no lingering visual corruption.
8. Error handling leaves terminal canonical state restored.
9. Favorites persist when workspace is writable, and explain failure if USB readonly.
10. Run path and result model match CLI output (same core function).
