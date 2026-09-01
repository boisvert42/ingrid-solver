//! This module implements code for configuring a crossword-filling operation, independent of the
//! specific fill algorithm.

use fancy_regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::{GlyphId, WordId};
use crate::util::build_glyph_counts_by_cell;
use crate::word_list::WordList;

/// An identifier for the intersection between two slots; these correspond one-to-one with checked
/// squares in the grid and are used to track weights (i.e., how often each square is involved in
/// a domain wipeout).
pub type CrossingId = usize;

/// An identifier for a given slot, based on its index in the `GridConfig`'s `slot_configs` field.
pub type SlotId = usize;

/// Zero-indexed x and y coords for a cell in the grid, where y = 0 in the top row.
pub type GridCoord = (usize, usize);

/// The direction that a slot is facing.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[allow(dead_code)]
pub enum Direction {
    Across,
    Down,
}

/// A struct representing a crossing between one slot and another, referencing the other slot's id
/// and the location of the intersection within the other slot.
#[derive(Debug, Clone)]
pub struct Crossing {
    pub other_slot_id: SlotId,
    pub other_slot_cell: usize,
    pub crossing_id: CrossingId,
}

/// A struct representing the aspects of a slot in the grid that are static during filling.
#[derive(Debug, Clone)]
pub struct SlotConfig {
    pub id: SlotId,
    pub start_cell: GridCoord,
    pub direction: Direction,
    pub length: usize,
    pub crossings: Vec<Vec<Crossing>>,
    pub min_score_override: Option<u16>,
    pub filter_pattern: Option<Regex>,
    pub cell_indices: Option<Vec<usize>>,
}

impl SlotConfig {
    /// Generate the coords for each cell of this slot.
    #[must_use]
    pub fn cell_coords(&self) -> Vec<GridCoord> {
        (0..self.length)
            .map(|cell_idx| match self.direction {
                Direction::Across => (self.start_cell.0 + cell_idx, self.start_cell.1),
                Direction::Down => (self.start_cell.0, self.start_cell.1 + cell_idx),
            })
            .collect()
    }

    /// Generate the indices of this slot's cells in a flat fill array like `GridConfig.fill`.
    #[must_use]
    pub fn cell_fill_indices(&self, grid_width: usize) -> Vec<usize> {
        if let Some(ref indices) = self.cell_indices {
            indices.clone()
        } else {
            self.cell_coords()
                .iter()
                .map(|loc| loc.0 + loc.1 * grid_width)
                .collect()
        }
    }

    /// Get the values of this slot's cells in a flat fill array like `GridConfig.fill`.
    #[must_use]
    pub fn fill(&self, fill: &[Option<GlyphId>], grid_width: usize) -> Vec<Option<GlyphId>> {
        self.cell_fill_indices(grid_width)
            .iter()
            .map(|&idx| fill[idx])
            .collect()
    }

    /// Get this slot's `fill` if and only if all of its cells are populated.
    #[must_use]
    pub fn complete_fill(
        &self,
        fill: &[Option<GlyphId>],
        grid_width: usize,
    ) -> Option<Vec<GlyphId>> {
        self.fill(fill, grid_width).into_iter().collect()
    }

    /// Generate a `SlotSpec` identifying this slot.
    #[must_use]
    pub fn slot_spec(&self) -> SlotSpec {
        SlotSpec {
            start_cell: self.start_cell,
            direction: self.direction,
            length: self.length,
        }
    }

    /// Generate a string key identifying this slot.
    #[must_use]
    pub fn slot_key(&self) -> String {
        self.slot_spec().to_key()
    }
}

