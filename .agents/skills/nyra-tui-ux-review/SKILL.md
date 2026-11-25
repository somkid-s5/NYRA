---
name: nyra-tui-ux-review
description: Use in the NYRA repository to review Ratatui terminal flows, keyboard and focus behavior, responsive layouts, accessibility, or TUI tests. Ignore unrelated repositories.
metadata:
  version: "1.0.0"
---

# NYRA TUI UX Review

Use this skill only when the current repository is NYRA. Read `AGENTS.md`, `docs/tui-ux.md`, `docs/experience.md`, and the relevant architecture and testing guidance before evaluating a flow.

## Review the real terminal task

- Follow the user's path from discovery through input, execution, results, cancellation, and return navigation. Use evidence from the implementation or tests; mark a mockup or planned behavior as unverified.
- Check wide, compact, and 80x24 layouts. Critical controls and errors must remain reachable and visible without silent clipping.
- Check focus and key handling in lists and editable fields. Typing must not trigger global shortcuts unexpectedly; F-key alternatives must exist when host terminals intercept them.
- Check resize, Esc/back, cancellation, Ctrl+C, and terminal restoration on normal and error paths.
- Check Thai and English display widths with Unicode-aware cell sizing. Do not assume string byte length equals terminal width.
- Check focus visibility, keyboard access, readable contrast, and status cues that do not rely on color alone.
- Check reduced motion, interruptible animation, and silent headless or CLI behavior. Audio must remain optional and must not block interaction.
- Verify that the TUI calls the shared tool implementation and does not duplicate CLI or workflow logic.

## Evidence and reporting

Prefer Ratatui TestBackend for render and state checks, plus PTY testing for terminal behavior where available. Use the project’s actual test setup; Playwright is not the primary test method for this terminal app. Report findings with the affected state or screen, reproduction path, impact, and evidence. Separate usability issues from unverified design proposals.
