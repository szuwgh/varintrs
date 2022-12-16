use std::io;
use std::io::Cursor;
use std::io::Result;

// MaxVarintLenN is the maximum length of a varint-encoded N-bit integer.
pub const MAX_VARINT_LEN16: usize = 3;
pub const MAX_VARINT_LEN32: usize = 5;
pub const MAX_VARINT_LEN64: usize = 10;

pub const CONTINUATION_BIT: u8 = 1 << 7;

#[inline]
pub fn low_bits_of_byte(byte: u8) -> u8 {
    byte & !CONTINUATION_BIT
}

#[inline]
pub fn low_bits_of_u64(val: u64) -> u8 {
    let byte = val & (std::u8::MAX as u64);
    low_bits_of_byte(byte as u8)
}

pub trait WriteBinary {
    fn put_vu64(buf: &mut [u8], x: u64) -> usize;
    fn put_vi64(buf: &mut [u8], x: i64) -> usize;
    fn put_leb128_u64(buf: &mut [u8], x: u64) -> usize;
    fn put_leb128_i64(buf: &mut [u8], x: i64) -> usize;
}

pub trait ReadBinary {
    fn vu64(buf: &[u8]) -> (u64, i32);
    fn vi64(buf: &[u8]) -> (i64, i32);
    fn read_vu64<T: ReadU8 + ?Sized>(t: &mut T) -> (u64, i32);
    fn read_vi64<T: ReadU8 + ?Sized>(t: &mut T) -> (i64, i32);
    fn read_leb128_i64<T: ReadU8 + ?Sized>(t: &mut T) -> Result<i64>;
    fn read_leb128_u64<T: ReadU8 + ?Sized>(t: &mut T) -> Result<u64>;
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

    #[inline]
    fn put_leb128_u64(buf: &mut [u8], mut x: u64) -> usize {
        let mut i = 0;
        while x > 0 {
            let mut byte = low_bits_of_u64(x);
            x >>= 7;
            if x != 0 {
                byte |= CONTINUATION_BIT;
            }
            buf[i] = byte;
            i += 1;
        }
        i
    }

    #[inline]
    fn put_leb128_i64(buf: &mut [u8], mut x: i64) -> usize {
        let mut i = 0;
        loop {
            let mut byte = x as u8;
            x >>= 6;
            let done = x == 0 || x == -1;
            if done {
                byte &= !CONTINUATION_BIT;
            } else {
                x >>= 1;
                byte |= CONTINUATION_BIT;
            }
            buf[i] = byte;
            i += 1;
            if done {
                break;
            }
        }
        i
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

    #[inline]
    fn read_leb128_i64<T: ReadU8 + ?Sized>(t: &mut T) -> Result<i64> {
        let mut result: i64 = 0;
        let mut shift = 0;
        loop {
            let byte = t.read_u8()?;
            result |= i64::from(byte & 0x7F) << shift;
            if shift >= 57 {
                let continuation_bit = (byte & 0x80) != 0;
                let sign_and_unused_bit = ((byte << 1) as i8) >> (64 - shift);
                if continuation_bit || (sign_and_unused_bit != 0 && sign_and_unused_bit != -1) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "Invalid leb128 i64",
                    ));
                }
                return Ok(result);
            }
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
        }
        let ashift = 64 - shift;
        Ok((result << ashift) >> ashift)
    }

    #[inline]
    fn read_leb128_u64<T: ReadU8 + ?Sized>(t: &mut T) -> Result<u64> {
        let byte = u64::from(t.read_u8()?);
        if (byte & 0x80) == 0 {
            return Ok(byte);
        }
        let mut result = byte & 0x7F;
        let mut shift = 7;
        loop {
            let byte = u64::from(t.read_u8()?);
            result |= (byte & 0x7F) << shift;
            if shift >= 57 && (byte >> (64 - shift)) != 0 {
                // The continuation bit or unused bits are set.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Invalid leb128 u64",
                ));
            }
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
        }
        Ok(result)
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

    #[inline]
    fn write_leb128_u64<T: WriteBinary>(&mut self, x: u64) -> Result<usize> {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        let i = T::put_leb128_u64(&mut buf, x);
        self.write_all(&buf[..i])?;
        Ok(i)
    }

    #[inline]
    fn write_leb128_i64<T: WriteBinary>(&mut self, x: i64) -> Result<usize> {
        let mut buf = [0u8; MAX_VARINT_LEN64];
        let i = T::put_leb128_i64(&mut buf, x);
        self.write_all(&buf[..i])?;
        Ok(i)
    }
}

pub trait ReadU8 {
    fn read_u8(&mut self) -> Result<u8>;
}

pub trait ReadU8Ext {
    fn read_u8(&mut self) -> Result<u8>;
}

pub trait ReadBytesVarExt: ReadU8 {
    #[inline]
    fn read_vu64<T: ReadBinary>(&mut self) -> (u64, i32) {
        T::read_vu64(self)
    }

    #[inline]
    fn read_vi64<T: ReadBinary>(&mut self) -> (i64, i32) {
        T::read_vi64(self)
    }

    #[inline]
    fn read_led128_u64<T: ReadBinary>(&mut self) -> Result<u64> {
        T::read_leb128_u64(self)
    }

    #[inline]
    fn read_led128_i64<T: ReadBinary>(&mut self) -> Result<i64> {
        T::read_leb128_i64(self)
    }
}

impl<R: io::Read + ?Sized> ReadU8 for R {
    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

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

    #[test]
    fn test_read_led128_u64() {
        let mut rdr = Cursor::new(vec![0u8; MAX_VARINT_LEN64]);
        for x in uvar_test {
            rdr.write_leb128_u64::<Binary>(x).unwrap();
            rdr.set_position(0);
            let v = rdr.read_led128_u64::<Binary>().unwrap();
            rdr.set_position(0);
            assert!(x == v);
        }
    }

    #[test]
    fn test_read_led128_i64() {
        let mut rdr = Cursor::new(vec![0u8; MAX_VARINT_LEN64]);
        for x in ivar_test {
            rdr.write_leb128_i64::<Binary>(x).unwrap();
            rdr.set_position(0);
            let v = rdr.read_led128_i64::<Binary>().unwrap();
            rdr.set_position(0);
            assert!(x == v);
        }
    }
}
