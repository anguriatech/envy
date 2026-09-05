---
name: envy landing
description: Violet-black landing where the product console sits center stage under a plain-spoken hero, and the page persuades by demonstrating, not decorating.
colors:
  bg: "#0d0716"
  bg-raise: "#150c24"
  bg-sink: "#0a0512"
  line: "#2a1b45"
  line-soft: "#1d1233"
  ink: "#ece7f8"
  ink-2: "#b1a5d6"
  ink-3: "#7f71a8"
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
    backgroundColor: "linear-gradient(135deg, {colors.violet}, {colors.violet-2})"
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

The page will behave like the product it sells: a real terminal console, wide and unmistakably the loudest object on the page, running the actual workstation recording right under a short, plain-spoken hero. Everything below the fold will stay editorial and quiet so the console keeps the spotlight. Depth will come from tonal layering (three violet-black surfaces) plus translucent scrims, never from ornamental chrome. Color will be scarce: one brand gradient spent on the brand's strongest moments only, and three status colors that only ever mean state.

A future builder will extend this page by adding editorial rows, table rows, or FAQ entries inside the existing containers; the system will absorb new content without new visual vocabulary. New sections will reuse the `section` / `.wrap` rhythm (lines 71-72 of the stylesheet block), the `.frow` row pattern, and the token set on `:root` rather than introducing parallel primitives.

