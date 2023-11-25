// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Block-level primitives.

use crate::error::{ApfsError, Result};
use apfs_types::common::PhysicalObjectIdentifierRaw;
use apfs_types::object::ObjectHeaderParsed;
use apfs_types::ParsedDiskStruct;
use bytes::{Bytes, BytesMut};
use std::ops::Deref;
use thiserror::Error;

/// Error for a block reading operation.
#[derive(Debug, Error)]
pub enum BlockReadError {
    #[error("block number {0} is out of bounds")]
    BlockBounds(PhysicalObjectIdentifierRaw),
    #[error("I/O error reading block data: {0}")]
    Io(#[from] std::io::Error),
    #[error("other block reading error: {0}")]
    Other(&'static str),
}

/// Interface for reading blocks.
pub trait BlockReader {
    /// Obtain the size of blocks in bytes.
    fn block_size(&self) -> usize;

    /// Read a block's data into the specified bytes buffer.
    ///
    /// Implementations must guarantee the following when returning Ok:
    ///
    /// * The [BytesMut] has length set to the container's block size.
    /// * The full block size is read into the [BytesMut]. No partial reads.
    ///
    /// These conditions can be achieved by calling `buf.resize(block_size)`
    /// and `read_exact(buf)` on a [std::io::Read] instance.
    fn read_block_into<N: Into<PhysicalObjectIdentifierRaw>>(
        &self,
        block_number: N,
        buf: &mut BytesMut,
    ) -> Result<(), BlockReadError>;

    /// Read block data into a new buffer allocated by this function.
    fn read_block_data<N: Into<PhysicalObjectIdentifierRaw>>(
        &self,
        block_number: N,
    ) -> Result<Bytes, BlockReadError> {
        let mut buf = BytesMut::zeroed(self.block_size());
        self.read_block_into(block_number, &mut buf)?;

        Ok(buf.freeze())
    }

    /// Resolve a [Block] instance for a specified block number.
    ///
    /// The default implementation will read the block data and construct
    /// a new instance.
    ///
    /// Custom implementations could implement their own caching layer
    /// that avoids I/O.
    fn get_block<N: Into<PhysicalObjectIdentifierRaw>>(
        &self,
        block_number: N,
    ) -> Result<Block, BlockReadError> {
        let number = block_number.into();
        let buf = self.read_block_data(number)?;

        Ok(Block::new(number, buf))
    }

    /// Get a block and validate its checksum.
    ///
    /// You should call this instead of [Self::get_block] when you know the
    /// block you are reading has a physical object header / checksum.
    fn get_block_validated<N: Into<PhysicalObjectIdentifierRaw>>(
        &self,
        block_number: N,
    ) -> Result<Block, ApfsError> {
        let block = self.get_block(block_number)?;
        block.validate_checksum()?;

        Ok(block)
    }
}

fn fletcher64(input: &[u8]) -> u64 {
    let mut sum1 = 0u64;
    let mut sum2 = 0u64;

    for chunk in input.chunks(4) {
        sum1 += u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as u64;
        sum2 += sum1;
    }

    let c1 = sum1 + sum2;
    let c1 = 0xffffffff - (c1 % 0xffffffff);
    let c2 = sum1 + c1;
    let c2 = 0xffffffff - (c2 % 0xffffffff);

    (c2 << 32) | c1
}

/// A container block and its underlying data.
pub struct Block {
    number: PhysicalObjectIdentifierRaw,
    buf: Bytes,
}

impl Deref for Block {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}

impl Block {
    /// Construct an instance from its block number and read data.
    pub fn new(number: PhysicalObjectIdentifierRaw, buf: Bytes) -> Self {
        Self { number, buf }
    }

    /// The block number.
    ///
    /// 0 is the first block.
    pub fn number(&self) -> PhysicalObjectIdentifierRaw {
        self.number
    }

    /// Obtain the raw bytes backing this block.
    pub fn bytes(&self) -> Bytes {
        self.buf.clone()
    }

    /// Compute the fletcher checksum for this block containing a physical object.
    ///
    /// This will checksum the full block minus the first 8 bytes, which are used to
    /// store the checksum.
    pub fn checksum_object(&self) -> u64 {
        fletcher64(&self.buf.as_ref()[8..])
    }

    /// Ensure the checksum is valid, returning an error if not.
    pub fn validate_checksum(&self) -> Result<(), ApfsError> {
        let header = self.object_header()?;

        if header.checksum == self.checksum_object() {
            Ok(())
        } else {
            Err(ApfsError::InvalidChecksum)
        }
    }

    /// Resolve a parsed common object header from this block.
    ///
    /// Blocks are guaranteed to be large enough to hold the common object header.
    /// However, not all blocks contain the object header. Calling this on
    /// headerless blocks will return garbage values in the header.
    ///
    /// It is up to callers to validate the block's validity.
    pub fn object_header(&self) -> Result<ObjectHeaderParsed> {
        Ok(ObjectHeaderParsed::from_bytes(self.buf.clone())?)
    }
}
