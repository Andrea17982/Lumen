---
name: Lumen
description: A quiet desktop workspace for AI conversations, in restrained Liquid Glass.
colors:
  surface-light: '#eef3f8'
  glass-light: 'rgba(246,249,252,.6)'
  panel-light: 'rgba(250,252,254,.97)'
  text-light: '#292e36'
  muted-light: '#5f6a7a'
  faint-light: '#d3dbe5'
  accent-light: '#3f6896'
  hover-light: 'rgba(81,110,145,.08)'
  selected-light: 'rgba(255,255,255,.9)'
  input-light: 'rgba(255,255,255,.62)'
  line-light: 'rgba(99,119,144,.16)'
  error-light: '#a93540'
  ok-light: '#477759'
  button-light: '#303c4f'
  surface-dark: '#0d131c'
  glass-dark: 'rgba(28,34,44,.66)'
  panel-dark: 'rgba(29,35,44,.98)'
  text-dark: '#eef2f7'
  muted-dark: '#98a4b6'
  faint-dark: '#3a4454'
  accent-dark: '#a9c8ef'
  hover-dark: 'rgba(190,210,235,.08)'
  selected-dark: 'rgba(190,210,235,.14)'
  input-dark: 'rgba(255,255,255,.055)'
  line-dark: 'rgba(190,208,232,.12)'
  error-dark: '#ff9fa8'
  ok-dark: '#8fd3a4'
  button-dark: '#d7dde6'
motion:
  enter-easing: 'cubic-bezier(0.16,1,0.3,1)'
  exit-easing: 'cubic-bezier(0.4,0,1,1)'
  standard-easing: 'cubic-bezier(0.2,0.75,0.25,1)'
  durations:
    fast: '140ms'
    normal: '180ms'
    spatial: '240ms'
  guidance: 'Keep motion brief, purposeful, and responsive to reduced-motion preferences.'
  waiting: '#b5833e'
  autopilot-light: '#956022'
  autopilot-dark: '#e4b576'
  autopilot-background: 'rgba(174,123,48,.10)'
typography:
  headline:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: 'clamp(30px,3.2vw,44px)'
    fontWeight: 450
    lineHeight: 1.19
    letterSpacing: '-1.4px'
  title:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: '30px'
    fontWeight: 450
    lineHeight: 1.2
    letterSpacing: '-.9px'
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: '14px'
    lineHeight: 1.8
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif'
    fontSize: '11px'
  code:
    fontFamily: '"Cascadia Code", Consolas, monospace'
    fontSize: '.9em'
rounded:
  micro: '4px'
  segment: '6px'
  control: '7px'
  row: '8px'
  inset: '9px'
  card: '10px'
  message: '12px'
  composer: '22px'
  circle: '50%'
spacing:
  tight: '4px'
  small: '8px'
  control: '12px'
  rail: '16px'
  section: '22px'
  frame: '30px'
components:
  button-primary:
    backgroundColor: '{colors.button-light}'
    textColor: '{colors.surface-light}'
    rounded: '{rounded.control}'
    padding: '9px'
  button-primary-dark:
    backgroundColor: '{colors.button-dark}'
    textColor: '{colors.surface-dark}'
    rounded: '{rounded.control}'
    padding: '9px'
  button-primary-hover:
    backgroundColor: '{colors.accent-light}'
  button-icon:
    textColor: '{colors.muted-light}'
    rounded: '{rounded.control}'
    width: '28px'
    height: '28px'
  new-conversation:
    backgroundColor: '{colors.selected-light}'
    rounded: '{rounded.card}'
    padding: '11px 12px'
  suggestion:
    backgroundColor: '{colors.input-light}'
    rounded: '{rounded.card}'
    padding: '13px 14px'
  composer:
    backgroundColor: '{colors.input-light}'
    rounded: '{rounded.composer}'
    padding: '18px 18px 12px'
  session-row:
    rounded: '{rounded.row}'
    padding: '11px 10px'
  field:
    backgroundColor: '{colors.surface-light}'
    textColor: '{colors.text-light}'
    rounded: '{rounded.control}'
    padding: '8px'
  provider-segment:
    textColor: '{colors.muted-light}'
    rounded: '{rounded.segment}'
    padding: '6px 10px'
  permission-autopilot:
    textColor: '{colors.autopilot-light}'
    backgroundColor: '{colors.autopilot-background}'
    rounded: '{rounded.segment}'
    padding: '6px'
  permission-autopilot-dark:
    textColor: '{colors.autopilot-dark}'
    backgroundColor: '{colors.autopilot-background}'
    rounded: '{rounded.segment}'
    padding: '6px'