**Key Characteristics:**
- The hero will read in one breath: gradient wordmark, one-sentence thesis, console.
- The ENVY gradient (#8A2BE2 → #7B68EE) will appear only on the brand mark, the wordmark, and the primary action.
- Fira Code will own every terminal, code, and data surface; Bricolage Grotesque will own display; Fira Sans will own prose.
- Depth will be tonal plus scrim, with exactly two hard shadows on the page.
- All authored motion will sit inside a `prefers-reduced-motion` gate.
- Copy voice will be plain, honest, and free of emoji.

## Colors

The palette will be a violet-black world with three surface depths, three text tiers, one brand hue in two steps, and three functional status colors.

### Primary
- **Violet** (#8a2be2): the brand hue at full strength. It will start the wordmark gradient and the primary button gradient, tint the featured table row (`tr.ours`), square the security list bullets, and anchor the selection and scrollbar states. It will never appear as a scattered accent.

### Secondary
- **Violet 2** (#7b68ee): the soft end of the brand hue. It will end both gradients, color links, focus rings, the copy confirmation border pair, the FAQ plus glyph, and the small square list markers.

### Tertiary (functional status only)
- **Green** (#4cc38a): positive verdicts (`td.yes`), the copied state (`.copy.done`), and the live vault indicator in the console bar.
- **Amber** (#d8a13a): appears only inside the drawn console chrome dots.
- **Red** (#f26155): negative verdicts (`td.no`) and the first chrome dot.

### Neutral
- **Ground** (#0d0716): the page background, body default.
- **Raised** (#150c24): nav, console bar, table header, install bar base, inline code chips.
- **Sink** (#0a0512): the console interior and scrollbar trough; the darkest recess.
- **Line** (#2a1b45): container borders (buttons, console, install bar, table frame).
- **Line soft** (#1d1233): hairline dividers between siblings (rows, cells, details, footer).
- **Ink** (#ece7f8): primary text and emphasized `<b>` spans.
- **Ink 2** (#b1a5d6): body and secondary prose (`.lede`, `td`, `.frow p`).
- **Ink 3** (#7f71a8): metadata, hints, table header labels, key sublines.

### Named Rules
**The Single Gradient Rule.** The linear gradient #8A2BE2 → #7B68EE will live only on the brand's loudest moments: the nav mark, the hero wordmark, and the primary button. Buttons, borders, body text, and dividers will stay flat otherwise; a secondary button will be a raised fill with a violet border on hover, never a gradient fill.

**The Status Colors Mean State Rule.** Green, amber, and red will signal a verdict or a live state (table cells, copied confirmation, console chrome). They will never be used decoratively, and no fourth status color will be added.

## Typography

**Display Font:** Bricolage Grotesque (sans-serif fallback), weights 300-800 loaded, used at 600-800.
**Body Font:** Fira Sans (sans-serif fallback), weights 400-600.
**Label/Mono Font:** Fira Code, weights 400-500, for every terminal, code, and data surface.

**Character:** A terminal product speaking in its own face. Display type will be tight, heavy, and confident; mono type will annotate like the TUI itself; prose will stay neutral and readable at 17px.

### Hierarchy
- **Display** (800, clamp(40px, 6.4vw, 86px), 0.98, -0.03em): the gradient wordmark only.
- **Hero statement** (600, clamp(24px, 2.6vw, 34px), 1.05): the h1 "The commit is the distribution."
- **Headline** (700, clamp(28px, 4vw, 44px), 1.05): section h2.
- **Title** (650, 20px): feature row h3, each carrying a mono key subline `.k` (500, 14px, ink 3); FAQ summaries at 600 17px.
- **Body** (400, 17px, 1.65): Fira Sans everywhere; ledes at clamp(16px, 1.4vw, 19px) held to a 68ch measure; feature paragraphs capped at 62ch.
- **Label** (500, 12-14px): Fira Code for panel legend annotations (13px), console title (13px), table headers (12.5px uppercase, 0.06em tracking), install hint, and the FAQ plus glyph (22px).
- **Code** (400, 0.92em): all `code` and `pre` inherit Fira Code.

### Named Rules
**The Product's Face Rule.** Fira Code will render anything that is terminal, data, or machine voice: annotations, keys, table headers, hints, commands. Bricolage Grotesque will render only display headings and the wordmark. Fira Sans will carry all persuasion prose. A future builder will never set prose in mono or data in Bricolage.

## Layout

The hero will read top-down in one column: the copy block (gradient wordmark, h1, lede) centered at a 1180px measure, then the console at full container width 76px below, then the panel legend, then the sticky install bar. The console will keep the recording at its natural aspect ratio (1200×653) so the TUI panels render at readable size on desktop; the panel legend grid (1fr 1.72fr 1fr) will mirror the recording's three panel proportions underneath it.

Below the hero, every section will use top padding only, clamp(56px, 9vw, 120px), creating a single downward rhythm; spacing between blocks will come from each section's top padding, never bottom padding. Content will live in `.wrap` at max-width 1180px with `--pad` clamp(20px, 5vw, 72px) side gutters. Feature rows will be a two-column grid, minmax(210px, 300px) and 1fr, 28px gap, 30px row padding.

Breakpoints will collapse in three steps: at 900px the panel legend will stack to one column; at 760px feature rows and the security list will collapse to one column; at 720px nav links will hide except the pinned GitHub link. Z-index will stay a two-step scale: nav 50, install bar 40.

### Named Rules
**The Console Is The Exhibit Rule.** The console will remain the widest, heaviest object in the hero - full container width, drawn chrome, natural-height recording. A future builder will never shrink it into a side-by-side split or bury it below other content; if the hero gains content, the console still follows the copy directly.

**The Scroll, Don't Shrink Rule.** The comparison table will keep `min-width: 760px` inside an `overflow-x: auto` frame. New columns will extend the scroll, never squeeze cells or hide columns.

## Elevation & Depth

Depth will be tonal first: ground, raised, and sink will stack as three surfaces, and anything floating over the page will be a translucent scrim of a surface color plus backdrop blur, never an opaque fill. The nav will sit on `--bg` at 82% with 12px blur, the install bar on raised at 92% with 10px blur.

### Shadow Vocabulary
- **Console slab** (`box-shadow: 0 24px 80px -24px rgba(138,43,226,.28), 0 4px 18px rgba(0,0,0,.5)`): the one large object on the page, lifted by a violet-tinted glow that keeps it in the brand world.
- **Install bar lift** (`box-shadow: 0 18px 50px -18px rgba(0,0,0,.65)`): the one floating control.

### Named Rules
**The Two-Shadow Rule.** The page will carry exactly two shadow definitions, one per floating surface. New components will not receive their own shadow; they will earn depth from surface color, border, or scrim.

## Shapes

The form language will be rounded-rectangular and quietly terminal: 14px on large containers (console, install bar), 12px on the table frame, 10px on buttons, 9px on the copy control, 5-6px on chips, the mark, and the scrollbar thumb, and 2px on the security list square markers. Borders will be one pixel throughout: `--line` will frame standalone containers, `--line-soft` will divide siblings inside them. Violet will touch a border only as a hover response (buttons, copy control) and as the featured row tint; no element will ship with a violet border at rest. Recurring geometry will be square and mono-flavored: the 9px rounded-square list bullets, the traffic-light dots, and the rotating plus glyph on FAQ accordions.

## Components

### Navigation
- Fixed top bar on an 82% ground scrim with 12px blur and a soft hairline bottom border.
- **Wordmark:** 19px Bricolage 800 with the 22px gradient mark; the mark's notch will be punched in ground color.
- **Links:** 15px ink 2, hover to ink with no underline; the GitHub link will stay visible at every width.

### Buttons
- **Shape:** 10px radius, 11px 20px padding, 15px Fira Sans.
- **Primary:** gradient fill (violet → violet 2), transparent border, 600 weight; hover will lift brightness only, no glow.
- **Secondary:** raised fill, `--line` border, ink text; hover will shift the border to violet and translate up 1px.
- **Copy:** ground fill, `--line` border, 9px radius; hover shifts border to violet; the done state flips border and label to green for 1.8 seconds.

### Console (signature)
- The workstation frame: 14px radius, sink fill, `--line` border, the console slab shadow.
- **Chrome bar:** drawn, not part of the image. Raised strip with three 11px dots in red, amber, green and a 13px mono title (`envy - ~/projects/demo`); on the right, a green-pip live label reading "vault unlocked" - the single status surface for the vault state. The drawn bar will stay in markup even if the asset changes.
- **Recording:** a block-level image at natural aspect ratio (1200×653) filling the frame width.
- **Panel legend:** a three-column strip matching the recording's panel proportions (grid `1fr 1.72fr 1fr`), each cell a mono annotation pair: bold label in ink, whisper note in ink 3. Legend columns and panel columns will stay aligned; a future recording crop will preserve the panel widths or the grid will be updated with it.

### Install bar (signature)
- Sticky pill at the viewport bottom: raised 92% scrim, 10px blur, `--line` border, 14px radius, 860px max width.
- Contents in one row: mono hint in ink 3, the command with the subcommand in violet 2, the copy control, and the primary button. The command text and the copy payload will stay in sync (`data-copy`).

### Feature rows
- Two-column editorial rows: h3 plus mono key subline left, prose right, hairline dividers between rows. Emphasis inside prose will use ink-weighted `<b>`, never color.

### Security list
- Two-column list with 9px violet-2 square markers, hairline underlines, bold ink lead terms. One claim per line; no icons.

### Comparison table
- Scroll container with 12px radius; 760px min-width table; mono uppercase headers on raised; verdict cells colored by class only (`td.yes` green, `td.no` red); the envy row tinted with 9% violet and its label in ink. Honest dashes will mark non-applicable cells. Row order and the tint will single out envy exactly once.

### FAQ accordion
- Native `details` elements, hairline dividers, 17px Bricolage summaries, plus glyph rotating 45 degrees when open. Answers held to the 68ch measure with chip-style inline code on raised.

### Footer
- Hairline top border, flex row wrapping at 18px 40px gaps, ink 3 meta text with the display wordmark leading. The footer will state product facts (language, license) and repo links only - it will not carry team identity, company links, or origin stories.

## Do's and Don'ts

### Do:
- **Do** pull every color, font, radius, and spacing step from the `:root` tokens; the frontmatter above mirrors them exactly.
- **Do** keep the gradient confined to the brand mark, the wordmark, and the primary button, per the Single Gradient Rule.
- **Do** build secondary actions as raised fills with a violet hover border.
- **Do** wrap any new animation inside the existing `@media (prefers-reduced-motion: no-preference)` block alongside the boot reveal and smooth scroll. Nothing outside that gate will animate.
- **Do** keep table columns at full width inside the scroll container and extend the scroll when columns grow.
- **Do** set all terminal, code, and data text in Fira Code, per the Product's Face Rule.
- **Do** keep copy plain and honest: concrete numbers, admitted limits, no marketing gloss.

### Don't:
- **Don't** apply the gradient to borders, backgrounds, or body text, or add a second competing gradient; dilution kills the brand moments.
- **Don't** use emoji anywhere on the page; the voice is typographic only.
- **Don't** shrink the table below its 760px min-width or hide columns responsively.
- **Don't** add a second surface reporting the same state; each state will speak through one channel: the live label for vault state, the copy button's green flip, the table verdict classes. No badges, toasts, or duplicate indicators.
- **Don't** ship un-gated motion, new shadows, or new status colors.
- **Don't** rebuild the console chrome or legend as images; they will stay drawn in markup so they never fight the recording's compression.
- **Don't** put team, company, or people claims in the footer or anywhere else; the page sells the product, and identity lives in the repo.

---

shipped
