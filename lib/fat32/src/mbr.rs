use core::fmt;
use shim::const_assert_size;
use shim::io;

use crate::traits::BlockDevice;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHS {
    // FIXME: Fill me in.
    _head: u8,
    sector_and_cylinder_higher: u8,
    _cylinder_lower: u8,
}

impl CHS {
    fn sector(&self) -> u8 {
        self.sector_and_cylinder_higher & 0b111_111
    }
}

// FIXME: implement Debug for CHS
impl fmt::Debug for CHS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CHS")
            .field("sector", &format_args!("{}", self.sector()))
            .finish()
    }
}

const_assert_size!(CHS, 3);

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct PartitionEntry {
    // FIXME: Fill me in.
    boot_indicator: u8,
    starting_chs: CHS,
    partition_type: u8,
    ending_chs: CHS,
    sector_offset: u32,
    num_sectors: u32,
}

impl PartitionEntry {
    const BOOTABLE: u8 = 0x80;

    pub fn starting_sector(&self) -> u32 {
        self.sector_offset
    }
}

// FIXME: implement Debug for PartitionEntry
impl fmt::Debug for PartitionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartitionEntry")
            .field("boot_indicator", &self.boot_indicator)
            .field("starting_chs", &self.starting_chs)
            .field("partition_type", &self.partition_type)
            .field("ending_chs", &self.ending_chs)
            .field("sector_offset", &{ self.sector_offset })
            .field("num_sectors", &{ self.num_sectors })
            .finish()
    }
}

const_assert_size!(PartitionEntry, 16);

/// The master boot record (MBR).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MasterBootRecord {
    // FIXME: Fill me in.
    bootstrap: [u8; 436],
    disk_id: [u8; 10],
    partitions: [PartitionEntry; 4],
    magic: [u8; 2],
}

// FIXME: implemente Debug for MaterBootRecord
impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterBootRecord")
            .field("partitions", &self.partitions)
            .field("magic", &self.magic)
            .finish()
    }
}

const_assert_size!(MasterBootRecord, 512);

#[derive(Debug)]
pub enum Error {
    /// There was an I/O error while reading the MBR.
    Io(io::Error),
    /// Partiion `.0` (0-indexed) contains an invalid or unknown boot indicator.
    UnknownBootIndicator(u8),
    /// The MBR magic signature was invalid.
    BadSignature,
}

impl MasterBootRecord {
    /// Reads and returns the master boot record (MBR) from `device`.
    ///
    /// # Errors
    ///
    /// Returns `BadSignature` if the MBR contains an invalid magic signature.
    /// Returns `UnknownBootIndicator(n)` if partition `n` contains an invalid
    /// boot indicator. Returns `Io(err)` if the I/O error `err` occured while
    /// reading the MBR.
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        let mut buf: [u8; 512] = [0; 512];

        match device.read_sector(0, &mut buf) {
            Err(e) => Err(Error::Io(e)),
            Ok(n) => {
                if n != buf.len() {
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Insufficient bytes from sector",
                    )));
                }
                let mbr = unsafe { &*(buf.as_ptr() as *const MasterBootRecord) };
                if mbr.magic != [0x55, 0xAA] {
                    return Err(Error::BadSignature);
                }
                for i in 0..4usize {
                    let indicator = mbr.partitions[i].boot_indicator;
                    if indicator != 0x0 && indicator != PartitionEntry::BOOTABLE {
                        return Err(Error::UnknownBootIndicator(i as u8));
                    }
                }
                Ok(*mbr)
            }
        }
    }

    pub fn fat32_partition(&self) -> Option<&PartitionEntry> {
        self.partitions
            .iter()
            .find(|x| x.partition_type == 0xB || x.partition_type == 0xC)
    }
}
