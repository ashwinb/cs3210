use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use shim::io;

use crate::util::VecExt;
use crate::traits::BlockDevice;

#[derive(Debug)]
struct CacheEntry {
    data: Vec<u8>,
    dirty: bool,
}

#[derive(Debug)]
pub struct Partition {
    /// The physical sector where the partition begins.
    pub start: u64,
    /// Number of sectors
    pub num_sectors: u64,
    /// The size, in bytes, of a logical sector in the partition.
    pub sector_size: u64,
}

pub struct CachedPartition {
    device: Box<dyn BlockDevice>,
    cache: HashMap<u64, CacheEntry>,
    partition: Partition,
}

impl CachedPartition {
    /// Creates a new `CachedPartition` that transparently caches sectors from
    /// `device` and maps physical sectors to logical sectors inside of
    /// `partition`. All reads and writes from `CacheDevice` are performed on
    /// in-memory caches.
    ///
    /// The `partition` parameter determines the size of a logical sector and
    /// where logical sectors begin. An access to a sector `0` will be
    /// translated to physical sector `partition.start`. Virtual sectors of
    /// sector number `[0, num_sectors)` are accessible.
    ///
    /// `partition.sector_size` must be an integer multiple of
    /// `device.sector_size()`.
    ///
    /// # Panics
    ///
    /// Panics if the partition's sector size is < the device's sector size.
    pub fn new<T>(device: T, partition: Partition) -> CachedPartition
    where
        T: BlockDevice + 'static,
    {
        assert!(partition.sector_size >= device.sector_size());
        assert!(partition.sector_size % device.sector_size() == 0);

        CachedPartition {
            device: Box::new(device),
            cache: HashMap::new(),
            partition: partition,
        }
    }

    /// Returns the number of physical sectors that corresponds to
    /// one logical sector.
    fn factor(&self) -> u64 {
        self.partition.sector_size / self.device.sector_size()
    }

    /// Maps a user's request for a sector `virt` to the physical sector.
    /// Returns `None` if the virtual sector number is out of range.
    fn virtual_to_physical(&self, virt: u64) -> Option<u64> {
        if virt >= self.partition.num_sectors {
            return None;
        }

        let physical_offset = virt * self.factor();
        let physical_sector = self.partition.start + physical_offset;

        Some(physical_sector)
    }

    fn cache_entry(&mut self, sector: u64) -> io::Result<&mut CacheEntry> {
        let start = self.virtual_to_physical(sector);
        if start.is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid logical sector"));
        }
        let factor = self.factor();

        match self.cache.entry(sector) {
            Entry::Occupied(o) => Ok(o.into_mut()),
            Entry::Vacant(v) => {
                // force the buf to be at least 4-byte aligned so our SD card reader doesn't suffer
                let vec_aligned: Vec<u32> = Vec::with_capacity(128);
                let mut vec: Vec<u8> = unsafe { vec_aligned.cast() };
                for i in 0..factor {
                    self.device.read_all_sector(start.unwrap() + i, &mut vec)?;
                }
                Ok(v.insert(CacheEntry {
                    data: vec,
                    dirty: false,
                }))
            }
        }

        // The following will never work because Rust isn't smart enough to know that the
        // mutable borrow later is not conflicting with the immutable one earlier. The fact that
        // you are returning a value makes the compiler keep the borrow alive until the end of
        // the lexical function.
        //
        // if let Some(entry) = self.cache.get(&sector) {
        //     return Ok(&entry.data);
        // }

        // let mut vec: Vec<u8> = Vec::new();
        // self.device.read_all_sector(sector, &mut vec)?;
        // let ee = self.cache.insert(sector, CacheEntry { data: vec, dirty: false });
        // Ok(&ee.unwrap().data)
    }

    /// Returns a mutable reference to the cached sector `sector`. If the sector
    /// is not already cached, the sector is first read from the disk.
    ///
    /// The sector is marked dirty as a result of calling this method as it is
    /// presumed that the sector will be written to. If this is not intended,
    /// use `get()` instead.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an error reading the sector from the disk.
    pub fn get_mut(&mut self, sector: u64) -> io::Result<&mut [u8]> {
        self.cache_entry(sector).map(|x| {
            x.dirty = true;
            x.data.as_mut()
        })
    }

    /// Returns a reference to the cached sector `sector`. If the sector is not
    /// already cached, the sector is first read from the disk.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an error reading the sector from the disk.
    pub fn get(&mut self, sector: u64) -> io::Result<&[u8]> {
        self.cache_entry(sector).map(|x| x.data.as_ref())
    }
}

// FIXME: Implement `BlockDevice` for `CacheDevice`. The `read_sector` and
// `write_sector` methods should only read/write from/to cached sectors.
impl BlockDevice for CachedPartition {
    fn sector_size(&self) -> u64 {
        self.partition.sector_size
    }

    fn read_sector(&mut self, sector: u64, buf: &mut [u8]) -> io::Result<usize> {
        let cached_buf = self.get(sector)?;
        let n = ::core::cmp::min(buf.len(), cached_buf.len());
        buf.copy_from_slice(&cached_buf[..n]);
        Ok(n)
    }

    fn write_sector(&mut self, sector: u64, buf: &[u8]) -> io::Result<usize> {
        let writable_cache = self.get_mut(sector)?;
        let n = ::core::cmp::min(buf.len(), writable_cache.len());
        writable_cache.copy_from_slice(&buf[..n]);
        Ok(n)
    }
}

impl fmt::Debug for CachedPartition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CachedPartition")
            .field("device", &"<block device>")
            .field("partition", &self.partition)
            .field("cache", &self.cache)
            .finish()
    }
}
