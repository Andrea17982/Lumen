---
name: Lumen
description: A quiet desktop workspace for AI conversations, in restrained Liquid Glass.
colors:
  surface-light: "#f8fafb"
  glass-light: "rgba(239,244,249,.72)"
  panel-light: "rgba(250,252,254,.97)"
  text-light: "#292e36"
  muted-light: "#646e7c"
  faint-light: "#dce2e9"
  accent-light: "#416b9a"
  hover-light: "rgba(81,110,145,.08)"
  selected-light: "rgba(255,255,255,.83)"
  input-light: "rgba(255,255,255,.92)"
  line-light: "rgba(99,119,144,.14)"
  error-light: "#a93540"
  ok-light: "#477759"
  button-light: "#303c4f"
  surface-dark: "#181c23"
  glass-dark: "rgba(32,39,50,.78)"
  panel-dark: "rgba(29,35,44,.98)"
  text-dark: "#e4e8ee"
  muted-dark: "#a0acbd"
  faint-dark: "#3b4656"
  accent-dark: "#9fbfe9"
  hover-dark: "rgba(156,184,221,.10)"
  selected-dark: "rgba(95,118,148,.25)"
  input-dark: "#232a35"
  line-dark: "rgba(171,190,214,.14)"
  error-dark: "#ff9fa8"
  ok-dark: "#91cfa5"
  button-dark: "#c4d4e8"
  waiting: "#b5833e"
  autopilot-light: "#956022"
  autopilot-dark: "#e4b576"
  autopilot-background: "rgba(174,123,48,.10)"
typography:
  headline:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: "clamp(30px,3.2vw,44px)"
    fontWeight: 450
    lineHeight: 1.19
    letterSpacing: "-1.4px"
  title:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: "30px"
    fontWeight: 450
    lineHeight: 1.2
    letterSpacing: "-.9px"
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: "14px"
    lineHeight: 1.8
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: "11px"
  code:
    fontFamily: '"Cascadia Code", Consolas, monospace'
    fontSize: ".9em"
rounded:
  micro: "4px"
  segment: "6px"
  control: "7px"
  row: "8px"
  inset: "9px"
  card: "10px"
  message: "12px"
  composer: "17px"
  circle: "50%"
spacing:
  tight: "4px"
  small: "8px"
  control: "12px"
  rail: "16px"
  section: "22px"
  frame: "30px"
components:
  button-primary:
    backgroundColor: "{colors.button-light}"
    textColor: "{colors.surface-light}"
    rounded: "{rounded.control}"
    padding: "9px"
  button-primary-dark:
    backgroundColor: "{colors.button-dark}"
    textColor: "{colors.surface-dark}"
    rounded: "{rounded.control}"
    padding: "9px"
  button-primary-hover:
    backgroundColor: "{colors.accent-light}"
  button-icon:
    textColor: "{colors.muted-light}"
    rounded: "{rounded.control}"
    width: "28px"
    height: "28px"
  new-conversation:
    backgroundColor: "{colors.selected-light}"
    rounded: "{rounded.card}"
    padding: "11px 12px"
  suggestion:
    backgroundColor: "{colors.input-light}"
    rounded: "{rounded.card}"
    padding: "13px 14px"
  composer:
    backgroundColor: "{colors.input-light}"
    rounded: "{rounded.composer}"
    padding: "17px 16px 12px"
  session-row:
    rounded: "{rounded.row}"
    padding: "11px 10px"
  field:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.control}"
    padding: "8px"
  provider-segment:
    textColor: "{colors.muted-light}"
    rounded: "{rounded.segment}"
    padding: "6px 10px"
  permission-autopilot:
    textColor: "{colors.autopilot-light}"
    backgroundColor: "{colors.autopilot-background}"
    rounded: "{rounded.segment}"
    padding: "6px"
  permission-autopilot-dark:
    textColor: "{colors.autopilot-dark}"
    backgroundColor: "{colors.autopilot-background}"
    rounded: "{rounded.segment}"
    padding: "6px"
---

# Design System: Lumen

## Overview

**Creative North Star: "A clear space for a brighter idea."** This is the implemented invitation and the organizing metaphor, rather than a new visual direction. Lumen uses the user's pinned Apple Liquid Glass aesthetic for an English, Windows-first Tauri 2 desktop application. The conversation remains the dominant surface: a compact rail, a quiet header, and an anchored composer frame a clear reading area.

Cool frosted material, soft blue accents, system typography, and restrained controls define the identity. Light and dark themes share the same composition. Advanced controls stay close to their task and appear on request. The product works in Operate mode; legibility and familiar desktop behavior lead.

Key characteristics:

