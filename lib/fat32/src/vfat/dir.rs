use alloc::string::String;
use alloc::vec::Vec;

use core::char::{decode_utf16, REPLACEMENT_CHARACTER};
use core::iter;
use core::marker::PhantomData;

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;

use crate::traits;
use crate::traits::{Dir as DirTrait, Entry as EntryTrait};
use crate::util::{SliceExt, VecExt};
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFat, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    // FIXME: Fill me in.
    pub start_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatRegularDirEntry {
    // FIXME: Fill me in.
    name: [u8; 8],
    ext: [u8; 3],
    attr: u8,
    _nt_reserved: u8,
    _creation_time_tenths: u8,
    creation_time: Time,
    creation_date: Date,
    accessed_date: Date,
    cluster_high: u16,
    modified_time: Time,
    modified_date: Date,
    cluster_low: u16,
    file_size: u32,
}

impl VFatRegularDirEntry {
    fn cluster(&self) -> u32 {
        (self.cluster_high as u32) << 16 | (self.cluster_low as u32)
    }
}

const_assert_size!(VFatRegularDirEntry, 32);

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatLfnDirEntry {
    // FIXME: Fill me in.
    seq: u8,
    name_chars: [u16; 5],
    attr: u8,
    _type: u8,
    checksum: u8,
    name_chars_2: [u16; 6],
    _ignored_zeroes: u16,
    name_chars_3: [u16; 2],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct VFatUnknownDirEntry {
    // FIXME: Fill me in.
    id: u8,
    _ignored: [u8; 10],
    attr: u8,
    _also_ignored: [u8; 20],
}

const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    pub fn rootdir(vfat: HANDLE, cluster: Cluster) -> Self {
        Dir::<HANDLE> {
            vfat,
            start_cluster: cluster,
            name: "<FAT32 ROOT DIRECTORY>".into(),
            metadata: Metadata::default(),
        }
    }

    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        name.as_ref()
            .to_str()
            .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "invalid utf-8 in name"))
            .and_then(|needle| {
                self.entries().and_then(|mut iter| {
                    iter.find(|entry| entry.name().eq_ignore_ascii_case(needle))
                        .ok_or(io::Error::new(io::ErrorKind::NotFound, "File not found"))
                })
            })
    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    // FIXME: Implement `trait::Dir` for `Dir`.
    /// The type of entry stored in this directory.
    type Entry = Entry<HANDLE>;

    /// A type that is an iterator over the entries in this directory.
    type Iter = Iter<HANDLE>;

    /// Returns an iterator over the entries in this directory.
    fn entries(&self) -> io::Result<Self::Iter> {
        self.vfat.lock(|vfat: &mut VFat<HANDLE>| {
            let mut vec = Vec::new();
            vfat.read_chain(self.start_cluster, &mut vec)?;
            Ok(Iter::<HANDLE>::new(self.vfat.clone(), vec))
        })
    }
}

pub struct Iter<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    vfat: HANDLE,
    entries: Vec<VFatDirEntry>,
    index: usize,
    finished: bool,
}

impl<HANDLE: VFatHandle> Iter<HANDLE> {
    fn new(vfat: HANDLE, vec: Vec<u8>) -> Self {
        Self {
            phantom: PhantomData,
            vfat: vfat,
            index: 0,
            finished: false,
            entries: unsafe { vec.cast() },
        }
    }
}

fn is_space(ch: u8) -> bool {
    ch == 0x0 || ch == 0x20
}

fn make_name(name: &[u8], ext: &[u8], long_name_entries: &mut [&VFatLfnDirEntry]) -> String {
    if long_name_entries.len() == 0 {
        macro till_spaces($arr:ident) {
            $arr.iter().take_while(|&c| !is_space(*c)).map(|&x| x as char)
        }

        let mut s = till_spaces!(name).collect::<String>();
        if !is_space(ext[0]) {
            s.extend(iter::once('.').chain(till_spaces!(ext)));
        }
        s
    } else {
        long_name_entries.sort_by_key(|x| x.seq & 0x1F);

        let mut vec: Vec<u16> = Vec::new();
        for lfn in long_name_entries {
            vec.extend(
                [
                    &{ lfn.name_chars }[..],
                    &{ lfn.name_chars_2 }[..],
                    &{ lfn.name_chars_3 }[..],
                ]
                .iter()
                .flat_map(|&it| it)
                .take_while(|&ch| *ch != 0x00 && *ch != 0xFFFF),
            );
        }

        // I think you can avoid this allocation by chaining the iterators as follows
        // but that runs into unsafe borrows of packed fields (name_chars, etc. are certainly
        // misaligned, and it is unclear if the iter chaining will end up referencing their
        // addresses or not.)
        // let iter = long_name_entries
        //     .into_iter()
        //     .flat_map(|lfn| {
        //         lfn.name_chars.iter().chain(
        //             lfn.name_chars_2.iter().chain(
        //                 lfn.name_chars_3.iter()
        //             )
        //         )
        //     })
        //     .take_while(|&ch| *ch != 0x00 && *ch != 0xFFFF)
        //     .map(|&x| x);

        decode_utf16(vec.into_iter())
            .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
            .collect::<String>()
    }
}

fn make_entry<HANDLE: VFatHandle>(
    vfat: HANDLE,
    regular: &VFatRegularDirEntry,
    long_name_entries: &mut [&VFatLfnDirEntry],
) -> Entry<HANDLE> {
    let attr = Attributes(regular.attr);
    let metadata = Metadata::from(
        attr,
        Timestamp {
            date: regular.creation_date,
            time: regular.creation_time,
        },
        Timestamp {
            date: regular.modified_date,
            time: regular.modified_time,
        },
        regular.accessed_date,
    );

    let name = make_name(&regular.name, &regular.ext, long_name_entries);
    let start_cluster = Cluster::from(regular.cluster());
    if attr.is_dir() {
        Entry::<HANDLE>::Dir_(Dir::<HANDLE> {
            vfat,
            name,
            start_cluster,
            metadata,
        })
    } else {
        Entry::<HANDLE>::File_(File::<HANDLE>::new(
            vfat,
            start_cluster,
            name,
            metadata,
            regular.file_size,
        ))
    }
}

impl<HANDLE: VFatHandle> Iterator for Iter<HANDLE> {
    type Item = Entry<HANDLE>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.entries.len() || self.finished {
            return None;
        }

        let mut long_name_entries: Vec<&VFatLfnDirEntry> = Vec::new();
        for (pos, entry) in self.entries[self.index..].iter().enumerate() {
            unsafe {
                match (entry.unknown.id, entry.unknown.attr, entry) {
                    (0x0, _, _) => {
                        self.finished = true;
                        return None;
                    }
                    (0xE5, _, _) => (),
                    (_seq, 0xF, VFatDirEntry { long_filename }) => {
                        // seq range :: (1 -> 31)
                        long_name_entries.push(long_filename);
                    }
                    (_, _, VFatDirEntry { regular }) => {
                        self.index += pos + 1;
                        return Some(make_entry(self.vfat.clone(), regular, &mut long_name_entries));
                    }
                }
            }
        }
        None
    }
}
