# Project Hand-off: Wasm-Based Interactive Solver for Non-Standard Crosswords

This document summarizes findings and provides a roadmap for adapting the `ingrid-core` Rust engine to run as a WebAssembly (Wasm) backend for an interactive, assisted-fill crossword constructor.

---

## 1. Solver Core Architecture & Non-Standard Layouts

A code review of `ingrid_core` reveals that the core backtracking search and arc-consistency engines are **highly abstract** and completely decoupled from standard 2D physical grid layouts:

- **No 2D Hardcoding in Solvers**: Neither [`backtracking_search.rs`](file:///Users/alexboisvert/GitHub/ingrid_core/src/backtracking_search.rs) nor [`arc_consistency.rs`](file:///Users/alexboisvert/GitHub/ingrid_core/src/arc_consistency.rs) make any assumptions about 2D grid coordinates, directions (`Across`/`Down`), width, or height. They treat the grid purely as a constraint graph of variables (slots) and crossings.
- **Topology Definition**: The layout is defined by a slice of [`SlotConfig`](file:///Users/alexboisvert/GitHub/ingrid_core/src/grid_config.rs#L52) structs:
  ```rust
  pub struct SlotConfig {
      pub id: SlotId,
      pub length: usize,
      pub crossings: Vec<Option<Crossing>>,
      // Note: start_cell and direction are present but can be populated with dummy/stub values
      pub start_cell: GridCoord,
      pub direction: Direction,
      ...
  }
  ```
- **Crossings Map**: Intersections are tracked using the [`Crossing`](file:///Users/alexboisvert/GitHub/ingrid_core/src/grid_config.rs#L44) struct, which maps a cell index in one slot to a cell index in another slot, using a unique `crossing_id` for tracking CSP heuristics.

### Adapting to Non-Standard Layouts (e.g., Hexagonal, 3D, Spherical)
To build a solver for a custom topology, you do not need to modify the search algorithm. You only need to write a custom generator that:
1. Identifies every fillable cell in the layout and maps it to a unique index in a flat 1D array.
2. Represents each word slot as a sequence of these flat cell indices.
3. Computes the overlapping cell indices between slots to generate the list of `Crossing` records and a total `crossing_count`.

---

## 2. WebAssembly (Wasm) Export Readiness

The codebase is already highly compatible with WebAssembly:
- **Wasm-Friendly Dependencies**: The crate uses [`instant`](file:///Users/alexboisvert/GitHub/ingrid_core/Cargo.toml#L12) with the `wasm-bindgen` feature enabled to replace OS-dependent timing APIs.
- **Randomness Support**: The standard library's `rand` configuration uses `getrandom`. To compile for browser environments (`wasm32-unknown-unknown`), ensure the `js` feature for `getrandom` is active in `Cargo.toml`:
  ```toml
  getrandom = { version = "0.4", features = ["js"] }
  ```

---

## 3. Implementing the Interactive "Assisted Fill" Engine

For an interactive constructor where selecting a word for one slot immediately narrows down the viable options in all other slots, you will utilize the engine's built-in **Arc Consistency** solver instead of running a full backtracking search.

### Execution Loop:
1. **Represent Grid State**: Maintain a 1D array `fill: Vec<Option<GlyphId>>` representing the characters current placed in the grid.
2. **Filter Initial Options**: When a letter changes, run [`generate_slot_options`](file:///Users/alexboisvert/GitHub/ingrid_core/src/grid_config.rs#L434) on all slots to filter the global dictionary down to words matching the slot's current fixed characters.
3. **Propagate Constraints (AC-3)**: Run [`establish_arc_consistency_for_static_grid`](file:///Users/alexboisvert/GitHub/ingrid_core/src/arc_consistency.rs#L517). This will analyze intersections and prune words from other slots that would make the grid unfillable (e.g., if choosing "CAT" for Slot 1 leaves no possible intersecting words for Slot 2, "CAT" is removed).
4. **Update Frontend UI**: Return the updated lists of remaining valid options for each slot. Because AC-3 constraint propagation is extremely fast, this loop runs in milliseconds, providing instant feedback.

---

## 4. Frontend Blueprint

- **Web Workers**: Keep the Wasm solver inside a Web Worker. This isolates the CPU-heavy dictionary filtering and constraint propagation from the main UI thread.
- **Wasm Interface**: Create a thin Rust wrapper using `wasm-bindgen` that accepts the JSON-serialized grid topology/fill and returns the arrays of valid options per slot.