- One principal reading surface, with generous space in the empty state.
- A frosted session rail and a soft intersecting three-bar mark.
- Compact controls, subtle dividers, muted metadata, and a minimal visual treatment.
- Auto routing selected by default, with provider controls disclosed progressively.

This document records `src/styles.css` and the components in `src/App.tsx`, `src/components/` and `src/settings/`. Tokens above are extracted values; CSS remains the runtime source of truth. Screenshots of the current implementation are in `docs/images/`.

## Colors

The palette is cool neutral with a restrained blue primary accent. Frontmatter suffixes identify light and dark values; runtime CSS exposes their semantic names without suffixes through `:root` and `:root[data-theme=dark]`.

### Primary

`accent` is the blue used for leading suggestion icons, links, caret, and running indicators. `button` supplies the contrasting solid send and primary-action fill; its foreground is `surface`. Hovered primary actions use `accent`.

### Neutral

`surface` is the reading canvas. `glass` is the sidebar material. `panel` is the almost opaque settings and popup material. `input` supports the composer and suggestion controls. `text` carries content; `muted` carries metadata and secondary labels. `line` is a low-contrast separator, while `faint` supports scrollbars and quotation rules. `hover` is a tonal interaction fill; `selected` lifts active navigation and segmented selections.

`error`, `ok`, and the fixed amber `waiting` express status. Session status is also supplied as accessible text; errors have visible text. Theme-specific semantic colors should be used for new controls rather than copying a light value. The CSS also contains fixed decorative colors for the mark, provider avatars, scrim, and selection; those are implementation details rather than a second semantic palette.

Autopilot uses its own amber foreground in each theme and a shared translucent amber background, recorded in the `autopilot-*` tokens. Read only and File edits on retain the muted permission-control treatment; File edits on does not use the former blue toggle emphasis. The visible mode label accompanies the color.

Theme selection offers Light, Dark, and System. It persists locally, and System responds to the OS color-scheme media query.

## Typography

The shared system stack resolves to Segoe UI Variable or Segoe UI on Windows. There is no downloaded display face. Headings use balanced wrapping and restrained negative tracking.

- **Welcome headline:** frontmatter `headline`; 46px at widths of at least 1500px, 37px at 1050px and below, 36px at 800px and below, 30px at 520px and below. Weight 450 preserves the quiet tone.
- **Settings title:** frontmatter `title`; 30px, weight 450. Section titles are 13px/600; the about heading is 20px/1.3/450.
- **Conversation body:** 14px/1.8, dropping to 13px at 520px and below. Composer text is 14px/1.6. Markdown headings use 24px, 20px, and 17px at weight 600.
- **Controls:** chiefly 11-12px. Metadata and composer location/permission controls range from 9-10px. The ALPHA badge has been removed. These intentionally compact desktop sizes are observed values, not evidence that all text meets enlarged-text accessibility needs.
- **Code:** Cascadia Code, Consolas, monospace; inline code is .9em, fenced blocks are 12px/1.7, activity diagnostics are 11px/1.6.

Body line length is governed by a 760px conversation column rather than a character-count token. Long prose and diagnostics wrap; code blocks and tables scroll horizontally as needed.

## Layout

The shell is a horizontal flex composition at `100dvh`, with a 480px minimum height and clipped outer overflow. The desktop sidebar is 256px wide with `27px 16px 15px` padding. The workspace owns its independently scrolling conversation; the composer remains below it, with 24px bottom padding on its region (15px at 520px and below). The header is 77px high with 30px horizontal padding. Shell footer slogans, the AI-written corner link, the rail privacy tagline, and the model tagline have been removed. Authorship, thanks, and support remain in About.

Messages and composer share a 760px maximum width and `calc(100% - 100px)` width. The welcome area is at most 750px wide and vertically centers with automatic margins. Its three suggestions form one row on wide layouts. The composer uses a 59px minimum textarea height, content-based sizing, and a 200px maximum height. Settings overlays the workspace from the right rather than reflowing the conversation.

Responsive states, in CSS cascade order:

| Condition | Implemented changes |
| --- | --- |
| At least 1500px | Sidebar 280px; welcome headline 46px; more welcome bottom space; composer bottom margin 12px. |
| At most 1050px | Sidebar 225px with 12px inline padding; reading/composer gutters total 55px; denser suggestions; shorter title allowance; new-conversation keyboard hint hides; workspace/folder controls move together to a full-width second toolbar row with an 8px top gap and fine divider, preserving provider, permissions, and send on the first row. |
| At most 800px | Sidebar becomes a 256px sliding overlay with scrim and menu button; header 66px; gutters total 42px; settings 400px. |
| At most 520px | Suggestions stack at 225px width; gutters total 28px; settings fills width; provider advanced fields stack; permission shield icon and keyboard caption hide while the permission dropdown remains; welcome mark hides; textarea minimum becomes 44px. |