/// A struct holding references to all of the information needed as input to a crossword filling
/// operation.
#[allow(dead_code)]
#[derive(Clone)]
pub struct GridConfig<'a> {
    /// The word list used to fill the grid; see `word_list.rs`.
    pub word_list: &'a WordList,

    /// A flat array of letters filled into the grid, in order of row and then column. `None` can
    /// represent a block or an unfilled cell.
    pub fill: &'a [Option<GlyphId>],

    /// Config representing all of the slots in the grid and their crossings.
    pub slot_configs: &'a [SlotConfig],

    /// An array of available words for each (respective) slot, based on both the word list config
    /// and the existing letters filled into the grid.
    pub slot_options: &'a [Vec<WordId>],

    /// A 4D index mapping `[slot_id][cell_index][glyph_id]` to `Vec<WordId>` of options
    /// for that slot containing that glyph in that cell position.
    pub slot_options_by_glyph: &'a [Vec<Vec<Vec<WordId>>>],

    /// The width and height of the grid.
    pub width: usize,
    pub height: usize,

    /// The number of distinct crossings represented in all of the `slot_configs`.
    pub crossing_count: usize,

    /// An optional atomic flag that can be set to signal that the fill operation should be canceled.
    pub abort: Option<&'a AtomicBool>,
}

/// A struct that owns a copy of each piece of information needed by `GridConfig`.
pub struct OwnedGridConfig {
    pub word_list: WordList,
    pub fill: Vec<Option<GlyphId>>,
    pub slot_configs: Vec<SlotConfig>,
    pub slot_options: Vec<Vec<WordId>>,
    pub slot_options_by_glyph: Vec<Vec<Vec<Vec<WordId>>>>,
    pub width: usize,
    pub height: usize,
    pub crossing_count: usize,
    pub abort: Option<Arc<AtomicBool>>,
}

impl OwnedGridConfig {
    #[allow(dead_code)]
    #[must_use]
    pub fn to_config_ref(&self) -> GridConfig<'_> {
        GridConfig {
            word_list: &self.word_list,
            fill: &self.fill,
            slot_configs: &self.slot_configs,
            slot_options: &self.slot_options,
            slot_options_by_glyph: &self.slot_options_by_glyph,
            width: self.width,
            height: self.height,
            crossing_count: self.crossing_count,
            abort: self.abort.as_deref(),
        }
    }
}

/// Given a configured grid, reorder the options for each slot so that the "best" choices are at the
/// front. This is a balance between fillability (the most important factor, since our odds of being
/// able to find a fill in a reasonable amount of time depend on how many tries it takes us to find
/// a usable word for each slot) and quality metrics like word score and letter score.
#[allow(clippy::cast_lossless)]
pub fn sort_slot_options(
    word_list: &WordList,
    slot_configs: &[SlotConfig],
    slot_options: &mut [Vec<WordId>],
) {
    // To calculate the fillability score for each word, we need statistics about which letters are
    // most likely to appear in each position for each slot.
    let glyph_counts_by_cell_by_slot: Vec<_> = slot_configs
        .iter()
        .map(|slot_config| {
            build_glyph_counts_by_cell(word_list, slot_config.length, &slot_options[slot_config.id])
        })
        .collect();

    // Now we can actually sort the options.
    for slot_idx in 0..slot_configs.len() {
        let slot_config = &slot_configs[slot_idx];
        let slot_options = &mut slot_options[slot_idx];

        slot_options.sort_by_cached_key(|&option| {
            let word = &word_list.words[slot_config.length][option];

            // To calculate the fill score for a word, average the logarithms of the number of
            // crossing options that are compatible with each letter (based on the grid geometry).
            // This is kind of arbitrary, but it seems like it makes sense because we care a lot
            // more about the difference between 1 option and 5 options or 5 options and 20 options
            // than 100 options and 500 options.
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

            // This is arbitrary, based on visual inspection of the ranges for each value. Generally
            // increasing the weight of `fill_score` relative to the other two will reduce fill
            // time.
            -((fill_score * 900.0) as i64
                + ((word.letter_score as f32) * 5.0) as i64
                + ((word.score as f32) * 5.0) as i64)
        });
    }
}

/// Given the slot configs and sorted slot options, generate an index mapping
/// `[slot_id][cell_index][glyph_id]` to a `Vec<WordId>` of options for that slot that have
/// that glyph at that cell position.
#[must_use]
pub fn build_slot_options_by_glyph(
    word_list: &WordList,
    slot_configs: &[SlotConfig],
    slot_options: &[Vec<WordId>],
) -> Vec<Vec<Vec<Vec<WordId>>>> {
    slot_configs
        .iter()
        .map(|slot_config| {
            let mut options_by_cell_and_glyph: Vec<Vec<Vec<WordId>>> = (0..slot_config.length)
                .map(|_| vec![vec![]; word_list.glyphs.len()])
                .collect();

            for &word_id in &slot_options[slot_config.id] {
                let word = &word_list.words[slot_config.length][word_id];
                for (cell_idx, &glyph_id) in word.glyphs.iter().enumerate() {
                    options_by_cell_and_glyph[cell_idx][glyph_id].push(word_id);
                }
            }

            options_by_cell_and_glyph
        })
        .collect()
}

