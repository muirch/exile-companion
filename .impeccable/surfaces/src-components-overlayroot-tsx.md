---
version: 1
slug: "src-components-overlayroot-tsx"
primary_target: "src/components/OverlayRoot.tsx"
related_targets: ["src/components/StatusPanel.tsx","src/components/RoutePanel.tsx","src/components/SettingsApp.tsx","src/components/ReplayControls.tsx","src/styles.css"]
---

# Overlay

Scope: the always-on-top overlay window (locked route view, unlocked controls, startup notice). Mode: Operate.

Audience and job: a player mid-campaign glancing at the next action between fights and loading screens. Constraints: transparent window fitted to the content, anchored by its bottom center to the monitor, click-through when locked, English UI, no behavior or IPC changes.

Decided unattended (the user asked to proceed while away). The direction is pinned by the brief: Path of Exile 1's own panels and item tooltips, no neon.

## Direction contract

THESIS: The route reads like a Path of Exile item tooltip for the place you stand in: the zone is the item name in a tinted header band, what to do here are its properties, and the way out is the last line, "Then:". It refuses the web-app card and the generic brown-and-gold fantasy panel.

OWN-WORLD: The game's own tooltip grammar: see-through black body with no frame, a header band tinted by kind (sand for towns as currency, white for areas as normal items) with thin tint rules and scrollwork corners, short separators in the tint, everything centered in small caps. Text colours as the game uses them: grey labels, white values, magic blue for waypoints and portals, corrupted red for bosses, quest green, gem teal, unique brown for trials and arenas, hints in grey italic. Alegreya Sans SC throughout, Alegreya Sans italic for hints.

STORY: One glance answers where am I and what do I do here. A second glance shows what comes next.

FIRST VIEWPORT: The window is fitted to the tooltip and anchored by its bottom center to a fraction of the monitor, default in the gap between the flask bar and the skill bar, just above the experience bar. Header band with zone name and a subtitle of act, class level and step n / total. This area's actions at 15px or "Nothing to do here". Separator. "Then:" with the leaving step. Holding the tilde key opens a small tooltip-styled menu at the cursor. Settings live in their own window.

FORM: PoE item tooltip, candidate 1 of 7 (brief-pinned over the roll's assignment, the roll's challengers lend data discipline: tabular, aligned numerals, and size as rank, current steps brighter and larger than the preview). seed:9183aa20c9ada033

FINISH: unreviewed and undocumented is unfinished. This build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
