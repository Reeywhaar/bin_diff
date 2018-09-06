//! Functions for creating, applying and combining diffs

use bytes_serializer::IntoBytesSerializer;
use diff_block::DiffBlock;
use diff_iterator::DiffIterator;
use diff_reader::DiffReader;
use drain::Drainable;
use functions::{read_n, vec_shift, vec_to_u32_be};
use indexes::WithIndexes;
use readslice::ReadSlice;
use std::io::{copy, BufWriter, Error, ErrorKind, Read, Result as IOResult, Seek, Write};

/// Creates and writes diff of two `WithIndexes` Implementations
pub fn create_diff<T: WithIndexes, U: WithIndexes, W: Write>(
	original: &mut T,
	edited: &mut U,
	output: &mut W,
) -> IOResult<()> {
	let mut dit = DiffIterator::new(original, edited).or(Err(Error::new(
		ErrorKind::Other,
		"Error while creating DiffIterator",
	)))?;

	let mut stdo = BufWriter::with_capacity(1024 * 64, output);

	let mut buf = vec![0u8; 1024 * 64];
	while let Some(block) = dit.next_ref() {
		let mut block = block
			.or(Err(Error::new(ErrorKind::Other, "Cannot get diff block")))
			.map(|x| x.into_bytes())?;
		loop {
			let x = block.read(&mut buf)?;
			if x == 0 {
				break;
			}
			stdo.write(&buf[0..x])?;
		}
	}
	stdo.flush()?;
	Ok(())
}

/// Takes file and applies binary diff
pub fn apply_diff<T: Read, U: Read, W: Write>(
	mut file: &mut T,
	mut diff: &mut U,
	mut output: &mut W,
) -> IOResult<()> {
	let mut buf = vec![0; 1024 * 64];
	let mut output = BufWriter::with_capacity(8, &mut output);

	loop {
		let res = read_n(&mut diff, &mut buf, 2);

		if res.is_err() {
			break;
		}

		let slice: &[u8] = &buf[0..2].to_vec();
		match slice.as_ref() {
			[0x00, 0x00] => {
				read_n(&mut diff, &mut buf, 4)?;
				let len = vec_to_u32_be(&buf[0..4]);
				let mut r = (&mut file).take(len as u64);
				copy(&mut r, &mut output)?;
			}
			[0x00, 0x01] => {
				read_n(&mut diff, &mut buf, 4)?;
				let len = vec_to_u32_be(&buf[0..4]);
				let mut r = (&mut diff).take(len as u64);
				copy(&mut r, &mut output)?;
			}
			[0x00, 0x02] => {
				read_n(&mut diff, &mut buf, 4)?;
				let len = vec_to_u32_be(&buf[0..4]);
				file.drain(len as u64).get_drained()?;
			}
			[0x00, 0x03] => {
				read_n(&mut diff, &mut buf, 4)?;
				let remove = vec_to_u32_be(&buf[0..4]);
				read_n(&mut diff, &mut buf, 4)?;
				let add = vec_to_u32_be(&buf[0..4]);
				file.drain(remove as u64).get_drained()?;
				let mut r = (&mut diff).take(add as u64);
				copy(&mut r, &mut output)?;
			}
			[0x00, 0x04] => {
				read_n(&mut diff, &mut buf, 4)?;
				let size = vec_to_u32_be(&buf[0..4]);
				file.drain(size as u64).get_drained()?;
				let mut r = (&mut diff).take(size as u64);
				copy(&mut r, &mut output)?;
			}
			_ => {
				return Err(Error::new(
					ErrorKind::Other,
					"Unknown Action: possibly corrupted file or diff",
				));
			}
		}
	}
	return output.flush();
}

#[cfg(test)]
mod apply_diff_tests {
	use super::{apply_diff, create_diff};
	use functions::compute_hash;
	use std::io::{Cursor, Seek, SeekFrom};
	use test_mod::TextFile;

