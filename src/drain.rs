use std::cmp::min;
use std::io::{copy, sink, Error, ErrorKind, Read, Result};

pub struct Drain<T: Read> {
	inner: T,
	limit: u64,
	drained: u64,
}

impl<T: Read> Drain<T> {
	pub fn new(inner: T, limit: u64) -> Self {
		return Self {
			inner,
			limit,
			drained: 0,
		};
	}

	#[allow(dead_code)]
	pub fn into_inner(self) -> T {
		return self.inner;
	}

	#[allow(dead_code)]
	pub fn get_ref(&self) -> &T {
		return &self.inner;
	}

	#[allow(dead_code)]
	pub fn get_mut(&mut self) -> &mut T {
		return &mut self.inner;
	}

	pub fn get_drained(&mut self) -> Result<()> {
		let mut drainb = sink();
		let mut attempts = 0;
		loop {
			if self.drained >= self.limit {
				break;
			}
			let mut take = self
				.inner
				.by_ref()
				.take(min(1024 * 64, self.limit - self.drained));
			let d = copy(&mut take, &mut drainb)?;
			if d == 0 {
				attempts += 1;
				if attempts >= 10 {
					return Err(Error::from(ErrorKind::UnexpectedEof));
				};
			};
			self.drained += d;
		}

		return Ok(());
	}
}

impl<T: Read> Read for Drain<T> {
	fn read(&mut self, mut buffer: &mut [u8]) -> Result<usize> {
		self.get_drained()?;
		return self.inner.read(&mut buffer);
	}
}

pub trait Drainable: Read + Sized {
	fn drain(self, limit: u64) -> Drain<Self>;
}

impl<T: Read> Drainable for T {
	fn drain(self, limit: u64) -> Drain<Self> {
		return Drain::new(self, limit);
	}
}

#[cfg(test)]
mod drain_tests {
	use super::Drainable;
	use std::io::{Cursor, Read};

	#[test]
	fn drain_test() {
		let mut data = Cursor::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).drain(3);
		let mut o = vec![];
		data.read_to_end(&mut o).unwrap();
		assert_eq!(&o, &[4, 5, 6, 7, 8, 9, 10])
	}
}
