---
name: Exile Companion
description: A passive Path of Exile leveling overlay that reads like the game's item tooltip for the area you stand in.
colors:
  void: "#000000"
  char: "#0a0a0a"
  soot: "#1b1b1b"
  edge: "#3a3a3a"
  edge-bright: "#6b6b6b"
  label: "#8a8a8a"
  plain: "#a8a8a8"
  normal: "#c8c8c8"
  value: "#ececec"
  currency: "#aa9e82"
  magic: "#8888ff"
  quest: "#4ae63a"
  corrupted: "#e0443a"
  unique: "#c2703a"
  gem: "#1ba29b"
  crafted: "#b4b4ff"
typography:
  display:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "19px"
    fontWeight: 500
    lineHeight: 1.25
  subtitle:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "14px"
    fontWeight: 500
    lineHeight: 1.25
  body:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "15px"
    fontWeight: 500
    lineHeight: 1.375
    fontFeature: "tnum"
  body-sm:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "14px"
    fontWeight: 500
    lineHeight: 1.375
    fontFeature: "tnum"
  label:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: 1.375
    fontFeature: "tnum"
  button:
    fontFamily: "Alegreya Sans SC, Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1
  flavour:
    fontFamily: "Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.375
  caption:
    fontFamily: "Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.375
  key-hint:
    fontFamily: "Alegreya Sans, ui-sans-serif, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.375
    fontFeature: "tnum"
rounded:
  none: "0px"
spacing:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
  2xl: "20px"
components:
  tooltip-header-area:
    textColor: "{colors.currency}"
    typography: "{typography.display}"
    rounded: "{rounded.none}"
    padding: "4px 30px 5px"
  tooltip-header-town:
    textColor: "{colors.currency}"
    typography: "{typography.display}"
    rounded: "{rounded.none}"
    padding: "4px 30px 5px"
  tooltip-body:
    backgroundColor: "{colors.void}"
    textColor: "{colors.value}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    padding: "6px 16px 12px"
  overlay-plate:
    backgroundColor: "{colors.void}"
    rounded: "{rounded.none}"
    width: "404px"
  button:
    backgroundColor: "{colors.soot}"
    textColor: "{colors.normal}"
    typography: "{typography.button}"
    rounded: "{rounded.none}"
    padding: "5px 9px 6px"
  button-hover:
    textColor: "{colors.value}"
  button-pressed:
    textColor: "{colors.value}"
  button-active:
    backgroundColor: "{colors.char}"
  menu:
    backgroundColor: "{colors.void}"
    rounded: "{rounded.none}"
    width: "228px"
  menu-row:
    textColor: "{colors.value}"
    typography: "{typography.body-sm}"
    padding: "6px 12px"
  settings-section:
    backgroundColor: "{colors.void}"
    rounded: "{rounded.none}"
    padding: "12px 20px 16px"
  settings-row-label:
    textColor: "{colors.label}"
    typography: "{typography.label}"
    width: "128px"
---

# Design System: Exile Companion

## Overview

**Creative North Star: "The Item Tooltip of Where You Stand"**

Every surface Exile Companion draws is a Path of Exile item tooltip. The zone is the item name in a tinted header band, the things to do there are its properties, and the way out is the last line, "Then:". The overlay, the quick menu and each settings group use the same parts. There's a see-through black body with no frame, a header band with rules along both edges and wedge ends that meet in a diamond, short glowing separators, centered small caps, and text colours that mean what they mean in the game.

The system is dense and quiet. It is read in glances over a busy, dark game scene, so it has no fills, gradients or accents beyond what the game's own tooltips carry, and colour is spent only on meaning. A boss name is red because the game's corrupted red is the danger colour, a waypoint is blue because it is magic. The overlay window is transparent and fitted to its content. Only the plate paints. The settings window has no system frame. It has a title bar like the overlay's move bar, a 1px Edge window edge, and groups that are still tooltips standing on a dark ground.

It rejects the web-app card (rounded corners, drop shadows, coloured buttons) and the generic brown-and-gold fantasy panel. The chrome is neutral grey and black. The only warm hue in the frame is Currency Sand in the header bands.

**Key Characteristics:**
- See-through black bodies, opacity set by the player (70 to 100 percent on the overlay, lower washes out over bright scenes).
- A header band per tooltip, tinted by kind, with 1px rules along its top and bottom edges and, at each end, a wedge drawn from both corners to a 7px diamond on the middle.
- Alegreya Sans SC at medium weight for all tooltip text. Alegreya Sans italic for flavour.
- Square corners everywhere.
- Tabular numerals throughout.

