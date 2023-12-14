// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{anyhow, Result};
use apfs_core::block::BlockReader;
use apfs_core::btree::BTreeNodeBlock;
use apfs_core::container::{CheckpointMapBlockParsed, ContainerSuperblockParsed};
use apfs_core::filesystem::FileSystemRecord;
use apfs_core::object::ObjectType;
use apfs_core::object_map::{ObjectMap, ObjectMapBlock};
use apfs_core::read::container::{ContainerReader, SuperblockReader};
use apfs_core::space_manager::{
    ChunkInfoAddressesBlockParsed, ChunkInfoBlockParsed, SpaceManagerBlock, SpaceManagerDeviceType,
    SpaceManagerFreeQueueType,
};
use apfs_core::volume::VolumeSuperblockParsed;
use apfs_core::ParsedDiskStruct;
use clap::{ArgAction, Args, Parser, Subcommand};
use log::LevelFilter;
use std::fs::File;
use std::path::PathBuf;

fn print_checkpoint_map(map: &CheckpointMapBlockParsed) -> Result<()> {
    println!("{:#?}", map);

    Ok(())
}

fn print_object_map(reader: &impl BlockReader, om: &ObjectMapBlock) -> Result<()> {
    println!("{:#?}", om.header());
    println!("{:#?}", om.btree().header());

    om.walk(reader, |k, v| {
        println!("{:?} => {:?}", k, v);
        Ok(())
    })?;

    Ok(())
}

trait CliCommand {
    fn run(&self) -> Result<()>;
}

#[derive(Args)]
struct FilesystemSource {
    /// Path to an APFS filesystem.
    #[arg(long)]
    pub path: Option<PathBuf>,
}

impl FilesystemSource {
    fn reader(&self) -> Result<ContainerReader> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| anyhow!("must define source"))?;

        let fh = Box::new(File::open(&path)?);
        let reader = ContainerReader::new(fh)?;

        Ok(reader)
    }

    fn latest_superblock_reader(&self) -> Result<SuperblockReader> {
        Ok(self.reader()?.latest_superblock_reader()?)
    }
}

#[derive(Parser)]
struct DumpBlocks {
    #[command(flatten)]
    source: FilesystemSource,

    /// Print blocks of unknown types.
    #[arg(long)]
    unknown: bool,
}

