# NYRA — Design References and Mockup Notes

## Visual inspiration
The original user referred to the Capri cyberpunk-style terminal/AI companion concept. The aesthetic direction was a realistic terminal-native TUI with neon colors, panels, scan effects, startup personality and optional sound. This does **not** authorize copying Capri's proprietary code, logo, artwork or sounds; inspect licenses for any actual reuse.

## Agreed visual approach
- Realistic Ratatui terminal layout; no fake GPU effects or web UI pretending to be TUI.
- Shared spatial layout for all four palettes: header + status, search, category nav, tool list, selected tool configuration, result console and shortcut/footer.
- Wide three-column dashboard with smaller two-column and one-column fallback.
- Primary flow example shown in mockup: TCP Check at host 192.168.1.10 port 443; any response and latency shown are **illustrative only**, not live measurements.
- Semantic color tokens and customize-from-config; colors change while layout/interaction remain stable.

## Four themes
1. **Cyber Cyan**: original cyan/magenta/navy.
2. **Neo Tokyo**: purple/pink/orange.
3. **Acid Grid**: lime/cyan/yellow on very dark green.
4. **Crimson Noir**: crimson/violet/black.

See [Appearance Spec](experience.md) for exact starter HEX tokens.

## Repo-based editable mockups
Four SVG *wireframe concepts* are included in assets/mockups/. They intentionally use terminal-compatible text/panels rather than recreating raster renders. These are **new planning previews**, not the exact previously generated chat images:
- [Cyber Cyan](../assets/mockups/nyra-cyber-cyan.svg)
- [Neo Tokyo](../assets/mockups/nyra-neo-tokyo.svg)
- [Acid Grid](../assets/mockups/nyra-acid-grid.svg)
- [Crimson Noir](../assets/mockups/nyra-crimson-noir.svg)

**Original chat-generated PNG mockups:** those binary assets were created in the conversation but are not uploaded here as GitHub repository blobs by this text-only connector workflow. Treat SVGs as editable source-of-truth visual guidance; if raster originals are required in the repository, upload/commit them separately with permissions/licensing verified.

## Intended design fidelity
The wireframes define hierarchy and terminal-cell feasibility. Production Ratatui renders must be compared using screenshot/snapshot tests and manual viewing in Windows Terminal and Linux terminal emulators. No preview is evidence of implemented functionality.
