use clap::Parser;
use ingrid_core::backtracking_search::find_fill;
use ingrid_core::grid_config::{generate_grid_config_from_template_string, render_grid};
use ingrid_core::word_list::{WordList, WordListSourceConfig, WordListSourceConfigProvider};
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

const STWL_RAW: &str = include_str!("../resources/spreadthewordlist.dict");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the grid file, as ASCII with # representing blocks and . representing empty squares
    grid_path: Option<String>,

    /// Path to a file containing custom slots in batch mode
    #[arg(short, long)]
    batch: Option<String>,

    /// Path to a scored wordlist file [default: (embedded copy of Spread the Wordlist)]
    #[arg(long)]
    wordlist: Option<String>,

    /// Minimum allowable word score
    #[arg(long, default_value_t = 50)]
    min_score: u16,

    /// Maximum shared substring length between entries [default: none]
    #[arg(long)]
    max_shared_substring: Option<usize>,

    /// Print timing information along with the grid
    #[arg(short, long, default_value_t = false)]
    time: bool,
}

struct Error(String);

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0) // Print error unquoted
    }
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    if args.grid_path.is_none() && args.batch.is_none() {
        return Err(Error("Must specify either a grid file path or a batch file path (-b)".into()));
    }
    if args.grid_path.is_some() && args.batch.is_some() {
        return Err(Error("Cannot specify both a grid file path and a batch file path (-b)".into()));
    }

    let mut max_side = 0;
    let mut raw_grid_content = String::new();
    let mut raw_batch_content = String::new();

    if let Some(grid_path) = &args.grid_path {
        raw_grid_content = fs::read_to_string(grid_path)
            .map_err(|_| Error(format!("Couldn't read file '{}'", grid_path)))?
            .trim()
            .lines()
            .map(|line| line.trim().to_lowercase().nfc().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        let height = raw_grid_content.lines().count();
        if height == 0 {
            return Err(Error("Grid must have at least one row".into()));
        }

        if raw_grid_content
            .lines()
            .map(|line| line.chars().count())
            .collect::<HashSet<_>>()
            .len()
            != 1
        {
            return Err(Error("Rows in grid must all be the same length".into()));
        }

        let width = raw_grid_content.lines().next().unwrap().chars().count() - 1;
        max_side = width.max(height);
    } else if let Some(batch_path) = &args.batch {
        raw_batch_content = fs::read_to_string(batch_path)
            .map_err(|_| Error(format!("Couldn't read batch file '{}'", batch_path)))?;

        let parsed = ingrid_core::grid_config::parse_slots_string(&raw_batch_content);
        max_side = parsed.slot_configs.iter().map(|s| s.length).max().unwrap_or(0);
    }

    if !args
        .max_shared_substring
        .map_or(true, |mss| (3..=10).contains(&mss))
    {
        return Err(Error(
            "If given, max shared substring must be between 3 and 10".into(),
        ));
    }

    let start = Instant::now();

    let word_list = WordList::new(
        vec![match &args.wordlist {
            Some(wordlist_path) => WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::File {
                    path: wordlist_path.into(),
                },
                normalization: None,
            },
            None => WordListSourceConfig {
                id: "0".into(),
                enabled: true,
                provider: WordListSourceConfigProvider::FileContents { contents: STWL_RAW },
                normalization: None,
            },
        }],
        None,
        Some(max_side),
        args.max_shared_substring,
    );

    let word_list_time = start.elapsed();

    #[allow(clippy::comparison_chain)]
    if let Some(errors) = word_list.get_source_errors().get("0") {
        if errors.len() == 1 {
            return Err(Error(format!("{}", errors[0])));
        } else if errors.len() > 1 {
            let mut full_error: String = "".into();
            for error in errors {
                full_error.push_str(&format!("\n- {error}"));
            }
            return Err(Error(full_error));
        }
    }

    if word_list.word_id_by_string.is_empty() {
        return Err(Error("Word list is empty".into()));
    }

    let grid_config = if args.batch.is_some() {
        ingrid_core::grid_config::generate_grid_config_from_slots_string(word_list, &raw_batch_content, args.min_score)
    } else {
        generate_grid_config_from_template_string(word_list, &raw_grid_content, args.min_score)
    };

    let result = find_fill(&grid_config.to_config_ref(), None, None)
        .map_err(|_| Error("Unfillable grid".into()))?;

    let fill_time = start.elapsed() - word_list_time;

    if args.batch.is_some() {
        for (slot_id, slot_config) in grid_config.slot_configs.iter().enumerate() {
            let choice = result.choices.iter().find(|c| c.slot_id == slot_id).unwrap();
            let word = &grid_config.word_list.words[slot_config.length][choice.word_id];
            println!("Slot {}: {}", slot_id + 1, word.normalized_string);
        }
    } else {
        println!(
            "{}",
            render_grid(&grid_config.to_config_ref(), &result.choices).replace('.', "#")
        );
    }

    if args.time {
        eprintln!("{word_list_time:?} loading word list, {fill_time:?} finding fill");
    }

    Ok(())
}
