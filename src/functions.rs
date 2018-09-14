//! Common functions for manipulating binary data

use sha2::{Digest, Sha256};
use std::cmp::max;
use std::io::{BufReader, Error, ErrorKind, Read, Result as IOResult};
use std::mem::transmute_copy;

/// Serializes u16 to u8 array
pub fn u16_to_u8_be_vec(n: u16) -> [u8; 2] {
	let bytes: [u8; 2] = unsafe { transmute_copy::<u16, [u8; 2]>(&n.to_be()) };
	bytes
}

#[test]
fn u16_to_u8_be_vec_test() {
	let b = u16_to_u8_be_vec(10u16);
	assert_eq!(b, [0x00, 10]);
}

/// Serializes u32 to u8 array
pub fn u32_to_u8_be_vec(n: u32) -> [u8; 4] {
	let bytes: [u8; 4] = unsafe { transmute_copy::<u32, [u8; 4]>(&n.to_be()) };
	bytes
}

#[test]
fn u32_to_u8_be_vec_test() {
	let b = u32_to_u8_be_vec(10u32);
	assert_eq!(b, [0x00, 0x00, 0x00, 10]);
}

/// Serializes u64 to u8 array
pub fn u64_to_u8_be_vec(n: u64) -> [u8; 8] {
	let bytes: [u8; 8] = unsafe { transmute_copy::<u64, [u8; 8]>(&n.to_be()) };
	bytes
}

#[test]
fn u64_to_u8_be_vec_test() {
	assert_eq!(
		u64_to_u8_be_vec(10u64),
		[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 10]
	);
}

/// Converts u8 slice to usize
pub fn vec_to_usize_be(input: &[u8]) -> usize {
	let mut o: usize = 0;
	let len = input.len();
	for (index, item) in input.iter().enumerate() {
		let shift = len - index - 1;
		let s = (*item as usize) << (shift * 8);
		o |= s;
	}
	o
}

/// Converts u8 slice to u32 BigEndian
pub fn vec_to_u32_be(input: &[u8]) -> u32 {
	let mut o: u32 = 0;
	let len = input.len();
	for (index, item) in input.iter().enumerate() {
		let shift = len - index - 1;
		let s = u32::from(*item) << (shift * 8);
		o |= s;
	}
	o
}

#[test]
fn vec_to_u32_be_test() {
	assert_eq!(vec_to_u32_be(&[0x00, 0x00, 0x00, 0x10]), 16);
}

/// Converts u8 slice to u32 LittleEndian
pub fn vec_to_usize_le(input: &[u8]) -> usize {
	let mut o: usize = 0;
	for (index, item) in input.iter().enumerate() {
		let s = (*item as usize) << (index * 8);
		o |= s;
	}
	o
}

/// Converts u8 slice to i16 BigEndian
pub fn vec_to_i16_be(n: &[u8]) -> i16 {
	let n = vec_to_usize_be(n);
	unsafe { transmute_copy::<usize, i16>(&n) }
}

#[test]
fn vec_to_i16_be_test() {
	assert_eq!(vec_to_i16_be(&[0b0000_0001]), 1);
	assert_eq!(vec_to_i16_be(&[0b1000_0000, 0b0000_0000]), -32768);
	assert_eq!(vec_to_i16_be(&[0b1111_1111, 0b1111_1111]), -1);
}

/// Converts u16 slice to i16
pub fn u_to_i16_be(n: u16) -> i16 {
	unsafe { transmute_copy::<u16, i16>(&n) }
}

/// Reads n bytes from reader and converts to usize BigEndian
pub fn read_usize_be<T: Read>(input: &mut T, size: usize) -> Result<usize, Error> {
	let mut buf = vec![0u8; size];
	input.read_exact(&mut buf)?;
	Ok(vec_to_usize_be(&buf))
}

/// Reads n bytes from reader and converts to usize LittleEndian
pub fn read_usize_le<T: Read>(input: &mut T, size: usize) -> Result<usize, Error> {
	let mut buf = vec![0u8; size];
	input.read_exact(&mut buf)?;
	Ok(vec_to_usize_le(&buf))
}

/// used to compare T: Read
pub fn cmp_read<'a, T: Read>(
	a: &'a mut T,
	b: &'a mut T,
	buffer_size_in_kb: usize,
) -> IOResult<bool> {
	let mut a = BufReader::with_capacity(1024 * buffer_size_in_kb, a);
	let mut b = BufReader::with_capacity(1024 * buffer_size_in_kb, b);
	let mut buf_a = vec![0; 1024 * buffer_size_in_kb];
	loop {
		let read_a = a.read(&mut buf_a)?;
		let mut buf_b = vec![0; max(read_a, 1)];
		let read_b = b.read_exact(&mut buf_b);
		if read_a == 0 && read_b.is_err() {
			return Ok(true);
		};
		if read_a == 0 && read_b.is_ok() {
			return Ok(false);
		};
		if read_b.is_err() {
			return Ok(false);
		}
		if read_a == 0 {
			return Ok(true);
		};
		if buf_a[..read_a] != buf_b[..] {
			return Ok(false);
		};
	}
}

/// Removes first item from vector and returns it
pub fn vec_shift<T>(vec: &mut Vec<T>) -> Option<T> {
	if vec.is_empty() {
		return None;
	};
	Some(vec.remove(0))
}

/// for comparation of medium chunks with buffer of 64kb, for comparation of small chunks use cmp_read_small
pub fn cmp_read_medium<'a, T: Read>(mut a: &'a mut T, mut b: &'a mut T) -> IOResult<bool> {
	cmp_read(&mut a, &mut b, 64)
}

/// used to compare small chunks, buffer is 1kb
pub fn cmp_read_small<'a, T: Read>(mut a: &'a mut T, mut b: &'a mut T) -> IOResult<bool> {
	cmp_read(&mut a, &mut b, 1)
}

/// Reads n bytes from reader
pub fn read_n<T: Read>(mut input: &mut T, buf: &mut [u8], size: u32) -> IOResult<usize> {
	let mut taken = (&mut input).take(u64::from(size));
	let mut read: usize = 0;
	let mut attempts = 0;
	while read < size as usize {
		let r = taken.read(&mut buf[read..])?;
		read += r;
		if r == 0 {
			attempts += 1;
			if attempts >= 10 {
				return Err(Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF"));
			}
		} else {
			attempts = 0;
		}
	}
	Ok(read)
}

/// Computes sha256 hash of the input reader
pub fn compute_hash<T: Read>(input: &mut T) -> String {
	let mut hasher = Sha256::default();

	let mut buf: &mut [u8] = &mut [0u8; 1024 * 64];
	while let Ok(x) = input.read(&mut buf) {
		if x == 0 {
			break;
		}
		let slice = &buf[0..x];
		hasher.input(slice);
	}

	hasher
		.result()
		.iter()
		.map(|b| format!("{:02x}", b))
		.collect::<String>()
}

#[cfg(test)]
mod functions_tests {
	use super::*;
	use std::io::Cursor;

	#[test]
	fn cmp_read_test() {
		let mut a = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut b = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		assert_eq!(cmp_read_small(&mut a, &mut b).unwrap(), true);

		let mut a = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut b = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 10]);
		assert_eq!(cmp_read_small(&mut a, &mut b).unwrap(), false);

		let mut a = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut b = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
		assert_eq!(cmp_read_small(&mut a, &mut b).unwrap(), false);

		let mut a = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut b = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7]);
		assert_eq!(cmp_read_small(&mut a, &mut b).unwrap(), false);
	}
}