---

# Design System: Lumen

## Overview

**Creative North Star: "A clear space for a brighter idea."** This is the implemented invitation and the organizing metaphor, rather than a new visual direction. Lumen uses the user's pinned Apple Liquid Glass aesthetic for a Windows-first Tauri 2 desktop application with English and Italian interface languages. The conversation remains the dominant surface: a compact rail, a quiet header, and an anchored composer frame a clear reading area.

Cool frosted material, soft blue accents, system typography, and restrained controls define the identity. Light and dark themes share the same composition. Advanced controls stay close to their task and appear on request. The product works in Operate mode; legibility and familiar desktop behavior lead.

Key characteristics:

- One principal reading surface, with generous space in the empty state.
- A frosted session rail and a soft intersecting three-bar mark.
- Compact controls, subtle dividers, muted metadata, and a minimal visual treatment.
- Direct native provider selected by default, with model/reasoning controls disclosed progressively; Auto/Orchestra remain explicit choices.

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

- **Welcome headline:** frontmatter `headline`; `clamp(34px, 4.1vw, 62px)` with weight 400 and -1.8px tracking, dropping to 30px at 520px and below.
- **Settings title:** frontmatter `title`; 30px, weight 450. Section titles are 13px/600; the about heading is 20px/1.3/450.
- **Conversation body:** 14px/1.8, dropping to 13px at 520px and below. Composer text is 14px/1.6. Markdown headings use 24px, 20px, and 17px at weight 600.
- **Controls:** chiefly 11-12px. Metadata and composer location/permission controls range from 9-10px. The ALPHA badge has been removed. These intentionally compact desktop sizes are observed values, not evidence that all text meets enlarged-text accessibility needs.
- **Code:** Cascadia Code, Consolas, monospace; inline code is .9em, fenced blocks are 12px/1.7, activity diagnostics are 11px/1.6.

Body line length is governed by a 760px conversation column rather than a character-count token. Long prose and diagnostics wrap; code blocks and tables scroll horizontally as needed.

## Layout

The shell is a horizontal flex composition at `100dvh`, with a 480px minimum height and clipped outer overflow. The desktop sidebar is 280px wide (300px at 1500px and wider) with `26px 16px 16px` padding. The workspace owns its independently scrolling conversation; the composer remains below it, with 24px bottom padding on its region (15px at 520px and below). The header is 72px high with 26px horizontal padding. Shell footer slogans, the AI-written corner link, the rail privacy tagline, and the model tagline have been removed. Authorship, thanks, and support remain in About.

Messages and composer share a `--reading-width` maximum of 820px and a `calc(100% - 100px)` width, so the composer stays aligned with the reading column. The welcome headline scales with `clamp(34px, 4.1vw, 62px)`, and the welcome area is at most 750px wide and vertically centers with automatic margins. Its three suggestions form one row on wide layouts. The composer uses a 59px minimum textarea height, content-based sizing, and a 200px maximum height. Settings overlays the workspace from the right rather than reflowing the conversation. Its near-opaque frosted panel uses a softly tinted header, compact section tabs, and a language selector above appearance controls in General.

Responsive states, in CSS cascade order:

| Condition       | Implemented changes                                                                                                                                                                                                                                                                                                                           |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| At least 1500px | Sidebar 300px; more welcome bottom space; composer bottom margin 12px.                                                                                                                                                                                                                                                 |
| At most 1050px  | Sidebar 225px with 12px inline padding; reading/composer gutters total 55px; denser suggestions; shorter title allowance; new-conversation keyboard hint hides; workspace/folder controls move together to a full-width second toolbar row with an 8px top gap and fine divider, preserving provider, permissions, and send on the first row. |
| At most 800px   | Sidebar becomes a 256px sliding overlay with scrim and menu button; header 66px; gutters total 42px; settings 400px.                                                                                                                                                                                                                          |
| At most 520px   | Suggestions stack at 225px width; gutters total 28px; settings fills width; provider advanced fields stack; permission shield icon and keyboard caption hide while the permission dropdown remains; provider segmented labels drop their icons so the send button stays on the first toolbar row; welcome mark hides; textarea minimum becomes 44px.                                                                                        |

Spacing uses small 4–16px control gaps, around 22–30px section/frame separation, and larger welcome-state pauses. Exact role-specific values take priority over rounding every measurement to a uniform grid.

