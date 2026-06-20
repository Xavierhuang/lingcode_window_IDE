# Contributing to LingCode

Thanks for helping make LingCode better!

All activity in LingCode community spaces is subject to our
[Code of Conduct](./CODE_OF_CONDUCT.md).

## Contribution ideas

We especially welcome PRs that are:

- Fixing or extending the docs.
- Fixing bugs.
- Small enhancements to existing features so they work for more people (more
  platforms, modes, edge cases).
- Small extra features, like keybindings or actions you miss from other editors.

If you're thinking about a larger feature, open an issue or discussion first so we
can align on the design before you invest the effort.

## Sending changes

The best way to propose a change is a pull request. To give it the best chance of
being merged:

- Make sure the change is **desired** — confirm features with us before building.
- Include a clear description of **what you're solving**, and why it matters.
- Include **tests**.
- If it changes the UI, attach **screenshots** or a screen recording.
- Keep the PR about **one thing** — don't bundle a bugfix, a feature, and a refactor.
- Keep AI assistance under your own judgement: we're unlikely to merge code the
  author doesn't understand.

## UI/UX checklist

When your changes affect the UI, consult this checklist:

**Accessibility / Ergonomics**
- Do all keyboard shortcuts work, and are they discoverable (tooltips, menus, docs)?
- Do all mouse actions work (drag, context menus, resizing, scrolling)?
- Does it look great in light and dark mode? Are hover/focus/active states clear?
- Is it usable keyboard-only?

**Responsiveness**
- Does the UI scale on narrow panes, short panes, and high-DPI displays?
- Do dialogs/modals stay centered and within the viewport?

**Performance**
- All interactions must have instant feedback; show progress for slow work.
- Handle large files and big projects without degrading. Frames under ~8ms (120fps).

**Consistency & Text**
- Match the existing design language (spacing, typography, icons) and voice.
- Keep strings concise, clear, and jargon-free.

**Edge Cases**
- Consider the unhappy path: errors, offline/online, authenticated/unauthenticated,
  and missing or delayed data. Are error messages actionable?

## Things we will (probably) not merge

- Anything better provided by an extension (e.g. a new language or theme).
- Giant refactorings, or non-trivial changes with no tests.
- Stylistic-only changes that don't alter app logic.
- Anything that appears AI-generated without the author understanding it.