Spacing uses small 4–16px control gaps, around 22–30px section/frame separation, and larger welcome-state pauses. Exact role-specific values take priority over rounding every measurement to a uniform grid.

## Elevation & Depth

The principal glass material belongs to the sidebar: a 155-degree cool tint over `glass`, 30px backdrop blur, and a fine right border. Reading and writing surfaces use stable tonal fills. The CSS mark simulates glass with translucent gradients and inset highlights; it does not blur the reading area.

Current settings is a secondary layering exception to sidebar-only glass: `panel` is 97% opaque in light and 98% opaque in dark, but `.settings-panel` also applies 30px backdrop blur. Preserve this distinction when documenting the shipped appearance; do not claim that blur currently exists exclusively in the rail. Settings uses a left edge shadow and border; there is no desktop full-screen dimming scrim for settings.

Shadow vocabulary:

- **Selected/raised control, light:** `0 8px 24px -15px rgba(42,61,83,.22)`.
- **Selected/raised control, dark:** `0 8px 26px -12px rgba(0,0,0,.45)`.
- **Composer:** `0 6px 30px -16px rgba(55,77,105,.28),0 0 0 1px var(--line)`.
- **Popup:** `0 8px 25px -8px #0003`.
- **Settings:** `-20px 0 70px -35px #263b5545`.
- **Active provider segment:** `0 2px 5px -3px #354a6a44`.

Motion is small and local: button color/background transitions take .16s; sidebar translation takes .2s; settings enters from x=35 with opacity over .2s. The welcome mark floats 5px on an 8s ease-in-out cycle. Running status breathes over 1.4s; the working spinner rotates over 1.3s. New messages scroll into view smoothly. Reduced motion disables CSS animations/transitions and uses immediate scrolling and a zero-duration welcome animation. Settings skips its initial animation and exits without translation or fading, with a zero-duration transition under reduced motion.

## Shapes

Use small softened rectangles for compact controls, not oversized pill buttons. Radii range from 4px for keyboard hints and inline code to 6-8px for controls, segments, fields, and rows; suggestions and new conversation use 10px. User message panels use 12px; the composer is the largest recurring rounded container at 17px. Send and avatar controls are circular. Borders are generally 1px in `line`.

## Components

### Buttons and navigation

New conversation is a softly raised full-width 12px control with an icon and keyboard hint. Session rows are compact, left aligned, ellipsized, and 8px rounded; selection uses `selected` plus the theme shadow. Status dots accompany accessible status text, and pins remain small supporting marks. Icon buttons are normally 28px square; account-row icon buttons are 22px wide. Disabled buttons reduce opacity to .42 and stop advertising a pointer cursor. These sizes reflect desktop density, not a verified touch-target system.

Suggestion cards use `input`, a 1px `line` border, 10px corners, and 13px/14px padding. They populate a draft and focus the composer. Secondary text actions use `accent`; destructive actions use `error`. Global hover treatment applies only to enabled buttons. Controls do not add a visual focus ring; text fields retain the native caret.

### Composer and fields

The composer is an opaque-looking input panel with a soft edge shadow, 17px radius, and wrapping toolbar beneath the textarea. Auto/Claude/GPT form a segmented control with `aria-pressed` states. Selecting Claude or GPT opens the optional account/model/reasoning controls; the chevron can collapse them. The 31px circular send button becomes a stop action while a response runs. Enter sends, Shift+Enter adds a line, and IME composition is excluded from the send shortcut. Only the right-aligned keyboard hint remains below the composer.

Workspace selection and the conversation folder button sit together immediately after the AI selector and its optional chevron. The workspace select uses 10px type, 5px/2px padding, and a 70–112px width range. The folder control uses a 14px icon, 10px type, 6px padding, and 6px corners; its label truncates at 95px on wide layouts and 170px in the grouped second row at 1050px and below. Tooltips expose the workspace name and full folder path. Named workspace association and conversation folder are separate; location controls disable while queued/running or changing a folder. A long label must not displace send onto the location row.

Permissions is a native dropdown with Read only, File edits on, and Autopilot. Read only is the startup default. Autopilot receives the amber semantic treatment and a tooltip describing available CLI access without approval prompts. This expresses provider permission configuration, not a built-in graphical computer-control engine, administrator privileges, or a guarantee that a provider has a specific tool. Subsequent writer/synthesis turns use the chosen permission mode; review and parallel-analysis workers remain read-only, as described in the 0.2 interaction contract.