	#[test]
	fn works_test() {
		#[cfg_attr(rustfmt, rustfmt_skip)]
		let mut file = Cursor::new(vec![
			0xd0, 0x4b, 0x51, 0x00, 0x25, 0xb6, 0x95, 0xf3,
			0xb0, 0xa9, 0x59, 0xdc, 0x30, 0x35, 0x16, 0x7d,
			0x06, 0xa1, 0xf7, 0x66, 0x64, 0x33, 0x05, 0xee,
			0x2b, 0x35, 0xa9, 0x38, 0x80, 0x7f, 0x1c, 0x90,
			0x2c, 0x29, 0x2a, 0x49, 0x79, 0x66, 0x83, 0x55,
			0x8e, 0xce, 0x78, 0xd4, 0xef, 0x0f, 0xaa, 0xaa,
			0x1c, 0x41, 0xaf, 0xa2, 0xed, 0x85, 0xb6, 0x16,
			0x22, 0xe5, 0x83, 0x7a, 0xf7, 0x73, 0x78, 0xf5,
			0xf5, 0x63, 0x3b, 0x0a, 0x6d, 0xe5, 0x0b, 0x36,
			0x4b, 0x97, 0xc2, 0xfe, 0x84, 0x40, 0x3f, 0x74,
			0x20, 0x4b, 0xbb, 0xfe, 0x4c, 0xe1, 0x87, 0xc2,
			0x55, 0x71, 0xa3, 0x87, 0x55, 0xad, 0x87, 0xad,
			0x08, 0x69, 0x39, 0x0f, 0x8d, 0xe2, 0x9a, 0xef,
		]);

		#[cfg_attr(rustfmt, rustfmt_skip)]
		let mut diff = Cursor::new(vec![
			0x00, 0x00, 0x00, 0x00, 0x00, 0x10, // skip 16
			0x00, 0x01, 0x00, 0x00, 0x00, 0x20, // add 32
			0xef, 0x22, 0xe4, 0x2c, 0x5f, 0x3c, 0xde, 0x10, //
			0x8d, 0x27, 0x6c, 0xdd, 0xbc, 0xc6, 0xff, 0xf9, //
			0x5c, 0xe1, 0x81, 0x53, 0xda, 0x3b, 0xa6, 0x7e, //
			0xa9, 0xee, 0xe0, 0x00, 0x67, 0x24, 0x25, 0x78, // added 32 data
			0x00, 0x00, 0x00, 0x00, 0x00, 0x08, // skip 8
			0x00, 0x02, 0x00, 0x00, 0x00, 0x10, // remove 16
			0x00, 0x00, 0x00, 0x00, 0x00, 0x10, // skip 16
			0x00, 0x03, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x20, // replace 16 with 32
			0x23, 0x2a, 0xe9, 0x85, 0xfa, 0x6d, 0xb6, 0x78, //
			0xcd, 0x55, 0x66, 0xc2, 0x03, 0x80, 0x33, 0x3d, //
			0xc1, 0x8c, 0x62, 0xfb, 0xbb, 0xde, 0xe2, 0x53, //
			0xc7, 0x41, 0x0e, 0x82, 0xff, 0x60, 0x40, 0xf0, // added 32 data
			0x00, 0x00, 0x00, 0x00, 0x00, 0x20, // skip 32
		]);

		#[cfg_attr(rustfmt, rustfmt_skip)]
		let result = vec![
			0xd0, 0x4b, 0x51, 0x00, 0x25, 0xb6, 0x95, 0xf3, //
			0xb0, 0xa9, 0x59, 0xdc, 0x30, 0x35, 0x16, 0x7d, // skipped
			0xef, 0x22, 0xe4, 0x2c, 0x5f, 0x3c, 0xde, 0x10, //
			0x8d, 0x27, 0x6c, 0xdd, 0xbc, 0xc6, 0xff, 0xf9, //
			0x5c, 0xe1, 0x81, 0x53, 0xda, 0x3b, 0xa6, 0x7e, //
			0xa9, 0xee, 0xe0, 0x00, 0x67, 0x24, 0x25, 0x78, // added
			0x06, 0xa1, 0xf7, 0x66, 0x64, 0x33, 0x05, 0xee, // skipped
			// removed 16
			0x8e, 0xce, 0x78, 0xd4, 0xef, 0x0f, 0xaa, 0xaa, //
			0x1c, 0x41, 0xaf, 0xa2, 0xed, 0x85, 0xb6, 0x16, // skipped 16
			// removed 16 and replaced ->
			0x23, 0x2a, 0xe9, 0x85, 0xfa, 0x6d, 0xb6, 0x78, //
			0xcd, 0x55, 0x66, 0xc2, 0x03, 0x80, 0x33, 0x3d, //
			0xc1, 0x8c, 0x62, 0xfb, 0xbb, 0xde, 0xe2, 0x53, //
			0xc7, 0x41, 0x0e, 0x82, 0xff, 0x60, 0x40, 0xf0, //added 32
			0x4b, 0x97, 0xc2, 0xfe, 0x84, 0x40, 0x3f, 0x74, //
			0x20, 0x4b, 0xbb, 0xfe, 0x4c, 0xe1, 0x87, 0xc2, //
			0x55, 0x71, 0xa3, 0x87, 0x55, 0xad, 0x87, 0xad, //
			0x08, 0x69, 0x39, 0x0f, 0x8d, 0xe2, 0x9a, 0xef, // skipped 32
		];

		let mut output = Cursor::new(vec![0, 136]);
		apply_diff(&mut file, &mut diff, &mut output).unwrap();
		assert_eq!(output.get_ref(), &result);
	}

