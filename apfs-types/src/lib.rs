// Copyright 2023 Gregory Szorc.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(not(feature = "std"), no_std)]

//! Apple file system (APFS) data structures, constants, and enumerations.
//!
//! This crate defines the data structures and other important primitives
//! that constitute the on-disk data for APFS file systems.
//!
//! # Minimal by Design
//!
//! This crate aims to be minimal to facilitate maximal reuse by
//! different APFS implementations.
//!
//! The crate is `no_std` by default. And without the `derive` feature, it should be
//! `no_alloc` as well. This should hopefully make this crate suitable for use in
//! kernel contexts, if desired.
//!
//! The `std` feature does enable std features but the surface area of those
//! features is very small.
//!
//! There is support for parsing on-disk data structures gated behind the
//! `derive` feature.
//!
//! The nominal surface area of code in this crate is related to defining
//! APFS data structures and support code to materialize them to/from bytes in
//! memory. We purposefully omit higher-level business logic, such as walking
//! B-trees, resolving identifiers in object maps, finding superblocks, etc.
//!
//! # Note on Endianness
//!
//! Most APFS data structures are little-endian. When using the APIs
//! generated by the `derive` feature, parsing/serializing is endian aware.
//! However, if using the raw data structures and transmuting bytes, callers
//! need to be aware of endianness.
//!
//! # Typing Variations from Apple's Definitions
//!
//! Various data structures have variable length data following a static
//! sized header. We generally represent these by a 0 sized array as the final
//! field in a struct. e.g. [InodeRecordValueRaw::extended_fields]. Readers can
//! take the address of this empty array and cast/parse it as necessary.
//!
//! Since Apple's definitions are for C, which has more limited type expression
//! than Rust, we've taken the liberty of introducing new data structures.
//!
//! For example, related C constants are often combined into a Rust enum rather
//! than defined as N separate Rust constants.
//!
//! C enums tracking bit flags are also represented by Rust structs using the
//! `bitflags` crate.
//!
//! We've also introduced stronger typing in some situations. For example,
//! various C structs are defined in terms of an `oid_t`, which is a typedef
//! to a u64. An `oid_t` can represent a physical, ephemeral, or virtual
//! object identifier. In scenarios where we know the storage type of an object
//! identifier, we use the more strongly typed [PhysicalObjectIdentifierRaw],
//! [EphemeralObjectIdentifierRaw], and [VirtualObjectIdentifierRaw] to denote a
//! type to help prevent type conflation.
//!
//! # Struct Flavors
//!
//! Each distinct APFS data structure is defined by distinct Rust struct
//! variants. Each variant is described in the sections below.
//!
//! ## `*Raw`
//!
//! The lowest level `repr(C)` types defining the on-disk APFS types are
//! suffixed with `*Raw`. These structs are used to model the layout of
//! bytes.
//!
//! ## `*Parsed`
//!
//! Each `*Raw` struct has a corresponding `*Parsed` variant derived via
//! `#[derive(ApfsData)]`. These `*Parsed` variants represent a loaded /
//! deserialized variant of the raw `*Raw` struct.
//!
//! Internally, a `*Parsed` struct is effectively a `Cow<T>` to its `*Raw`
//! variant. The constructor for `*Parsed` variants can 0-copy memory
//! to materialize a `*Raw` data structure if running on little-endian machines
//! and the source memory is properly aligned. Otherwise, the raw source bytes
//! are *parsed* into an owned/non-borrowed struct.
//!
//! `*Parsed` instances implement `Deref<T>` so they quack like the `*Raw`
//! variant.
//!
//! `*Parsed` instances may hold onto the [bytes::Bytes] from which they were
//! constructed. [bytes::Bytes] is effectively an `Arc<Vec<u8>>`. This means
//! that `*Parsed` instances can hold onto the memory allocation that spawned
//! them. This can result in unwanted retention of memory resembling a memory
//! leak.
//!
//! For variable sized data structures (those that have data after the fixed
//! size header), the `*Parsed` instances hold onto the memory used to construct
//! them so that the *trailing bytes* are accessible and can be parsed into
//! additional data structures. This means callers can instantiate instances
//! from e.g. a full block's memory and not have to worry about holding onto
//! memory beyond the parsed fixed-size data structure / header in order to
//! fully reconstitute the data later.
//!
//! This approach may sound a bit scary in terms of memory efficiency. However,
//! APFS blocks are 4096 bytes by default and 4096 is also the size of a memory
//! page on x86-64 and other architectures. So when not using a memory
//! allocator, retaining a reference to 1 byte *costs* the same as a reference to
//! 4096 and you aren't wasting memory.

extern crate alloc;
extern crate core;

use core::fmt::{Debug, Display, Formatter};
use core::ops::RangeBounds;

#[cfg(doc)]
use crate::{common::*, filesystem::*};

