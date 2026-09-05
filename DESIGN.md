---
name: envy landing
description: Violet-black landing where the live terminal workstation fills the first viewport and the page persuades by demonstrating, not decorating.
colors:
  bg: "#0d0716"
  bg-raise: "#150c24"
  bg-sink: "#0a0512"
  line: "#2a1b45"
  line-soft: "#1d1233"
  ink: "#ece7f8"
  ink-2: "#b1a5d6"
  ink-3: "#9184bb"
  violet: "#8a2be2"
  violet-2: "#7b68ee"
  green: "#4cc38a"
  amber: "#d8a13a"
  red: "#f26155"
typography:
  display:
    fontFamily: "Bricolage Grotesque, sans-serif"
    fontSize: "clamp(40px, 6.4vw, 86px)"
    fontWeight: 800
    lineHeight: 0.98
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "Bricolage Grotesque, sans-serif"
    fontSize: "clamp(28px, 4vw, 44px)"
    fontWeight: 700
    lineHeight: 1.05
    letterSpacing: "-0.02em"
  title:
    fontFamily: "Bricolage Grotesque, sans-serif"
    fontSize: "20px"
    fontWeight: 650
    lineHeight: 1.05
    letterSpacing: "-0.02em"
  body:
    fontFamily: "Fira Sans, sans-serif"
    fontSize: "17px"
    fontWeight: 400
    lineHeight: 1.65
  label:
    fontFamily: "Fira Code, monospace"
    fontSize: "13px"
    fontWeight: 500
  code:
    fontFamily: "Fira Code, monospace"
    fontSize: "0.92em"
    fontWeight: 400
rounded:
  sm: "5px"
  md: "10px"
  lg: "14px"
spacing:
  pad: "clamp(20px, 5vw, 72px)"
  section-top: "clamp(56px, 9vw, 120px)"
  row-gap: "28px"
  row-pad: "30px"
components:
  button-primary:
    backgroundColor: "{colors.violet}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "11px 20px"
  button-secondary:
    backgroundColor: "{colors.bg-raise}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "11px 20px"
  button-copy:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.ink}"
    rounded: "9px"
    padding: "9px 16px"
---

# Design System: envy landing

## Overview

**Creative North Star: "The Self-Demonstrating Workstation"**

The page will behave like the product it sells: a real terminal console will fill the entire first viewport, running the actual workstation recording edge to edge, annotated in the product's own mono voice. Everything below the fold will stay editorial and quiet so the console stays the loudest object on the page. Depth will come from tonal layering (three violet-black surfaces) plus translucent scrims, never from ornamental chrome. Color will be scarce: one brand gradient reserved for the wordmark, one flat violet for action, three status colors that only ever mean state.

A future builder will extend this page by adding editorial rows, table rows, or FAQ entries inside the existing containers; the system will absorb new content without new visual vocabulary. New sections will reuse the `section` / `.wrap` rhythm (lines 71-72), the `.frow` row pattern (lines 180-188), and the token set on `:root` (lines 42-53) rather than introducing parallel primitives.