/// A struct identifying a specific slot in the grid.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SlotSpec {
    pub start_cell: GridCoord,
    pub direction: Direction,
    pub length: usize,
}

impl SlotSpec {
    /// Parse a string like "1,2,down,5" into a `SlotSpec` struct.
    pub fn from_key(key: &str) -> Result<SlotSpec, String> {
        let key_parts: Vec<&str> = key.split(',').collect();
        if key_parts.len() != 4 {
            return Err(format!("invalid slot key: {key}"));
        }

        let x: Result<usize, _> = key_parts[0].parse();
        let y: Result<usize, _> = key_parts[1].parse();
        let direction: Option<Direction> = match key_parts[2] {
            "across" => Some(Direction::Across),
            "down" => Some(Direction::Down),
            _ => None,
        };
        let length: Result<usize, _> = key_parts[3].parse();

        if let (Ok(x), Ok(y), Some(direction), Ok(length)) = (x, y, direction, length) {
            Ok(SlotSpec {
                start_cell: (x, y),
                direction,
                length,
            })
        } else {
            Err(format!("invalid slot key: {key:?}"))
        }
    }

    /// Represent this slot as a string like "1,2,down,5".
    #[must_use]
    pub fn to_key(&self) -> String {
        let direction = match self.direction {
            Direction::Across => "across",
            Direction::Down => "down",
        };
        format!(
            "{},{},{},{}",
            self.start_cell.0, self.start_cell.1, direction, self.length,
        )
    }

    /// Does this spec match the given slot config?
    #[must_use]
    pub fn matches_slot(&self, slot: &SlotConfig) -> bool {
        self.start_cell == slot.start_cell
            && self.direction == slot.direction
            && self.length == slot.length
    }

    /// Generate the coords for each cell of this entry.
    #[must_use]
    pub fn cell_coords(&self) -> Vec<GridCoord> {
        (0..self.length)
            .map(|cell_idx| match self.direction {
                Direction::Across => (self.start_cell.0 + cell_idx, self.start_cell.1),
                Direction::Down => (self.start_cell.0, self.start_cell.1 + cell_idx),
            })
            .collect()
    }
}

/// Serialize a `SlotSpec` into a string key.
#[cfg(feature = "serde")]
impl Serialize for SlotSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_key())
    }
}

/// Deserialize a `SlotSpec` from a string key.
#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for SlotSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw_string = String::deserialize(deserializer)?;
        SlotSpec::from_key(&raw_string).map_err(serde::de::Error::custom)
    }
}

