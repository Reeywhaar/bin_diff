use diff_block::{DiffBlock, DiffBlockN};
use difference::{Changeset, Difference};
use indexes::WithIndexes;
use lines_with_hash_iterator::LinesWithHashIterator;
use readslice::ReadSlice;
use std::io::SeekFrom;

pub struct DiffIterator<T: WithIndexes> {
	file: T,
	diff: Vec<DiffBlockN<u32>>,
	pos: usize,
	file_pos: u64,
}

impl<T: WithIndexes> DiffIterator<T> {
	pub fn new<U: WithIndexes>(file_a: U, file_b: T) -> Result<Self, String> {
		let (_file_a, ind_a) = {
			let mut it = LinesWithHashIterator::new(file_a)?;
			let ind: Vec<_> = it.by_ref().collect();
			let r = it.get_read();
			(r, ind.clone())
		};
		let (file_b, ind_b) = {
			let mut it = LinesWithHashIterator::new(file_b)?;
			let ind: Vec<_> = it.by_ref().collect();
			let r = it.get_read();
			(r, ind.clone())
		};

		let ind_a_h = (&ind_a)
			.into_iter()
			.by_ref()
			.map(|x| x.3.clone())
			.collect::<Vec<String>>()
			.join("\n");
		let ind_b_h = (&ind_b)
			.into_iter()
			.by_ref()
			.map(|x| x.3.clone())
			.collect::<Vec<String>>()
			.join("\n");

		let diffs = {
			let changeset = Changeset::new(&ind_a_h, &ind_b_h, "\n");
			changeset.diffs
		};

		let diffs = Self::process_diff(&diffs);
		let diffs = Self::process_diff_2(&diffs, &ind_a, &ind_b);

		return Ok(Self {
			file: file_b,
			diff: diffs,
			pos: 0,
			file_pos: 0,
		});
	}

	fn process_diff(diffs: &Vec<Difference>) -> Vec<DiffBlockN<usize>> {
		let mut o: Vec<DiffBlockN<usize>> = vec![DiffBlockN::Skip(0)];

		for d in diffs {
			match d {
				Difference::Same(x) => {
					let blocks_n = x.matches("\n").count() + 1;
					let last_item = o[o.len() - 1].clone();
					if let DiffBlockN::Skip(n) = last_item {
						let last_index = o.len() - 1;
						o[last_index] = DiffBlockN::Skip(n + blocks_n);
					} else {
						o.push(DiffBlockN::Skip(blocks_n));
					};
				}
				Difference::Rem(x) => {
					o.push(DiffBlockN::Remove(x.matches("\n").count() + 1));
				}
				Difference::Add(x) => {
					let blocks_n = x.matches("\n").count() + 1;
					let last_item = o[o.len() - 1].clone();
					if let DiffBlockN::Remove(n) = last_item {
						let last_index = o.len() - 1;
						o[last_index] = DiffBlockN::Replace(n, blocks_n);
					} else {
						o.push(DiffBlockN::Add(blocks_n));
					};
				}
			}
		}

		return o;
	}

	fn process_diff_2(
		diffs: &Vec<DiffBlockN<usize>>,
		indexes_a: &Vec<(String, u64, u64, String)>,
		indexes_b: &Vec<(String, u64, u64, String)>,
	) -> Vec<DiffBlockN<u32>> {
		let mut o: Vec<DiffBlockN<u32>> = vec![];
		let mut i_a = indexes_a.into_iter().map(|x| x.2 as u32);
		let mut i_b = indexes_b.into_iter().map(|x| x.2 as u32);

		for item in diffs {
			match item {
				DiffBlockN::Skip(n) => {
					let size = (&mut i_a).by_ref().take(*n).fold(0, |acc, x| acc + x);
					let _: Vec<_> = (&mut i_b).by_ref().take(*n).collect();
					if size != 0 {
						o.push(DiffBlockN::Skip(size));
					}
				}
				DiffBlockN::Add(n) => {
					let size = (&mut i_b).by_ref().take(*n).fold(0, |acc, x| acc + x);
					if size != 0 {
						o.push(DiffBlockN::Add(size));
					}
				}
				DiffBlockN::Remove(n) => {
					let size = (&mut i_a).by_ref().take(*n).fold(0, |acc, x| acc + x);
					if size != 0 {
						o.push(DiffBlockN::Remove(size));
					}
				}
				DiffBlockN::Replace(r, a) => {
					let remove = (&mut i_a).by_ref().take(*r).fold(0, |acc, x| acc + x);
					let add = (&mut i_b).by_ref().take(*a).fold(0, |acc, x| acc + x);
					if remove != 0 && add != 0 {
						if add == remove {
							o.push(DiffBlockN::ReplaceWithSameLength(add));
						} else {
							o.push(DiffBlockN::Replace(remove, add));
						}
					} else if remove != 0 {
						o.push(DiffBlockN::Remove(remove));
					} else if add != 0 {
						o.push(DiffBlockN::Add(add));
					}
				}
				_ => panic!("Strange situation when process_diff returns unidentifiable block"),
			}
		}

		return o;
	}

