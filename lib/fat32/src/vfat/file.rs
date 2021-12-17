use alloc::string::String;

use shim::io::{self, SeekFrom};

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFat, VFatHandle};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    // FIXME: Fill me in.
    pub start_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub file_size: u64,
    offset: u64,
    current_cluster: Cluster,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    pub fn new(vfat: HANDLE, start_cluster: Cluster, name: String, metadata: Metadata, file_size: u32) -> Self {
        File::<HANDLE> {
            vfat,
            start_cluster,
            name,
            metadata,
            file_size: file_size as u64,
            offset: 0,
            current_cluster: start_cluster,
        }
    }
}

// FIXME: Implement `traits::File` (and its supertraits) for `File`.
impl<HANDLE: VFatHandle> traits::File for File<HANDLE> {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }

    /// Returns the size of the file in bytes.
    fn size(&self) -> u64 {
        self.file_size as u64
    }
}

impl<HANDLE: VFatHandle> io::Read for File<HANDLE> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.offset >= self.file_size || buf.len() == 0 {
            Ok(0)
        } else {
            // we need to clone it otherwise the immutable borrow of `self.vfat` collides with the
            // mutable borrow used in the closure. you _can_ refactor the closure so it never refers
            // to `self` and we perform mutations after the closure returns. (That might also be preferable
            // since the critical section would be smaller.) But for now, this is fine.
            let handle = self.vfat.clone();
            handle.lock(|vfat: &mut VFat<HANDLE>| {
                let cluster_size = vfat.cluster_size() as u64;
                let cluster_offset = self.offset % cluster_size;

                // go to the boundary of this cluster unless we are beyond file size
                let to_read = ::core::cmp::min(
                    buf.len() as u64,
                    ::core::cmp::min(cluster_size - cluster_offset, self.file_size - self.offset),
                ) as usize;
                let num_read = vfat.read_cluster(self.current_cluster, cluster_offset as usize, &mut buf[..to_read])?;
                self.offset += num_read as u64;

                if cluster_offset + num_read as u64 == cluster_size {
                    match vfat.next_cluster(self.current_cluster)? {
                        Some(c) => self.current_cluster = c,
                        None => {
                            if self.offset < self.file_size {
                                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOC found"));
                            }
                        }
                    };
                }

                Ok(num_read)
            })
        }
    }
}

impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        panic!("omg write")
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<HANDLE: VFatHandle> io::Seek for File<HANDLE> {
    /// Seek to offset `pos` in the file.
    ///
    /// A seek to the end of the file is allowed. A seek _beyond_ the end of the
    /// file returns an `InvalidInput` error.
    ///
    /// If the seek operation completes successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with SeekFrom::Start.
    ///
    /// # Errors
    ///
    /// Seeking before the start of a file or beyond the end of the file results
    /// in an `InvalidInput` error.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (mut offset, add) = match pos {
            SeekFrom::Start(p) => (p, 0),
            SeekFrom::End(pn) => (self.file_size, pn),
            SeekFrom::Current(pn) => (self.offset, pn),
        };

        if add.is_negative() {
            if add.wrapping_abs() as u64 > offset {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
            }
            offset -= add.wrapping_abs() as u64;
        } else {
            offset += add as u64;
        }

        if offset >= self.file_size {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "seek past EOF"))
        } else {
            self.offset = offset;
            Ok(self.offset)
        }
    }
}
