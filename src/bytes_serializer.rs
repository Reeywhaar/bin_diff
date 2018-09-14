use std::io::{Read, Result};

pub struct BytesSerializer<T> {
	pos: usize,
	value: T,
	closure: Box<dyn FnMut(&mut usize, &mut T, &mut [u8]) -> Result<usize>>,
}

impl<T> BytesSerializer<T> {
	pub fn new(
		value: T,
		closure: Box<dyn FnMut(&mut usize, &mut T, &mut [u8]) -> Result<usize>>,
	) -> Self {
		Self {
			pos: 0,
			value,
			closure,
		}
	}
}

impl<T> Read for BytesSerializer<T> {
	fn read(&mut self, mut buffer: &mut [u8]) -> Result<usize> {
		(self.closure)(&mut self.pos, &mut self.value, &mut buffer)
	}
}

pub trait IntoBytesSerializer {
	type Item;

	fn into_bytes(self) -> BytesSerializer<Self::Item>;
}