Account, Model, and Reasoning use native dropdowns. Model choices come from the selected account's official CLI catalog; reasoning choices follow the selected model, with Provider default available. Model selection is not free text. Loading or unavailable catalogs disable dependent selectors and show explicit placeholders; discovery errors appear as recoverable alert text. A refresh icon requests a new catalog. The send control stays disabled for a desktop explicit-provider conversation until a catalog is ready. Fixture screenshots demonstrate these UI states, not real-account entitlement or successful inference.

The account heading includes a pencil for inline rename and a settings action for account management; settings also offers account rename. Rename is an input with save/cancel icon buttons, an 80-character limit, and a disabled save for blank names or while saving. It updates the display name while preserving account identity, isolation folder, and login state. Field-heading icon controls are 24px square; the heading/reasoning label row is 28px high.

Fields use 7px corners, `surface`, a `line` border, and 8px padding. The search field is an unboxed rail control. Error banners use readable text, an error icon, a dismiss action, and `role="alert"`.

### Conversation and activity

Assistant prose sits directly on the reading surface. User prose uses a softly filled, content-sized 12px rounded panel. Assistant responses offer copy. Activity and diagnostics are nested native disclosure elements, keeping orchestration inspectable without dominating the transcript.

Markdown uses GFM lists and tables. `MessageBody` renders link destinations as visible text and images as `[Image: alt]` placeholders: links and images inside responses are not interactive previews. Do not promise richer media behavior from these styles.

### Settings modal side panel

Settings is a right-side `role="dialog"` with `aria-modal="true"` and 420px desktop width. A fixed header holds the close button, the 22px title and a **Simple | Advanced** segmented control, above a tab row (General · Accounts · Auto · Teamwork · Tools · About; Tools only in Advanced). The selected tab uses `text` and a 2px `accent` underline. Only the section body scrolls (0/30px padding); a fixed footer keeps the version and feedback link. In Simple mode, a muted line counts Advanced options that are on, with a **Show** link. Opening focuses its first button. Tab/Shift+Tab wrap through visible enabled controls; closing restores the previously focused element. The sidebar and workspace become `inert` while settings is open. Escape and explicit Workspace/close controls dismiss it. Appearance is a three-way segmented control; accounts, conversation folder, and about/support are separated by fine rules. Account rename is available beside the account's existing sign-in/check/remove actions. Settings errors have a dismissible alert inside the panel. Returning from settings preserves the selected model where still available.

Opening settings closes the navigation sidebar and conversation action menu before focusing the panel. This clears the narrow sidebar and its scrim from the modal path, even though their z-indices are higher than settings. No screen-reader or native-webview focus certification is implied by the implemented focus trap.

### Notices

One-time notices (for example Antigravity in File edits) use the centered dialog: a 13px bold title, muted 12px explanation, a **Don't show again** checkbox and a single OK button. They never block work beyond one click.

Controls draw no focus ring, at the owner's request; hover and selected fills carry the state instead. Popovers open beside the control that opened them (Usage beside its button, Rename beside the conversation), over an invisible click-catcher that never changes color.

### Orchestra card

Orchestra steps render as a card inside the assistant message: `input` fill, 1px `line` border, 12px radius, 14/16px padding, 12px text. The header pairs an accent workflow icon with **Orchestra** and the orchestrator model in muted 10px type. Questions use stacked labels with `surface` inputs prefilled with the recommendation; the plan uses rows separated by `line`, each with title, muted difficulty/files/dependency metadata, an agent dropdown and a remove icon. Progress rows are disclosures with a status icon (spinner, `ok` check, `error` alert, muted dot), the agent and tabular elapsed time; output opens in 11px monospace, at most 240px high. Interrupted runs show an amber note under the header; superseded steps collapse to a muted line. Primary actions (Continue, Approve, Keep branches) use the primary button; secondary ones are text buttons. The read-only notice uses the amber `waiting` color. Closed steps collapse to a single muted line. The composer shows a small accent **Orchestra** badge next to Auto while the setting is on.

## Do's and Don'ts

- **Do** keep the user's Liquid Glass identity, English copy, system typography, and light/dark semantic pairing.
- **Do** keep the transcript dominant and the composer aligned with its 760px reading column.
- **Do** preserve inert modal background, status text, disclosure controls, and reduced-motion preferences.
- **Do** use subdued tonal fills and fine dividers for ordinary reading and writing surfaces.
- **Don't** spread heavy transparency or blur across message content or the composer.
- **Don't** turn every secondary action into a prominent card or primary button.
- **Don't** invent provider performance, native behavior, accessibility conformance, or live-account validation from a browser screenshot.
- **Don't** treat the documentation sidecar's illustrative controls as replacements for application logic or accessible dialog behavior.
