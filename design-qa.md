# QZip M0 Design QA

**Comparison target**

- Source visual truth: `PIC/ChatGPT Image 2026年7月27日 10_36_35 (1).png`
- Implementation: `http://localhost:1420/`
- Implementation screenshot: `tests/e2e/qzip-home-actual.png`
- Combined evidence: `tests/e2e/qzip-home-comparison.png`
- Viewport and CSS size: `1448 x 1086`, `deviceScaleFactor: 1`
- Source and implementation pixels: both `1448 x 1086`; no density normalization was required.
- State: light mode, mint accent, home screen, no popover or toast open.

## Findings

No actionable P0, P1, or P2 differences remain in the M0 home screen.

- [P3] Illustration micro-detail differs from the source image.
  - Location: `.qzip-home-card__illustration`.
  - Evidence: the generated archive asset matches the pastel mint pouch, folder, leaf, transparency, and centered composition of the visual target, but intentionally does not reproduce the target's small sparkle marks and `ZIP` badge verbatim.
  - Impact: minor decorative difference only; hierarchy, scale, and the soft green visual language remain aligned.
  - Follow-up: replace with a licensed final brand illustration when one is available.

## Required fidelity surfaces

- **Fonts and typography:** system Chinese UI font stack, bold 64px main heading, secondary text, button copy, and header hierarchy were checked against the source. The main heading and controls remain single-line at the reference viewport.
- **Spacing and layout rhythm:** the card is `1118 x 792` at the reference viewport and begins at `x=165, y=218`, matching the source composition. Card radius, vertical rhythm, two-button grid, and wide empty breathing space were visually compared.
- **Colors and visual tokens:** light/mint state uses the shared `@qzip/ui` tokens for neutral surface, mint gradient primary CTA, mint border CTA, divider, and shadow. Dark mode plus ocean accent was also rendered and checked.
- **Image quality and asset fidelity:** `apps/desktop/src/assets/archive-hero.png` is a transparent raster asset with no opaque background or halo. It was generated for this project to match the reference art direction; standard interface icons use Lucide.
- **Copy and content:** header, headline, subtitle, CTAs, and supported-format hint match the source wording and intent. M0 actions clearly announce deferred file handling instead of simulating an unavailable workflow.

## Comparison history

1. Initial comparison found P2 visual drift: the main card sat too high, its corners were too tight, and CTA height was materially smaller than the source.
   - Fix: adjusted the desktop card offset, set the 40px home-card radius, increased the top rhythm, and increased CTA height to 100px.
   - Post-fix evidence: `tests/e2e/qzip-home-comparison.png` shows aligned card bounds and control proportions at `1448 x 1086`.
2. Minimum-window check at `760 x 520` found P2 vertical clipping caused by the narrow-layout stacked CTA rule.
   - Fix: added a short-window compact layout with a smaller illustration, condensed type scale, and two-column actions for the PRD minimum width.
   - Post-fix evidence: browser check reported `overflowX: false` and `overflowY: false` at `760 x 520`.

## Interaction and runtime evidence

- Primary CTA displays the M0 milestone notice.
- Task button opens the empty task panel.
- Settings opens the appearance panel; light/dark/system modes and all six accent swatches are wired. Browser check confirmed `data-mode=dark` and `data-accent=ocean`.
- Browser console check: no errors.

## Implementation checklist

- [x] Compare the reference and rendered home screen at the same viewport.
- [x] Correct P0-P2 visual drift found in the comparison loop.
- [x] Verify theme, task panel, primary CTA feedback, and minimum-window layout.
- [x] Preserve the final comparison and implementation screenshots.

## Follow-up polish

- [P3] Swap the generated archive illustration for the final licensed/owned brand illustration if one is produced later.

final result: passed
