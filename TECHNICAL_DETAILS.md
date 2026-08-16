# Technical Details: Non-Standard Solver & Web Interface

This document describes the findings, architecture, and roadmap for building an interactive solver web interface using the `ingrid-core` Rust engine, adapted to handle arbitrary layouts (such as hexagonal grids with 3-way crossings).

---

## 1. Grid Topology & The $k$-Way Crossing Challenge

In standard 2D crosswords, every filled cell is the intersection of exactly two slots (one Across and one Down). Thus, a slot crossing at any cell index is at most a single partner slot. 
This is represented in `ingrid_core` as:
```rust
pub struct SlotConfig {
    ...
    pub crossings: Vec<Option<Crossing>>,
}
```

However, in non-standard layouts—such as the hexagonal grid in the user's example—cells are shared by **three** crossing slots (Horizontal, Diagonal-1, Diagonal-2). For example:
- Cell `a` is shared by slots `a b c`, `a e j`, and `a f l s z`.

To support this natively without losing constraint propagation in any direction, we must extend `crossings` to support multiple partner slots per cell index:
```rust
pub struct SlotConfig {
    ...
    pub crossings: Vec<Vec<Crossing>>, // Empty vector represents no crossing
}
```

---

## 2. Code Changes Required in `ingrid_core`

Fortunately, the solvers (`backtracking_search` and `arc_consistency`) are entirely decoupled from physical coordinates and only read `crossings` to propagate constraints. The necessary refactoring is small:

### A. `src/grid_config.rs`
1. Update `SlotConfig::crossings` to `Vec<Vec<Crossing>>`.
2. Update `SlotConfig::fill` and `complete_fill` to map indices directly.
3. Update `sort_slot_options` to average the letter fill scores over all partner slots crossing at each cell:
   ```rust
   let fill_score = slot_config
       .crossings
       .iter()
       .zip(&word.glyphs)
       .map(|(crossings, &glyph)| {
           if crossings.is_empty() {
               0.0
           } else {
               crossings.iter().map(|crossing| {
                   let crossing_counts_by_cell =
                       &glyph_counts_by_cell_by_slot[crossing.other_slot_id];
                   (crossing_counts_by_cell[crossing.other_slot_cell][glyph] as f32).log10()
               }).sum::<f32>() / (crossings.len() as f32)
           }
       })
       .fold(0.0, |a, b| a + b)
       / (slot_config.length as f32);
   ```

### B. `src/arc_consistency.rs`
1. **Queuing Cells (Line 239)**: Update to check if any crossing in the vector is non-fixed:
   ```rust
   crossing_list.iter().any(|crossing| !fixed_slots[crossing.other_slot_id])
   ```
2. **Propagating Eliminations (Line 341)**: Loop through all crossings for the cell:
   ```rust
   for crossing in &slot_config.crossings[cell_idx] {
       if fixed_slots[crossing.other_slot_id] {
           continue;
       }
       // ... check glyph counts and enqueue ...
   }
   ```
3. **Selecting Cells by Weight (Line 403)**: Find the maximum or average crossing weight:
   ```rust
   let crossings = &config.slot_configs[slot_id].crossings[cell_idx];
   let max_weight = crossings.iter().map(|c| crossing_weights[c.crossing_id]).fold(0.0, f32::max);
   Reverse(FloatOrd(max_weight))
   ```
4. **Pruning Neighbors (Line 413)**: Iterate over all partner slots for the given cell index and prune their domains:
   ```rust
   for crossing in &config.slot_configs[slot_id].crossings[cell_idx] {
       let other_slot_id = crossing.other_slot_id;
       let other_slot_cell = crossing.other_slot_cell;
       // ... run elimination loop ...
   }
   ```

### C. `src/backtracking_search.rs`
1. Update `calculate_slot_weight` to sum/average weights across all crossings at a cell:
   ```rust
   config.slot_configs[slot_id]
       .crossings
       .iter()
       .map(|crossings| {
           crossings.iter().map(|crossing| {
               if slots[crossing.other_slot_id].remaining_option_count > 1 {
                   crossing_weights[crossing.crossing_id]
               } else {
                   0.0
               }
           }).sum::<f32>()
       })
       .sum()
   ```

---

## 3. Parsing Custom Topological Input

We can write a parser that converts the user's string of slots into a complete, valid `OwnedGridConfig` configuration:

1. **Parse Input**: Split by `;` (or `\n` if no `;` exists) and then by whitespace to extract a list of slots, where each slot is a sequence of arbitrary cell name strings (e.g., `["a", "b", "c"]`).
2. **Map Cells**: Deduplicate all cell names across all slots to produce a flat list of unique cells. Map each name to its 1D index `0..N`.
3. **Build Crossings**:
   - For each cell, identify which slots contain it and at what index.
   - For every pair of slots sharing a cell, create a symmetric `Crossing` pair sharing a unique `CrossingId`.
4. **Generate Grid Config**: Assemble the custom `SlotConfig` objects using the 1D cell indices and crossings, and initialize the `OwnedGridConfig`.

---

## 4. Web Interface & Worker Design

1. **Web Worker**: Wraps the Wasm solver to run CPU-bound AC-3 constraint propagation off the UI thread.
2. **API**:
   - `init_solver(slots_string, dictionary_file)` -> Sets up the grid topology and loads the dictionary.
   - `update_cell(cell_name, letter_or_none)` -> Sets/clears a cell's letter, triggers dictionary filtering, runs AC-3, and returns the list of remaining options for each slot.
3. **Frontend UI**:
   - A clean textbox/editor to paste custom slot definitions.
   - A dynamic visual grid layout rendering the cells and their connections.
   - Side panels showing the slots list, their current fills, and a scrollable list of valid candidate words for the active slot. Selecting a candidate instantly fills it and propagates constraints.
