# Static Frame Filtering Design

**Date**: 2026-06-08
**Status**: Approved for implementation
**Author**: Brainstorming session

## Problem

Surveillance cameras pointed at static scenes (empty rooms, hallways, parking lots at night) produce
many consecutive frames with near-identical content. Extracting and storing all these frames wastes
storage, bandwidth, and compute.

## Solution Overview

Add a new `RuleConfig::StaticFrame` rule variant that compares each decoded frame's Y (luminance)
plane against the previous decoded frame, skipping frames whose difference falls below a
configurable threshold. Three comparison methods are available, selectable per-rule.

## Data Structures

### `RuleConfig` — New Variant

```rust
pub enum RuleConfig {
    // ... existing variants
    StaticFrame {
        threshold: f64,
        method: ComparisonMethod,
        force: bool,  // default false
    },
}
```

### `ComparisonMethod`

```rust
pub enum ComparisonMethod {
    PixelDiff,       // Y-plane pixel-wise absolute difference, normalized to [0.0, 1.0]
    PerceptualHash,  // 64-bit perceptual hash, Hamming distance as ratio [0.0, 1.0]
    Ssim,            // Structural similarity index [0.0, 1.0], 1.0 = identical
}
```

### `FrameComparator`

Owned by `RuleEngine`, holds the previous frame's Y plane:

```rust
pub struct FrameComparator {
    prev_y: Option<Vec<u8>>,
    prev_width: u32,
    prev_height: u32,
    method: ComparisonMethod,
    threshold: f64,
}
```

Methods:
- `new(method, threshold) -> Self`
- `is_static(&mut self, y_plane, width, height) -> Result<bool>` — updates `prev_y` on call

## Pipeline Integration

The static frame check sits between scdet filter evaluation and YUV plane copy in the decode loop:

```
Decode loop (per frame):
  1. avcodec_send_packet + avcodec_receive_frame
  2. scdet filter → scene_change_score
  3. FrameComparator.is_static(y_plane, width, height)
     → if static → set flag `is_static_frame = true` (continue to rule eval)
  4. YUV planes copy → DecodedFrame
  5. Rule engine evaluation (all rules still evaluated)
  6. If no rule triggered AND is_static_frame → skip (drop frame)
     Otherwise → JPEG encode → send to channel
  7. (Potential optimization: skip rule eval when only StaticFrame rules exist)
```

### Key Decisions

- **Comparison before YUV copy**: Comparison uses `AVFrame.data[0]` directly (zero copy). Full
  YUV copy always occurs (the Y plane is needed for JPEG encoding anyway, and the copy is fast).
  The static check is a pre-filter flag, not a gate.
- **Rule evaluation always runs**: The static flag is checked AFTER rule evaluation, not before.
  This ensures other rule types (Interval, SceneChange) can override the static gate and force
  extraction. Optimization (skipping rule eval when only StaticFrame rules exist) is deferred.
- **First frame**: Always treated as non-static (prev_y is None).
- **First frame**: Always treated as non-static (prev_y is None).
- **Resolution change**: Comparator is reset, first frame of new resolution treated as non-static.
- **Timing instrumentation**: Added to the existing `Pipeline timing` log block as `t_static_sum`,
  reported every `STAGE_REPORT_INTERVAL` frames.

## Comparison Methods

### PixelDiff

- Computation: `sum(|curr[i] - prev[i]|) / (width * height * 255)`
- Threshold: `[0.0, 1.0]`, e.g., 0.05 = 5% average pixel brightness change
- Complexity: O(n), ~0.05ms @ 1080p
- Pros: Fastest, zero extra memory
- Cons: Sensitive to global illumination changes

### PerceptualHash

- Downsample Y plane to 8×8 (bilinear, via 1/4 reduction first), apply DCT, take low-frequency
  8×8 block, generate 64-bit hash
- Threshold: `distance / 64`, `[0.0, 1.0]`. E.g., 0.156 = 10 bits different out of 64.
- Complexity: ~0.3ms @ 1080p
- Pros: Tolerant of lighting changes, good for indoor surveillance
- Cons: More complex implementation

### SSIM

- Standard SSIM over Y plane with 8×8 sliding window (luminance, contrast, structure components)
- Threshold: `[0.0, 1.0]`, e.g., 0.95 means SSIM < 0.95 = change detected
- Complexity: O(n), ~2ms @ 1080p
- Pros: Best perceptual accuracy
- Cons: Highest computational cost

## Rule Interaction Logic

**Standalone**: `StaticFrame` acts as a gate — static = no extraction, changed = extract.

**In `Composite { Any }`**: Frame is extracted if ANY rule triggers, including `StaticFrame`.
  - `[Interval {5s}, StaticFrame {..}]`: Guaranteed minimum 1 frame / 5s, plus on-change capture.
  - This is the primary use case.

**In `Composite { All }`**: Not useful for `StaticFrame` + interval rules; included for completeness.

**Static frame skip refinement**:
- If `StaticFrame` evaluates as static BUT another rule in the composite (non-StaticFrame) would
  trigger, the frame is still extracted (the other rule overrides the static skip).
- If only `StaticFrame` rules exist, static frames are dropped entirely.

**`force: true`**: Extracts the frame even when static. Useful for debugging or monitoring.

**Rate limiting**: `RateLimited { rule: StaticFrame {..}, max_per_minute: 60 }` prevents
over-capture from wind/leaves while still capturing real motion.

## Hot Reload

- Rules updated via `PATCH /api/v1/streams/:id/rules` trigger `rebuild()` on `RuleEngine`.
- `FrameComparator` is rebuilt when `StaticFrame` rules change (prev_y discarded).
- No configuration changes to the API layer: `RuleConfig` serialization handles the new variant
  transparently.

## Error Handling & Edge Cases

| Scenario | Behavior |
|----------|----------|
| First frame (prev_y = None) | Treated as non-static, Y plane saved for next comparison |
| Resolution change | Comparator reset, first frame of new resolution treated as non-static |
| Corrupt Y plane data | Log warning, treat as non-static (safe mode) |
| Decoder flush / seek | Comparator reset |
| Frequent hot-reload method changes | Comparator rebuilt, prev_y discarded |
| stride > width | Only compare height × width region, skip stride padding |
| Threshold = 0.0 | All frames treated as non-static (disabled) |
| Threshold = 1.0 | All frames treated as static (unless other rule or force) |

## Testing

1. **Unit tests**: `FrameComparator` correctness for all three methods — identical frames,
   different frames, gradual-change frames.
2. **Threshold boundary tests**: 0.0, 1.0, mid-range values, verify classification boundaries.
3. **Integration tests**: API registration with `StaticFrame` rule, verify static frames skipped,
   motion frames captured.
4. **Composite rule tests**: `Interval + StaticFrame`, verify minimum frame rate guarantee.
5. **E2E regression**: Extend `tests/e2e/test_full_flow.py` to cover new rule type.

## Files to Change

| File | Change |
|------|--------|
| `src/pipeline/rule.rs` | Add `StaticFrame` variant, `FrameComparator`, `ComparisonMethod` |
| `src/pipeline/decode.rs` | Add static frame check + timing instrumentation in decode loop |
| `src/types.rs` | No changes needed (Y plane data already available in `AVFrame`) |
| `src/api/rules.rs` | No changes needed (generic `RuleConfig` serialization) |
| `tests/e2e/test_full_flow.py` | Add static frame filtering test scenario |
