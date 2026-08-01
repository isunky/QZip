# QZip Windows icon refresh

> The zipper-based round was rejected on 2026-08-01. Its files remain only as decision history and must not be exported as runtime icons.

## Current brief

- Windows-only application and archive-file icon family.
- The zipper, folder, suitcase, document, and package metaphors are retired.
- Explore a professional animal identity built around a hedgehog without cat or squirrel cues.
- Keep the application mark compact and legible at 16–32 px; avoid mascot faces, fur, tiny feet, and decorative detail.
- Use a dark graphite rounded tile with a jade/teal mark and restrained mint highlights.
- Archive-file icons will be designed only after the application mark is selected.

## Application candidates

| ID | Direction | SHA-256 |
| --- | --- | --- |
| A | Front-facing cream folder with a bold graphite zipper | `a9685d1137d91f8eb8c14dc27287e2c8596792463bfc6275f16e0331ee01f46c` |
| B | Two soft folder panels joined by a green zipper | `34d9d316423144ee39285955762eaa0436c1385d5fab72eb890ec953511d9bc7` |
| C | Flat negative-space folder glyph and zipper seam | `06cca2e747c1a9a025e2142ed0f2d7a89ac71e0dc1b43717f69dfe92c4138a42` |
| D | Folder with one folded document and metallic zipper | `ba4223f5cb8d271c974a5ffbdb5df51e6525cffe8143a6746a4a1d373d0b667e` |

All candidates were generated with the built-in ImageGen route on a flat magenta chroma-key background. The checked-in candidate PNGs were processed with the ImageGen skill's chroma-key removal helper. They are 1254×1254 RGBA images with fully transparent corners.

## Review recommendation

Candidate C is the strongest small-size system icon because it has the fewest tonal boundaries and the clearest silhouette. Candidate D best preserves the current product illustration's friendly 2.5D character but will require a simplified small-size tier.

## Round 2: zipper-free identity directions

| ID | Direction | SHA-256 |
| --- | --- | --- |
| E | Compression layers converging toward a compact core | `850fc18138043dc31101d342d995fdffd7b8aea1dae10ff424407ef3a508ac6b` |
| F | Geometric Q monogram with nested compression space | `4d15acf405d7f79c4009f951406fc43aec78aa15693c8ea8635784abd8da3964` |
| G | Compact layered cube with a warm inner plane | `55f395dd0ff1a1401a027d7174db77e8db30887f33d2b752a3735521267a36c1` |
| H | Opposing forms compressing toward a bright center | `57fea4028df32003cd03289815ab386cfd30fd433f1c0c2ef194142cef1455f0` |

Round 2 explicitly prohibits zipper, folder, suitcase, belt, document, package, and format-label imagery in the application mark. Candidate F has the strongest proprietary brand potential and remains readable without relying on archive-software clichés. Candidate E communicates compression most directly.

## Round 3: hard geometric hedgehog-Q directions

The first friendly hedgehog mascot study was rejected for looking generic, floral, and overly cute. This round removes facial features and combines a hedgehog silhouette with the QZip initial.

| ID | File | Direction | SHA-256 |
| --- | --- | --- | --- |
| A | `candidates-v4/hedgehog-q-curled.png` | Curled hedgehog and Q as one continuous mark | `c31e2a0e1645c2c3850652909fc7da0d9d087b2e44f250ad7aaa9925c30a52a6` |
| B | `candidates-v4/hedgehog-q-silhouette.png` | Low geometric hedgehog profile with a circular counter | `c5c366e7f43deacded9daad65aa4f48120e90fffae4e00455e3b8b1a18c365e3` |
| C | `candidates-v4/hedgehog-q-armored.png` | Bold Q monogram with an armored quill crown | `4fed690bea534ef068d84ee638afdf469c0aa3adb0713327df7711bbdb18099e` |

All three files were generated with the built-in ImageGen route, processed from a flat magenta chroma key, and validated as 1254×1254 RGBA PNGs with fully transparent corners. Candidate B reads most clearly as a hedgehog; candidate C reads most clearly as a professional QZip application mark.

## Implemented reference set: fox/ermine QZip mark

The uploaded reference board was selected for implementation on 2026-08-01. The large left application mark is preserved as the identity source: white curled animal, mint/teal rounded tile, pointed ears, closed-eye arc, and spiral tail.

- Source master: `final/qzip-fox-master.png`
- Runtime variants: `apps/desktop/src/assets/app-icons/{light,dark}-{mint,ocean,lavender,amber,coral,cyan-slate}.png`
- Windows app icon: `apps/desktop/src-tauri/icons/icon.ico`
- Windows file icons: `apps/desktop/src-tauri/icons/file-types/*.ico`
- Export manifest: `final/icon-export-manifest.json`

The application has 12 runtime variants (six accent themes × light/dark). Windows installation and Start-menu resources remain the light mint default because static shell icons are cached by Windows. File icons use fixed format colors and independent ProgIDs for 7Z, ZIP, RAR, TAR, GZ, TGZ, XZ, TXZ, BZ2, ISO, CAB, and WIM; the generic archive fallback is retained for upgrades.