## Elevation & Depth

The principal glass material belongs to the sidebar: a soft top highlight over `glass`, 30px backdrop blur, and a fine right border. The workspace behind the conversation carries a decorative blurred "aurora" (`.aurora`, clipped to the workspace, `z-index: -1`) made of the `--aurora-*` tokens; it never sits above text. The composer is glass too: a 1px `line` border, an inset top highlight line, and `--composer-shadow`. Message content keeps stable tonal fills. The CSS mark simulates glass with translucent gradients and inset highlights.

Current settings is a secondary layering exception to sidebar-only glass: `panel` is 97% opaque in light and 98% opaque in dark, but `.settings-panel` also applies 30px backdrop blur. Preserve this distinction when documenting the shipped appearance; do not claim that blur currently exists exclusively in the rail. Settings uses a left edge shadow and border; there is no desktop full-screen dimming scrim for settings.

Shadow vocabulary:

- **Selected/raised control, light:** `0 8px 24px -15px rgba(42,61,83,.22)`.
- **Selected/raised control, dark:** `0 8px 26px -12px rgba(0,0,0,.45)`.
- **Composer, light:** `0 24px 60px -32px rgba(42,61,83,.38)`.
- **Composer, dark:** `0 28px 60px -30px rgba(0,0,0,.7)`.
- **Popup:** `0 8px 25px -8px #0003`.
- **Settings:** `-20px 0 70px -35px #263b5545`.
- **Active provider segment:** `0 2px 5px -3px #354a6a44`.

Motion is small and local: button color/background transitions take .16s; sidebar translation takes .2s; settings enters from x=35 with opacity over .2s. The welcome mark floats 5px on an 8s ease-in-out cycle. Running status breathes over 1.4s; the working spinner rotates over 1.3s. New messages scroll into view smoothly. Reduced motion disables CSS animations/transitions and uses immediate scrolling and a zero-duration welcome animation. Settings skips its initial animation and exits without translation or fading, with a zero-duration transition under reduced motion.

## Shapes

Use small softened rectangles for compact controls, not oversized pill buttons. Radii range from 4px for keyboard hints and inline code to 6-8px for controls, segments, fields, and rows; suggestions and new conversation use 10px. User message panels use 12px; the composer is the largest recurring rounded container at 17px. Send and avatar controls are circular. Borders are generally 1px in `line`.

## Components

Settings uses progressive disclosure: Simple exposes only the Orchestra switch; Advanced exposes the remaining sections. General contains a two-step Restore defaults action. Reset copy states that accounts and conversations are preserved.

### Buttons and navigation

New conversation is a softly raised full-width 12px control with an icon and keyboard hint. Session rows are compact, left aligned, ellipsized, and 8px rounded; selection uses `selected` plus the theme shadow. Status dots accompany accessible status text, and pins remain small supporting marks. Icon buttons are normally 28px square; account-row icon buttons are 22px wide. Disabled buttons reduce opacity to .42 and stop advertising a pointer cursor. These sizes reflect desktop density, not a verified touch-target system.

Suggestion cards use `input`, a 1px `line` border, 10px corners, and 13px/14px padding. They populate a draft and focus the composer. Secondary text actions use `accent`; destructive actions use `error`. Global hover treatment applies only to enabled buttons. Incumbent button controls do not add a visual focus ring; text fields retain the native caret. The execution disclosure's keyboard-focus outline is the narrow exception described above.

### Composer and fields

The composer is a glass input panel with a 22px radius, a fine `line` border, and a wrapping toolbar beneath the textarea. The mode picker is a bordered segmented group; the send button is a 34px circle filled with `button`, which is light in dark theme. Auto/Claude/GPT form a segmented control with `aria-pressed` states. Selecting Claude or GPT opens the optional account/model/reasoning controls; the chevron can collapse them. The 31px circular send button becomes a stop action while a response runs. Enter sends, Shift+Enter adds a line, and IME composition is excluded from the send shortcut. Only the right-aligned keyboard hint remains below the composer.

Workspace selection and the conversation folder button sit together immediately after the AI selector and its optional chevron. The workspace select uses 10px type, 5px/2px padding, and a 70–112px width range. The folder control uses a 14px icon, 10px type, 6px padding, and 6px corners; its label truncates at 95px on wide layouts and 170px in the grouped second row at 1050px and below. Tooltips expose the workspace name and full folder path. Named workspace association and conversation folder are separate; location controls disable while queued/running or changing a folder. A long label must not displace send onto the location row.

