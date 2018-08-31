use indexes::{Indexes, WithIndexes};
use std::fs::read_to_string;
use std::io::{Cursor, Read, Result as IOResult, Seek, SeekFrom};

pub struct TextFile {
	file: Cursor<String>,
}

impl TextFile {
	pub fn new(contents: String) -> Self {
		return Self {
			file: Cursor::new(contents),
		};
	}

	pub fn from_path(path: &str) -> Self {
		let c = read_to_string(path).unwrap();
		return Self {
			file: Cursor::new(c),
		};
	}
}

impl Read for TextFile {
	fn read(&mut self, mut buffer: &mut [u8]) -> IOResult<usize> {
		return self.file.read(&mut buffer);
	}
}

impl Seek for TextFile {
	fn seek(&mut self, from: SeekFrom) -> IOResult<u64> {
		return self.file.seek(from);
	}
}

impl WithIndexes for TextFile {
	fn get_indexes(&mut self) -> Result<Indexes, String> {
		let mut ind = Indexes::new();
		let mut read = 0;
		for (index, line) in self.file.get_mut().lines().enumerate() {
			let size = line.len() + 1;
			ind.insert(format!("line_{}", index), read, size as u64);
			read += size as u64;
		}

		return Ok(ind);
	}
}
