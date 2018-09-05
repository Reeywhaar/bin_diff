use diff_block::DiffBlock;
use functions::{read_n, vec_shift, vec_to_u32_be};
use readslice::ReadSlice;
use std::io::{Error, ErrorKind, Result as IOResult, Seek, SeekFrom};

enum Either<'a, 'b: 'a> {
	Input(&'a mut ReadSlice<'b>),
	Vector(&'a mut Vec<DiffBlock<'b, u32>>),
}

pub struct DiffReader<'a, 'b: 'a> {
	input: Either<'a, 'b>,
	buffer: [u8; 4],
}

impl<'a, 'b: 'a> DiffReader<'a, 'b> {
	pub fn new(input: &'a mut ReadSlice<'b>) -> Self {
		Self {
			input: Either::Input(input),
			buffer: [0; 4],
		}
	}

	pub fn new_from_vector(input: &'a mut Vec<DiffBlock<'b, u32>>) -> Self {
		Self {
			input: Either::Vector(input),
			buffer: [0; 4],
		}
	}

	pub fn next(&mut self) -> IOResult<Option<DiffBlock<'b, u32>>> {
		match &mut self.input {
			Either::Input(ref mut input) => {
				let read_size = read_n(input, &mut self.buffer, 2);
				match read_size {
					Err(e) => match e.kind() {
						ErrorKind::UnexpectedEof => {
							return Ok(None);
						}
						_ => {
							return Err(e);
						}
					},
					Ok(x) => {
						if x == 0 {
							return Ok(None);
						}
					}
				};
				let action = vec_to_u32_be(&self.buffer[0..2]);
				match action {
					0 => {
						read_n(input, &mut self.buffer, 4)?;
						let size = vec_to_u32_be(&self.buffer);
						return Ok(Some(DiffBlock::Skip { size }));
					}
					1 => {
						read_n(input, &mut self.buffer, 4)?;
						let size = vec_to_u32_be(&self.buffer);
						let mut data = ReadSlice::take_from_current(input, size as u64);
						ReadSlice::seek(input, SeekFrom::Current(size as i64))?;
						return Ok(Some(DiffBlock::Add { data }));
					}
					2 => {
						read_n(input, &mut self.buffer, 4)?;
						let size = vec_to_u32_be(&self.buffer);
						return Ok(Some(DiffBlock::Remove { size }));
					}
					3 => {
						read_n(input, &mut self.buffer, 4)?;
						let remove_size = vec_to_u32_be(&self.buffer);
						read_n(input, &mut self.buffer, 4)?;
						let size = vec_to_u32_be(&self.buffer);
						let mut data = ReadSlice::take_from_current(input, size as u64);
						ReadSlice::seek(input, SeekFrom::Current(size as i64))?;
						return Ok(Some(DiffBlock::Replace { remove_size, data }));
					}
					4 => {
						read_n(input, &mut self.buffer, 4)?;
						let size = vec_to_u32_be(&self.buffer);
						let mut data = ReadSlice::take_from_current(input, size as u64);
						ReadSlice::seek(input, SeekFrom::Current(size as i64))?;
						return Ok(Some(DiffBlock::ReplaceWithSameLength { data }));
					}
					_ => return Err(Error::new(ErrorKind::InvalidData, "Unknown Action")),
				}
			}
			Either::Vector(ref mut input) => {
				return Ok(vec_shift(input));
			}
		}
	}

	#[allow(dead_code)]
	pub fn consume(&mut self) -> IOResult<Vec<DiffBlock<'b, u32>>> {
		let mut out = vec![];

		while let Some(block) = self.next()? {
			out.push(block);
		}

		return Ok(out);
	}
}
