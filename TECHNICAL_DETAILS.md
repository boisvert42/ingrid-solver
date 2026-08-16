# Ingrid Core: Project Architecture & Technical Details

This document provides a comprehensive technical overview of the `ingrid_core` crossword construction engine. It describes the design philosophy, file structures, algorithms, and topological models, serving as a developer/LLM guide to understanding the codebase.

---

## 1. High-Level Architecture

`ingrid_core` is a high-performance, topology-agnostic crossword generation engine written in Rust. It compiles both as a command-line binary and a WebAssembly (Wasm) library for interactive constructor interfaces.

### Core Philosophy: Topology Agnosticism
Neither the backtracking search nor the arc consistency constraint propagation engine has any knowledge of physical 2D grid dimensions (width, height), directions (across/down), or planar layouts. 
Instead, they treat the crossword puzzle purely as a **Constraint Satisfaction Problem (CSP)** graph:
* **Variables**: The word slots.
* **Domains**: The lists of candidate words of matching lengths matching current fixed letters.
* **Constraints**: Intersections (crossings) where overlapping slots must agree on the same letter.

By separating the topology model from the solving algorithms, the library can solve arbitrary crossword layouts (planar 2D, 3D cubes, hexagonal grids, or spherical surfaces) without modifying the search logic.

---

## 2. Key Modules & Important Files

### `src/word_list.rs`
Handles loading, normalising, scoring, and storing the global dictionary:
* Loads wordlists from raw strings, memory verves (`Memory`), or files.
* Bucket-allocates words by length to optimize queries.
* Pre-allocates unique letters (glyphs) and maps them to `GlyphId` (usize) for fast matching.

### `src/grid_config.rs`
Defines the structure of the crossword grid and slots:
* **`SlotConfig`**: Defines a word slot, including its length, crossings, and a list of `cell_indices` mapping the slot to a flat 1D cell array.
* **`Crossing`**: Maps the index of a cell in one slot to another index in a partner slot, associated with a unique `CrossingId`.
* **`GridConfig`**: Combines the active `WordList`, the flat grid `fill` (letters array), slot configs, and candidate options lists.

### `src/arc_consistency.rs`
Implements the crossword-optimized **AC-3 (Arc Consistency)** constraint propagation algorithm:
* Prunes candidate word domains for slots when letters are filled or when other domains shrink.
* Eliminates candidate words from partner slots that would make the board unfillable.
* Runs in milliseconds, serving as the backbone for the interactive constructor's live assisted-fill feedback.

### `src/backtracking_search.rs`
Implements the core backtracking generator search algorithm to fill empty grids:
* Uses the **`wdeg` (Weighted Degree)** heuristic to prioritize filling variables (slots) involved in the most constraint violations.
* Periodically establishes arc consistency during the search tree traversal to fail early.

---

## 3. Support for $k$-Way Crossings (Hexagonal, 3D, and Arbitrary topologies)

In standard 2D grids, every cell is crossed by exactly **two** slots (one Across, one Down). In non-standard grids (like hexagonal layouts), cells are shared by **three** or more intersecting slots (e.g. Horizontal, Diagonal-1, Diagonal-2).

To support these topologies natively, the `crossings` structure in `SlotConfig` is represented as a nested vector:
```rust
pub struct SlotConfig {
    pub id: SlotId,
    pub length: usize,
    pub crossings: Vec<Vec<Crossing>>, // A vector of crossings per cell index
    pub cell_indices: Option<Vec<usize>>, // Maps slot positions to a flat 1D cell array
    ...
}
```

### Building Custom Topologies
To configure `ingrid_core` for an arbitrary layout:
1. Map every fillable cell in the layout to a unique 1D cell index.
2. Represent each word slot as a list of its 1D cell indices.
3. Compute crossings: For each cell, find all slots that cross it. Generate symmetric `Crossing` entries sharing a unique `CrossingId` for every pair of slots intersecting at that cell.
4. Set `cell_indices` on `SlotConfig` to map the slot's letters to the 1D fill array.
