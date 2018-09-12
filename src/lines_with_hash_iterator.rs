//! Contains `LinesWithHashIterator`

use functions::compute_hash;
use indexes::{Indexes, WithIndexes};
use std::io::{Read, SeekFrom};

/// Yields indexes with appended hashes
pub struct LinesWithHashIterator<T: WithIndexes> {
	file: T,
	indexes: Indexes,
	pos: usize,
}

impl<T: WithIndexes> LinesWithHashIterator<T> {
	pub fn new(mut file: T) -> Result<Self, String> {
		let indexes = file.get_indexes()?.get_ends();
		return Ok(Self {
			file: file,
			indexes: indexes,
			pos: 0,
		});
	}

	pub fn get_read(self) -> T {
		return self.file;
	}
}

impl<T: WithIndexes> Iterator for LinesWithHashIterator<T> {
	type Item = (String, u64, u64, String);

	fn next(&mut self) -> Option<Self::Item> {
		if self.pos >= self.indexes.len() {
			return None;
		};
		let item = self.indexes.next().unwrap();
		self.file.seek(SeekFrom::Start(item.1)).unwrap();
		let hash = {
			let mut sl = &mut self.file.by_ref().take(item.2);
			compute_hash(&mut sl)
		};

		self.pos += 1;
		return Some((item.0, item.1, item.2, hash));
	}
}