/// Given `GridEntry` structs specifying the positions of the slots in a grid, generate
/// `SlotConfig`s containing derived information about crossings, etc.
#[must_use]
pub fn generate_slot_configs(entries: &[SlotSpec]) -> (Vec<SlotConfig>, usize) {
    #[derive(Debug)]
    struct GridCell {
        entries: Vec<(usize, usize)>, // (entry index, cell index within entry)
        number: Option<u32>,
    }

    let mut slot_configs: Vec<SlotConfig> = vec![];

    // Build a map from cell location to entries involved, which we can then use to calculate
    // crossings.
    let mut cell_by_loc: HashMap<GridCoord, GridCell> = HashMap::new();

    for (entry_idx, entry) in entries.iter().enumerate() {
        for (cell_idx, &loc) in entry.cell_coords().iter().enumerate() {
            let grid_cell = cell_by_loc.entry(loc).or_insert_with(|| GridCell {
                entries: vec![],
                number: None,
            });
            grid_cell.entries.push((entry_idx, cell_idx));
        }
    }

    let mut ordered_coords: Vec<_> = cell_by_loc.keys().copied().collect();
    ordered_coords.sort_by_key(|&(x, y)| (y, x));
    let mut current_number = 1;
    for coord in ordered_coords {
        if cell_by_loc[&coord]
            .entries
            .iter()
            .any(|&(_, cell_idx)| cell_idx == 0)
        {
            cell_by_loc.get_mut(&coord).unwrap().number = Some(current_number);
            current_number += 1;
        }
    }

    // This is slightly tricky. When we're generating a Crossing, if
    // `(current_slot_id, crossing_slot_id)` is in this list, use its index; if not, use
    // `constraint_id_cache.len()` as the id and push `(crossing_slot_id, current_id)` into the list
    // so we can reuse it when we see the crossing from the other side. This wouldn't work if the
    // grid topology weren't 2D, so that each crossing is guaranteed to be seen by exactly two slots.
    let mut constraint_id_cache: Vec<(SlotId, SlotId)> = vec![];

    // Now we can build the actual slot configs.
    for (entry_idx, entry) in entries.iter().enumerate() {
        let crossings: Vec<Vec<Crossing>> = entry
            .cell_coords()
            .iter()
            .map(|&loc| {
                let crossing_idxs: Vec<_> = cell_by_loc[&loc]
                    .entries
                    .iter()
                    .filter(|&&(e, _)| e != entry_idx)
                    .collect();

                if crossing_idxs.is_empty() {
                    vec![]
                } else if crossing_idxs.len() > 1 {
                    panic!("More than two entries crossing in cell?");
                } else {
                    let &(other_slot_id, other_slot_cell) = crossing_idxs[0];

                    let crossing_id = if let Some(found_constraint_id) = constraint_id_cache
                        .iter()
                        .enumerate()
                        .find(|&(_, &id_pair)| id_pair == (entry_idx, other_slot_id))
                        .map(|(crossing_id, _)| crossing_id)
                    {
                        found_constraint_id
                    } else {
                        constraint_id_cache.push((other_slot_id, entry_idx));
                        constraint_id_cache.len() - 1
                    };

                    vec![Crossing {
                        other_slot_id,
                        other_slot_cell,
                        crossing_id,
                    }]
                }
            })
            .collect();

        slot_configs.push(SlotConfig {
            id: entry_idx,
            start_cell: entry.start_cell,
            direction: entry.direction,
            length: entry.length,
            crossings,
            min_score_override: None,
            filter_pattern: None,
            cell_indices: None,
        });
    }

    (slot_configs, constraint_id_cache.len())
}

/// Given a single slot's fill, minimum score, and optional filter pattern, generate the possible
/// options for that slot by starting with the complete word list and then removing words that
/// contradict the criteria. If `allowed_word_ids` is provided, the given words will be included in
/// the options as long as they don't contradict the fill, regardless of whether they match the min
/// score and filter pattern.
pub fn generate_slot_options(
    word_list: &mut WordList,
    entry_fill: &[Option<GlyphId>],
    min_score: u16,
    filter_pattern: Option<&Regex>,
    allowed_word_ids: Option<&HashSet<WordId>>,
) -> Vec<WordId> {
    let length = entry_fill.len();

    let options: Vec<WordId> = (0..word_list.words[length].len())
        .filter(|&word_id| {
            let word = &word_list.words[length][word_id];
            let enforce_criteria = allowed_word_ids
                .is_none_or(|allowed_word_ids| !allowed_word_ids.contains(&word_id));

            if enforce_criteria {
                if word.hidden || word.score < min_score {
                    return false;
                }

                if let Some(filter_pattern) = filter_pattern.as_ref() {
                    if !filter_pattern
                        .is_match(&word.normalized_string)
                        .unwrap_or(false)
                    {
                        return false;
                    }
                }
            }

            entry_fill.iter().enumerate().all(|(cell_idx, cell_fill)| {
                cell_fill
                    .map(|g| g == word.glyphs[cell_idx])
                    .unwrap_or(true)
            })
        })
        .collect();

    options
}

