use readseek::ReadSeek;
use std::cmp::min;
use std::convert::From;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::fs::File;
use std::io::{Cursor, Read, Result as IOResult, Seek, SeekFrom};
use std::rc::Rc;
use std::sync::Mutex;

struct Slice<'a> {
	v: Rc<Mutex<Box<dyn ReadSeek + 'a>>>,
	size: u64,
	initial_position: u64,
	position: u64,
}

impl<'a> Slice<'a> {
	fn new<T: 'a + ReadSeek>(mut v: T) -> Self {
		let init_pos = v.seek(SeekFrom::Current(0)).unwrap();
		let end = v.seek(SeekFrom::End(0)).unwrap();
		let size = end - init_pos;
		v.seek(SeekFrom::Start(init_pos)).unwrap();
		Self {
			v: Rc::new(Mutex::new(Box::new(v))),
			size: size,
			initial_position: init_pos,
			position: 0,
		}
	}

	pub fn offset(&self, offset: u64) -> Self {
		let mut clone = self.clone();
		let offset = min(clone.size, offset);
		clone.size = clone.size - offset;
		clone.initial_position += offset;
		clone.position = if offset > clone.position {
			0
		} else {
			clone.position - offset
		};
		return clone;
	}

	pub fn take(&self, size: u64) -> Self {
		if size >= self.size {
			return self.clone();
		};

		let mut clone = self.clone();
		if self.size > size {
			clone.size = size;
		};
		if clone.position > size {
			clone.position = size;
		};
		return clone;
	}
}

impl<'a> Debug for Slice<'a> {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(
			f,
			"Slice {{ initial_position: {}, position: {}, size: {} }}",
			self.initial_position, self.position, self.size,
		)
	}
}

impl<'a> Clone for Slice<'a> {
	fn clone(&self) -> Self {
		Self {
			v: self.v.clone(),
			size: self.size.clone(),
			initial_position: self.initial_position,
			position: self.position,
		}
	}
}

impl<'a> Read for Slice<'a> {
	fn read(&mut self, buffer: &mut [u8]) -> IOResult<usize> {
		let pos = self.position;
		self.seek(SeekFrom::Start(pos))?;
		let maxread = (self.size - self.position) as usize;
		let end = min(maxread, buffer.len());
		let read = self.v.lock().unwrap().read(&mut buffer[0..end])?;
		self.position += read as u64;
		return Ok(read);
	}
}

impl<'a> Seek for Slice<'a> {
	fn seek(&mut self, from: SeekFrom) -> IOResult<u64> {
		match from {
			SeekFrom::Start(x) => {
				let pos = self.initial_position + x;
				let seek = self.v.lock().unwrap().seek(SeekFrom::Start(pos))?;
				self.position = seek - self.initial_position;
				return Ok(self.position);
			}
			SeekFrom::End(x) => {
				let seek = self.v.lock().unwrap().seek(SeekFrom::End(x))?;
				self.position = seek - self.initial_position;
				return Ok(self.position);
			}
			SeekFrom::Current(x) => {
				let seek = self.v.lock().unwrap().seek(SeekFrom::Current(x))?;
				self.position = seek - self.initial_position;
				return Ok(self.position);
			}
		}
	}
}

pub struct ReadSlice<'a> {
	slices: Vec<Slice<'a>>,
	size: u64,
}

impl<'a> Debug for ReadSlice<'a> {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(
			f,
			"ReadSlice {{ slices: {:?}, size: {} }}",
			self.slices, self.size,
		)
	}
}

impl<'a> ReadSlice<'a> {
	pub fn new<T: 'a + ReadSeek>(v: T) -> Self {
		let slice = Slice::new(v);
		let size = slice.size;
		Self {
			slices: vec![slice],
			size: size,
		}
	}

	pub fn position(&self) -> u64 {
		let mut pos = 0;
		for item in self.slices.iter() {
			pos += item.position;
			if item.position < item.size {
				break;
			};
		}
		return pos;
	}

	pub fn offset(&self, offset: u64) -> Self {
		let mut clone = self.clone();
		let mut c_size = 0;
		let mut cut = 0;
		for item in clone.slices.iter_mut() {
			if c_size + item.size > offset {
				*item = item.offset(offset - c_size);
				break;
			}
			c_size += item.size;
			cut += 1;
		}
		if cut > 0 {
			clone.slices.drain(0..cut);
		};
		clone.size = clone.slices.iter().fold(0, |c, x| c + x.size);
		return clone;
	}

	pub fn offset_mut(&mut self, offset: u64) {
		let mut c_size = 0;
		let mut cut = 0;
		for item in self.slices.iter_mut() {
			if c_size + item.size > offset {
				*item = item.offset(offset - c_size);
				break;
			}
			c_size += item.size;
			cut += 1;
		}
		if cut > 0 {
			self.slices.drain(0..cut);
		};
		self.size = self.slices.iter().fold(0, |c, x| c + x.size);
	}

	pub fn take(&self, size: u64) -> Self {
		let mut clone = self.clone();
		let mut c_size = size;
		let mut cut = 0;
		for item in clone.slices.iter_mut() {
			if item.size > c_size {
				*item = Slice::take(item, c_size);
				break;
			}
			cut += 1;
			c_size -= item.size;
		}
		if cut + 1 <= clone.slices.len() {
			clone.slices.drain(cut + 1..);
		};
		clone.size = clone.slices.iter().fold(0, |c, x| c + x.size);
		return clone;
	}

	pub fn take_from_current(&self, size: u64) -> Self {
		let mut clone = self.clone();
		let pos = clone.position();
		clone.offset_mut(pos);
		return ReadSlice::take(&clone, size);
	}

	pub fn chain<T: 'a + ReadSeek>(&self, other: T) -> Self {
		let mut clone = self.clone();
		let slice = Slice::new(other);
		let slice_size = slice.size;
		clone.slices.push(slice);
		clone.size += slice_size;
		return clone;
	}

	#[allow(dead_code)]
	pub fn rewind(&mut self) -> &mut Self {
		self.seek(SeekFrom::Start(0)).unwrap();
		return self;
	}

	pub fn size(&self) -> u64 {
		return self.size;
	}
}

