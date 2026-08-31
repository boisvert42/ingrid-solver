use wasm_bindgen::prelude::*;
use crate::grid_config::{SlotConfig, GridConfig};
use crate::types::GlyphId;
use crate::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
use crate::arc_consistency::{establish_arc_consistency_for_static_grid, EliminationSet};
use crate::backtracking_search::find_fill;
use std::collections::HashMap;

use serde_derive::Serialize;

#[derive(Serialize)]
struct JsSlotConfig {
    id: usize,
    cells: Vec<String>,
    length: usize,
}

#[wasm_bindgen]
pub struct WasmSolver {
    word_list: Option<WordList>,
    slot_configs: Vec<SlotConfig>,
    crossing_count: usize,
    cell_names: Vec<String>,
    cell_values: HashMap<String, char>,
    min_score: u16,
}

#[wasm_bindgen]
impl WasmSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(slots_string: &str) -> Result<WasmSolver, JsValue> {
        let parsed = crate::grid_config::parse_slots_string(slots_string);
        
        Ok(WasmSolver {
            word_list: None,
            slot_configs: parsed.slot_configs,
            crossing_count: parsed.crossing_count,
            cell_names: parsed.cell_names,
            cell_values: parsed.cell_values,
            min_score: 50,
        })
    }

    pub fn load_dictionary(&mut self, dict_contents: &str) -> Result<(), JsValue> {
        let max_length = self.slot_configs.iter().map(|s| s.length).max().unwrap_or(0);
        let mut words = vec![];
        for line in dict_contents.lines() {
            let line_parts: Vec<&str> = line.split(';').collect();
            let canonical = line_parts[0].trim().to_string();
            if canonical.is_empty() {
                continue;
            }
            let score = if line_parts.len() < 2 {
                50
            } else {
                line_parts[1].trim().parse::<u16>().unwrap_or(50)
            };
            words.push((canonical, score));
        }

        let word_list = WordList::new(
            vec![WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::Memory { words },
                normalization: None,
            }],
            None,
            Some(max_length),
            None,
        );
        self.word_list = Some(word_list);
        Ok(())
    }

    #[must_use] 
    pub fn get_cell_names(&self) -> String {
        serde_json::to_string(&self.cell_names).unwrap_or_else(|_| "[]".to_string())
    }

    #[must_use]
    pub fn get_slot_configs(&self) -> String {
        let serialized_slots: Vec<JsSlotConfig> = self.slot_configs.iter().map(|slot| {
            let cells = slot.cell_indices.as_ref().map(|idxs| {
                idxs.iter().map(|&idx| self.cell_names[idx].clone()).collect()
            }).unwrap_or_else(Vec::new);
            JsSlotConfig {
                id: slot.id,
                cells,
                length: slot.length,
            }
        }).collect();
        serde_json::to_string(&serialized_slots).unwrap_or_else(|_| "[]".to_string())
    }

    #[must_use]
    pub fn get_prefills(&self) -> String {
        serde_json::to_string(&self.cell_values).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn set_min_score(&mut self, min_score: u16) {
        self.min_score = min_score;
    }

    pub fn run_ac3(&mut self, fill_json: &str) -> Result<String, JsValue> {
        let word_list = self.word_list.as_mut().ok_or_else(|| JsValue::from_str("Dictionary not loaded"))?;
        
        let fill_map: HashMap<String, String> = serde_json::from_str(fill_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
            
        let mut fill: Vec<Option<GlyphId>> = vec![None; self.cell_names.len()];
        for (i, cell_name) in self.cell_names.iter().enumerate() {
            if let Some(letter) = fill_map.get(cell_name) {
                if let Some(c) = letter.chars().next() {
                    fill[i] = Some(word_list.glyph_id_for_char(c));
                }
            }
        }
        
        let mut slot_options = crate::grid_config::generate_all_slot_options(
            word_list,
            &fill,
            &self.slot_configs,
            1, // width (dummy)
            self.min_score,
        );
        
        crate::grid_config::sort_slot_options(word_list, &self.slot_configs, &mut slot_options);
        let slot_options_by_glyph = crate::grid_config::build_slot_options_by_glyph(
            word_list,
            &self.slot_configs,
            &slot_options,
        );
        
        let config = GridConfig {
            word_list,
            fill: &fill,
            slot_configs: &self.slot_configs,
            slot_options: &slot_options,
            slot_options_by_glyph: &slot_options_by_glyph,
            width: 1,
            height: 1,
            crossing_count: self.crossing_count,
            abort: None,
        };
        
        let mut elimination_sets = EliminationSet::build_all(&self.slot_configs, word_list);
        
        let _ = establish_arc_consistency_for_static_grid(&config, &mut elimination_sets);
        
        let mut results: Vec<Vec<String>> = vec![];
        for (slot_id, slot_config) in self.slot_configs.iter().enumerate() {
            let mut valid_words = vec![];
            for &word_id in &slot_options[slot_id] {
                if !elimination_sets[slot_id].contains(word_id) {
                    let word = &word_list.words[slot_config.length][word_id];
                    valid_words.push(word.normalized_string.clone());
                }
            }
            results.push(valid_words);
        }
        
        serde_json::to_string(&results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn validate_candidate(&mut self, slot_id: usize, word: &str, fill_json: &str) -> bool {
        let word_list = match &mut self.word_list {
            Some(wl) => wl,
            None => return false,
        };
        
        let fill_map: HashMap<String, String> = match serde_json::from_str(fill_json) {
            Ok(m) => m,
            Err(_) => return false,
        };
            
        let mut fill: Vec<Option<GlyphId>> = vec![None; self.cell_names.len()];
        for (i, cell_name) in self.cell_names.iter().enumerate() {
            if let Some(letter) = fill_map.get(cell_name) {
                if let Some(c) = letter.chars().next() {
                    fill[i] = Some(word_list.glyph_id_for_char(c));
                }
            }
        }
        
        let slot_config = &self.slot_configs[slot_id];
        if slot_config.length != word.chars().count() {
            return false;
        }
        
        for (char_idx, c) in word.chars().enumerate() {
            if let Some(cell_idx) = slot_config.cell_indices.as_ref().map(|idx| idx[char_idx]) {
                fill[cell_idx] = Some(word_list.glyph_id_for_char(c));
            }
        }
        
        let mut slot_options = crate::grid_config::generate_all_slot_options(
            word_list,
            &fill,
            &self.slot_configs,
            1,
            self.min_score,
        );
        
        crate::grid_config::sort_slot_options(word_list, &self.slot_configs, &mut slot_options);
        let slot_options_by_glyph = crate::grid_config::build_slot_options_by_glyph(
            word_list,
            &self.slot_configs,
            &slot_options,
        );
        
        let config = GridConfig {
            word_list,
            fill: &fill,
            slot_configs: &self.slot_configs,
            slot_options: &slot_options,
            slot_options_by_glyph: &slot_options_by_glyph,
            width: 1,
            height: 1,
            crossing_count: self.crossing_count,
            abort: None,
        };
        
        find_fill(&config, None, None).is_ok()
    }
}