/// Given an input fill and an array of slot configs, generate the possible options for each slot
/// by starting with the complete word list and then removing words that contradict any fill that's
/// already present in the grid or violate criteria like minimum score or filter pattern.
pub fn generate_all_slot_options(
    word_list: &mut WordList,
    fill: &[Option<GlyphId>],
    slot_configs: &[SlotConfig],
    grid_width: usize,
    global_min_score: u16,
) -> Vec<Vec<WordId>> {
    slot_configs
        .iter()
        .map(|slot| {
            generate_slot_options(
                word_list,
                &slot.fill(fill, grid_width),
                slot.min_score_override.unwrap_or(global_min_score),
                slot.filter_pattern.as_ref(),
                None,
            )
        })
        .collect()
}

/// Generate an `OwnedGridConfig` representing a grid with specified entries.
#[must_use]
pub fn generate_grid_config<'a>(
    mut word_list: WordList,
    entries: &'a [SlotSpec],
    raw_fill: &'a [Option<String>],
    width: usize,
    height: usize,
    min_score: u16,
) -> OwnedGridConfig {
    let (slot_configs, crossing_count) = generate_slot_configs(entries);

    let fill: Vec<Option<GlyphId>> = raw_fill
        .iter()
        .map(|cell_str| {
            cell_str
                .as_ref()
                .map(|cell_str| word_list.glyph_id_for_char(cell_str.chars().next().unwrap()))
        })
        .collect();

    let mut slot_options =
        generate_all_slot_options(&mut word_list, &fill, &slot_configs, width, min_score);

    sort_slot_options(&word_list, &slot_configs, &mut slot_options);

    let slot_options_by_glyph =
        build_slot_options_by_glyph(&word_list, &slot_configs, &slot_options);

    OwnedGridConfig {
        word_list,
        fill,
        slot_configs,
        slot_options,
        slot_options_by_glyph,
        width,
        height,
        crossing_count,
        abort: None,
    }
}

/// Generate a list of `SlotSpec`s from a template string with . representing empty cells, # representing
/// blocks, and letters representing themselves.
#[allow(dead_code)]
#[must_use]
pub fn generate_slots_from_template_string(template: &str) -> Vec<SlotSpec> {
    fn build_words(template: &[Vec<char>]) -> Vec<Vec<GridCoord>> {
        let mut result: Vec<Vec<GridCoord>> = vec![];

        for (y, line) in template.iter().enumerate() {
            let mut current_word_coords: Vec<GridCoord> = vec![];

            for (x, &cell) in line.iter().enumerate() {
                if cell == '#' {
                    if current_word_coords.len() > 1 {
                        result.push(current_word_coords);
                    }
                    current_word_coords = vec![];
                } else {
                    current_word_coords.push((x, y));
                }
            }

            if current_word_coords.len() > 1 {
                result.push(current_word_coords);
            }
        }

        result
    }

    let template: Vec<Vec<char>> = template
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                Some(line.chars().collect())
            }
        })
        .collect();

    let mut slot_specs: Vec<SlotSpec> = vec![];

    for coords in build_words(&template) {
        slot_specs.push(SlotSpec {
            start_cell: coords[0],
            length: coords.len(),
            direction: Direction::Across,
        });
    }

    let transposed_template: Vec<Vec<char>> = (0..template[0].len())
        .map(|y| (0..template.len()).map(|x| template[x][y]).collect())
        .collect();

    for coords in build_words(&transposed_template) {
        let coords: Vec<GridCoord> = coords.iter().copied().map(|(y, x)| (x, y)).collect();
        slot_specs.push(SlotSpec {
            start_cell: coords[0],
            length: coords.len(),
            direction: Direction::Down,
        });
    }

    slot_specs
}

/// Generate an `OwnedGridConfig` from a template string with . representing empty cells, # representing
/// blocks, and letters representing themselves.
#[allow(dead_code)]
#[must_use]
pub fn generate_grid_config_from_template_string(
    word_list: WordList,
    template: &str,
    min_score: u16,
) -> OwnedGridConfig {
    let slot_specs = generate_slots_from_template_string(template);

    let fill: Vec<Vec<Option<String>>> = template
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                Some(
                    line.chars()
                        .map(|c| {
                            if c == '.' || c == '#' {
                                None
                            } else {
                                Some(c.to_lowercase().to_string())
                            }
                        })
                        .collect(),
                )
            }
        })
        .collect();

    let width = fill[0].len();
    let height = fill.len();

    generate_grid_config(
        word_list,
        &slot_specs,
        &fill.into_iter().flatten().collect::<Vec<_>>(),
        width,
        height,
        min_score,
    )
}