	pub fn next_ref(&mut self) -> Option<Result<DiffBlock<u32>, String>> {
		if self.pos >= self.diff.len() {
			return None;
		};

		let item = &self.diff[self.pos];
		self.pos += 1;

		match item {
			DiffBlockN::Skip(size) => {
				self.file_pos += *size as u64;
				return Some(Ok(DiffBlock::Skip { size: *size }));
			}
			DiffBlockN::Add(size) => {
				let res = self.file.seek(SeekFrom::Start(self.file_pos));
				if res.is_err() {
					return Some(Err("Error while seeking file".to_string()));
				};
				let slice: ReadSlice =
					ReadSlice::take_from_current(&mut ReadSlice::new(&mut self.file), *size as u64);
				self.file_pos += *size as u64;
				return Some(Ok(DiffBlock::Add { data: slice }));
			}
			DiffBlockN::Remove(size) => {
				return Some(Ok(DiffBlock::Remove { size: *size }));
			}
			DiffBlockN::Replace(remove, add) => {
				let res = self.file.seek(SeekFrom::Start(self.file_pos));
				if res.is_err() {
					return Some(Err("Error while seeking file".to_string()));
				};
				let slice =
					ReadSlice::take_from_current(&mut ReadSlice::new(&mut self.file), *add as u64);
				self.file_pos += *add as u64;
				return Some(Ok(DiffBlock::Replace {
					remove_size: *remove,
					data: slice,
				}));
			}
			DiffBlockN::ReplaceWithSameLength(size) => {
				let res = self.file.seek(SeekFrom::Start(self.file_pos));
				if res.is_err() {
					return Some(Err("Error while seeking file".to_string()));
				};
				let slice =
					ReadSlice::take_from_current(&mut ReadSlice::new(&mut self.file), *size as u64);
				self.file_pos += *size as u64;
				return Some(Ok(DiffBlock::ReplaceWithSameLength { data: slice }));
			}
		}
	}

	pub fn next_size(&mut self) -> Option<u64> {
		if self.pos >= self.diff.len() {
			return None;
		};

		let item = &self.diff[self.pos];
		self.pos += 1;

		match item {
			DiffBlockN::Skip(_size) => {
				return Some(6);
			}
			DiffBlockN::Add(size) => {
				return Some(6 + (*size as u64));
			}
			DiffBlockN::Remove(_size) => {
				return Some(6);
			}
			DiffBlockN::Replace(_remove, add) => {
				return Some(10 + (*add as u64));
			}
			DiffBlockN::ReplaceWithSameLength(size) => {
				return Some(6 + (*size as u64));
			}
		}
	}
}

#[cfg(test)]
mod diff_iterator_tests {
	use super::DiffIterator;
	use test_mod::TextFile;

	#[test]
	fn works_test() {
		let file_a = TextFile::new(
			"
			hey fellas
			have you header the news
			the becky back in town
			you should be worried
			cos i heard shes been down
			to the alley where the mishief runs
			"
				.to_string(),
		);
		let file_b = TextFile::new(
			"
			hey fellas
			have you header the news
			the becky back in town
			maybe tomorrow
			you should be worried
			cos i heard shes been frown
			to the alley where the mishief runs
			"
				.to_string(),
		);
		let mut it = DiffIterator::new(file_a, file_b).unwrap();
		let mut i = 0;
		while let Some(_block) = it.next_ref() {
			i += 1;
		}
		assert_eq!(i, 5);
	}
}