## Colors

A greyscale frame with the game's item text colours as the only hues, each used for its in-game meaning.

### Primary
- **Normal Item White** (#c8c8c8): the chrome colour. Button text and pressed borders, the slider thumb, the focus outline, the replay progress fill.

### Secondary
- **Currency Sand** (#aa9e82): the one header tint. Every header band, its frame, diamonds and separators use it, on the overlay, the quick menu and the settings groups.

### Tertiary
These are route semantics, never decoration.
- **Magic Blue** (#8888ff): waypoints, portals and logout.
- **Quest Green** (#4ae63a): quest hand-ins and quest items, "Act N done" on the first step of the next act and the end-of-campaign line. Also the "On" state in the quick menu.
- **Corrupted Red** (#e0443a): bosses to kill. Hotkey bind errors.
- **Unique Brown** (#c2703a): arenas, trials and ascension.
- **Gem Teal** (#1ba29b): gem rewards and the build's gem claims.
- **Crafted Lavender** (#b4b4ff): crafting recipes.

### Neutral
- **Void** (#000000): tooltip bodies, always with alpha (overlay at the player's opacity, menu 90 percent, settings groups 80 percent, startup notice 85 percent). Also the 1px keyline around slider and progress tracks.
- **Char** (#0a0a0a): the settings window ground under its two faint radial washes (#23251c top left, #251d16 bottom right). The bottom of the button gradient.
- **Soot** (#1b1b1b): the top of the button gradient. Slider track.
- **Edge** (#3a3a3a): button border, the move bar's bottom rule, progress track.
- **Edge Bright** (#6b6b6b): button border on hover.
- **Label Grey** (#8a8a8a): property labels, "Then:", settings row labels, hints, key hints, "Off".
- **Plain Grey** (#a8a8a8): default body text outside a tooltip role.
- **Value White** (#ececec): values, area names and the current actions. Hover text.

### Named Rules
**The Game's Meaning Rule.** A hue appears only where the game uses it for the same meaning. New content picks the matching item text colour. If the game has none for that meaning, it stays grey or white.

**The Grey Ladder Rule.** Brightness is rank. Label Grey is for what something is, Plain Grey for running text, Normal Item White for chrome, Value White for what the player acts on. On the overlay every grey starts one step up, at Plain Grey, because the plate sits over snow and bright light as often as over dark ground. Label Grey stays for the menu and settings, which have their own dark ground.

**The Tint by Kind Rule.** One tint per tooltip drives its band, frame, diamonds and separators together. Every header uses Currency Sand, which sits well on the black body. Towns are told apart by the "town" subtitle. Buttons and focus stay neutral grey.

## Typography

**Display Font:** Alegreya Sans SC (with Alegreya Sans, ui-sans-serif)
**Body Font:** Alegreya Sans SC at medium weight inside tooltips. Alegreya Sans for flavour and captions

**Character:** A humanist sans in true small caps, close to the game's tooltip voice without copying its face. The lowercase cut appears only where the game would switch voice or where caps would garble the text.

### Hierarchy
- **Display** (500, 19px, 1.25): the tooltip title (zone name, "Exile Companion", settings group name). Truncates on one line.
- **Subtitle** (500, 14px, 1.25, 85 percent opacity): the line under the title. On the overlay it carries act, class level and step.
- **Body** (500, 15px, 1.375): this area's actions.
- **Body small** (500, 14px, 1.375): the "Then:" line, one step under the current actions so they win the first glance. Quick menu rows, "route didn't load".
- **Label** (500, 13px, 1.375): the compact zone line, "Nothing to do here", settings row labels and values, the move bar's "Exile Companion".
- **Button** (500, 12px, 1): every button and the hold-key select.
- **Flavour** (Alegreya Sans italic, 400, 13px, 1.375): route hints, the startup notice, the missing-log hint (upright).
- **Caption** (Alegreya Sans italic, 400, 12px): settings explanations. Upright for bind errors.
- **Key hint** (Alegreya Sans, 400, 11px): shortcut names next to buttons and menu rows, the move bar hint, replay line count.

### Named Rules
**The Small Caps Voice Rule.** Everything in a tooltip is Alegreya Sans SC at 500. Switch to Alegreya Sans for flavour (italic) and for literal strings like key names, file paths and speed multipliers.

**The Size as Rank Rule.** The current actions are the largest body text (15px) and the brightest. The zone line and labels drop to 13px grey. Key hints sit at 11px. Do not add a size between these steps.

## Layout

Each window is laid out around a single centered column of tooltips.

The overlay plate is 404px wide at 100 percent text size and is scaled as a whole, via CSS zoom, by the player's text size (80 to 150 percent) times the monitor height over 1080, the way the game scales its own UI. The window is fitted to the plate plus 8px of padding and keeps its bottom center fixed on the monitor, so anything that opens (the move bar and its buttons, a longer step list) grows upward. Inside the body, content sits at 16px horizontal padding, 6px top, 12px bottom. Steps stack with 4px gaps. Separators take 7px above and below.

The quick menu is a 228px tooltip at the window's top left, 4px in. Rows are full-width, 6px by 12px, label left and state or key hint right.

The settings window has a 32px title bar on top, with "Exile Companion" in Normal Item White and "Settings" in Label Grey on the left, a 24px square close button on the right, black under an Edge rule. Dragging the bar moves the window. The content scrolls under it, with an 8px Edge scrollbar thumb. Below the bar it centers a column up to 34rem wide with 20px outer padding and 16px between groups. Inside a group, bodies are left-aligned with 20px sides, 12px top, 16px bottom, and rows 8px apart. Each row is at least 28px tall with a fixed 128px label column and a 12px gap. Explanatory text under a row indents 140px to line up with the controls.

The spacing scale is 4, 6, 8, 12, 16 and 20px. The header band's 4/30/5px padding is its own.

## Elevation & Depth

Flat. Depth comes from see-through black over the game, not from shadows. The tooltip body has no border and no shadow, and the only lift is the header band's own gradient (a 6 percent white sheen at the top fading to 40 percent black at the bottom, over the tint mixed 20 percent into near-black #050505, dark enough for the sand title to clear 4.5:1).

### Shadow Vocabulary
- **Button inner sheen** (`box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.04)`): on every button, inside the border.
- **Track keyline** (`box-shadow: 0 0 0 1px #000`): around the slider track and the replay progress bar.

### Named Rules
**The No Frame Rule.** A tooltip body has no border and no shadow. The only frame is the pair of 1px tint rules inside the header band, 2px from its top and bottom and stopping 40px short of each end. The one exception is state. An unlocked overlay gets a 1px Normal Item White outline at 60 percent, 2px out, while it can be dragged.

## Shapes

Square. Every surface, button, select and track has a 0px radius. There are no curves. The rotated forms are the header diamonds (7px squares turned 45 degrees, centered on the frame's short sides, with a 1px near-black ring) and the slider thumb (a 9px diamond). Separators are 1px lines, 30 percent of the body width, fading from the tint at the center to nothing at 70 percent.

Small marks are drawn as inline SVG in the current text colour. They are the 8-way direction arrow (14px, value white, rotated in 45 degree steps) and the "go" arrow (12px, label grey) that replaces the route text's arrow glyph.

### Named Rules
**The Square Corner Rule.** Nothing is rounded. Ornament lives only in the header's wedge ends and their diamonds. Every line in it meets another.

## Components

### Buttons
Small, dark and neutral, like the game's own panel buttons. Square, a 1px Edge border, a vertical gradient from Soot to Char with the inner sheen, Normal Item White text in the button face at 5px 9px 6px padding. Hover brightens the border to Edge Bright and the text to Value White over 150ms ease-out. Active drops the face to near-black. Pressed or toggled-on (`aria-pressed`) sets the border to Normal Item White and the text to Value White. This is how speed choices, the recording hotkey button and On/Off switches show state. Disabled is 40 percent opacity. Buttons carrying literal strings (speeds, key bindings) switch to Alegreya Sans. The hold-key select uses the button styling.

### Tooltip header band
The signature component. A tint is passed in. The band fills with it mixed 30 percent into near-black under the sheen gradient, its title and subtitle are set in the tint, its edge rules and wedge ends are the tint at 80 percent, and the diamonds on the wedge points are solid tint. Title is display type, centered and truncated. The subtitle sits below at 85 percent opacity.

### Tooltip body
See-through black, centered, small caps. On the overlay the order is the band (zone name, subtitle with act, class level and step, joined by middle dots), then actions (or "Nothing to do here" plus the way out in value white, gem steps after their quest), separator, "Then:" line. Hints under an action are flavour type in Plain Grey. When the player stands somewhere the current step isn't for (a hideout, a map, an area off the route), a Label-size Plain Grey line "Off route, showing your last step" sits above the actions. On the first step of an act a Label-size Quest Green "Act N done" for the act just finished sits there too, and on the last step of the campaign "That's the campaign. On to the maps." takes the place of the "Then:" line. When unlocked, a move bar sits above the band with "Exile Companion" on the left, the key hint ("Drag to move. Esc or Alt+M locks.") and a Lock button on the right, and under it a row of Back / Next buttons with the step shortcuts between them, each closed by an Edge rule. Nothing opens below the band, so the route keeps its place when the overlay locks again. Locking hands keyboard focus back to the game.

### Compact overlay
The same see-through black plate at 340px, with no header band, separators or hints. It is the default layout. Route hints stay hidden, except for six seconds after a tap of the menu key. A Plain Grey label line names the zone, act and step ("Lioneye's Watch · Act 1 · 2 / 24"), then the actions at Body size and the "Then:" line, all centered small caps at 4px gaps inside 12px sides, 6px top and 8px bottom. It is for players who know the campaign and want the route to cover less of the game. Either layout can fade out after 5, 10 or 30 seconds without a new step, over 700ms. A new step, a new area, the game coming back to the front or a tap of the menu key shows it again at once, and it never fades while unlocked.

### Quick menu
A tooltip whose header band carries only the title. Rows are body-small Value White text, no border. Hover and keyboard focus fill the row with white at 6 percent and drop the outline. Toggle rows (Show route, Full layout) show "On" in Quest Green or "Off" in Label Grey on the right. Separators split four groups. They are the toggles, Previous step and Next step with their keys (the menu stays open so a wrong step can be fixed in a few clicks), the window actions, and Quit, which asks "Click again to quit" first. Esc closes it, Up and Down walk the rows, and the first row takes focus on open.

### Settings groups
Each group is its own tooltip at 80 percent black with a Currency Sand header band. Rows pair a small-caps grey label with controls such as buttons, value text in Value White, sliders with a live percentage, and file paths in Alegreya Sans, truncated with the full path in the title. The build's presets are one row each under a single "Presets" label, with the preset name in Alegreya Sans Value White, truncated, and a 160px stage select on the right, or "No gems" in grey italic for a separator. Under the gem setting, a "Not in campaign" row lists the build's gems no quest or vendor gives this class, in Alegreya Sans Value White. Buttons that drop something (Start over, Clear, Stop) take two clicks. The first turns the button pressed and asks "Click again to …", and it settles back after 3 seconds. Log replay is a testing tool and sits behind a "Testing" row with a Show button, so "Pick Client.txt" stays the only action in the Game log group.

### Range slider
A 3px Soot track with the black keyline and a 9px Normal Item White diamond thumb with a black border, brightening to Value White on hover.

### Focus
Keyboard focus is a 1px Normal Item White outline 2px out, everywhere except menu rows, which use the row fill.

## Do's and Don'ts

### Do:
- Do build every new panel as a tooltip, with a header band tinted by kind over a see-through black body with no frame.
- Do colour route content only with the game's item text colours, by meaning (Magic Blue for waypoints and portals, Corrupted Red for bosses, Quest Green for quests, Unique Brown for trials and arenas, Gem Teal for gems).
- Do keep the overlay's bottom center fixed and let new content grow upward.
- Do set tooltip text in Alegreya Sans SC 500 and switch to Alegreya Sans for flavour, key names and paths.
- Do draw small marks as inline SVG in currentColor.
- Do show state on buttons with the pressed treatment (Normal Item White border, Value White text), not with a fill colour.

### Don't:
- Don't round corners or add drop shadows. The system is square and flat.
- Don't put a border around a tooltip body. The frame belongs to the header band.
- Don't bring back the brown-and-gold fantasy chrome. The frame is neutral grey and black.
- Don't use a route colour for decoration or for chrome.
- Don't add a type size between the existing steps (11, 12, 13, 14, 15, 19px).
- Don't use the game's header art or GGG logos. The header wedges and diamonds are Exile Companion's own.
