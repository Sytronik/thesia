use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::marker::PhantomData;

use super::spectrogram::SrWinNfft;
use identity_hash::IdentityHashable;

pub type TupleIntMap<K, V> = std::collections::HashMap<K, V, BuildTupleIntHasher<K>>;
pub type TupleIntSet<T> = std::collections::HashSet<T, BuildTupleIntHasher<T>>;

pub type BuildTupleIntHasher<T> = BuildHasherDefault<TupleIntHasher<T>>;

#[derive(Clone, Copy)]
pub struct TupleIntHasher<T>(u64, PhantomData<T>);

impl<T> fmt::Debug for TupleIntHasher<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("IdentityHasher").field(&self.0).finish()
    }
}

impl<T> Default for TupleIntHasher<T> {
    fn default() -> Self {
        TupleIntHasher(0, PhantomData)
    }
}

impl Hasher for TupleIntHasher<SrWinNfft> {
    fn write(&mut self, _: &[u8]) {
        panic!("Invalid use of TupleIntHasher")
    }

    fn write_u8(&mut self, n: u8) {
        self.0 = (self.0 << 16) + u64::from(n)
    }
    fn write_u16(&mut self, n: u16) {
        self.0 = (self.0 << 16) + u64::from(n)
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0 << 16) + u64::from(n)
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0 << 16) + n
    }
    fn write_usize(&mut self, n: usize) {
        self.0 = (self.0 << 16) + n as u64
    }

    fn write_i8(&mut self, n: i8) {
        self.0 = (self.0 << 16) + n as u64
    }
    fn write_i16(&mut self, n: i16) {
        self.0 = (self.0 << 16) + n as u64
    }
    fn write_i32(&mut self, n: i32) {
        self.0 = (self.0 << 16) + n as u64
    }
    fn write_i64(&mut self, n: i64) {
        self.0 = (self.0 << 16) + n as u64
    }
    fn write_isize(&mut self, n: isize) {
        self.0 = (self.0 << 16) + n as u64
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

macro_rules! impl_tuple_int_hasher {
    ( $($name:ident)+) => (
        impl<$($name: IdentityHashable),+> Hasher for TupleIntHasher<($($name),+)> {
            fn write(&mut self, _: &[u8]) {
                panic!("Invalid use of TupleIntHasher")
            }

            fn write_u8(&mut self, n: u8) {
                self.0 = (self.0 << 16) + u64::from(n)
            }
            fn write_u16(&mut self, n: u16) {
                self.0 = (self.0 << 16) + u64::from(n)
            }
            fn write_u32(&mut self, n: u32) {
                self.0 = (self.0 << 16) + u64::from(n)
            }
            fn write_u64(&mut self, n: u64) {
                self.0 = (self.0 << 16) + n
            }
            fn write_usize(&mut self, n: usize) {
                self.0 = (self.0 << 16) + n as u64
            }

            fn write_i8(&mut self, n: i8) {
                self.0 = (self.0 << 16) + n as u64
            }
            fn write_i16(&mut self, n: i16) {
                self.0 = (self.0 << 16) + n as u64
            }
            fn write_i32(&mut self, n: i32) {
                self.0 = (self.0 << 16) + n as u64
            }
            fn write_i64(&mut self, n: i64) {
                self.0 = (self.0 << 16) + n as u64
            }
            fn write_isize(&mut self, n: isize) {
                self.0 = (self.0 << 16) + n as u64
            }

            fn finish(&self) -> u64 {
                self.0
            }
        }
    );
}

impl_tuple_int_hasher! { T B }
impl_tuple_int_hasher! { T B C }
impl_tuple_int_hasher! { T B C D }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuple_int_hasher_works() {
        let mut h1 = TupleIntHasher::<(u8, usize)>::default();
        h1.write_u8(1);
        h1.write_usize(42);
        assert_eq!(2u64.pow(16) + 42, h1.finish());

        let mut h2 = TupleIntHasher::<(u16, usize)>::default();
        h2.write_u16(2);
        h2.write_usize(42);
        assert_eq!(2u64.pow(17) + 42, h2.finish());

        let mut h3 = TupleIntHasher::<(u32, usize)>::default();
        h3.write_u32(4);
        h3.write_usize(42);
        assert_eq!(2u64.pow(18) + 42, h3.finish());

        let mut h4 = TupleIntHasher::<(u64, usize)>::default();
        h4.write_u64(8);
        h4.write_usize(42);
        assert_eq!(2u64.pow(19) + 42, h4.finish());

        let mut h5 = TupleIntHasher::<(usize, usize)>::default();
        h5.write_usize(16);
        h5.write_usize(42);
        assert_eq!(2u64.pow(20) + 42, h5.finish());

        let mut h6 = TupleIntHasher::<(i8, isize)>::default();
        h6.write_i8(32);
        h6.write_isize(42);
        assert_eq!(2u64.pow(21) + 42, h6.finish());

        let mut h7 = TupleIntHasher::<(i16, isize)>::default();
        h7.write_i16(64);
        h7.write_isize(42);
        assert_eq!(2u64.pow(22) + 42, h7.finish());

        let mut h8 = TupleIntHasher::<(i32, isize)>::default();
        h8.write_i32(128);
        h8.write_isize(42);
        assert_eq!(2u64.pow(23) + 42, h8.finish());

        let mut h9 = TupleIntHasher::<(i64, isize)>::default();
        h9.write_i64(256);
        h9.write_isize(42);
        assert_eq!(2u64.pow(24) + 42, h9.finish());

        let mut h10 = TupleIntHasher::<(isize, isize)>::default();
        h10.write_isize(512);
        h10.write_isize(42);
        assert_eq!(2u64.pow(25) + 42, h10.finish());
    }
}