/// The parsed representation of a custom slots string definition.
#[derive(Debug, Clone)]
pub struct ParsedSlots {
    pub slot_configs: Vec<SlotConfig>,
    pub crossing_count: usize,
    pub cell_names: Vec<String>,
    pub cell_values: HashMap<String, char>,
}

/// Parse a custom slots string definition.
/// Supports comments starting with `#`.
#[allow(dead_code)]
#[must_use]
pub fn parse_slots_string(slots_string: &str) -> ParsedSlots {
    let clean_slots_string = slots_string
        .lines()
        .map(|line| {
            if let Some(pos) = line.find('#') {
                &line[..pos]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let lines: Vec<&str> = if clean_slots_string.contains(';') {
        clean_slots_string.split(';').collect()
    } else {
        clean_slots_string.lines().collect()
    };

    let mut slots_cell_names: Vec<Vec<String>> = vec![];
    let mut cell_values: HashMap<String, char> = HashMap::new();
    for line in lines {
        let mut names: Vec<String> = line.split_whitespace()
            .map(std::string::ToString::to_string)
            .filter(|s| !s.is_empty() && s != ";")
            .collect();
        if !names.is_empty() {
            let mut prefilled_word = None;
            if let Some(last_name) = names.last() {
                if last_name.starts_with('=') {
                    prefilled_word = Some(last_name[1..].to_string());
                }
            }
            if let Some(word) = prefilled_word {
                names.pop(); // Remove the "=WORD" token from the cell names
                for (idx, c) in word.chars().enumerate() {
                    if idx < names.len() && c != '.' && c != '?' {
                        cell_values.insert(names[idx].clone(), c);
                    }
                }
            }
            if !names.is_empty() {
                slots_cell_names.push(names);
            }
        }
    }

    let mut cell_names = vec![];
    for slot in &slots_cell_names {
        for cell in slot {
            if !cell_names.contains(cell) {
                cell_names.push(cell.clone());
            }
        }
    }

    let mut cell_occurrences: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (slot_id, slot) in slots_cell_names.iter().enumerate() {
        for (cell_idx, cell) in slot.iter().enumerate() {
            cell_occurrences.entry(cell.clone()).or_default().push((slot_id, cell_idx));
        }
    }

    let mut slot_configs: Vec<SlotConfig> = vec![];
    let mut crossings_by_slot: Vec<Vec<Vec<Crossing>>> = vec![vec![]; slots_cell_names.len()];

    for (slot_id, slot) in slots_cell_names.iter().enumerate() {
        crossings_by_slot[slot_id] = vec![vec![]; slot.len()];
    }

    let mut crossing_id_counter = 0;

    for occurrences in cell_occurrences.values() {
        if occurrences.len() > 1 {
            for i in 0..occurrences.len() {
                for j in (i + 1)..occurrences.len() {
                    let (s1, idx1) = occurrences[i];
                    let (s2, idx2) = occurrences[j];
                    let crossing_id = crossing_id_counter;
                    crossing_id_counter += 1;

                    crossings_by_slot[s1][idx1].push(Crossing {
                        other_slot_id: s2,
                        other_slot_cell: idx2,
                        crossing_id,
                    });

                    crossings_by_slot[s2][idx2].push(Crossing {
                        other_slot_id: s1,
                        other_slot_cell: idx1,
                        crossing_id,
                    });
                }
            }
        }
    }

    for (slot_id, slot) in slots_cell_names.iter().enumerate() {
        let cell_indices: Vec<usize> = slot.iter().map(|name| {
            cell_names.iter().position(|c| c == name).unwrap()
        }).collect();

        slot_configs.push(SlotConfig {
            id: slot_id,
            start_cell: (0, 0),
            direction: Direction::Across,
            length: slot.len(),
            crossings: crossings_by_slot[slot_id].clone(),
            min_score_override: None,
            filter_pattern: None,
            cell_indices: Some(cell_indices),
        });
    }

    ParsedSlots {
        slot_configs,
        crossing_count: crossing_id_counter,
        cell_names,
        cell_values,
    }
}

/// Generate an `OwnedGridConfig` from a slots definition string (space-delimited cells, semicolon-delimited slots).
#[allow(dead_code)]
#[must_use]
pub fn generate_grid_config_from_slots_string(
    mut word_list: WordList,
    slots_string: &str,
    min_score: u16,
) -> OwnedGridConfig {
    let parsed = parse_slots_string(slots_string);
    let mut fill = vec![None; parsed.cell_names.len()];

    for (cell_name, &ch) in &parsed.cell_values {
        if let Some(cell_idx) = parsed.cell_names.iter().position(|c| c == cell_name) {
            fill[cell_idx] = Some(word_list.glyph_id_for_char(ch.to_lowercase().next().unwrap()));
        }
    }

    let mut slot_options = generate_all_slot_options(
        &mut word_list,
        &fill,
        &parsed.slot_configs,
        1,
        min_score,
    );

    sort_slot_options(&word_list, &parsed.slot_configs, &mut slot_options);

    let slot_options_by_glyph =
        build_slot_options_by_glyph(&word_list, &parsed.slot_configs, &slot_options);

    OwnedGridConfig {
        word_list,
        fill,
        slot_configs: parsed.slot_configs,
        slot_options,
        slot_options_by_glyph,
        width: 1,
        height: 1,
        crossing_count: parsed.crossing_count,
        abort: None,
    }
}

/// A struct recording a slot assignment made during a fill process.
#[derive(Debug, Clone)]
pub struct Choice {
    pub slot_id: SlotId,
    pub word_id: WordId,
}

/// Turn the given grid config and fill choices into a rendered string.
#[allow(dead_code)]
#[must_use]
pub fn render_grid(config: &GridConfig, choices: &[Choice]) -> String {
    let mut grid: Vec<Option<char>> = config
        .fill
        .iter()
        .map(|&cell| cell.map(|glyph_id| config.word_list.glyphs[glyph_id]))
        .collect();

    for &Choice { slot_id, word_id } in choices {
        let slot_config = &config.slot_configs[slot_id];
        let word = &config.word_list.words[slot_config.length][word_id];

        for (cell_idx, &glyph) in word.glyphs.iter().enumerate() {
            let (x, y) = match slot_config.direction {
                Direction::Across => (
                    slot_config.start_cell.0 + cell_idx,
                    slot_config.start_cell.1,
                ),
                Direction::Down => (
                    slot_config.start_cell.0,
                    slot_config.start_cell.1 + cell_idx,
                ),
            };

            grid[y * config.width + x] = Some(config.word_list.glyphs[glyph]);
        }
    }

    grid.chunks(config.width)
        .map(|line| {
            line.iter()
                .map(|cell| cell.unwrap_or('.').to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use crate::grid_config::{Direction, SlotSpec};

    #[test]
    fn test_slot_spec_serialization() {
        let slot_spec = SlotSpec {
            start_cell: (1, 2),
            direction: Direction::Across,
            length: 5,
        };

        let slot_key = serde_json::to_string(&slot_spec).unwrap();

        assert_eq!(slot_key, "\"1,2,across,5\"");
    }

    #[test]
    fn test_slot_spec_deserialization() {
        let slot_spec: SlotSpec = serde_json::from_str("\"3,4,down,12\"").unwrap();

        assert_eq!(
            slot_spec,
            SlotSpec {
                start_cell: (3, 4),
                direction: Direction::Down,
                length: 12,
            }
        );
    }
}

#[cfg(test)]
mod custom_topology_tests {
    use crate::grid_config::generate_grid_config_from_slots_string;
    use crate::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
    use crate::backtracking_search::find_fill;

    #[test]
    fn test_custom_hexagonal_fill() {
        let slots_input = "
            a b c ;
            e f g h ;
            j k l m n ;
            q r s t ;
            x y z ;
            a e j ;
            b f k q ;
            c g l r x ;
            h m s y ;
            n t z ;
            c h n ;
            b g m t ;
            a f l s z ;
            e k r y ;
            j q x ;
        ";

        let words = vec![
            ("one".to_string(), 100),
            ("baas".to_string(), 100),
            ("sorns".to_string(), 100),
            ("idea".to_string(), 100),
            ("sed".to_string(), 100),
            ("obs".to_string(), 100),
            ("naoi".to_string(), 100),
            ("eards".to_string(), 100),
            ("snee".to_string(), 100),
            ("sad".to_string(), 100),
            ("ess".to_string(), 100),
            ("nana".to_string(), 100),
            ("oared".to_string(), 100),
            ("bode".to_string(), 100),
            ("sis".to_string(), 100),
        ];

        let word_list = WordList::new(
            vec![WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::Memory { words },
                normalization: None,
            }],
            None,
            Some(5),
            None,
        );

        let owned_config = generate_grid_config_from_slots_string(word_list, slots_input, 0);

        let result = find_fill(&owned_config.to_config_ref(), None, None)
            .expect("Failed to solve hexagonal crossword puzzle");

        let solved_words: Vec<String> = result.choices.iter().map(|choice| {
            let slot_len = owned_config.slot_configs[choice.slot_id].length;
            owned_config.word_list.words[slot_len][choice.word_id].normalized_string.clone()
        }).collect();

        assert_eq!(solved_words.len(), 15);
    }

    #[test]
    fn test_custom_slots_comments() {
        let slots_input = "
            # This is a comment line
            a b c ; # Inline comment
            # Entirely commented line
            d e f ; # Another comment style
        ";

        let words = vec![
            ("abc".to_string(), 100),
            ("def".to_string(), 100),
        ];

        let word_list = WordList::new(
            vec![WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::Memory { words },
                normalization: None,
            }],
            None,
            Some(3),
            None,
        );

        let owned_config = generate_grid_config_from_slots_string(word_list, slots_input, 0);
        assert_eq!(owned_config.slot_configs.len(), 2);
        assert_eq!(owned_config.slot_configs[0].length, 3);
        assert_eq!(owned_config.slot_configs[1].length, 3);
    }

    #[test]
    fn test_custom_slots_prefilled() {
        let slots_input = "
            a b c d =MARK ;
            c b a ;
        ";

        let words = vec![
            ("mark".to_string(), 100),
            ("kef".to_string(), 100),
            ("ram".to_string(), 100),
            ("spa".to_string(), 100),
        ];

        let word_list = WordList::new(
            vec![WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::Memory { words },
                normalization: None,
            }],
            None,
            Some(4),
            None,
        );

        let owned_config = generate_grid_config_from_slots_string(word_list, slots_input, 0);

        assert_eq!(owned_config.slot_configs[0].length, 4);

        let a_idx = owned_config.slot_configs[0].cell_indices.as_ref().unwrap()[0];
        let d_idx = owned_config.slot_configs[0].cell_indices.as_ref().unwrap()[3];

        let word_list_ref = &owned_config.word_list;
        assert_eq!(owned_config.fill[a_idx], Some(word_list_ref.glyph_id_by_char.get(&'m').copied().unwrap()));
        assert_eq!(owned_config.fill[d_idx], Some(word_list_ref.glyph_id_by_char.get(&'k').copied().unwrap()));
    }

    #[test]
    fn test_backwards_slots_with_prefill_non_word() {
        let slots_input = "
            a b c d e f g h =SCARCEST ;
            h g f ;
            e d c b a ;
        ";

        let words = vec![
            ("scarcest".to_string(), 100),
            ("tse".to_string(), 100),
            ("crane".to_string(), 100),
            ("clean".to_string(), 100),
            // Note: "cracs" is NOT in words!
        ];

        let word_list = WordList::new(
            vec![WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::Memory { words },
                normalization: None,
            }],
            None,
            Some(8),
            None,
        );

        let owned_config = generate_grid_config_from_slots_string(word_list, slots_input, 0);

        // Since slot 2 ("e d c b a") requires "cracs" which is not in the dictionary,
        // and slot 2 was NOT explicitly prefilled with "=CRACS", it should have 0 options
        // and find_fill should fail!
        let result = find_fill(&owned_config.to_config_ref(), None, None);
        assert!(
            result.is_err(),
            "Expected grid to be unfillable because 'cracs' is not in the dictionary"
        );
    }
}
