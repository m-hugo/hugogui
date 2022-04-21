use std::fs::File;
use std::path::Path;
mod predictionentry;
use predictionentry::PredictionEntry;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::BufRead;
use std::io::BufReader;
fn generate_ixs(entries: &[PredictionEntry]) -> HashMap<char, u32> {
	let mut ixs: HashMap<char, u32> = HashMap::new();

	// There will be no newlines in the input. This is a placeholder
	// until the first time the loop runs.
	let mut last_c = '\n';

	for (ix, entry) in entries.iter().enumerate() {
		let c = entry.word.chars().nth(0).unwrap();
		if c != last_c {
			ixs.insert(c, ix.try_into().unwrap());
			last_c = c;
		}
	}

	ixs
}

#[derive(Debug)]
pub struct SimpleWordPredictor {
	entries: Vec<PredictionEntry>,
	ixs: HashMap<char, u32>,
}

impl SimpleWordPredictor {
	/// Given an initial input string, return 10 predictions.
	pub fn predict(&self, input: &str) -> Vec<PredictionEntry> {
		let mut predictions: Vec<PredictionEntry> = Vec::new();
		let iter = self.entries.iter();
		let first_letter = input.chars().nth(0).unwrap();
		let skip_n = self.ixs.get(&first_letter);

		match skip_n {
			Some(n) => {
				let iter = iter.skip(*n as usize);
				for entry in iter {
					let word = &entry.word;
					if word.chars().nth(0).unwrap() != first_letter {
						break;
					}

					if word.starts_with(input) {
						predictions.push(entry.clone());
					}
				}

				predictions.sort_by(|a, b| b.score.cmp(&a.score));

				predictions.truncate(10);

				predictions
			}
			None => predictions,
		}
	}

	/// Load training data from a CSV file.
	pub fn from_file(path: &Path) -> SimpleWordPredictor {
		let mut entries = Vec::new();
		let file = BufReader::new(File::open(path).unwrap());
		for line_res in file.lines() {
			let line = line_res.unwrap();
			let line = line.trim();
			let str_entry: Vec<&str> = line.split(',').collect();
			let word: String = str_entry[0].to_string();
			let n = str_entry[1].parse().ok().unwrap();
			entries.push(PredictionEntry { word, score: n });
		}

		let ixs = generate_ixs(&entries);

		SimpleWordPredictor { entries, ixs }
	}
}