impl<'a> Clone for ReadSlice<'a> {
	fn clone(&self) -> Self {
		Self {
			slices: self.slices.clone(),
			size: self.size,
		}
	}
}

impl<'a> Read for ReadSlice<'a> {
	fn read(&mut self, buffer: &mut [u8]) -> IOResult<usize> {
		let mut read = 0;
		let buflen = buffer.len();
		for slice in self.slices.iter_mut() {
			while !(read >= buflen || slice.position >= slice.size) {
				read += slice.read(&mut buffer[read..])?;
			}
		}
		return Ok(read);
	}
}

impl<'a> Seek for ReadSlice<'a> {
	fn seek(&mut self, from: SeekFrom) -> IOResult<u64> {
		match from {
			SeekFrom::Start(x) => {
				let mut seek = 0;
				for slice in self.slices.iter_mut() {
					seek += slice.seek(SeekFrom::Start(x - seek))?;
					if seek >= x {
						break;
					}
				}
				return Ok(seek);
			}
			SeekFrom::End(x) => {
				let p = self.size as i64 - x;
				return self.seek(SeekFrom::Start(p as u64));
			}
			SeekFrom::Current(x) => {
				let mut pos = 0;
				for slice in self.slices.iter() {
					pos += slice.position;
					if slice.position < slice.size {
						break;
					}
				}
				let pos = pos as i64 + x;
				return self.seek(SeekFrom::Start(pos as u64));
			}
		}
	}
}

impl<'a, T: 'a + Read + Seek + AsRef<[u8]>> From<Cursor<T>> for ReadSlice<'a> {
	fn from(v: Cursor<T>) -> Self {
		Self::new(v)
	}
}

impl<'a> From<File> for ReadSlice<'a> {
	fn from(v: File) -> Self {
		Self::new(v)
	}
}

#[cfg(test)]
mod readslice_tests {
	use super::ReadSlice;
	use std::io::{copy, Cursor, Read};

	#[test]
	fn works_test() {
		let vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
		let data = Cursor::new(vec.clone());
		let mut slice = ReadSlice::new(data);
		let mut buf = vec![0; 10];
		slice.read(&mut buf).unwrap();
		assert_eq!(buf, vec);
	}

	#[test]
	fn chain_test() {
		let vec = vec![1, 2, 3, 4, 5];
		let vecb = vec![6, 7, 8, 9, 10];
		let data = Cursor::new(vec.clone());
		let datab = Cursor::new(vecb.clone());
		let mut slice = ReadSlice::chain(&ReadSlice::new(data), datab);
		let mut buf = vec![0; 10];
		slice.read(&mut buf).unwrap();
		assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	}

	#[test]
	fn chain_sl_test() {
		let vec = vec![1, 2, 3, 4, 5];
		let vecb = vec![6, 7, 8, 9, 10];
		let data = Cursor::new(vec.clone());
		let datab = Cursor::new(vecb.clone());
		let mut slice = ReadSlice::chain(&ReadSlice::new(data), ReadSlice::new(datab));
		let mut buf = Cursor::new(vec![]);
		copy(&mut slice, &mut buf).unwrap();
		assert_eq!(buf.into_inner(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
	}

	#[test]
	fn offset_test() {
		let vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
		let data = Cursor::new(vec.clone());
		let mut slice = ReadSlice::offset(&ReadSlice::new(data), 3);
		let mut buf = vec![0; 10];
		slice.read(&mut buf).unwrap();
		assert_eq!(buf, [4, 5, 6, 7, 8, 9, 10, 0, 0, 0]);
	}

	#[test]
	fn take_test() {
		let vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
		let data = Cursor::new(vec.clone());
		let mut slice = ReadSlice::take(&ReadSlice::new(data), 7);
		let mut buf = vec![0; 10];
		slice.read(&mut buf).unwrap();
		assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 0, 0, 0]);
	}

	#[test]
	fn fn_test() {
		let vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
		let vecb = vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
		let data = Cursor::new(vec.clone());
		let datab = Cursor::new(vecb.clone());
		let mut slice = ReadSlice::take(
			&ReadSlice::chain(&ReadSlice::new(data), datab).offset(11),
			5,
		);
		let mut buf = vec![0; 10];
		slice.read(&mut buf).unwrap();
		assert_eq!(buf, [12, 13, 14, 15, 16, 0, 0, 0, 0, 0]);
	}
}