Permissions is a native dropdown with Read only, File edits on, and Autopilot. Read only is the startup default. Autopilot receives the amber semantic treatment and a tooltip describing available CLI access without approval prompts. This expresses provider permission configuration, not a built-in graphical computer-control engine, administrator privileges, or a guarantee that a provider has a specific tool. Subsequent writer/synthesis turns use the chosen permission mode; review and parallel-analysis workers remain read-only, as described in the 0.2 interaction contract.

Account, Model, and Reasoning use native dropdowns. Model choices come from the selected account's official CLI catalog; reasoning choices follow the selected model, with Provider default available. Model selection is not free text. Loading or unavailable catalogs disable dependent selectors and show explicit placeholders; discovery errors appear as recoverable alert text. A refresh icon requests a new catalog. The send control stays disabled for a desktop explicit-provider conversation until a catalog is ready. Fixture screenshots demonstrate these UI states, not real-account entitlement or successful inference.

The account heading includes a pencil for inline rename and a settings action for account management; settings also offers account rename. Rename is an input with save/cancel icon buttons, an 80-character limit, and a disabled save for blank names or while saving. It updates the display name while preserving account identity, isolation folder, and login state. Field-heading icon controls are 24px square; the heading/reasoning label row is 28px high.

Fields use 7px corners, `surface`, a `line` border, and 8px padding. The search field is an unboxed rail control. Error banners use readable text, an error icon, a dismiss action, and `role="alert"`.

### Conversation and activity

Assistant prose sits directly on the reading surface. User prose uses a softly filled, content-sized 12px rounded panel. Assistant responses offer copy. Activity and diagnostics are nested native disclosure elements, keeping orchestration inspectable without dominating the transcript.

The 0.8 render-isolation tranche preserves this exact composition and the complete transcript DOM. Composer-only changes reuse the transcript; actual message updates render normally. Historical find, selection, disclosures, image enlargement, Both lanes and current-chat actions remain reachable. It adds no visual control or new scroll policy. Native light/dark captures and the scoped CPU/scroll evidence are documented in [Windows performance](docs/performance-0.8.md). The subsequent [opening optimization](docs/initial-open-0.8.md) keeps identical paragraph markup for literal text and the original Markdown path otherwise. The windowed prototype loses historical content access and stays excluded from ordinary builds; compatible full virtualization remains open.

The 0.8 working tree adds a single selected-turn execution disclosure above the composer, aligned to its existing column. Its 12px summary uses `text` for legibility over the aurora in both themes, an Activity icon, explicit chevron, wrapping tabular timing and only available agent/check counts. Expanded details scroll within a 230px maximum height and retain native keyboard disclosure/focus behavior. Stage milestones report local observed times; inputs, permissions, silence, errors and interruption remain distinct. "Turn ended" does not claim task completion. A read-only refresh updates local execution evidence; silence never triggers a new model request. The separate 0.8 logo/sidebar checkpoint below records its implementation. Native captures validate the tested desktop size, not screen-reader certification or every responsive layout.

Markdown uses GFM lists and tables. `MessageBody` renders links as buttons that use the validated backend opener, and images through `ResponseImage`. Local images are scoped to the message's project folder; remote images require explicit loading. Images nested in links do not create nested interactive controls. These are implemented affordances, not proof that every editor or remote image host works.

### Settings modal side panel

Settings is a right-side `role="dialog"` with `aria-modal="true"` and 420px desktop width. A fixed header holds the close button, the 22px title and a **Simple | Advanced** segmented control, above a tab row (General · Accounts · Auto · Teamwork · Tools · About; Tools only in Advanced). The selected tab uses `text` and a 2px `accent` underline. Only the section body scrolls (0/30px padding); a fixed footer keeps the version and feedback link. In Simple mode, a muted line counts Advanced options that are on, with a **Show** link. Opening focuses its first button. Tab/Shift+Tab wrap through visible enabled controls; closing restores the previously focused element. The sidebar and workspace become `inert` while settings is open. Escape and explicit Workspace/close controls dismiss it. Appearance is a three-way segmented control; accounts, conversation folder, and about/support are separated by fine rules. Account rename is available beside the account's existing sign-in/check/remove actions. Settings errors have a dismissible alert inside the panel. Returning from settings preserves the selected model where still available.

Opening settings closes the navigation sidebar and conversation action menu before focusing the panel. This clears the narrow sidebar and its scrim from the modal path, even though their z-indices are higher than settings. No screen-reader or native-webview focus certification is implied by the implemented focus trap.

