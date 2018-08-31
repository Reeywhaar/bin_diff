use bytes_serializer::{BytesSerializer, IntoBytesSerializer};
use clonable_read::ClonableRead;
use cmp::Cmp;
use drain::Drainable;
use functions::{u16_to_u8_be_vec, u32_to_u8_be_vec};
use std::fmt;
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

#[derive(Clone)]
pub enum DiffBlock<T: Add + AddAssign + Sub + SubAssign, U: Read> {
	Skip { size: T },
	Add { size: T, data: U },
	Remove { size: T },
	Replace { replace_size: T, size: T, data: U },
	ReplaceWithSameLength { size: T, data: U },
}

impl<U: 'static + Read> fmt::Debug for DiffBlock<u32, U> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			DiffBlock::Skip { size } => write!(f, "DiffBlock::Skip {{ size: {} }}", size),
			DiffBlock::Add { size, data: _ } => write!(f, "DiffBlock::Add {{ size: {} }}", size),
			DiffBlock::Remove { size } => write!(f, "DiffBlock::Remove {{ size: {} }}", size),
			DiffBlock::Replace {
				replace_size,
				size,
				data: _,
			} => write!(
				f,
				"DiffBlock::Replace {{ replace_size: {}, size: {} }}",
				replace_size, size
			),
			DiffBlock::ReplaceWithSameLength { size, data: _ } => {
				write!(f, "DiffBlock::ReplaceWithSameLength {{ size: {} }}", size)
			}
		}
	}
}

