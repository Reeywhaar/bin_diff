extern crate difference;
extern crate sha2;

#[cfg(test)]
mod test_mod;

pub mod bytes_serializer;
mod cmp;
pub mod diff;
pub mod diffblock;
pub mod drain;
pub mod functions;
pub mod indexes;
pub mod readseek;
pub mod readslice;