impl CliCommand for DumpBlocks {
    fn run(&self) -> Result<()> {
        let reader = self.source.reader()?;

        let largest_txn_id = reader.block_zero_superblock().next_transaction_identifier;

        for block_number in 0..reader.block_zero_superblock().block_count {
            let block = reader.get_block(block_number)?;

            let header = block.object_header()?;

            if block.validate_checksum().is_err() {
                continue;
            }

            match header.typ.object_type() {
                ObjectType::Invalid => {
                    continue;
                }
                ObjectType::Unknown(_) => {
                    if !self.unknown {
                        continue;
                    }
                }
                _ => {}
            }

            if header.transaction_identifier > largest_txn_id {
                continue;
            }

            println!("{} {:?}", block.number(), header);
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpBlock {
    #[command(flatten)]
    source: FilesystemSource,

    /// Block number to dump.
    number: u64,
}

impl CliCommand for DumpBlock {
    fn run(&self) -> Result<()> {
        let reader = self.source.reader()?;

        let block = reader.get_block(self.number)?;

        let header = block.object_header()?;

        match header.typ.object_type() {
            ObjectType::Invalid => {
                println!("not a valid object");
            }
            ObjectType::ContainerSuperblock => {
                let sb = ContainerSuperblockParsed::from_bytes(block.bytes())?;
                println!("{:#?}", sb);
            }
            ObjectType::BTreeRoot | ObjectType::BTreeNode => {
                let btree = BTreeNodeBlock::from_block(block)?;
                println!("{:#?}", *btree);

                if let Some(info) = btree.tree_info()? {
                    println!("{:#?}", info);
                }

                println!("{:#?}", btree.table_of_contents());
            }
            ObjectType::SpaceManagerHeader => {
                let sm = SpaceManagerBlock::new(block)?;
                println!("{:#?}", *sm);
            }
            ObjectType::SpaceManagerChunkInformationAddressBlock => {
                let cab = ChunkInfoAddressesBlockParsed::from_bytes(block.bytes())?;
                println!("{:#?}", cab);
            }
            ObjectType::SpaceManagerChunkInformationBlock => {
                let cib = ChunkInfoBlockParsed::from_bytes(block.bytes())?;
                println!("{:#?}", cib);
            }
            ObjectType::ObjectMap => {
                let om = ObjectMapBlock::new(&reader, block)?;
                print_object_map(&reader, &om)?;
            }
            ObjectType::VolumeSuperblock => {
                let vsb = VolumeSuperblockParsed::from_bytes(block.bytes())?;
                println!("{:#?}", *vsb);
            }
            ObjectType::Unknown(x) => {
                println!("unknown object type: {}", x);
            }
            x => {
                println!("don't yet know how to print {x:?}");
            }
        }

        Ok(())
    }
}

#[derive(Parser)]
pub struct DumpContainerObjectMap {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpContainerObjectMap {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;
        let om = reader.object_map()?;
        print_object_map(&reader, &om)?;

        Ok(())
    }
}

#[derive(Parser)]
struct DumpCheckpointBlocks {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpCheckpointBlocks {
    fn run(&self) -> Result<()> {
        let reader = self.source.reader()?;

        let sb = reader.superblock_latest()?;

        for map in reader.checkpoint_map_blocks(&sb)? {
            print_checkpoint_map(&map?)?;
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpSuperblock {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpSuperblock {
    fn run(&self) -> Result<()> {
        let reader = self.source.reader()?;
        println!("{:#?}", reader.superblock_latest()?);

        Ok(())
    }
}

#[derive(Parser)]
struct DumpVolumeSuperblocks {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpVolumeSuperblocks {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;

        for sb in reader.iter_volume_superblocks()? {
            let sb = sb?;
            println!("{:#?}", *sb);
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpExtentReferenceTree {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpExtentReferenceTree {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;
        for volume in reader.iter_volume_readers()? {
            let volume = volume?;

            for e in volume.iter_extent_reference_tree()? {
                let (k, v) = e?;
                println!("{:?} -> {:?}", k, v);
            }
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpFilesystemTree {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpFilesystemTree {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;

        for volume in reader.iter_volume_readers()? {
            let volume = volume?;

            volume.walk_root_tree(|k, v| {
                let id = k.id();

                let record = FileSystemRecord::new(k, v)?;
                println!("{} {:?}", id, record);

                Ok(())
            })?;
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpReaper {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpReaper {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;
        let reaper = reader.reaper()?;

        println!("{:#?}", reaper);

        Ok(())
    }
}

#[derive(Parser)]
struct DumpSnapshotMetadataTree {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpSnapshotMetadataTree {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;

        for volume in reader.iter_volume_readers()? {
            let volume = volume?;

            for e in volume.iter_snapshot_metadata_tree()? {
                let (k, v) = e?;
                println!("{:?} -> {:?}", k, v);
            }
        }

        Ok(())
    }
}

#[derive(Parser)]
struct DumpSpaceManager {
    #[command(flatten)]
    source: FilesystemSource,
}

impl CliCommand for DumpSpaceManager {
    fn run(&self) -> Result<()> {
        let reader = self.source.latest_superblock_reader()?;

        let sm = reader.space_manager()?;

        println!("Space Manager block #: {}", sm.block_number());
        println!("{:#?}", sm.space_manager());
        println!(
            "Internal pool bitmap XID: {}",
            sm.internal_pool_bitmap_xid()?
        );
        println!(
            "Internal pool bitmap offset: {}; block: {}",
            sm.internal_pool_bitmap_offset()?,
            sm.internal_pool_bitmap_block()?
        );
        println!(
            "Internal pool bitmap free next offsets: {:?}",
            sm.internal_pool_bitmap_free_next_offsets()?
        );

        sm.walk_free_queue(&reader, SpaceManagerFreeQueueType::InternalPool, |k, v| {
            println!("IP queue: {:?} -> {}", k, v);
            Ok(())
        })?;
        sm.walk_free_queue(&reader, SpaceManagerFreeQueueType::Main, |k, v| {
            println!("Main queue: {:?} -> {}", k, v);
            Ok(())
        })?;
        sm.walk_free_queue(&reader, SpaceManagerFreeQueueType::Tier2, |k, v| {
            println!("Tier2 queue: {:?} -> {}", k, v);
            Ok(())
        })?;

        for res in sm.iter_chunk_info_blocks(&reader, SpaceManagerDeviceType::Main)? {
            let (addr, cib) = res?;
            println!("Main device chunk info block: {} {:#?}", addr, cib);
        }
        for res in sm.iter_chunk_info_blocks(&reader, SpaceManagerDeviceType::Tier2)? {
            let (addr, cib) = res?;
            println!("Tier2 device chunk info block: {} {:#?}", addr, cib);
        }

        Ok(())
    }
}

#[derive(Parser)]
struct ExtractBlock {
    #[command(flatten)]
    source: FilesystemSource,

    /// Filesystem path to write the raw block data.
    #[arg(long)]
    output_path: Option<PathBuf>,

    /// Block number to extract.
    number: u64,
}

impl CliCommand for ExtractBlock {
    fn run(&self) -> Result<()> {
        let reader = self.source.reader()?;

        let block = reader.read_block_data(self.number)?;

        if let Some(path) = &self.output_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(path, block.as_ref())?;
        }

        Ok(())
    }
}

#[derive(Subcommand)]
enum Subcommands {
    /// Show information about blocks.
    DumpBlocks(DumpBlocks),
    /// Print decoded information from a block.
    ///
    /// This looks at the block object header and attempts to parse the block
    /// based on its advertised type.
    DumpBlock(DumpBlock),
    /// Dump the container's object map.
    DumpContainerObjectMap(DumpContainerObjectMap),
    /// Show information about checkpoint blocks.
    DumpCheckpointBlocks(DumpCheckpointBlocks),
    /// Print the volume physical extent reference tree.
    DumpExtentReferenceTree(DumpExtentReferenceTree),
    /// Print the filesystem tree.
    DumpFilesystemTree(DumpFilesystemTree),
    /// Dump the container reaper.
    DumpReaper(DumpReaper),
    /// Dump the volume snapshot metadata tree.
    DumpSnapshotMetadataTree(DumpSnapshotMetadataTree),
    /// Dump the container space manager state.
    DumpSpaceManager(DumpSpaceManager),
    /// Dump the superblock.
    DumpSuperblock(DumpSuperblock),
    /// Dump APFS volume superblocks.
    DumpVolumeSuperblocks(DumpVolumeSuperblocks),
    /// Extract the raw content of a block.
    ExtractBlock(ExtractBlock),
}

impl Subcommands {
    fn as_cli_command(&self) -> &dyn CliCommand {
        match self {
            Self::DumpBlocks(c) => c,
            Self::DumpBlock(c) => c,
            Self::DumpContainerObjectMap(c) => c,
            Self::DumpCheckpointBlocks(c) => c,
            Self::DumpExtentReferenceTree(c) => c,
            Self::DumpFilesystemTree(c) => c,
            Self::DumpReaper(c) => c,
            Self::DumpSnapshotMetadataTree(c) => c,
            Self::DumpSpaceManager(c) => c,
            Self::DumpSuperblock(c) => c,
            Self::DumpVolumeSuperblocks(c) => c,
            Self::ExtractBlock(c) => c,
        }
    }
}

#[derive(Parser)]
struct Cli {
    /// Increase logging verbosity. Can be specified multiple times
    #[arg(short = 'v', long, global = true, action = ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Subcommands,
}

impl Cli {
    fn run() -> Result<()> {
        let cli = Self::parse();

        let log_level = match cli.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };

        let mut builder = env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or(log_level.as_str()),
        );

        builder.init();

        let command = cli.command.as_cli_command();

        command.run()
    }
}

fn main() {
    let exit_code = match Cli::run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Error: {err}");
            1
        }
    };

    std::process::exit(exit_code)
}
