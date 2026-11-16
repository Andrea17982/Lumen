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
  segment: '8px'
  control: '8px'
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

# Lumen design system

Lumen is an Operate surface: a quiet desktop workspace in restrained Liquid Glass. Preserve the cool neutral palette, system typography, compact controls, English/Italian UI and light/dark themes. CSS in `src/styles.css` is the runtime source; the tokens above describe its roles. Product scope lives in [PRODUCT.md](PRODUCT.md), implementation evidence in [validation](docs/validation.md).

## Identity

The approved transparent satin-glass `lm` monogram in `public/lumen.png` is the sole app mark. Platform icons are generated from it with `tauri icon`; no CSS reconstruction. The top-left logo opens the glass project sidebar. It starts visible on desktop and closed on narrow screens; hiding it preserves drafts and focus, and hidden controls are inert. Brief motion respects reduced motion.

## Layout and material

The conversation and floating composer share an 820px reading width. The sidebar is 280px, 300px on large screens; below 800px it becomes an overlay. The settings panel overlays the workspace with an opaque frosted surface. At narrow widths it fills the window. Settings tabs wrap so every section, including About, remains reachable. Long prose wraps; code, tables and their own content may scroll horizontally.

Glass belongs to navigation, composer and menus. Use the existing `surface`, `glass`, `panel`, `input`, `text`, `muted`, `accent`, `line`, `hover` and `selected` variables in both themes. No hard-coded light menu colors in dark mode. Background blur stays behind readable text.

## Controls

Compact controls share an 8px corner radius, including send/stop, Usage, Context and project actions. Larger message containers use 12px; the floating composer uses 22px. Fields retain a fine `line` border and theme-aware input fill. Focus must remain visible.

All single-choice fields use `src/components/Select.tsx`. Its menu is rendered outside clipping panels, positioned within the viewport, softly rounded and briefly animated. It supports arrows, Home/End, type-to-select, Enter/Space, Escape, Tab and outside dismissal. Labels, selected state and disabled options remain accessible. The hidden native select preserves form values; it is not an additional visible control.

The provider bar starts with Auto and adds only configured providers, always visible using wrapping rather than horizontal scrolling. Explicit sessions disclose Account, Model and Reasoning. Models come from the account's official catalog. Google inference uses Antigravity with Google/Gemini models only. Permissions retain the separate explanatory picker and never change from opening a menu.

## Conversation and status

The composer has no permanent commit suggestion, keyboard instructions or provider/strategy banner. Auto records a quiet model/effort assignment in the chat, repeated at completion. Only final elapsed time is shown outside the transcript; detailed observations remain recorded for diagnostics. Errors, pending questions and permission approvals remain visible and actionable.

Usage lists only reported five-hour and weekly windows for configured providers; no unavailable rows, decorative status dots or manual refresh/close controls. Outside click and Escape dismiss it. Queries happen on opening the menu rather than an idle subprocess timer. Context explicitly marks an estimate and uses the larger of local prompt size and last provider input, plus the draft. Its explanation must retain limitations about compaction, attachments and estimated model capacity.

## Projects and settings

Show All conversations only when projects exist; there is no Unsorted folder. Conversations and projects expose context menus on right click; keyboard users can use Shift+F10 on conversation rows. Project deletion requires explicit confirmation, removes contained conversations and preserves filesystem files. Running conversations block deletion. Move conversations before deleting a project to preserve them.

General contains interface and window controls, then the enabled advanced-options notice, a small replay-tour link and Restore defaults. The system handles display scaling; there is no separate Larger text preference. Account identities and conversations survive restoring preferences. Settings navigation and menus must work at half-screen widths in both themes.

## Verification

Check rendered light/dark and narrow layouts; labels, keyboard navigation, dismissal, overflow and reduced motion matter as much as appearance. Screenshots and fixtures establish layout only, not real account access, inference quality or performance. Historical experiment details remain in their linked reports rather than being duplicated here.

### Monogram and anchored controls

Keep the LM monogram transparent and free of a visible tile, border or geometric background. Use its cropped content viewport, 32px navigation mark and 78px welcome mark, with the welcome mark grouped closely with the heading. Single-choice menus attach within 6px of their trigger, opening above when needed; short menus keep their natural height. Account/Model/Reasoning share typography and control height. Keep all connected providers on one row; move neighboring composer actions below rather than wrapping a provider onto a second line.

### Approved satin-glass Lm

The owner selected proposal 2, frosted/satin glass, on 13 November. Use the approved pale ice-blue Lm with its soft lavender fold, unchanged in both themes. The owner explicitly accepts its lower contrast on light surfaces; do not introduce a darker light-theme variant. The transparent PNG is `public/lumen.png`; the UI SVG viewport trims its transparent canvas. `.codex/lumen-satin-icon.svg` is the square, transparent build input for the platform icon pipeline. Keep the monogram free of a visible frame or tile.
