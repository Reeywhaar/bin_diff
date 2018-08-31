use std::io::{Read, Result as IOResult};
use std::rc::Rc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct ClonableRead<T: Read> {
	v: Rc<Mutex<T>>,
}

impl<T: Read> ClonableRead<T> {
	pub fn new(v: T) -> Self {
		Self {
			v: Rc::new(Mutex::new(v)),
		}
	}
}

impl<T: Read> Clone for ClonableRead<T> {
	fn clone(&self) -> Self {
		Self { v: self.v.clone() }
	}
}

impl<T: Read> Read for ClonableRead<T> {
	fn read(&mut self, mut buffer: &mut [u8]) -> IOResult<usize> {
		return self.v.lock().unwrap().read(&mut buffer);
	}
}
