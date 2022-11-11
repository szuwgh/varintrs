# varintrs
A Rust implementation of Golang Variable-Length Integers


Notes：
========
This is a rust implementation of golang variable-length integers. Variable-length integers are usually used in writing database storage, which can compress integer data storage and save disk space.


For example:
```rust
use std::io::Cursor;
use varintrs::{Binary, ReadBytesVarExt};

let mut rdr = Cursor::new(vec![228, 211, 247, 161, 22]);
let (v, x) = rdr.read_vu64::<Binary>();
assert_eq!(5976746468, v);
assert_eq!(5, x);
```


```rust
use std::io::Cursor;
use varintrs::{Binary,WriteBytesVarExt};

let mut rdr = Cursor::new(vec![0u8; 7]);
rdr.write_vu64::<Binary>(88748464645454).unwrap();
assert!(rdr.get_ref().eq(&vec![206, 202, 214, 229, 245, 150, 20]));
```