### Notices

One-time notices (for example Antigravity in File edits) use the centered dialog: a 13px bold title, muted 12px explanation, a **Don't show again** checkbox and a single OK button. They never block work beyond one click.

Incumbent button controls draw no focus ring, at the owner's request; hover and selected fills carry the state instead. The new execution disclosure summaries retain a visible 2px `accent` keyboard-focus outline, without changing the button policy. Popovers open beside the control that opened them (Usage beside its button, Rename beside the conversation), over an invisible click-catcher that never changes color.

Usage uses compact single-line quota rows with a thin meter, percentage and time to reset. Missing, stale or manually entered data has a discreet status label; its detail is available from the tooltip and accessible name. General → Appearance includes an optional, persistent 8% interface enlargement; the default remains unchanged. Icon-only actions use the shared `IconButton` component for consistent labels and title hints.

The context indicator sits beside Usage and reuses its button, popover, scrim and meter track. Its dot is green under 50%, orange from 50% and red from 80% of the model's window; the popover shows the exact `used / window` count, a thicker track in the same colour, the parts of the count and **Compact into a fresh session**. It states whether the figures were measured by the provider or estimated locally. Popovers opened from the sidebar are rendered on `document.body`, because the sidebar's transform would clip a fixed element.

### Orchestra card

Orchestra steps render as a card inside the assistant message: `input` fill, 1px `line` border, 12px radius, 14/16px padding, 12px text. The header pairs an accent workflow icon with **Orchestra** and the orchestrator model in muted 10px type. Questions use stacked labels with `surface` inputs prefilled with the recommendation; the plan uses rows separated by `line`, each with title, muted difficulty/files/dependency metadata, an agent dropdown and a remove icon. Progress rows are disclosures with a status icon (spinner, `ok` check, `error` alert, muted dot), the agent and tabular elapsed time; output opens in 11px monospace, at most 240px high. Interrupted runs show an amber note under the header; superseded steps collapse to a muted line. Primary actions (Continue, Approve, Keep branches) use the primary button; secondary ones are text buttons. The read-only notice uses the amber `waiting` color. Closed steps collapse to a single muted line. The composer shows a small accent **Orchestra** badge next to Auto while the setting is on.

## Do's and Don'ts

- **Do** keep the user's Liquid Glass identity, English/Italian copy, system typography, and light/dark semantic pairing.
- **Do** keep the aurora decorative and behind content, and keep text contrast readable over it in both themes.
- **Do** keep the transcript dominant and the composer aligned with its 760px reading column.
- **Do** preserve inert modal background, status text, disclosure controls, and reduced-motion preferences.
- **Do** use subdued tonal fills and fine dividers for ordinary reading and writing surfaces.
- **Don't** spread heavy transparency or blur across message content or the composer.
- **Don't** turn every secondary action into a prominent card or primary button.
- **Don't** invent provider performance, native behavior, accessibility conformance, or live-account validation from a browser screenshot.
- **Don't** treat the documentation sidecar's illustrative controls as replacements for application logic or accessible dialog behavior.

## 0.8 logo/sidebar checkpoint

The top-left mark is now the single sidebar disclosure on desktop and narrow layouts. Its 40px glass button stays at a fixed shell position, exposes translated Open/Close sidebar names and expanded/controls state, and has a visible keyboard focus outline. The rail is initially open on desktop; closing it expands the conversation area without unmounting the rail or changing the draft. Narrow layouts start closed and use an overlay. A closed rail is inert and hidden from the accessibility tree. Escape from the rail and scrim dismissal return focus to the mark; search opens and focuses the rail. Conversation-menu Escape dismisses only that menu.

The rail slides with the existing spatial/ease tokens and a single 360ms glint crosses the mark on activation. Reduced motion removes both effects. The header reserves space for the mark in the closed composition; the separate mobile menu icon is removed. Desktop chat selection keeps the rail available; narrow selection closes it. The original material, themes and composer remain. This is a scoped first implementation of the approved composition, not a general redesign or screen-reader certification. [Native and browser checks](docs/logo-sidebar-0.8.md).

## 0.7 documentation and capture contract

The GitHub surface is Read mode: a concise bilingual introduction, actual application gallery, working-mode explanation, platform package table and links to canonical guides. Keep the existing mark/material identity; documentation is not a product redesign. Native questions, child streams, Moli captures and inspector/layout affordances should be represented by actual implemented UI with isolated demonstration data. The gallery is not live-inference or accessibility certification.
