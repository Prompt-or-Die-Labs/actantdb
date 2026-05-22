---
name: design-system-colosseum
description: Creates implementation-ready design-system guidance with tokens, component behavior, and accessibility standards. Use when creating or updating UI rules, component specifications, or design-system documentation.
---

<!-- TYPEUI_SH_MANAGED_START -->

# Colosseum

## Mission
Deliver implementation-ready design-system guidance for Colosseum that can be applied consistently across dashboard web app interfaces.

## Brand
- Product/brand: Colosseum
- URL: https://arena.colosseum.org/hackathon
- Audience: authenticated users and operators
- Product surface: dashboard web app

## Style Foundations
- Visual style: structured, accessible, implementation-first
- Main font style: `font.family.primary=Inter`, `font.family.stack=Inter, Arial, sans-serif`, `font.size.base=16px`, `font.weight.base=400`, `font.lineHeight.base=24px`
- Typography scale: `font.size.xs=12px`, `font.size.sm=14px`, `font.size.md=16px`, `font.size.lg=18px`, `font.size.xl=20px`, `font.size.2xl=24px`
- Color palette: `color.text.primary=#a0a0a0`, `color.surface.base=#000000`, `color.text.tertiary=#ededed`, `color.text.inverse=#25d0ab`, `color.surface.muted=#0f0f0f`, `color.surface.raised=#2e2e2e`, `color.surface.strong=#f7f7f7`, `color.border.default=#e5e7eb`, `color.border.strong=#01453d`
- Spacing scale: `space.1=4px`, `space.2=6px`, `space.3=8px`, `space.4=12px`, `space.5=16px`, `space.6=24px`, `space.7=32px`
- Radius/shadow/motion tokens: `radius.xs=4px`, `radius.sm=6px` | `shadow.1=rgb(255, 255, 255) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 3px, rgba(0, 0, 0, 0) 0px 0px 0px 0px` | `motion.duration.instant=100ms`, `motion.duration.fast=150ms`, `motion.duration.normal=200ms`

## Accessibility
- Target: WCAG 2.2 AA
- Keyboard-first interactions required.
- Focus-visible rules required.
- Contrast constraints required.

## Writing Tone
concise, confident, implementation-focused

## Rules: Do
- Use semantic tokens, not raw hex values in component guidance.
- Every component must define required states: default, hover, focus-visible, active, disabled, loading, error.
- Responsive behavior and edge-case handling should be specified for every component family.
- Accessibility acceptance criteria must be testable in implementation.

## Rules: Don't
- Do not allow low-contrast text or hidden focus indicators.
- Do not introduce one-off spacing or typography exceptions.
- Do not use ambiguous labels or non-descriptive actions.

## Guideline Authoring Workflow
1. Restate design intent in one sentence.
2. Define foundations and tokens.
3. Define component anatomy, variants, and interactions.
4. Add accessibility acceptance criteria.
5. Add anti-patterns and migration notes.
6. End with QA checklist.

## Required Output Structure
- Context and goals
- Design tokens and foundations
- Component-level rules (anatomy, variants, states, responsive behavior)
- Accessibility requirements and testable acceptance criteria
- Content and tone standards with examples
- Anti-patterns and prohibited implementations
- QA checklist

## Component Rule Expectations
- Include keyboard, pointer, and touch behavior.
- Include spacing and typography token requirements.
- Include long-content, overflow, and empty-state handling.

## Quality Gates
- Every non-negotiable rule must use "must".
- Every recommendation should use "should".
- Every accessibility rule must be testable in implementation.
- Prefer system consistency over local visual exceptions.

<!-- TYPEUI_SH_MANAGED_END -->
