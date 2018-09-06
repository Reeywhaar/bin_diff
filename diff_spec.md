# Binary diff format specification

Binary diff consists of blocks followed each by another. Each block have 2 byte `action` and variable data. Format is BigEndian.

Binary diff is a metaformat and is not intended for bare use, therefore its binary representation doesn't contain any headers, signatures. Also package doens't contain any executables.
The reason for this format is to create with it subformats for each specific binary format specifications such as psd (my main reason), doc, zip, etc..

```
block_{n} : {...}
action: 2 // BE u16
			// 0 - skip
			// 1 - add
			// 2 - remove
			// 3 - replace
			// 4 - replace with same length
  # if action == 0 :
    data_length : 4 // BE u32
  # if action == 1 :
    data_length : 4 // BE u32
    data : data_length
  # if action == 2 :
    data_length : 4 // BE u32
  # if action == 3 :
    remove_length : 4 // BE u32
    data_length : 4 // BE u32
    data : data_length
  # if action == 4 :
    data_length : 4 // BE u32
    data : data_length
```