impl<U: 'static + Read> DiffBlock<u32, U> {
	pub fn as_boxed(self) -> DiffBlock<u32, Box<dyn Read>> {
		match self {
			DiffBlock::Skip { size } => {
				return DiffBlock::Skip { size };
			}
			DiffBlock::Add { size, data } => {
				return DiffBlock::Add {
					size,
					data: Box::new(data),
				};
			}
			DiffBlock::Remove { size } => {
				return DiffBlock::Remove { size };
			}
			DiffBlock::Replace {
				replace_size,
				size,
				data,
			} => {
				return DiffBlock::Replace {
					replace_size,
					size,
					data: Box::new(data),
				};
			}
			DiffBlock::ReplaceWithSameLength { size, data } => {
				return DiffBlock::ReplaceWithSameLength {
					size,
					data: Box::new(data),
				};
			}
		}
	}

	fn get_action_number<W: 'static + Read>(&self, other: &DiffBlock<u32, W>) -> u16 {
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

	pub fn diff<W: 'static + Read>(
		self,
		other: DiffBlock<u32, W>,
	) -> (
		Option<DiffBlock<u32, Box<dyn Read>>>,
		Option<DiffBlock<u32, Box<dyn Read>>>,
		Option<DiffBlock<u32, Box<dyn Read>>>,
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
			12 => return (Some(other.as_boxed()), Some(self.as_boxed()), None),

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
						replace_size: y,
						size: z,
						data,
					},
				) => match Cmp::cmp(x, y) {
					Cmp::Equal => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Add {
								size: z,
								data: Box::new(data),
							}),
						)
					}
					Cmp::Greater => {
						return (
							Some(DiffBlock::Remove { size: x }),
							Some(DiffBlock::Skip { size: x - y }),
							Some(DiffBlock::Add {
								size: z,
								data: Box::new(data),
							}),
						)
					}
					Cmp::Less => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Replace {
								replace_size: y - x,
								size: x,
								data: Box::new(data),
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
					DiffBlock::ReplaceWithSameLength { size: y, data },
				) => match Cmp::cmp(x, y) {
					Cmp::Equal => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Add {
								size: y,
								data: Box::new(data),
							}),
						)
					}
					Cmp::Greater => {
						return (
							Some(DiffBlock::Remove { size: x }),
							Some(DiffBlock::Skip { size: x - y }),
							Some(DiffBlock::Add {
								size: y,
								data: Box::new(data),
							}),
						)
					}
					Cmp::Less => {
						return (
							Some(DiffBlock::Remove { size: x }),
							None,
							Some(DiffBlock::Replace {
								replace_size: y - x,
								size: x,
								data: Box::new(data),
							}),
						)
					}
				},
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | skip(y) =
			// 	= add(x)
			// 	> add(y) next(add(y..x))
			// 	< add(x) next(nil , skip(y - x))
			21 => match (self, other) {
				(DiffBlock::Add { size: x, data }, DiffBlock::Skip { size: y }) => {
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							return (
								Some(DiffBlock::Add {
									size: x,
									data: Box::new(data),
								}),
								None,
								None,
							)
						}
						Cmp::Greater => {
							let da = ClonableRead::new(data);
							let db = da.clone();
							return (
								Some(DiffBlock::Add {
									size: y,
									data: Box::new(da.take(y as u64)),
								}),
								Some(DiffBlock::Add {
									size: x - y,
									data: Box::new(db.take((x - y) as u64)),
								}),
								None,
							);
						}
						Cmp::Less => {
							return (
								Some(DiffBlock::Add {
									size: x,
									data: Box::new(data),
								}),
								None,
								Some(DiffBlock::Skip { size: y - x }),
							)
						}
					}
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | add (y) = add(y) next(add(x))
			22 => match (self, other) {
				(
					DiffBlock::Add { size: x, data },
					DiffBlock::Add {
						size: y,
						data: datab,
					},
				) => {
					return (
						Some(DiffBlock::Add {
							size: y,
							data: Box::new(datab),
						}),
						Some(DiffBlock::Add {
							size: x,
							data: Box::new(data),
						}),
						None,
					);
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | remove(y) =
			// 	= nil
			// 	> next(add(y..x))
			// 	< next(nil, remove(y - x))
			23 => match (self, other) {
				(DiffBlock::Add { size: x, data }, DiffBlock::Remove { size: y }) => {
					match Cmp::cmp(x, y) {
						Cmp::Equal => {
							data.drain(x as u64).get_drained().unwrap();
							return (None, None, None);
						}
						Cmp::Greater => {
							return (
								None,
								Some(DiffBlock::Add {
									size: x - y,
									data: Box::new(data.drain(y as u64)),
								}),
								None,
							);
						}
						Cmp::Less => {
							data.drain(x as u64).get_drained().unwrap();
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
					DiffBlock::Add { size: x, data },
					DiffBlock::Replace {
						replace_size: y,
						size: z,
						data: odata,
					},
				) => match Cmp::cmp(x, y) {
					Cmp::Equal => {
						data.drain(x as u64).get_drained().unwrap();
						return (
							None,
							None,
							Some(DiffBlock::Add {
								size: z,
								data: Box::new(odata),
							}),
						);
					}
					Cmp::Greater => {
						return (
							Some(DiffBlock::Add {
								size: z,
								data: Box::new(odata),
							}),
							Some(DiffBlock::Add {
								size: y - x,
								data: Box::new(data.drain(y as u64)),
							}),
							None,
						);
					}
					Cmp::Less => {
						data.drain(x as u64).get_drained().unwrap();
						return (
							None,
							None,
							Some(DiffBlock::Replace {
								replace_size: y - x,
								size: z,
								data: Box::new(odata),
							}),
						);
					}
				},
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// add(x) | replace(y, z) =
			// 	x = y : next(nil, add(z))
			// 	x > y : add(z) next(add(y..x))
			// 	x < y : next(nil, replace(y - x, z))
			25 => match (self, other) {
				(
					DiffBlock::Add { size: x, data },
					DiffBlock::ReplaceWithSameLength {
						size: y,
						data: odata,
					},
				) => match Cmp::cmp(x, y) {
					Cmp::Equal => {
						data.drain(x as u64).get_drained().unwrap();
						return (
							None,
							None,
							Some(DiffBlock::Add {
								size: y,
								data: Box::new(odata),
							}),
						);
					}
					Cmp::Greater => {
						return (
							Some(DiffBlock::Add {
								size: y,
								data: Box::new(odata),
							}),
							Some(DiffBlock::Add {
								size: x - y,
								data: Box::new(data.drain(y as u64)),
							}),
							None,
						);
					}
					Cmp::Less => {
						data.drain(x as u64).get_drained().unwrap();
						return (
							None,
							None,
							Some(DiffBlock::Replace {
								replace_size: y - x,
								size: y,
								data: Box::new(odata),
							}),
						);
					}
				},
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// remove(x) | skip(y) = remove(x) next(nil, skip(y))
			31 => return (Some(self.as_boxed()), None, Some(other.as_boxed())),

			// remove(x) | add(y) = remove(x) nextb(nil, add(y))
			32 => return (Some(self.as_boxed()), None, Some(other.as_boxed())),

			// remove(x) | remove(y) = remove(x) nextb(nil, remove(y))
			33 => return (Some(self.as_boxed()), None, Some(other.as_boxed())),

			// remove(x) | replace(y, z) = remove(x) nextb(nil, replace(y, z))
			34 => return (Some(self.as_boxed()), None, Some(other.as_boxed())),

			// remove(x) | replace(y, z) = remove(x) nextb(nil, replace(y, z))
			35 => return (Some(self.as_boxed()), None, Some(other.as_boxed())),

			// replace(x, y) | skip(z) = remove(x) next(add(y), skip(z))
			41...45 => match self {
				DiffBlock::Replace {
					replace_size,
					size,
					data,
				} => {
					return (
						Some(DiffBlock::Remove { size: replace_size }),
						Some(DiffBlock::Add {
							size,
							data: Box::new(data),
						}),
						Some(other.as_boxed()),
					)
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			// replace(x, y) | skip(z) = remove(x) next(add(y), skip(z))
			51...55 => match self {
				DiffBlock::ReplaceWithSameLength { size, data } => {
					return (
						Some(DiffBlock::Remove { size: size }),
						Some(DiffBlock::Add {
							size,
							data: Box::new(data),
						}),
						Some(other.as_boxed()),
					)
				}
				_ => panic!("DiffBlock diff unwrap failed"),
			},

			_ => panic!("Unknown Action"),
		}
	}
}

impl<T: 'static + Read> Add for DiffBlock<u32, T> {
	type Output = (
		DiffBlock<u32, Box<dyn Read>>,
		Option<DiffBlock<u32, Box<dyn Read>>>,
	);

	fn add(self, other: DiffBlock<u32, T>) -> Self::Output {
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
			12 => return (self.as_boxed(), Some(other.as_boxed())),
			// skip(x) + remove(y)     = skip(x) remove(y)
			13 => return (self.as_boxed(), Some(other.as_boxed())),
			// skip(x) + replace(y, z) = skip(x) replace(y, z)
			14 => return (self.as_boxed(), Some(other.as_boxed())),
			// skip(x) + replace(y, z) = skip(x) replace(y, z)
			15 => return (self.as_boxed(), Some(other.as_boxed())),

			// add
			// add(x) + skip(y)       = add(x) skip(y)
			21 => return (self.as_boxed(), Some(other.as_boxed())),
			// add(x) + add(y)        = add(x + y)
			22 => match (self, other) {
				(
					DiffBlock::Add { size, data },
					DiffBlock::Add {
						size: osize,
						data: odata,
					},
				) => {
					let data = data.chain(odata);
					return (
						DiffBlock::Add {
							size: size + osize,
							data: Box::new(data),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// add(x) + remove(y)     = add(x) remove(y)
			23 => return (self.as_boxed(), Some(other.as_boxed())),
			// add(x) + replace(y, z) = add(x) replace(y, z)
			24 => return (self.as_boxed(), Some(other.as_boxed())),
			// add(x) + replace(y, z) = add(x) replace(y, z)
			25 => return (self.as_boxed(), Some(other.as_boxed())),

			// remove
			// remove(x) + skip(y) = remove(x) skip(y)
			31 => return (self.as_boxed(), Some(other.as_boxed())),
			// remove(x) + add(y) = replace(x, y)
			32 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::Add { size: osize, data }) => {
					if size == osize {
						return (
							DiffBlock::ReplaceWithSameLength {
								size: size,
								data: Box::new(data),
							},
							None,
						);
					}
					return (
						DiffBlock::Replace {
							replace_size: size,
							size: osize,
							data: Box::new(data),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + remove(y) = remove(x + y)
			33 => match (self, other) {
				(DiffBlock::Remove { size }, DiffBlock::Remove { size: osize }) => {
					return (DiffBlock::Remove { size: size + osize }, None);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + replace(y, z) = replace(x + y, z)
			34 => match (self, other) {
				(
					DiffBlock::Remove { size },
					DiffBlock::Replace {
						replace_size,
						size: osize,
						data,
					},
				) => {
					return (
						DiffBlock::Replace {
							replace_size: size + replace_size,
							size: osize,
							data: Box::new(data),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// remove(x) + replace(y, z) = replace(x + y, z)
			35 => match (self, other) {
				(
					DiffBlock::Remove { size },
					DiffBlock::ReplaceWithSameLength { size: osize, data },
				) => {
					return (
						DiffBlock::Replace {
							replace_size: size + osize,
							size: osize,
							data: Box::new(data),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},

			// replace
			// replace(x, y) + skip(z)       = replace(x, y) skip(z)
			41 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + add(z)        = replace(x, y + z)
			42 => match (self, other) {
				(
					DiffBlock::Replace {
						replace_size,
						size,
						data,
					},
					DiffBlock::Add {
						size: osize,
						data: odata,
					},
				) => {
					if replace_size == size + osize {
						return (
							DiffBlock::ReplaceWithSameLength {
								size: replace_size,
								data: Box::new(data.chain(odata)),
							},
							None,
						);
					}
					return (
						DiffBlock::Replace {
							replace_size: replace_size,
							size: size + osize,
							data: Box::new(data.chain(odata)),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// replace(x, y) + remove(z)     = replace(x, y) remove(z)
			43 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			44 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			45 => return (self.as_boxed(), Some(other.as_boxed())),

			// replace with same length
			// replace(x, y) + skip(z)       = replace(x, y) skip(z)
			51 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + add(z)        = replace(x, y + z)
			52 => match (self, other) {
				(
					DiffBlock::ReplaceWithSameLength { size, data },
					DiffBlock::Add {
						size: osize,
						data: odata,
					},
				) => {
					return (
						DiffBlock::Replace {
							replace_size: size,
							size: size + osize,
							data: Box::new(data.chain(odata)),
						},
						None,
					);
				}
				_ => panic!("DiffBlock unwrap failed"),
			},
			// replace(x, y) + remove(z)     = replace(x, y) remove(z)
			53 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			54 => return (self.as_boxed(), Some(other.as_boxed())),
			// replace(x, y) + replace(z, w) = replace(x, y) replace(z, w)
			55 => return (self.as_boxed(), Some(other.as_boxed())),
			_ => panic!("Unknown action"),
		}
	}
}

impl<U: Read> IntoBytesSerializer for DiffBlock<u32, U> {
	type Item = DiffBlock<u32, U>;

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
					DiffBlock::Add { size, ref mut data } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&1u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&size)[..]);
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
						replace_size,
						size,
						ref mut data,
					} => {
						if *position < 10 {
							let mut bytes = &mut [0u8; 2 + 4 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&3u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&replace_size)[..]);
							bytes[6..10].clone_from_slice(&u32_to_u8_be_vec(&size)[..]);
							let res = Cursor::new(&bytes[*position..])
								.chain(data)
								.read(&mut buffer)?;
							*position += res;
							return Ok(res);
						} else {
							return data.read(&mut buffer);
						}
					}
					DiffBlock::ReplaceWithSameLength { size, ref mut data } => {
						if *position < 6 {
							let mut bytes = &mut [0u8; 2 + 4][..];
							bytes[0..2].clone_from_slice(&u16_to_u8_be_vec(&4u16)[..]);
							bytes[2..6].clone_from_slice(&u32_to_u8_be_vec(&size)[..]);
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
	use super::super::bytes_serializer::IntoBytesSerializer;
	use super::DiffBlock;
	use std::io::{copy, Cursor, Read};

	#[test]
	fn diff_block_diff_add_greater_skip_test() {
		let data = Cursor::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		let da = DiffBlock::Add {
			size: 10u32,
			data: data,
		};
		let db: DiffBlock<u32, &mut Read> = DiffBlock::Skip { size: 4u32 };
		let op = da.diff(db);
		match op {
			(
				Some(DiffBlock::Add { size, mut data }),
				Some(DiffBlock::Add {
					size: osize,
					data: mut odata,
				}),
				None,
			) => {
				let mut buf = vec![];
				copy(&mut data, &mut buf).unwrap();
				assert_eq!(buf, [1, 2, 3, 4]);
				copy(&mut odata, &mut buf).unwrap();
				assert_eq!(size, 4);
				assert_eq!(osize, 6);
				assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
			}
			_ => panic!("wrong operation result"),
		}
	}

	#[test]
	fn diffblock_read_test() {
		let data = Cursor::new([1, 2, 3, 4, 5, 6]);
		let block = DiffBlock::Add {
			size: 6,
			data: data,
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