	#[test]
	fn action_fail_test() {
		#[cfg_attr(rustfmt, rustfmt_skip)]
		let mut file = Cursor::new(vec![
			0xd0, 0x4b, 0x51, 0x00, 0x25, 0xb6, 0x95, 0xf3,
			0xb0, 0xa9, 0x59, 0xdc, 0x30, 0x35, 0x16, 0x7d,
			0x06, 0xa1, 0xf7, 0x66, 0x64, 0x33, 0x05, 0xee,
			0x2b, 0x35, 0xa9, 0x38, 0x80, 0x7f, 0x1c, 0x90,
		]);

		#[cfg_attr(rustfmt, rustfmt_skip)]
		let mut diff = Cursor::new(vec![
			0x50, 0x53, 0x44, 0x44, 0x49, 0x46, 0x46, 0x31, // PSDDIFF2
			0x00, 0x01, // version
			0x4a, 0x00, 0x00, 0x00, 0x00, 0x10, // skip 16
			0x00, 0x01, 0x00, 0x00, 0x00, 0x20, // add 32
		]);

		let mut output = Cursor::new(vec![0, 136]);
		let res = apply_diff(&mut file, &mut diff, &mut output);
		assert_eq!(
			res.unwrap_err().to_string(),
			"Unknown Action: possibly corrupted file or diff".to_string()
		)
	}

	#[test]
	fn works_live_test() {
		#[cfg_attr(rustfmt, rustfmt_skip)]
		let inputs = [
			["a_a.txt", "a_b.txt"],
			["a_b.txt", "a_c.txt"],
			["a_a.txt", "a_c.txt"],
		];

		for pair in inputs.iter() {
			let pairs = [[pair[0], pair[1]], [pair[1], pair[0]]];
			for pair in pairs.iter() {
				let mut file_a = TextFile::from_path(&format!("./test_data/{}", pair[0]));
				let mut file_b = TextFile::from_path(&format!("./test_data/{}", pair[1]));

				let hash = compute_hash(&mut file_b);
				file_b.seek(SeekFrom::Start(0)).unwrap();

				let mut diff = Cursor::new(vec![]);
				create_diff(&mut file_a, &mut file_b, &mut diff).unwrap();
				diff.seek(SeekFrom::Start(0)).unwrap();

				file_a.seek(SeekFrom::Start(0)).unwrap();
				let mut restored = Cursor::new(vec![]);
				apply_diff(&mut file_a, &mut diff, &mut restored).unwrap();
				restored.seek(SeekFrom::Start(0)).unwrap();

				let res_hash = compute_hash(&mut restored);

				assert_eq!(hash, res_hash, "pair {:?} failed", pair);
			}
		}
	}
}

