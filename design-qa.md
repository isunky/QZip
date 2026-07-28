# Settings independent-selection design QA

## Comparison target

- Source: latest user-reported settings screenshot (`C:/Users/HB-SUN~1/AppData/Local/Temp/codex-clipboard-2ac714f2-d95d-4047-88a8-eaf92afd0d67.png`).
- Implementation: `http://127.0.0.1:1420/`, Settings > Appearance, light mode and mint accent.
- Implementation evidence: `artifacts/design-qa/settings-segmented-pill-style.png`.
- Combined comparison: `artifacts/design-qa/settings-source-vs-pill-style.png`.
- Comparison crops were normalized to the same visible settings region before visual review.

## Result

- P1 fixed: the shared-edge segmented style was replaced with independent inset selection cards.
- Every selected item now owns a complete one-pixel accent border, a small internal radius, and a subtle elevation shadow.
- The control group uses visible overflow and a three-pixel inner gap, so first and final selections cannot be clipped by the parent edge.
- Keyboard focus uses a separate two-pixel outline and does not affect layout dimensions.

## Verification

- Checked all settings segmented controls: theme, accent, UI scale, density, default archive format, compression level, and conflict policy.
- Exercised a final-position option (`青灰`) and confirmed a complete `1px solid` active border, visible overflow, and the expected shadow.
- Restored the default mint accent after the interaction check.
- UI and desktop TypeScript checks, desktop Vitest (`7/7`), and the production web build pass.
- Browser console: no errors.

## Comparison history

1. Before: selected borders shared the group edge and visually disappeared at the first item's rounded left edge.
2. First correction retained the shared-edge visual model and did not fully remove the perceived clipping in the user's environment.
3. Final correction changes the component model: each selected option is inset from the group surface and renders its own complete border.
4. After: the combined comparison shows consistent breathing room and an uninterrupted border on all four sides of each selected item.

final result: passed