pub mod btree;
pub mod common;
pub mod container;
pub mod data_stream;
pub mod efi_jumpstart;
pub mod encryption;
pub mod encryption_rolling;
pub mod filesystem;
pub mod filesystem_extended_fields;
pub mod fusion;
pub mod object;
pub mod object_map;
#[cfg(feature = "derive")]
pub mod pod;
pub mod reaper;
pub mod sealed_volume;
pub mod sibling;
pub mod snapshot;
pub mod space_manager;
pub mod volume;

/// An error when reading/parsing APFS data structures.
#[derive(Clone, Copy, Debug)]
pub enum ParseError {
    /// Data structure cannot be parsed because not enough input data provided.
    InputTooSmall,
    /// Data structure cannot be casted because memory address not aligned.
    NonAligned,
    /// Supposedly NULL terminated string data isn't NULL terminated.
    StringNotNullTerminated,
    /// Supposedly UTF-8 string data is not valid UTF-8.
    StringNotUtf8,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InputTooSmall => f.write_str("input too small"),
            Self::NonAligned => f.write_str("input memory not properly aligned"),
            Self::StringNotNullTerminated => f.write_str("string data is not NULL terminated"),
            Self::StringNotUtf8 => f.write_str("string data not UTF-8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Describes a data structure persisted to disk.
pub trait DiskStruct
where
    Self: Sized,
{
    /// Parse a slice of bytes into an owned version of a type.
    ///
    /// Implementations may receive slices too small for self. It is up
    /// to implementations to detect this and error accordingly.
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError>;
}

impl DiskStruct for u8 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 1 {
            Ok(data[0])
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

impl DiskStruct for u16 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 2 {
            let input: [u8; 2] = [data[0], data[1]];
            Ok(Self::from_le_bytes(input))
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

impl DiskStruct for i32 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 4 {
            let input: [u8; 4] = [data[0], data[1], data[2], data[3]];
            Ok(Self::from_le_bytes(input))
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

impl DiskStruct for u32 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 4 {
            let input: [u8; 4] = [data[0], data[1], data[2], data[3]];
            Ok(Self::from_le_bytes(input))
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

impl DiskStruct for i64 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 8 {
            let input: [u8; 8] = [
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ];
            Ok(Self::from_le_bytes(input))
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

impl DiskStruct for u64 {
    fn parse_bytes(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 8 {
            let input: [u8; 8] = [
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ];
            Ok(Self::from_le_bytes(input))
        } else {
            Err(ParseError::InputTooSmall)
        }
    }
}

/// Describes common behavior of a `*Parsed` struct.
#[cfg(feature = "derive")]
pub trait ParsedDiskStruct: Sized {
    /// Construct an instance from bytes.
    fn from_bytes(buf: bytes::Bytes) -> Result<Self, ParseError>;
}

/// Marker trait indicating a struct is static sized.
///
/// Mutually exclusive with [DynamicSized].
pub trait StaticSized: Sized + Clone {}

/// Indicates that a struct has variable length trailing data.
///
/// Various structs have a fixed size header followed by variable length
/// data. These structs typically have a final field of type `[u8; 0]` to
/// serve as a placeholder for the beginning of the remaining data.
///
/// Mutually exclusive with [StaticSized].
pub trait DynamicSized: Sized {
    /// Obtain the offset to the trailing data as measured from the type's start.
    ///
    /// The default implementation computes the memory size of the type it is
    /// operating on. This should be sufficient to point at the final
    /// `[u8; 0]` placeholder field in the struct.
    fn trailing_data_offset() -> usize {
        core::mem::size_of::<Self>()
    }

    type RangeBounds: RangeBounds<usize> + Debug;

    /// Obtain the desired size of the trailing data.
    ///
    /// For definite length values, implementations should return a
    /// [core::ops::Range] with 0 as the starting bound.
    ///
    /// For indefinite length values (e.g. those consuming the entire
    /// block), implementations should return a [core::ops::RangeFrom]
    /// with 0 as the starting bound.
    fn trailing_data_bounds(&self) -> Self::RangeBounds;
}

/// Describes how to parse a dynamically sized data structure.
#[cfg(feature = "derive")]
trait DynamicSizedParse: DynamicSized {
    /// The type of trailing data.
    type TrailingData;

    /// Attempts to parse bytes into another type representing the trailing data.
    ///
    /// Implementations may eagerly or lazily parse the bytes: it is up to them.
    fn parse_trailing_data(&self, data: bytes::Bytes) -> Result<Self::TrailingData, ParseError>;
}

/// Represents the key part of a file system record.
///
/// File system objects/records describe typed information about an entity in
/// the file system. The records are stored in b-trees.
///
/// Keys always begin with the common `j_key_t` / [FileSystemKeyRaw] header.
pub trait FileSystemRecordKey: Clone + Debug {}

/// Represents the value part of a file system record.
pub trait FileSystemRecordValue: Clone + Debug {}
