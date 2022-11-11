use std::io;
use std::io::Cursor;
use std::io::Result;

// MaxVarintLenN is the maximum length of a varint-encoded N-bit integer.
pub const MAX_VARINT_LEN16: usize = 3;
pub const MAX_VARINT_LEN32: usize = 5;
pub const MAX_VARINT_LEN64: usize = 10;

pub trait WriteBinary {
    fn put_vu64(buf: &mut [u8], x: u64) -> usize;
    fn put_vi64(buf: &mut [u8], x: i64) -> usize;
}

pub trait ReadBinary {
    fn vu64(buf: &[u8]) -> (u64, i32);
    fn vi64(buf: &[u8]) -> (i64, i32);
    fn read_vu64<T: ReadU8 + ?Sized>(t: &mut T) -> (u64, i32);
    fn read_vi64<T: ReadU8 + ?Sized>(t: &mut T) -> (i64, i32);
}

pub enum Binary {}

impl WriteBinary for Binary {
    // PutUvarint encodes a uint64 into buf and returns the number of bytes written.
    // If the buffer is too small, PutUvarint will panic.
    #[inline]
    fn put_vu64(buf: &mut [u8], mut x: u64) -> usize {
        let mut i: usize = 0;
        while x >= 0x80 {
            buf[i] = x as u8 | 0x80;
            x >>= 7;
            i += 1;
        }
        buf[i] = x as u8;
        i + 1
    }

    // PutVarint encodes an int64 into buf and returns the number of bytes written.
    // If the buffer is too small, PutVarint will panic.
    #[inline]
    fn put_vi64(buf: &mut [u8], x: i64) -> usize {
        let mut ux = (x as u64) << 1;
        if x < 0 {
            ux = !ux;
        }
        Self::put_vu64(buf, ux)
    }
}

impl ReadBinary for Binary {
    // Uvarint decodes a uint64 from buf and returns that value and the
    // number of bytes read (> 0). If an error occurred, the value is 0
    // and the number of bytes n is <= 0 meaning:
    //
    // 	n == 0: buf too small
    // 	n  < 0: value larger than 64 bits (overflow)
    // 	        and -n is the number of bytes read
    //
    #[inline]
    fn vu64(buf: &[u8]) -> (u64, i32) {
        Self::read_vu64(&mut Cursor::new(buf))
    }

    // Varint decodes an int64 from buf and returns that value and the
    // number of bytes read (> 0). If an error occurred, the value is 0
    // and the number of bytes n is <= 0 with the following meaning:
    //
    // 	n == 0: buf too small
    // 	n  < 0: value larger than 64 bits (overflow)
    // 	        and -n is the number of bytes read
    //
    #[inline]
    fn vi64(buf: &[u8]) -> (i64, i32) {
        let (ux, n) = Self::vu64(buf);
        let mut x = (ux >> 1) as i64;
        if ux & 1 != 0 {
            x = !x;
        }
        (x, n)
    }

    #[inline]
    fn read_vu64<T: ReadU8 + ?Sized>(t: &mut T) -> (u64, i32) {
        let mut x: u64 = 0;
        let mut s: u32 = 0;
        let mut i: usize = 0;
        while let Ok(b) = t.read_u8() {
            if i == MAX_VARINT_LEN64 {
                // Catch byte reads past MaxVarintLen64.
                return (0, -(i as i32 + 1));
            }
            if b < 0x80 {
                if i == MAX_VARINT_LEN64 - 1 && b > 1 {
                    return (0, -(i as i32 + 1)); // overflow
                }
                return (x | (b as u64) << s, i as i32 + 1);
            }
            x |= ((b & 0x7f) as u64) << s;
            s += 7;
            i += 1;
        }
        (0, 0)
    }

    #[inline]
    fn read_vi64<T: ReadU8 + ?Sized>(t: &mut T) -> (i64, i32) {
        let (ux, n) = Self::read_vu64(t);
        let mut x = (ux >> 1) as i64;
        if ux & 1 != 0 {
            x = !x;
        }
        (x, n)
    }
}

pub trait WriteBytesVarExt: io::Write {
    #[inline]
    fn write_vu64<T: WriteBinary>(&mut self, x: u64) -> Result<usize> {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        let i = T::put_vu64(&mut buf, x);
        self.write_all(&buf[..i])?;
        Ok(i)
    }

    #[inline]
    fn write_vi64<T: WriteBinary>(&mut self, x: i64) -> Result<usize> {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        let i = T::put_vi64(&mut buf, x);
        self.write_all(&buf[..i])?;
        Ok(i)
    }
}

pub trait ReadU8: io::Read {
    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

pub trait ReadBytesVarExt: ReadU8 {
    #[inline]
    fn read_vu64<T: ReadBinary>(&mut self) -> (u64, i32) {
        T::read_vu64(self)
    }

    fn read_vi64<T: ReadBinary>(&mut self) -> (i64, i32) {
        T::read_vi64(self)
    }
}

impl<R: io::Read + ?Sized> ReadU8 for R {}

impl<W: io::Write + ?Sized> WriteBytesVarExt for W {}

impl<R: ReadU8 + ?Sized> ReadBytesVarExt for R {}

#[cfg(test)]
mod tests {

    const ivar_test: [i64; 44] = [
        -1,
        -2,
        -10,
        -20,
        -63,
        -64,
        -65,
        -127,
        -128,
        -129,
        -255,
        -256,
        -257,
        -517,
        -768,
        -5976746468,
        -88748464645454,
        -5789627789625558,
        -18446744073709551,
        -184467440737095516,
        -1844674407370955161,
        0,
        1,
        2,
        10,
        20,
        63,
        64,
        65,
        127,
        128,
        129,
        255,
        256,
        257,
        517,
        768,
        5976746468,
        88748464645454,
        5789627789625558,
        18446744073709551,
        184467440737095516,
        1844674407370955161,
        1 << 63 - 1,
    ];

    const uvar_test: [u64; 24] = [
        0,
        1,
        2,
        10,
        20,
        63,
        64,
        65,
        127,
        128,
        129,
        255,
        256,
        257,
        517,
        768,
        5976746468,
        88748464645454,
        5789627789625558,
        18446744073709551,
        184467440737095516,
        1844674407370955161,
        18446744073709551615,
        1 << 64 - 1,
    ];
    use super::*;
    #[test]
    fn test_uvarint64() {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        for x in uvar_test {
            Binary::put_vu64(&mut buf, x);
            println!("{:?}", buf);
            let (v, _) = Binary::vu64(&buf);
            assert!(x == v);
        }
    }

    #[test]
    fn test_varint64() {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        for x in ivar_test {
            Binary::put_vi64(&mut buf, x);
            println!("{}:{:?}", x, buf);
            let (v, _) = Binary::vi64(&buf);
            assert!(x == v);
        }
    }

    use std::io::Cursor;
    #[test]
    fn test_read_uvarint64() {
        let mut rdr = Cursor::new(vec![0u8; MAX_VARINT_LEN64]);

        for x in uvar_test {
            rdr.write_vu64::<Binary>(x).unwrap();
            rdr.set_position(0);
            let (v, _) = rdr.read_vu64::<Binary>();
            rdr.set_position(0);
            assert!(x == v);
        }
    }

    #[test]
    fn test_read_varint64() {
        let mut rdr = Cursor::new(vec![0u8; MAX_VARINT_LEN64]);
        for x in ivar_test {
            rdr.write_vi64::<Binary>(x).unwrap();
            rdr.set_position(0);
            let (v, _) = rdr.read_vi64::<Binary>();
            rdr.set_position(0);
            assert!(x == v);
        }
    }
}
