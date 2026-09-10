# v6.5.30 XToys Position Geometry Fix

- XToys Position mode now maps 0..100% into the same effective stroke/depth window used by XToys Speed/pattern mode.
- 0% = depth - stroke; 100% = depth.
- Owner stroke/depth limits are applied through the existing clamp_input path.
- Sexync uses its existing dedicated mapping and is unchanged.
- SexCode diagnostic effective-position calculation now uses the Sexync mapper.
- Retains v6.5.29 Pawprint collision fix and earlier owner/Wi-Fi/XToys handoff fixes.
