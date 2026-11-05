# Native curve editor performance — 29 October 2026

Scope: the working tree based on `b58d62c`, version `0.9.0-dev.2`, Windows x64, Rust/Cargo 1.99.0. This is an original-code UI change. There are no hardware adapter, driver, startup permission or sampling-policy changes.

## Cause and implementation

Every mouse/keyboard presentation update previously cloned the complete curve, driver reference and up to 64 undo snapshots several times. It also reset and repopulated all 127 native point-selector rows, including when only the selected index changed.

The canvas now owns the live draft and history. Presentation reads small projections; switching curves and saving capture the complete current model. The point selector preserves unchanged rows, replaces a single edited row, and batches a rebuild for group edits. A trial using repeated delete/insert for group edits was slower and was discarded. Redraw suppression is paired only for visible controls: [Microsoft documents that enabling redraw can make a hidden control visible](https://learn.microsoft.com/en-us/windows/win32/gdi/wm-setredraw).

The editor also ignored the cursor position in the release event. The native regression test failed before the fix: the released point stayed at the previous mouse-move position. The final [WM_LBUTTONUP coordinates](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup) now finish an active gesture. Cancelled or lost gestures are not restarted. One drag remains one undo operation and GPU voltage anchors remain fixed.

## Local timing

The manual `curve_ui::tests::interaction_benchmark` uses actual hidden Win32 controls and a synthetic 127-point GPU curve with 64 undo entries. Each loop performs 1,000 model/caption/native-control updates, including numeric fields and enabled states. It does not access hardware, save profiles, display UAC, or run a fan helper.

| Workload | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Select a point and synchronize controls | 1,021.50 ms | 58.12 ms | 94.3% |
| Shift the whole curve and synchronize controls | 842.68 ms | 464.86 ms | 44.8% |

The baseline is one local release-mode sample; after values are medians of three runs after compilation completed on the same host. Selection samples were 57.83 / 58.77 / 58.12 ms; group-edit samples were 460.91 / 464.86 / 476.39 ms. The before run used the original production sources plus the same timing fixture. Runs during concurrent release builds were heavily affected by contention and are not used for this comparison.

This is a comparison of the synchronous editing path, not an end-to-end frame-rate, input-latency, startup or whole-app speed claim. Painting/composition, physical input and real driver latency are outside this benchmark. No absolute timing assertion is used in the test suite.

Reproduce explicitly:

```powershell
cargo test -p omen-hub-plus-plus --release curve_ui::tests::interaction_benchmark -- --ignored --nocapture --test-threads=1
```

The benchmark is ignored in ordinary test runs so timing does not affect correctness checks. The regular native fixtures cover final release coordinates, cancel, capture loss, GPU group gestures/anchors, undo/redo, point-list contents/visibility, curve switching and saving the live in-memory draft. All fixture data is synthetic; no profile is applied.

Two production release previews were rendered and visually inspected: `gpu-controls` (1380 × 940 outer window) and `gpu-draft-small` (960 × 710). The native labels, selected point, chart and action buttons were present; the compact view kept its numeric controls hidden. The previews only read the existing GPU curve. This is not a full multi-DPI or accessibility acceptance.

Current build identity and verification results belong in [Validation](../VALIDATION.md). No new total tray-memory measurement or physical hardware qualification is claimed.