**Key Characteristics:**
- First viewport will be a full-bleed product console under a fixed blurred nav, never a floating screenshot card.
- The ENVY gradient (#8A2BE2 → #7B68EE) will appear only on the brand mark and wordmark.
- Fira Code will own every terminal, code, and data surface; Bricolage Grotesque will own display; Fira Sans will own prose.
- Depth will be tonal plus scrim, with exactly two hard shadows on the page.
- All authored motion will sit inside a `prefers-reduced-motion` gate.
- Copy voice will be plain, honest, and free of emoji and em-dashes.

## Colors

The palette will be a violet-black world with three surface depths, three text tiers, one brand hue in two steps, and three functional status colors.

### Primary
- **Violet** (#8a2be2): the only action color. It will fill primary buttons flat (`.btn.primary`, line 100), tint the featured table row (`tr.ours`, line 215), square the security list bullets (line 199), and anchor the selection and scrollbar states (lines 61, 64). It will never carry a gradient on interactive elements.

### Secondary
- **Violet 2** (#7b68ee): the soft end of the brand hue. It will color links (line 65), focus rings (line 67), the copy confirmation border pair (line 168), the FAQ plus glyph (line 231), and the small square list markers. It will terminate the wordmark gradient.

### Tertiary (functional status only)
- **Green** (#4cc38a): positive verdicts (`td.yes`, line 213) and the copied state (`.copy.done`, line 176).
- **Amber** (#d8a13a): appears only inside the drawn console chrome dots (line 118).
- **Red** (#f26155): negative verdicts (`td.no`, line 214) and the first chrome dot (line 117).

### Neutral
- **Ground** (#0d0716): the page background, body default (line 57).
- **Raised** (#150c24): nav, console bar, table header, install bar base, inline code chips.
- **Sink** (#0a0512): the console interior and scrollbar trough; the darkest recess.
- **Line** (#2a1b45): container borders (buttons, console, install bar, table frame).
- **Line soft** (#1d1233): hairline dividers between siblings (rows, cells, details, footer).
- **Ink** (#ece7f8): primary text and emphasized `<b>` spans.
- **Ink 2** (#b1a5d6): body and secondary prose (`.lede`, `td`, `.frow p`).
- **Ink 3** (#9184bb): metadata, hints, table header labels, key sublines.

### Named Rules
**The Single Gradient Rule.** The linear gradient #8A2BE2 → #7B68EE will live only on the brand mark and wordmark: the nav `.mark` (line 85), the hero `.wordmark-big em` (lines 149-153), and the favicon. Buttons, borders, body text, and dividers will stay flat; a primary button will be flat `--violet` with a brightness lift on hover (lines 100-101), never a gradient fill.

**The Status Colors Mean State Rule.** Green, amber, and red will signal a verdict or a live state (table cells, copied confirmation, console chrome). They will never be used decoratively, and no fourth status color will be added.

## Typography

**Display Font:** Bricolage Grotesque (sans-serif fallback), weights 300-800 loaded, used at 600-800.
**Body Font:** Fira Sans (sans-serif fallback), weights 400-600.
**Label/Mono Font:** Fira Code, weights 400-500, for every terminal, code, and data surface.

**Character:** A terminal product speaking in its own face. Display type will be tight, heavy, and confident; mono type will annotate like the TUI itself; prose will stay neutral and readable at 17px.

### Hierarchy
- **Display** (800, clamp(40px, 6.4vw, 86px), 0.98, -0.03em): the gradient wordmark only (lines 145-148).
- **Hero statement** (600, clamp(24px, 2.6vw, 34px), 1.05): the h1 "The commit is the distribution." overlaid on the console (line 154).
- **Headline** (700, clamp(28px, 4vw, 44px), 1.05): section h2 (line 70).
- **Title** (650, 20px): feature row h3, each carrying a mono key subline `.k` (500, 14px, ink 3, line 185); FAQ summaries at 600 17px (line 228).
- **Body** (400, 17px, 1.65): Fira Sans everywhere; ledes at clamp(16px, 1.4vw, 19px) held to a 68ch measure (lines 52-53, 73); feature paragraphs capped at 62ch (line 186).
- **Label** (500, 12-14px): Fira Code for panel legend annotations (13px, line 130), console title (13px, line 120), table headers (12.5px uppercase, 0.06em tracking, line 208), install hint, scroll hint (12px, line 219), and the FAQ plus glyph (22px, line 231).
- **Code** (400, 0.92em): all `code` and `pre` inherit Fira Code (line 68).

### Named Rules
**The Product's Face Rule.** Fira Code will render anything that is terminal, data, or machine voice: annotations, keys, table headers, hints, commands. Bricolage Grotesque will render only display headings and the wordmark. Fira Sans will carry all persuasion prose. A future builder will never set prose in mono or data in Bricolage.

## Layout

The page will open full-bleed, then settle into a centered editorial column. The first viewport will be a flex column hero (`.hero`, line 104): 100svh tall with a thin outer gutter of clamp(10px, 1.6vw, 22px), a fixed nav above, the console flexing to fill the remaining height (`.console` flex:1, line 121), and the install bar sticking to the viewport's bottom edge (`.install-bar` sticky bottom 18px, lines 158-159). The console will clear the nav by a fixed 76px top margin (line 110), and the hero copy will sit at top 86px, left `--pad` (line 138).

Below the hero, every section will use top padding only, clamp(56px, 9vw, 120px) (line 71), creating a single downward rhythm; spacing between blocks will come from each section's top padding plus footer margin, never bottom padding. Content will live in `.wrap` at max-width 1180px (line 72) with `--pad` clamp(20px, 5vw, 72px) side gutters. Feature rows will be a two-column grid, minmax(210px, 300px) and 1fr, 28px gap, 30px row padding (lines 180-183).

Breakpoints will collapse in three steps: at 900px the hero will switch from viewport-height to natural flow, the hero copy will become static above the console without its scrim (line 144), and the panel legend will stack to one column (line 136); at 760px feature rows and the security list will collapse to one column (lines 188, 201); at 720px nav links will hide except GitHub and the table scroll hint will appear (lines 90, 220). Z-index will stay a three-step scale: nav 50, install bar 40, hero copy 5.

### Named Rules
**The Full-Bleed Workstation Rule.** The console will fill the first viewport edge to edge under the nav, with the install bar pinned to the bottom edge. A future builder will never demote the recording into a floating screenshot inside a hero card, and will never add content above the console in the reading order.

**The Top-Anchored Cover Rule.** The recording will render with `object-fit: cover; object-position: center top` (line 123). The crop will deliberately sacrifice the recording's own bottom status strip so the panel columns can fill the viewport; the drawn panel legend strip beneath the console will replace that information. Builders will not switch to `contain` (letterboxing), will not re-anchor to bottom, and will not treat the missing strip as a defect to fix.

**The Scroll, Don't Shrink Rule.** The comparison table will keep `min-width: 680px` inside an `overflow-x: auto` frame (lines 204-205). New columns will extend the scroll, never squeeze cells or hide columns; the mono scroll hint will announce sideways scrolling on small screens (lines 219-220).

## Elevation & Depth

Depth will be tonal first: ground, raised, and sink will stack as three surfaces, and anything floating over the recording or the page will be a translucent scrim of a surface color plus backdrop blur, never an opaque fill. The nav will sit on `--bg` at 82% with 12px blur (lines 81-82), the hero copy on a 94% → 88% → 62% `--bg` gradient with 8px blur (lines 139-141), the panel legend on raised at 90% (line 127), and the install bar on raised at 92% with 10px blur (lines 162-164).

### Shadow Vocabulary
- **Console slab** (`box-shadow: 0 24px 70px -28px rgba(0,0,0,.75), 0 4px 18px rgba(0,0,0,.5)`, line 109): the one large object on the page.
- **Install bar lift** (`box-shadow: 0 18px 50px -18px rgba(0,0,0,.65)`, line 165): the one floating control.

### Named Rules
**The Scrim Rule.** Anything overlaid on the recording, the nav, or the viewport edge will be a `color-mix` scrim of `--bg` or `--bg-raise` with backdrop blur. Opaque overlays will never cover the recording; opacity is how text stays legible while the product stays visible behind it.

**The Two-Shadow Rule.** The page will carry exactly two shadow definitions, one per floating surface. New components will not receive their own shadow; they will earn depth from surface color, border, or scrim.

## Shapes

The form language will be rounded-rectangular and quietly terminal: 14px on large containers (console, install bar, hero copy scrim, lines 107, 139, 163), 12px on the table frame (line 204), 10px on buttons (line 94), 9px on the copy control (line 172), 5-6px on chips, the mark, and the scrollbar thumb, and 2px on the security list square markers (line 198). Borders will be one pixel throughout: `--line` will frame standalone containers, `--line-soft` will divide siblings inside them. Violet will touch a border only as a hover response (lines 99, 175) and as the featured row tint (line 215); no element will ship with a violet border at rest. Recurring geometry will be square and mono-flavored: the 9px rounded-square list bullets, the traffic-light dots, and the rotating plus glyph on FAQ accordions (line 231).

## Components

### Navigation
- Fixed top bar on an 82% ground scrim with 12px blur and a soft hairline bottom border (lines 77-83).
- **Wordmark:** 19px Bricolage 800 with the 22px gradient mark; the mark's notch will be punched in ground color (lines 84-86).
- **Links:** 15px ink 2, hover to ink with no underline (lines 88-89); the GitHub link will keep its arrow glyph and stay visible at every width (line 90).

### Buttons
- **Shape:** 10px radius, 11px 20px padding, 15px Fira Sans (lines 93-98).
- **Primary:** flat `--violet` fill, transparent border, 600 weight; hover will lift brightness 1.12% only, no gradient, no glow (lines 100-101).
- **Secondary:** raised fill, `--line` border, ink text; hover will shift the border to violet and translate up 1px (lines 96-99).
- **Copy:** ground fill, `--line` border, 9px radius; hover shifts border to violet; the done state flips border and label to green for 1.8 seconds (lines 170-176, 515-516).

### Console (signature)
- The workstation frame: 14px radius, sink fill, `--line` border, the console slab shadow (lines 106-111).
- **Chrome bar:** drawn, not part of the image. Raised strip with three 11px dots in red, amber, green (lines 112-119) and a 13px mono title. The drawn bar will stay in markup even if the asset changes.
- **Recording:** a single absolutely positioned image filling the frame with the top-anchored cover crop (lines 122-123).
- **Panel legend:** a three-column strip matching the recording's panel proportions (grid `1fr 1.72fr 1fr`, line 125), each cell a mono annotation pair: bold label in ink, whisper note in ink 3 (lines 129-135). Legend columns and panel columns will stay aligned; a future recording crop will preserve the panel widths or the grid will be updated with it.
- **Hero copy overlay:** gradient scrim card at top 86px holding the gradient wordmark and the h1 (lines 138-154); it will never exceed 640px and will never move to the right edge.

### Install bar (signature)
- Sticky pill at the viewport bottom: raised 92% scrim, 10px blur, `--line` border, 14px radius, 860px max width (lines 158-166).
- Contents in one row: mono hint in ink 3, the command with the subcommand in violet 2 (line 168), the copy control, and the primary button. The command text and the copy payload will stay in sync (`data-copy`, line 299).

### Feature rows
- Two-column editorial rows: h3 plus mono key subline left, prose right, hairline dividers between rows (lines 180-188). Emphasis inside prose will use ink-weighted `<b>`, never color.

### Security list
- Two-column list with 9px violet-2 square markers, hairline underlines, bold ink lead terms (lines 191-201). One claim per line; no icons.

### Comparison table
- Scroll container with 12px radius; 680px min-width table; mono uppercase headers on raised; verdict cells colored by class only (`td.yes` green, `td.no` red); the envy row tinted with 9% violet and its label in ink (lines 204-217). Honest dashes will mark non-applicable cells. Row order and the tint will single out envy exactly once.

### FAQ accordion
- Native `details` elements, hairline dividers, 17px Bricolage summaries, plus glyph rotating 45 degrees when open (lines 223-234). Answers held to the 68ch measure with chip-style inline code on raised.

### Footer
- Hairline top border, flex row wrapping at 18px 40px gaps, ink 3 meta text with the display wordmark leading (lines 237-246).

## Do's and Don'ts

### Do:
- **Do** pull every color, font, radius, and spacing step from the `:root` tokens (lines 42-53); the frontmatter above mirrors them exactly.
- **Do** keep the gradient confined to the brand mark and wordmark, per the Single Gradient Rule.
- **Do** build primary actions as flat `--violet` buttons with the brightness hover.
- **Do** overlay text on the recording only through the scrim pattern, per the Scrim Rule.
- **Do** wrap any new animation inside the existing `@media (prefers-reduced-motion: no-preference)` block (lines 249-255) alongside the boot reveal and smooth scroll (lines 250-254). Nothing outside that gate will animate.
- **Do** keep table columns at full width inside the scroll container and extend the scroll hint pattern when columns grow.
- **Do** set all terminal, code, and data text in Fira Code, per the Product's Face Rule.
- **Do** keep copy plain and honest: concrete numbers, admitted limits, no marketing gloss.

### Don't:
- **Don't** apply the gradient to buttons, borders, backgrounds, or body text; a second gradient surface will dilute the wordmark.
- **Don't** use emoji anywhere on the page; the voice is typographic only.
- **Don't** introduce em-dashes into page copy; the only sanctioned dashes are the terminal-native separators inside the drawn console title and the table's empty-cell glyph. Prose will use commas, colons, and periods.
- **Don't** place an opaque layer over the recording or re-crop it to `contain`; the intentional loss of the recording's bottom status strip is part of the composition, replaced by the drawn panel legend.
- **Don't** add a second surface reporting the same state; each state will speak through one channel: the copy button's green flip, the table verdict classes, the featured row tint. No badges, toasts, or duplicate indicators.
- **Don't** shrink the table below its 680px min-width or hide columns responsively.
- **Don't** ship un-gated motion, new shadows, or new status colors.
- **Don't** rebuild the console chrome or legend as images; they will stay drawn in markup so they never fight the recording's compression.

---

shipped
