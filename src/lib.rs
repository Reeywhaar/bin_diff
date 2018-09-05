extern crate difference;
extern crate sha2;

#[cfg(test)]
mod test_mod;

mod bytes_serializer;
mod cmp;
pub mod diff;
mod diff_block;
mod diff_iterator;
mod diff_reader;
mod drain;
pub mod functions;
pub mod indexes;
mod lines_with_hash_iterator;
mod readseek;
mod readslice;
