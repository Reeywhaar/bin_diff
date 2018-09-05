use bytes_serializer::{BytesSerializer, IntoBytesSerializer};
use cmp::Cmp;
use functions::{u16_to_u8_be_vec, u32_to_u8_be_vec};
use readslice::ReadSlice;
use std::io::{Cursor, Read};
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Clone, Debug)]
pub enum DiffBlockN<T: Add + AddAssign + Sub + SubAssign> {
	Skip(T),
	Add(T),
	Remove(T),
	Replace(T, T),
	ReplaceWithSameLength(T),
}

#[derive(Clone, Debug)]
pub enum DiffBlock<'a, T: Add + AddAssign + Sub + SubAssign> {
	Skip { size: T },
	Add { data: ReadSlice<'a> },
	Remove { size: T },
	Replace { remove_size: T, data: ReadSlice<'a> },
	ReplaceWithSameLength { data: ReadSlice<'a> },
}

impl<'a> DiffBlock<'a, u32> {
	fn get_action_number<'b>(&self, other: &DiffBlock<'b, u32>) -> u8 {
		let mut action = 0;
		match &self {
			DiffBlock::Skip { .. } => action += 10,
			DiffBlock::Add { .. } => action += 20,
			DiffBlock::Remove { .. } => action += 30,
			DiffBlock::Replace { .. } => action += 40,
			DiffBlock::ReplaceWithSameLength { .. } => action += 50,
		}
		match &other {
			DiffBlock::Skip { .. } => action += 1,
			DiffBlock::Add { .. } => action += 2,
			DiffBlock::Remove { .. } => action += 3,
			DiffBlock::Replace { .. } => action += 4,
			DiffBlock::ReplaceWithSameLength { .. } => action += 5,
		}
		return action;
	}

	pub fn diff(
		self,
		other: DiffBlock<'a, u32>,
	) -> (
		Option<DiffBlock<'a, u32>>,
		Option<DiffBlock<'a, u32>>,
		Option<DiffBlock<'a, u32>>,
	) {
		let action = self.get_action_number(&other);
		match action {
			// skip(x) | skip(y) =
			// 	= skip(x)
			// 	> skip(y) next(skip(x - y))
			// 	< skip(x) next(nil, skip(y - x))
			11 => match (self, other) {
				(DiffBlock::Skip { size: x }, DiffBlock::Skip { size: y }) => {
					match Cmp::cmp(x, y) {
						Cmp::Equal => return (Some(DiffBlock::Skip { size: x }), None, None),
						Cmp::Greater => {
							return (
								Some(DiffBlock::Skip { size: y }),
								Some(DiffBlock::Skip { size: x - y }),
								None,
							)
						}
						Cmp::Less => {
							return (
								Some(DiffBlock::Skip { size: x }),
								None,
								Some(DiffBlock::Skip { size: y - x }),
							)
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// skip(x) | add(y) =
			// 	= add(y) next(skip(x))
			12 => return (Some(other), Some(self), None),

			// skip(x) | remove(y) =
			// 	= remove(x)
			// 	> remove(y) next(skip(x - y))
			// 	< remove(x) next(nil , remove(y - x))
			13 => match (self, other) {
				(DiffBlock::Skip { size: x }, DiffBlock::Remove { size: y }) => {
					match Cmp::cmp(x, y) {
						Cmp::Equal => return (Some(DiffBlock::Remove { size: x }), None, None),
						Cmp::Greater => {
							return (
								Some(DiffBlock::Remove { size: y }),
								Some(DiffBlock::Skip { size: x - y }),
								None,
							)
						}
						Cmp::Less => {
							return (
								Some(DiffBlock::Remove { size: x }),
								None,
								Some(DiffBlock::Remove { size: y - x }),
							)
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// skip(x) | replace(y, z) =
			// 	x == y : remove(x) next(nil, add(z))
			// 	x > y  : remove(x) next(skip(x - y), add(z))
			// 	x < y  : remove(x) next(nil, replace(y - x, z))
			14 => match (self, other) {
				(
					DiffBlock::Skip { size: x },
					DiffBlock::Replace {
						remove_size: y,
						data: datab,
					},
				) => match Cmp::cmp(x, y) {
					Cmp::Equal => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Add { data: datab }),
						)
					}
					Cmp::Greater => {
						return (
							Some(DiffBlock::Remove { size: x }),
							Some(DiffBlock::Skip { size: x - y }),
							Some(DiffBlock::Add { data: datab }),
						)
					}
					Cmp::Less => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Replace {
								remove_size: y - x,
								data: datab,
							}),
						)
					}
				},
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// skip(x) | replace(y, z) =
			// 	x == y : remove(x) next(nil, add(z))
			// 	x > y  : remove(x) next(skip(x - y), add(z))
			// 	x < y  : remove(x) next(nil, replace(y - x, z))
			15 => match (self, other) {
				(
					DiffBlock::Skip { size: x },
					DiffBlock::ReplaceWithSameLength { data: mut datab },
				) => {
					let y = datab.size() as u32;
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							return (
								Some(DiffBlock::Remove { size: x }),
								None,
								Some(DiffBlock::Add { data: datab }),
							)
						}
						Cmp::Greater => {
							return (
								Some(DiffBlock::Remove { size: x }),
								Some(DiffBlock::Skip { size: x - y }),
								Some(DiffBlock::Add { data: datab }),
							)
						}
						Cmp::Less => {
							return (
								Some(DiffBlock::Remove { size: x }),
								None,
								Some(DiffBlock::Replace {
									remove_size: y - x,
									data: datab,
								}),
							)
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | skip(y) =
			// 	= add(x)
			// 	> add(y) next(add(y..x))
			// 	< add(x) next(nil , skip(y - x))
			21 => match (self, other) {
				(DiffBlock::Add { mut data }, DiffBlock::Skip { size: y }) => {
					let x = data.size() as u32;
					match Cmp::cmp(x, y) {
						Cmp::Equal => return (Some(DiffBlock::Add { data: data }), None, None),
						Cmp::Greater => {
							let da = ReadSlice::take(&mut data, y as u64);
							let db = data.offset(y as u64);
							return (
								Some(DiffBlock::Add { data: da }),
								Some(DiffBlock::Add { data: db }),
								None,
							);
						}
						Cmp::Less => {
							return (
								Some(DiffBlock::Add { data: data }),
								None,
								Some(DiffBlock::Skip { size: y - x }),
							)
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | add (y) = add(y) next(add(x))
			22 => return (Some(other), Some(self), None),

			// add(x) | remove(y) =
			// 	= nil
			// 	> next(add(y..x))
			// 	< next(nil, remove(y - x))
			23 => match (self, other) {
				(DiffBlock::Add { mut data }, DiffBlock::Remove { size: y }) => {
					let x = data.size() as u32;
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							return (None, None, None);
						}
						Cmp::Greater => {
							return (
								None,
								Some(DiffBlock::Add {
									data: data.offset(y as u64),
								}),
								None,
							);
						}
						Cmp::Less => {
							return (None, None, Some(DiffBlock::Remove { size: y - x }));
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | replace(y, z) =
			// 	x = y : next(nil, add(z))
			// 	x > y : add(z) next(add(y..x))
			// 	x < y : next(nil, replace(y - x, z))
			24 => match (self, other) {
				(
					DiffBlock::Add { mut data },
					DiffBlock::Replace {
						remove_size: y,
						data: mut datab,
					},
				) => {
					let x = data.size() as u32;
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							return (None, None, Some(DiffBlock::Add { data: datab }));
						}
						Cmp::Greater => {
							return (
								Some(DiffBlock::Add { data: datab }),
								Some(DiffBlock::Add {
									data: data.offset(y as u64),
								}),
								None,
							);
						}
						Cmp::Less => {
							return (
								None,
								None,
								Some(DiffBlock::Replace {
									remove_size: y - x,
									data: datab,
								}),
							);
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | replace(y, z) =
			// 	x = y : next(nil, add(z))
			// 	x > y : add(z) next(add(y..x))
			// 	x < y : next(nil, replace(y - x, z))
			25 => match (self, other) {
				(
					DiffBlock::Add { mut data },
					DiffBlock::ReplaceWithSameLength { data: mut datab },
				) => {
					let x = data.size();
					let y = datab.size();
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							return (None, None, Some(DiffBlock::Add { data: datab }));
						}
						Cmp::Greater => {
							return (
								Some(DiffBlock::Add { data: datab }),
								Some(DiffBlock::Add {
									data: data.offset(y as u64),
								}),
								None,
							);
						}
						Cmp::Less => {
							return (
								None,
								None,
								Some(DiffBlock::Replace {
									remove_size: (y - x) as u32,
									data: datab,
								}),
							);
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// remove(x) | skip(y) = remove(x) next(nil, skip(y))
			31 => return (Some(self), None, Some(other)),

			// remove(x) | add(y) = remove(x) nextb(nil, add(y))
			32 => return (Some(self), None, Some(other)),

			// remove(x) | remove(y) = remove(x) nextb(nil, remove(y))
			33 => return (Some(self), None, Some(other)),

			// remove(x) | replace(y, z) = remove(x) nextb(nil, replace(y, z))
			34 => return (Some(self), None, Some(other)),

			// remove(x) | replace(y, z) = remove(x) nextb(nil, replace(y, z))
			35 => return (Some(self), None, Some(other)),

			// replace(x, y) | skip(z) = remove(x) next(add(y), skip(z))
			41...45 => match self {
				DiffBlock::Replace { remove_size, data } => {
					return (
						Some(DiffBlock::Remove { size: remove_size }),
						Some(DiffBlock::Add { data: data }),
						Some(other),
					)
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// replace(x, y) | skip(z) = remove(x) next(add(y), skip(z))
			51...55 => match self {
				DiffBlock::ReplaceWithSameLength { mut data } => {
					let size = data.size();
					return (
						Some(DiffBlock::Remove { size: size as u32 }),
						Some(DiffBlock::Add { data: data }),
						Some(other),
					);
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			_ => panic!("Unknown Action"),
		}
	}
}

impl<'a> Add for DiffBlock<'a, u32> {
	type Output = (DiffBlock<'a, u32>, Option<DiffBlock<'a, u32>>);

	fn add(self, other: DiffBlock<'a, u32>) -> Self::Output {
		let action = self.get_action_number(&other);
		match action {
			// skip
			// skip(x) + skip(y) = skip(x + y)
			11 => match (self, other) {
				(DiffBlock::Skip { size: s1 }, DiffBlock::Skip { size: s2 }) => {
					return (DiffBlock::Skip { size: s1 + s2 }, None);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// skip(x) + add(y) = skip(x) add(y)
			12 => return (self, Some(other)),
			// skip(x) + remove(y)     = skip(x) remove(y)
			13 => return (self, Some(other)),
			// skip(x) + replace(y, z) = skip(x) replace(y, z)
			14 => return (self, Some(other)),
			// skip(x) + replace(y, z) = skip(x) replace(y, z)
			15 => return (self, Some(other)),

			// add
			// add(x) + skip(y)       = add(x) skip(y)
			21 => return (self, Some(other)),
			// add(x) + add(y)        = add(x + y)
			22 => match (self, other) {
				(DiffBlock::Add { mut data }, DiffBlock::Add { data: mut datab }) => {
					return (
						DiffBlock::Add {
							data: ReadSlice::chain(&mut data, datab),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// add(x) + remove(y)     = add(x) remove(y)
			23 => return (self, Some(other)),
			// add(x) + replace(y, z) = add(x) replace(y, z)
			24 => return (self, Some(other)),
			// add(x) + replace(y, z) = add(x) replace(y, z)
			25 => return (self, Some(other)),

			// remove
			// remove(x) + skip(y) = remove(x) skip(y)
			31 => return (self, Some(other)),
			// remove(x) + add(y) = replace(x, y)
			32 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::Add { mut data }) => {
					let sizeb = data.size() as u32;
					if size == sizeb {
						return (DiffBlock::ReplaceWithSameLength { data: data }, None);
					}
					return (
						DiffBlock::Replace {
							remove_size: size,
							data: data,
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + remove(y) = remove(x + y)
			33 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::Remove { size: sizeb }) => {
					return (DiffBlock::Remove { size: size + sizeb }, None);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + replace(y, z) = replace(x + y, z)
			34 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::Replace { remove_size, data }) => {
					return (
						DiffBlock::Replace {
							remove_size: size + remove_size,
							data: data,
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + replace(y, z) = replace(x + y, z)
			35 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::ReplaceWithSameLength { mut data }) => {
					let sizeb = data.size() as u32;
					return (
						DiffBlock::Replace {
							remove_size: size + sizeb,
							data: data,
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},

			// replace
			// replace(x, y) + skip(z)       = replace(x, y) skip(z)
			41 => return (self, Some(other)),
			// replace(x, y) + add(z)        = replace(x, y + z)
			42 => match (self, other) {
				(
					DiffBlock::Replace {
						remove_size,
						mut data,
					},
					DiffBlock::Add { data: mut datab },
				) => {
					let size = data.size() as u32;
					let sizeb = datab.size() as u32;
					if remove_size == size + sizeb {
						return (
							DiffBlock::ReplaceWithSameLength {
								data: ReadSlice::chain(&mut data, datab),
							},
							None,
						);
					}
					return (
						DiffBlock::Replace {
							remove_size: remove_size,
							data: ReadSlice::chain(&mut data, datab),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// replace(x, y) + remove(z)     = replace(x, y) remove(z)
			43 => return (self, Some(other)),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			44 => return (self, Some(other)),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			45 => return (self, Some(other)),

			// replace with same length
			// replace(x, y) + skip(z)       = replace(x, y) skip(z)
			51 => return (self, Some(other)),
			// replace(x, y) + add(z)        = replace(x, y + z)
			52 => match (self, other) {
				(
					DiffBlock::ReplaceWithSameLength { mut data },
					DiffBlock::Add { data: mut datab },
				) => {
					let size = data.size() as u32;
					return (
						DiffBlock::Replace {
							remove_size: size,
							data: ReadSlice::chain(&mut data, datab),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// replace(x, y) + remove(z)     = replace(x, y) remove(z)
			53 => return (self, Some(other)),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			54 => return (self, Some(other)),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			55 => return (self, Some(other)),
			_ => panic!("Unknown action"),
		}
	}
}

impl<'a> IntoBytesSerializer for DiffBlock<'a, u32> {
	type Item = DiffBlock<'a, u32>;

	fn into_bytes(self) -> BytesSerializer<Self::Item> {
		return BytesSerializer::new(
			self,
			Box::new(
				|position: &mut usize, val, mut buffer: &mut [u8]| match val {
					DiffBlock::Skip { size } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&0u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&size)[..]);
							let res = Cursor::new(&bytes[*position..]).read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return Ok(0);
						}
					}
					DiffBlock::Add { ref mut data } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&1u16)[..]);
							bytes[2..6]
								.clone_from_slice(&u32_to_u8_be_vec(&(data.size() as u32))[..]);
							let res = Cursor::new(&bytes[*position..])
								.chain(data)
								.read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return data.read(&mut buffer);
						}
					}
					DiffBlock::Remove { size } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&2u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&size)[..]);
							let res = Cursor::new(&bytes[*position..]).read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return Ok(0);
						}
					}
					DiffBlock::Replace {
						remove_size,
						ref mut data,
					} => {
						if *position < 10 {
							let mut bytes = &mut [0u8; 2 + 4 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&3u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&remove_size)[..]);
							bytes[6..10]
								.clone_from_slice(&u32_to_u8_be_vec(&(data.size() as u32))[..]);
							let res = Cursor::new(&bytes[*position..])
								.chain(data)
								.read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return data.read(&mut buffer);
						}
					}
					DiffBlock::ReplaceWithSameLength { ref mut data } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&4u16)[..]);
							bytes[2..6]
								.clone_from_slice(&u32_to_u8_be_vec(&(data.size() as u32))[..]);
							let res = Cursor::new(&bytes[*position..])
								.chain(data)
								.read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return data.read(&mut buffer);
						}
					}
				},
			),
		);
	}
}

#[cfg(test)]
mod diff_block_tests {
	use super::DiffBlock;
	use bytes_serializer::IntoBytesSerializer;
	use readslice::ReadSlice;
	use std::io::{copy, Cursor, Read};

	#[test]
	fn diff_block_diff_add_greater_skip_test() {
		let data = Cursor::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		let da = DiffBlock::Add {
			data: ReadSlice::new(data),
		};
		let db: DiffBlock<u32> = DiffBlock::Skip { size: 4u32 };
		let op = da.diff(db);
		match op {
			(Some(DiffBlock::Add { mut data }), Some(DiffBlock::Add { data: mut datab }), None) => {
				let mut buf = vec![];
				copy(&mut data, &mut buf).unwrap();
				assert_eq!(buf, [1, 2, 3, 4]);
				copy(&mut datab, &mut buf).unwrap();
				assert_eq!(data.size(), 4);
				assert_eq!(datab.size(), 6);
				assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
			}
			_ => panic!("wrong operation result"),
		}
	}

	#[test]
	fn diffblock_read_test() {
		let data = Cursor::new([1, 2, 3, 4, 5, 6]);
		let block = DiffBlock::Add {
			data: ReadSlice::new(data),
		};
		let mut buf = vec![0; 2 + 4 + 6];
		block.into_bytes().read_exact(&mut buf).unwrap();
		assert_eq!(
			buf,
			[
				0x00, 0x01, // action
				0x00, 0x00, 0x00, 6, //size
				1, 2, 3, 4, 5, 6 // data
			]
		);
	}
}
