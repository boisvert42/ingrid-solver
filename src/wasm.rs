use wasm_bindgen::prelude::*;
use crate::grid_config::{Direction, SlotConfig, Crossing, GridConfig};
use crate::types::GlyphId;
use crate::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
use crate::arc_consistency::{establish_arc_consistency_for_static_grid, EliminationSet};
use std::collections::HashMap;

#[wasm_bindgen]
pub struct WasmSolver {
    word_list: Option<WordList>,
    slot_configs: Vec<SlotConfig>,
    crossing_count: usize,
    cell_names: Vec<String>,
    min_score: u16,
}

#[wasm_bindgen]
impl WasmSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(slots_string: &str) -> Result<WasmSolver, JsValue> {
        let lines: Vec<&str> = if slots_string.contains(';') {
            slots_string.split(';').collect()
        } else {
            slots_string.lines().collect()
        };
        
        let mut slots_cell_names: Vec<Vec<String>> = vec![];
        for line in lines {
            let names: Vec<String> = line.split_whitespace()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty() && s != ";")
                .collect();
            if !names.is_empty() {
                slots_cell_names.push(names);
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
                cell_occurrences.entry(cell.clone()).or_insert_with(Vec::new).push((slot_id, cell_idx));
            }
        }
        
        let mut slot_configs: Vec<SlotConfig> = vec![];
        let mut crossings_by_slot: Vec<Vec<Vec<Crossing>>> = vec![vec![]; slots_cell_names.len()];
        
        for (slot_id, slot) in slots_cell_names.iter().enumerate() {
            crossings_by_slot[slot_id] = vec![vec![]; slot.len()];
        }
        
        let mut crossing_id_counter = 0;
        
        for (_cell_name, occurrences) in &cell_occurrences {
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
        
        Ok(WasmSolver {
            word_list: None,
            slot_configs,
            crossing_count: crossing_id_counter,
            cell_names,
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

    pub fn get_cell_names(&self) -> String {
        serde_json::to_string(&self.cell_names).unwrap_or_else(|_| "[]".to_string())
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
        
        let config = GridConfig {
            word_list,
            fill: &fill,
            slot_configs: &self.slot_configs,
            slot_options: &slot_options,
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
}
