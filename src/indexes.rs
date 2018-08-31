use std::io::{Read, Seek};
use std::path::PathBuf;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Indexes {
	order: Box<Vec<(String, u64, u64)>>,
	pos: usize,
}

impl Indexes {
	pub fn new() -> Self {
		return Self {
			order: Box::new(vec![]),
			pos: 0,
		};
	}

	pub fn insert(&mut self, label: String, start: u64, size: u64) {
		if self.has(&label) {
			panic!("Attempt to put a dublicate");
		}
		self.order.push((label.clone(), start, size));
	}

	pub fn has(&self, label: &str) -> bool {
		return self
			.order
			.iter()
			.by_ref()
			.position(|(p, _, _)| p == &label)
			.is_some();
	}

	pub fn get(&self, label: &str) -> Option<(u64, u64)> {
		return self
			.order
			.iter()
			.by_ref()
			.find(|(p, _, _)| p == &label)
			.map(|(_, start, size)| (*start, *size));
	}

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

	pub fn get_ends(&mut self) -> Indexes {
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

pub trait WithIndexes: Read + Seek {
	fn get_indexes(&mut self) -> Result<Indexes, String>;
}

impl<'a, T: WithIndexes> WithIndexes for &'a mut T {
	fn get_indexes(&mut self) -> Result<Indexes, String> {
		return WithIndexes::get_indexes(*self);
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