fn combine_diffs_to_vec<'a, 'b: 'a>(
	mut blocksa: DiffReader<'a, 'b>,
	mut blocksb: DiffReader<'a, 'b>,
) -> IOResult<Vec<DiffBlock<'b, u32>>> {
	let mut out = vec![];
	let mut da = None;
	let mut db = None;

	loop {
		if da.is_none() {
			da = blocksa.next()?;
		}
		if db.is_none() {
			db = blocksb.next()?;
		}
		match (da.is_none(), db.is_none()) {
			(true, true) => break,
			(false, true) => {
				out.push(da.unwrap());
				da = None;
			}
			(true, false) => {
				out.push(db.unwrap());
				db = None;
			}
			(false, false) => {
				let (outblock, ba, bb) = da.unwrap().diff(db.unwrap());
				if outblock.is_some() {
					out.push(outblock.unwrap())
				}
				da = ba;
				db = bb;
			}
		};
	}

	if out.len() < 1 {
		return Ok(vec![]);
	}

	if out.len() == 1 {
		return Ok(out);
	}

	let mut compressed = false;
	let mut started = false;
	let mut processed = vec![];
	while !(started && compressed) {
		started = true;
		compressed = true;
		let mut blocka = out.remove(0);
		let len = out.len();
		for i in 0..len {
			let mut blockb = out.remove(0);
			let op = blocka + blockb;
			processed.push(op.0);
			if op.1.is_some() {
				if i != len - 1 {
					blocka = op.1.unwrap();
				} else {
					processed.push(op.1.unwrap());
					// satisfying compilator
					blocka = DiffBlock::Skip { size: 0 };
				}
			} else {
				if i != len - 1 {
					blocka = processed.pop().unwrap();
					compressed = false;
				} else {
					// satisfying compilator
					blocka = DiffBlock::Skip { size: 0 };
				}
			}
		}
		out = processed;
		processed = vec![];
	}

	return Ok(out);
}

/// Combines two binary diffs into one
pub fn combine_diffs<'a, T: 'a + Read + Seek, U: 'a + Read + Seek, W: Write>(
	blocksa: T,
	blocksb: U,
	mut output: &mut W,
) -> IOResult<()> {
	let mut blocksa = ReadSlice::new(blocksa);
	let blocksa = DiffReader::new(&mut blocksa);
	let mut blocksb = ReadSlice::new(blocksb);
	let blocksb = DiffReader::new(&mut blocksb);
	let blocks = combine_diffs_to_vec(blocksa, blocksb)?;

	for block in blocks {
		copy(&mut block.into_bytes(), &mut output)?;
	}

	Ok(())
}

#[cfg(test)]
mod combine_diffs_tests {
	use super::{apply_diff, combine_diffs, create_diff};
	use functions::compute_hash;
	use std::io::{Cursor, Seek, SeekFrom};
	use test_mod::TextFile;

	#[test]
	fn works_live_test() {
		let files = [
			[
				"./test_data/a_b.txt",
				"./test_data/a_c.txt",
				"./test_data/a_a.txt",
			],
			[
				"./test_data/a_a.txt",
				"./test_data/a_b.txt",
				"./test_data/a_c.txt",
			],
			[
				"./test_data/a_c.txt",
				"./test_data/a_a.txt",
				"./test_data/a_b.txt",
			],
		];

		for set in files.iter() {
			let mut file_a = TextFile::from_path(set[0]);
			let mut file_b = TextFile::from_path(set[1]);
			let mut file_c = TextFile::from_path(set[2]);

			let hash = compute_hash(&mut file_c);
			file_c.seek(SeekFrom::Start(0)).unwrap();

			let mut diff_a_b = Cursor::new(vec![]);
			create_diff(&mut file_a, &mut file_b, &mut diff_a_b).unwrap();

			file_b.seek(SeekFrom::Start(0)).unwrap();
			let mut diff_b_c = Cursor::new(vec![]);
			create_diff(&mut file_b, &mut file_c, &mut diff_b_c).unwrap();

			diff_a_b.seek(SeekFrom::Start(0)).unwrap();
			diff_b_c.seek(SeekFrom::Start(0)).unwrap();
			let mut diff_a_b_c = Cursor::new(vec![]);
			combine_diffs(&mut diff_a_b, &mut diff_b_c, &mut diff_a_b_c).unwrap();

			file_a.seek(SeekFrom::Start(0)).unwrap();
			diff_a_b_c.seek(SeekFrom::Start(0)).unwrap();
			let mut restored = Cursor::new(vec![]);
			apply_diff(&mut file_a, &mut diff_a_b_c, &mut restored).unwrap();
			restored.seek(SeekFrom::Start(0)).unwrap();

			let rhash = compute_hash(&mut restored);

			assert_eq!(hash, rhash);
		}
	}
}

