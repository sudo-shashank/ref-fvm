use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use std::panic;

use cid::Cid;
use fvm_ipld_encoding::{from_slice, Cbor};
use fvm_shared::address::Address;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::syscall_error;

pub struct Context<'a, K> {
    pub kernel: &'a mut K,
    pub memory: &'a mut Memory,
}

#[repr(transparent)]
pub struct Memory([u8]);

impl Deref for Memory {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Memory {
    /// Create a new `Memory` from the passed byte slice.
    #[allow(clippy::needless_lifetimes)]
    pub fn new<'a>(m: &'a mut [u8]) -> &'a mut Memory {
        // We explicitly specify the lifetimes here to ensure that the cast doesn't inadvertently
        // change them.
        unsafe { &mut *(m as *mut [u8] as *mut Memory) }
    }

    pub fn check_bounds(&self, offset: u32, len: u32) -> Result<()> {
        if (offset as u64) + (len as u64) <= (self.0.len() as u64) {
            Ok(())
        } else {
            Err(
                syscall_error!(IllegalArgument; "buffer {} (length {}) out of bounds", offset, len)
                    .into(),
            )
        }
    }

    /// Returns an immutable slice of wasm memory, checking to make sure it's in-bounds.
    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        self.get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ErrorNumber::IllegalArgument)
    }

    /// Returns a mutable slice of wasm memory, checking to see if it's in-bounds.
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]> {
        self.get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ErrorNumber::IllegalArgument)
    }

    /// Returns many mutable slices into wasm memory.
    ///
    /// 1. The slices must not overlap, and must be in-range.
    /// 2. Empty slices must be in-range, but are never considered to overlap.
    pub fn try_slice_many<'a, const S: usize>(
        &'a mut self,
        ranges: [(u32, u32); S],
    ) -> Result<[&'a mut [u8]; S]> {
        // Algorithm:
        //
        // 1. Create an index of the ranges, and sort by range start.
        // 2. Slice from memory in-order, checking that ranges don't overlapp and are "in-bounds".
        // 3. Return an array of slices in the original order.

        // Helper function to generate arrays of empty mutable slices.
        fn empty_slices<'a, const S: usize, T>() -> [&'a mut [T]; S] {
            // We could do this with some unsafety, but I assume rust will just optimize this out
            // either way.
            [(); S].map(|_| &mut [][..])
        }

        // First, check the two base-cases (0 & 1). Given that S is a constant, most of this logic
        // should compile down to a no-op.
        match S {
            0 => return Ok(empty_slices()),
            1 => {
                let (off, len) = ranges[0];
                let mut ret = empty_slices();
                ret[0] = self.try_slice_mut(off, len)?;
                return Ok(ret);
            }
            _ => {}
        }

        // 1. Create an index by range start...
        let mut sorted_indexes: [usize; S] = [0; S];
        for (i, element) in sorted_indexes.iter_mut().enumerate() {
            *element = i;
        }
        // ...and sort the index by range start.
        sorted_indexes.sort_unstable_by_key(|&i| ranges[i].0);

        // 3. Split into sub-slices.
        //
        // We could use uninitialzied memory here, but I assume this code will optimize well
        // anyways.
        let mut output: [&mut [u8]; S] = empty_slices();

        let mut mem = &mut self.0; // The memory we can still address.
        let mut mem_offset = 0u64; // The offset where `mem` begins.
        let addressable_range = mem.len() as u64; // The total addressable range.

        for idx in sorted_indexes {
            let (off, len) = ranges[idx];
            // Do everything with u64 to avoid having to do overflow checks. We're just doing
            // addition, no multiplication.
            let off = off as u64;
            let len = len as u64;

            // Make sure we're in-bounds.
            let end = off + len;
            if end > addressable_range {
                return Err(syscall_error!(IllegalArgument; "memory out of bounds").into());
            }

            // Now skip zero-length slices. We don't do anything else here, and they _can't_
            // overlap.
            if len == 0 {
                continue;
            }

            // Make sure we're not overlapping with the previous slice.
            if off < mem_offset {
                return Err(syscall_error!(IllegalArgument; "overlapping ranges").into());
            }

            // Finally, slice.
            let (slice, rest) = mem[(off - mem_offset) as usize..].split_at_mut(len as usize);

            // Update the memory offset, and the remaining memory.
            mem_offset = end;
            mem = rest;

            // 4. And record the slice.
            output[idx] = slice;
        }
        Ok(output)
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid> {
        // NOTE: Be very careful when changing this code.
        //
        // We intentionally read the CID till the end of memory. We intentionally do not "slice"
        // with a fixed end.
        // - We _can't_ slice MAX_CID_LEN because there may not be MAX_CID_LEN addressable memory
        //   after the offset.
        // - We can safely read from an "arbitrary" sized slice because `Cid::read_bytes` will never
        //   read more than 4 u64 varints and 64 bytes of digest.
        Cid::read_bytes(
            self.0
                .get(offset as usize..)
                .ok_or_else(|| format!("cid at offset {} is out of bounds", offset))
                .or_error(ErrorNumber::IllegalArgument)?,
        )
        .or_error(ErrorNumber::IllegalArgument)
        .context("failed to parse cid")
    }

    pub fn write_cid(&mut self, k: &Cid, offset: u32, len: u32) -> Result<u32> {
        let out = self.try_slice_mut(offset, len)?;

        let mut buf = Cursor::new([0u8; MAX_CID_LEN]);
        // At the moment, all CIDs are gauranteed to fit in 100 bytes (statically) because the max
        // digest size is 64, the max varint size is 9, and there are 4 varints plus the digest.
        k.write_bytes(&mut buf).expect("failed to format a cid");
        let len = buf.position() as usize;
        if len > out.len() {
            return Err(syscall_error!(BufferTooSmall; "cid output buffer is too small").into());
        }
        out[..len].copy_from_slice(&buf.get_ref()[..len]);
        Ok(len as u32)
    }

    pub fn read_address(&self, offset: u32, len: u32) -> Result<Address> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes).or_error(ErrorNumber::IllegalArgument)
    }

    pub fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        // Catch panics when decoding cbor from actors, _just_ in case.
        match panic::catch_unwind(|| from_slice(bytes).or_error(ErrorNumber::IllegalArgument)) {
            Ok(v) => v,
            Err(e) => {
                log::error!("panic when decoding cbor from actor: {:?}", e);
                Err(syscall_error!(IllegalArgument; "panic when decoding cbor from actor").into())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use super::*;

    const RAW: u64 = 0x55;
    const SHA2_256: u64 = 0x12;
    const HASH: &[u8] = b"\x2C\x26\xB4\x6B\x68\xFF\xC6\x8F\xF9\x9B\x45\x3C\x1D\x30\x41\x34\x13\x42\x2D\x70\x64\x83\xBF\xA0\xF9\x8A\x5E\x88\x62\x66\xE7\xAE";

    macro_rules! expect_syscall_err {
        ($code:ident, $res:expr) => {
            match $res.expect_err("expected syscall to fail") {
                $crate::kernel::ExecutionError::Syscall($crate::kernel::SyscallError(
                    _,
                    fvm_shared::error::ErrorNumber::$code,
                )) => {}
                $crate::kernel::ExecutionError::Syscall($crate::kernel::SyscallError(
                    msg,
                    code,
                )) => {
                    panic!(
                        "expected {}, got {}: {}",
                        fvm_shared::error::ErrorNumber::$code,
                        code,
                        msg
                    )
                }
                $crate::kernel::ExecutionError::Fatal(err) => {
                    panic!("got unexpected fatal error: {}", err)
                }
                $crate::kernel::ExecutionError::OutOfGas => {
                    panic!("got unexpected out of gas")
                }
            }
        };
    }

    #[test]
    fn test_read_cid() {
        let hash = cid::multihash::Multihash::wrap(SHA2_256, HASH).unwrap();
        let k = Cid::new_v1(RAW, hash);
        let mut k_bytes = k.to_bytes();
        let mem = Memory::new(&mut k_bytes);
        let k2 = mem.read_cid(0).expect("failed to read cid");
        assert_eq!(k, k2);
    }

    #[test]
    fn test_read_cid_truncated() {
        let hash = cid::multihash::Multihash::wrap(SHA2_256, HASH).unwrap();
        let k = Cid::new_v1(RAW, hash);
        let mut k_bytes = k.to_bytes();
        let mem = Memory::new(&mut k_bytes[..20]);
        expect_syscall_err!(IllegalArgument, mem.read_cid(0));
    }

    #[test]
    fn test_read_cid_out_of_bounds() {
        let mem = Memory::new(&mut []);
        expect_syscall_err!(IllegalArgument, mem.read_cid(200));
    }

    #[test]
    fn test_read_slice_out_of_bounds() {
        let mem = Memory::new(&mut []);
        expect_syscall_err!(IllegalArgument, mem.try_slice(10, 0));
        expect_syscall_err!(IllegalArgument, mem.try_slice(0, 10));
        expect_syscall_err!(IllegalArgument, mem.try_slice(1, 1));
        expect_syscall_err!(IllegalArgument, mem.try_slice(u32::MAX, 0));
        expect_syscall_err!(IllegalArgument, mem.try_slice_many([(1, 0)]));
        for perm in (0u32..4).permutations(4) {
            match *perm {
                [a, b, c, d] => {
                    expect_syscall_err!(IllegalArgument, mem.try_slice_many([(a, b), (c, d)]))
                }
                _ => panic!("expected a vector of length 4"),
            }
        }

        for perm in [0, u32::MAX].into_iter().permutations(4) {
            match *perm {
                [a, b, c, d] => {
                    expect_syscall_err!(IllegalArgument, mem.try_slice_many([(a, b), (c, d)]))
                }
                _ => panic!("expected a vector of length 4"),
            }
        }
    }

    #[test]
    fn test_read_slice_empty() {
        let mem = Memory::new(&mut []);
        assert!(mem.try_slice(0, 0).expect("slice was in bounds").is_empty());
        assert!(mem
            .try_slice_many([])
            .expect("slice was in bounds")
            .is_empty());
        let [slice] = mem.try_slice_many([(0, 0)]).expect("slice was in bounds");
        assert!(slice.is_empty());
        assert!(mem
            .try_slice_many([(0, 0), (0, 0), (0, 0)])
            .expect("slice was in bounds")
            .into_iter()
            .all(|s| s.is_empty()))
    }

    #[test]
    fn test_read_slice_many() {
        macro_rules! assert_slices {
            ([$($($e:expr),*);*], $ee:expr) => {
                let expected = [$(&mut [$($e),*][..]),*];
                let _: &[&mut [u8]] = &expected[..]; // type hint
                assert_eq!(expected, ($ee).unwrap())
            };
        }

        let mut vec: Vec<u8> = (1u8..=100).collect();
        let mem = Memory::new(&mut vec);
        assert!(mem
            .try_slice_many([])
            .expect("no slices always work")
            .is_empty());
        assert_slices!([1; 2], mem.try_slice_many([(0, 1), (1, 1)]));
        assert_slices!([;3;2;;1], mem.try_slice_many([(0, 0), (2, 1), (1, 1), (5, 0), (0, 1)]));
        // zero-legnth doesn't count as overlapping.
        assert_slices!([1, 2, 3;], mem.try_slice_many([(0, 3), (1, 0)]));
        // but these do overlap
        expect_syscall_err!(IllegalArgument, mem.try_slice_many([(0, 3), (1, 1)]));

        // make sure we can index at the end.
        assert_slices!([100;], mem.try_slice_many([(99, 1), (0, 0)]));

        // non-zero-length out-of-bounds
        expect_syscall_err!(IllegalArgument, mem.try_slice_many([(100, 1), (0, 0)]));

        // zero-length almost out-of-bounds
        assert_slices!([;], mem.try_slice_many([(100, 0), (0, 0)]));

        // zero-length out-of-bounds
        expect_syscall_err!(IllegalArgument, mem.try_slice_many([(101, 1), (0, 0)]));
    }
}
