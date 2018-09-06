//! Module with `Indexes` struct and `WithIndexes` trait

use std::io::{Read, Seek};
use std::path::PathBuf;

/// Indexes struct
///
/// Indexes represent binary file blocks (lines) and have similar to `HashMap` idea.
/// You have string `label`s followed by `start` of the block and `size` of the block.
///
/// Example text representation of `Indexes` can be as follows:
/// ```bash
/// header 0 16
/// data_length 16 2
/// data_item_1 18 16
/// data_item_2 34 16
/// misc_info_length 50 2
/// misc_info 52 8
/// ```
///
/// Text file representation, for example, is as simple as:
/// ```bash
/// line_1 0 10
/// line_2 10 10
/// line_3 20 5
/// line_4 25 10
/// ```
///
/// Diff algorithm uses this blocks to compute hashes and compare them one by one
///
/// Indexes implementation varies from format to format and is supposed to be implemented manually for each format
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Indexes {
	order: Box<Vec<(String, u64, u64)>>,
	pos: usize,
}

impl Indexes {
	/// Creates new `Indexes` instance
	pub fn new() -> Self {
		return Self {
			order: Box::new(vec![]),
			pos: 0,
		};
	}

	/// Inserts item to `Indexes`
	pub fn insert(&mut self, label: String, start: u64, size: u64) {
		if self.has(&label) {
			panic!("Attempt to put a dublicate");
		}
		self.order.push((label.clone(), start, size));
	}

	/// Chech if `Indexes` has label
	pub fn has(&self, label: &str) -> bool {
		return self
			.order
			.iter()
			.by_ref()
			.position(|(p, _, _)| p == &label)
			.is_some();
	}

	/// Returns `Indexes` item
	pub fn get(&self, label: &str) -> Option<(u64, u64)> {
		return self
			.order
			.iter()
			.by_ref()
			.find(|(p, _, _)| p == &label)
			.map(|(_, start, size)| (*start, *size));
	}

	/// Removes item from `Indexes` by label
	pub fn remove(&mut self, label: &str) -> bool {
		let index = self.order.iter().by_ref().position(|(l, _, _)| l == &label);
		if index.is_none() {
			return false;
		};
		let index = index.unwrap();
		self.order.remove(index);
		if index <= self.pos {
			self.pos -= 1;
		};
		return true;
	}

	/// Returns only ends of the `Indexes`
	///
	/// In compare with file system this function will return only files while omitting directories.
	/// Imagine we have `Indexes` like this
	/// ```bash
	/// header: 0 16
	/// header/signature 0 4
	/// header/version 4 4
	/// header/additinal_info 8 8
	/// data_section: 16 32
	/// data_section/item_1 16 16
	/// data_section/item_2 32 16
	/// ```
	///
	/// The given function will return ends as so
	/// ```bash
	/// header/signature 0 4
	/// header/version 4 4
	/// header/additinal_info 8 8
	/// data_section/item_1 16 16
	/// data_section/item_2 32 16
	/// ```
	pub fn get_ends(&mut self) -> Self {
		let mut out = Indexes::new();
		let mut set = vec![];
		{
			let labels = self.map(|x| x.0);
			for label in labels {
				let l = PathBuf::from(&label);
				let parent = l.parent();
				if parent.is_none() {
					set.push(label);
					continue;
				};
				let parent_string = parent.unwrap().to_str().unwrap().to_string();
				if &parent_string == "" {
					set.push(label);
					continue;
				};
				let parent_index = set.iter().by_ref().position(|x| x == &parent_string);
				match parent_index {
					Some(index) => {
						set.remove(index);
						set.push(label);
					}
					None => {
						set.push(label);
					}
				};
			}
		};
		for label in set {
			let item = self.get(&label).unwrap();
			out.insert(label, item.0, item.1);
		}
		return out;
	}
}

impl Iterator for Indexes {
	type Item = (String, u64, u64);

	fn next(&mut self) -> Option<Self::Item> {
		if self.pos >= self.order.len() {
			return None;
		};

		let item = &self.order[self.pos];
		self.pos += 1;
		return Some(item.clone());
	}
}

impl ExactSizeIterator for Indexes {
	fn len(&self) -> usize {
		return self.order.len();
	}
}

impl DoubleEndedIterator for Indexes {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.order.len() - self.pos == 0 {
			return None;
		};

		let item = &self.order[self.pos];
		self.pos -= 1;
		return Some(item.clone());
	}
}

/// Trait implies that structure that implements it can be diffed
pub trait WithIndexes: Read + Seek {
	fn get_indexes(&mut self) -> Result<Indexes, String>;
}

impl<'a, T: WithIndexes> WithIndexes for &'a mut T {
	fn get_indexes(&mut self) -> Result<Indexes, String> {
		return (**self).get_indexes();
	}
}

#[cfg(test)]
mod indexes_tests {
	use super::Indexes;

	#[test]
	fn iterator_test() {
		let mut ind = Indexes::new();
		ind.insert("line_1".to_string(), 0, 26);
		ind.insert("line_1/signature".to_string(), 0, 4);
		ind.insert("line_1/data".to_string(), 4, 22);
		ind.insert("line_1/data/part_a".to_string(), 4, 12);
		ind.insert("line_1/data/part_b".to_string(), 16, 6);
		ind.insert("text_data".to_string(), 26, 40);
		ind.insert("text_data/length".to_string(), 26, 2);
		ind.insert("text_data/data".to_string(), 28, 12);

		let mut len = 0;
		for _ in ind {
			len += 1;
		}

		assert_eq!(len, 8);
	}

	#[test]
	fn get_non_nested_test() {
		let mut ind = Indexes::new();
		ind.insert("line_1".to_string(), 0, 26);
		ind.insert("line_1/signature".to_string(), 0, 4);
		ind.insert("line_1/data".to_string(), 4, 22);
		ind.insert("line_1/data/part_a".to_string(), 4, 12);
		ind.insert("line_1/data/part_b".to_string(), 16, 10);
		ind.insert("text_data".to_string(), 26, 40);
		ind.insert("text_data/length".to_string(), 26, 2);
		ind.insert("text_data/data".to_string(), 28, 12);

		let mut model = Indexes::new();
		model.insert("line_1/signature".to_string(), 0, 4);
		model.insert("line_1/data/part_a".to_string(), 4, 12);
		model.insert("line_1/data/part_b".to_string(), 16, 10);
		model.insert("text_data/length".to_string(), 26, 2);
		model.insert("text_data/data".to_string(), 28, 12);

		let ends = ind.get_ends();

		assert_eq!(ends, model);
	}
}
