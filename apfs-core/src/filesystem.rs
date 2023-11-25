// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Filesystem structs.

use crate::btree::NodeValue;
use crate::error::ApfsError;
use crate::error::Result;
use crate::snapshot::SnapshotNameRecordKeyParsed;
use apfs_types::data_stream::{
    DataStreamIdRecordKeyParsed, DataStreamIdRecordValueParsed, FileExtentRecordKeyParsed,
    FileExtentRecordValueParsed, PhysicalExtentRecordKeyParsed, PhysicalExtentRecordValueParsed,
};
use apfs_types::encryption::{EncryptionStateRecordKeyParsed, EncryptionStateRecordValueParsed};
pub use apfs_types::filesystem::*;
use apfs_types::sealed_volume::{FileInfoRecordKeyParsed, FileInfoRecordValueParsed};
use apfs_types::sibling::{
    SiblingLinkRecordKeyParsed, SiblingLinkRecordValueParsed, SiblingMapRecordKeyParsed,
    SiblingMapRecordValueParsed,
};
use apfs_types::snapshot::{
    SnapshotMetadataRecordKeyParsed, SnapshotMetadataRecordValueParsed,
    SnapshotNameRecordValueParsed,
};
use apfs_types::ParsedDiskStruct;
use std::fmt::Debug;

/// Represents the distinct types of filesystem record key-value pairs.
#[derive(Clone, Debug)]
pub enum FileSystemRecord {
    SnapshotMetadata(
        SnapshotMetadataRecordKeyParsed,
        SnapshotMetadataRecordValueParsed,
    ),
    PhysicalExtent(
        PhysicalExtentRecordKeyParsed,
        PhysicalExtentRecordValueParsed,
    ),
    Inode(InodeRecordKeyParsed, InodeRecordValueParsed),
    ExtendedAttribute(
        ExtendedAttributeRecordKeyParsed,
        ExtendedAttributeRecordValueParsed,
    ),
    SiblingLink(SiblingLinkRecordKeyParsed, SiblingLinkRecordValueParsed),
    DataStreamId(DataStreamIdRecordKeyParsed, DataStreamIdRecordValueParsed),
    EncryptionState(
        EncryptionStateRecordKeyParsed,
        EncryptionStateRecordValueParsed,
    ),
    FileExtent(FileExtentRecordKeyParsed, FileExtentRecordValueParsed),
    DirectoryEntry(
        DirectoryEntryRecordKeyParsed,
        DirectoryEntryRecordValueParsed,
    ),
    DirectoryEntryHashed(
        DirectoryEntryRecordHashedKeyParsed,
        DirectoryEntryRecordValueParsed,
    ),
    DirectoryStats(
        DirectoryInformationRecordKeyParsed,
        DirectoryInformationRecordValueParsed,
    ),
    SnapshotName(SnapshotNameRecordKeyParsed, SnapshotNameRecordValueParsed),
    SiblingMap(SiblingMapRecordKeyParsed, SiblingMapRecordValueParsed),
    FileInfo(FileInfoRecordKeyParsed, FileInfoRecordValueParsed),
}

impl FileSystemRecord {
    pub fn new(key: FileSystemKeyParsed, value: NodeValue) -> Result<Self> {
        match key.object_type() {
            FileSystemObjectType::Any => Err(ApfsError::InvalidFileSystemObjectType),
            FileSystemObjectType::SnapshotMetadata => Ok(Self::SnapshotMetadata(
                SnapshotMetadataRecordKeyParsed::from_bytes(key.bytes())?,
                SnapshotMetadataRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::Extent => Ok(Self::PhysicalExtent(
                PhysicalExtentRecordKeyParsed::from_bytes(key.bytes())?,
                PhysicalExtentRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::Inode => Ok(Self::Inode(
                InodeRecordKeyParsed::from_bytes(key.bytes())?,
                InodeRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::ExtendedAttribute => Ok(Self::ExtendedAttribute(
                ExtendedAttributeRecordKeyParsed::from_bytes(key.bytes())?,
                ExtendedAttributeRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::SiblinkLink => Ok(Self::SiblingLink(
                SiblingLinkRecordKeyParsed::from_bytes(key.bytes())?,
                SiblingLinkRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::DataStreamId => Ok(Self::DataStreamId(
                DataStreamIdRecordKeyParsed::from_bytes(key.bytes())?,
                DataStreamIdRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::EncryptionState => Ok(Self::EncryptionState(
                EncryptionStateRecordKeyParsed::from_bytes(key.bytes())?,
                EncryptionStateRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::FileExtent => Ok(Self::FileExtent(
                FileExtentRecordKeyParsed::from_bytes(key.bytes())?,
                FileExtentRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::DirectoryRecord => Ok(Self::DirectoryEntryHashed(
                DirectoryEntryRecordHashedKeyParsed::from_bytes(key.bytes())?,
                DirectoryEntryRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::DirectoryStats => Ok(Self::DirectoryStats(
                DirectoryInformationRecordKeyParsed::from_bytes(key.bytes())?,
                DirectoryInformationRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::SnapshotName => Ok(Self::SnapshotName(
                SnapshotNameRecordKeyParsed::from_bytes(key.bytes())?,
                SnapshotNameRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::SiblingMap => Ok(Self::SiblingMap(
                SiblingMapRecordKeyParsed::from_bytes(key.bytes())?,
                SiblingMapRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::FileInfo => Ok(Self::FileInfo(
                FileInfoRecordKeyParsed::from_bytes(key.bytes())?,
                FileInfoRecordValueParsed::from_bytes(value.into())?,
            )),
            FileSystemObjectType::Invalid => Err(ApfsError::InvalidFileSystemObjectType),
            FileSystemObjectType::Unknown(_) => Err(ApfsError::InvalidFileSystemObjectType),
        }
    }
}
