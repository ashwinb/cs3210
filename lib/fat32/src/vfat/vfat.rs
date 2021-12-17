use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::{Component, Path};
use Component::*;

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::traits::{Dir as DirTrait, Entry as EntryTrait};
use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

/// A generic trait that handles a critical section as a closure
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    rootdir_cluster: Cluster,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
    where
        T: BlockDevice + 'static,
    {
        let mbr = MasterBootRecord::from(&mut device)?;
        let part = mbr.fat32_partition().ok_or(Error::NotFound)?;

        let pblock = BiosParameterBlock::from(&mut device, part.starting_sector() as u64)?;
        // println!("{:#?}", mbr);
        // println!("{:#?}", pblock);
        let vfat = VFat {
            phantom: PhantomData,
            device: CachedPartition::new(
                device,
                Partition {
                    start: part.starting_sector() as u64,
                    num_sectors: pblock.logical_sectors() as u64,
                    sector_size: pblock.bytes_per_sector as u64,
                },
            ),
            bytes_per_sector: pblock.bytes_per_sector,
            sectors_per_cluster: pblock.sectors_per_cluster,
            sectors_per_fat: pblock.sectors_per_fat,
            fat_start_sector: pblock.reserved_sectors as u64,
            data_start_sector: pblock.reserved_sectors as u64 + pblock.num_fats as u64 * pblock.sectors_per_fat as u64,
            rootdir_cluster: Cluster::from(pblock.root_cluster),
        };
        Ok(HANDLE::new(vfat))
    }

    fn start_sector(&self, cluster: Cluster) -> io::Result<u64> {
        if cluster.num() < 2 {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid cluster number"))
        } else {
            Ok(self.data_start_sector + (cluster.num() - 2) as u64 * self.sectors_per_cluster as u64)
        }
    }

    pub(crate) fn cluster_size(&self) -> u64 {
        self.bytes_per_sector as u64 * self.sectors_per_cluster as u64
    }

    // TODO: The following methods may be useful here:
    //
    //  * A method to read from an offset of a cluster into a buffer.
    //
    pub(crate) fn read_cluster(&mut self, cluster: Cluster, offset: usize, buf: &mut [u8]) -> io::Result<usize> {
        let start_sector = self.start_sector(cluster)?;

        let mut n = 0;
        let mut start;

        let sec_size = self.bytes_per_sector as usize;
        for i in 0..self.sectors_per_cluster as usize {
            if offset >= (i + 1) * sec_size {
                continue;
            }
            let bytes = self.device.get(start_sector + i as u64)?;
            if bytes.len() != sec_size {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "invalid sector length"));
            }

            if offset > i * sec_size {
                start = offset - i * sec_size;
            } else {
                start = 0;
            }
            let len = ::core::cmp::min(sec_size - start, buf.len() - n);
            &buf[n..n + len].copy_from_slice(&bytes[start..start + len]);

            n += len;
            if n >= buf.len() {
                break;
            }
        }
        Ok(n)
    }

    fn read_cluster_all(&mut self, cluster: Cluster, vec: &mut Vec<u8>) -> io::Result<usize> {
        let start_sector = self.start_sector(cluster)?;
        let mut n = 0;
        for i in 0..self.sectors_per_cluster as usize {
            n += self.device.read_all_sector(start_sector + i as u64, vec)?;
        }
        Ok(n)
    }

    // A method to read all of the clusters chained from a starting cluster
    // into a vector.
    pub(crate) fn read_chain(&mut self, start: Cluster, vec: &mut Vec<u8>) -> io::Result<usize> {
        let mut n = 0;
        let mut cluster = start;
        loop {
            n += self.read_cluster_all(cluster, vec)?;
            match self.next_cluster(cluster)? {
                Some(c) => cluster = c,
                None => break,
            }
        }
        Ok(n)
    }

    pub(crate) fn next_cluster(&mut self, current: Cluster) -> io::Result<Option<Cluster>> {
        match self.fat_entry(current)?.status() {
            Status::Data(next_cluster) => Ok(Some(next_cluster)),
            Status::Eoc(_) => Ok(None),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected cluster type in chain",
            )),
        }
    }

    //
    // A method to return a reference to a `FatEntry` for a cluster where the
    // reference points directly into a cached sector.
    //
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let entry_offset = cluster.num() * 4;
        let logical_sector = self.fat_start_sector + entry_offset as u64 / self.bytes_per_sector as u64;

        let index = (entry_offset % self.bytes_per_sector as u32) / 4;
        let bytes = self.device.get(logical_sector)?;
        let fat_entries: &[FatEntry] = unsafe { bytes.cast() };
        Ok(&fat_entries[index as usize])
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Entry = Entry<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        let mut iter = path.as_ref().components().skip_while(|c| match c {
            Normal(_) => false,
            _ => true,
        });

        let cluster = self.lock(|vfat: &mut VFat<HANDLE>| vfat.rootdir_cluster);
        let rootdir = Entry::<HANDLE>::Dir_(Dir::<HANDLE>::rootdir(self.clone(), cluster));

        iter.try_fold(rootdir, |entry: Self::Entry, comp: Component| {
            entry
                .into_dir()
                .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "not a directory"))?
                .find(comp)
        })
    }
}