fn combine_diffs_vec_to_vec<'a, T: 'a + Read + Seek>(
	mut diffs: &mut Vec<T>,
) -> IOResult<Vec<DiffBlock<'a, u32>>> {
	if diffs.len() < 2 {
		return Err(Error::new(
			ErrorKind::InvalidInput,
			"Number of diff must be greater than one",
		));
	};

	let mut out = {
		let diffsa = diffs.remove(0);
		let mut sla = ReadSlice::new(diffsa);
		let blocksa = DiffReader::new(&mut sla);
		let diffsb = diffs.remove(0);
		let mut slb = ReadSlice::new(diffsb);
		let blocksb = DiffReader::new(&mut slb);
		combine_diffs_to_vec(blocksa, blocksb)?
	};

	while let Some(block) = vec_shift(&mut diffs) {
		out = {
			let blocksa = DiffReader::new_from_vector(&mut out);
			let mut slb = ReadSlice::new(block);
			let blocksb = DiffReader::new(&mut slb);
			combine_diffs_to_vec(blocksa, blocksb)?
		}
	}

	Ok(out)
}

/// Combines diffs into vector of Read objects
///
/// The reason to have this function is an ability to pass vector of lightweit read objects (instead of binary data)
pub fn combine_diffs_vec_to_read_vec<'a, 'b: 'a, T: 'b + Read + Seek>(
	diffs: &'a mut Vec<T>,
) -> IOResult<Vec<impl Read + 'b>> {
	let mut blocks = combine_diffs_vec_to_vec(diffs)?;
	let mut reads = vec![];
	while let Some(item) = vec_shift(&mut blocks) {
		reads.push(item.into_bytes());
	}
	Ok(reads)
}

/// Combines multiple binary diffs into one
pub fn combine_diffs_vec<'a, T: 'a + Read + Seek, W: Write>(
	mut diffs: &mut Vec<T>,
	mut output: &mut W,
) -> IOResult<()> {
	let mut blocks = combine_diffs_vec_to_read_vec(&mut diffs)?;

	for block in blocks.iter_mut() {
		copy(block, &mut output)?;
	}

	Ok(())
}

#[cfg(test)]
mod combine_diffs_vec_tests {
	use super::{apply_diff, combine_diffs_vec, create_diff};
	use functions::compute_hash;
	use std::io::{Cursor, Seek, SeekFrom};
	use test_mod::TextFile;

	#[test]
	fn works_live_test() {
		let files = [
			("./test_data/a_a.txt", "./test_data/a_b.txt"),
			("./test_data/a_b.txt", "./test_data/a_c.txt"),
			("./test_data/a_c.txt", "./test_data/a_d.txt"),
		];

		let mut diffs = files
			.iter()
			.map(|(a, b)| {
				let mut out = Cursor::new(vec![]);
				let mut filea = TextFile::from_path(a);
				let mut fileb = TextFile::from_path(b);
				create_diff(&mut filea, &mut fileb, &mut out).unwrap();
				out.seek(SeekFrom::Start(0)).unwrap();
				out
			})
			.collect();

		let mut original = TextFile::from_path("./test_data/a_a.txt");
		let mut acc_diff = {
			let mut x = Cursor::new(vec![]);
			combine_diffs_vec(&mut diffs, &mut x).unwrap();
			x.seek(SeekFrom::Start(0)).unwrap();
			x
		};
		let hash = compute_hash(&mut TextFile::from_path(files[files.len() - 1].1));
		let restoredhash = {
			let mut x = Cursor::new(vec![]);
			apply_diff(&mut original, &mut acc_diff, &mut x).unwrap();
			x.seek(SeekFrom::Start(0)).unwrap();
			compute_hash(&mut x)
		};

		assert_eq!(hash, restoredhash);
	}
}
