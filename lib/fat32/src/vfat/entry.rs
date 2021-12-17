use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle, Timestamp};
use core::fmt;

use traits::{Entry as EntryT, Metadata as MetadataT, Timestamp as TimestampT};

// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    // suffix "_" to disambiguate from the trait `type File` declarations
    File_(File<HANDLE>),
    Dir_(Dir<HANDLE>),
}

// TODO: Implement any useful helper methods on `Entry`.
impl<HANDLE: VFatHandle> fmt::Display for Entry<HANDLE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut write_bool = |b: bool, c: char| {
            if b {
                write!(f, "{}", c)
            } else {
                write!(f, "-")
            }
        };

        write_bool(self.is_dir(), 'd')?;
        write_bool(self.is_file(), 'f')?;
        write_bool(self.metadata().read_only(), 'r')?;
        write_bool(self.metadata().hidden(), 'h')?;
        write!(f, "\t")?;

        let mut write_timestamp = |ts: Timestamp| {
            write!(
                f,
                "{:02}/{:02}/{} {:02}:{:02}:{:02} ",
                ts.month(),
                ts.day(),
                ts.year(),
                ts.hour(),
                ts.minute(),
                ts.second()
            )
        };

        write_timestamp(self.metadata().created())?;
        write_timestamp(self.metadata().modified())?;
        write_timestamp(self.metadata().accessed())?;
        write!(f, "\t")?;

        write!(f, "{}", self.name())?;
        Ok(())
    }
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    // FIXME: Implement `traits::Entry` for `Entry`.
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;

    /// The name of the file or directory corresponding to this entry.
    fn name(&self) -> &str {
        match self {
            Entry::File_(f) => f.name.as_str(),
            Entry::Dir_(d) => d.name.as_str(),
        }
    }

    /// The metadata associated with the entry.
    fn metadata(&self) -> &Self::Metadata {
        match self {
            Entry::File_(f) => &f.metadata,
            Entry::Dir_(d) => &d.metadata,
        }
    }

    /// If `self` is a file, returns `Some` of a reference to the file.
    /// Otherwise returns `None`.
    fn as_file(&self) -> Option<&Self::File> {
        match self {
            Entry::File_(f) => Some(f),
            _ => None,
        }
    }

    /// If `self` is a directory, returns `Some` of a reference to the
    /// directory. Otherwise returns `None`.
    fn as_dir(&self) -> Option<&Self::Dir> {
        match self {
            Entry::Dir_(d) => Some(d),
            _ => None,
        }
    }

    /// If `self` is a file, returns `Some` of the file. Otherwise returns
    /// `None`.
    fn into_file(self) -> Option<Self::File> {
        match self {
            Entry::File_(f) => Some(f),
            _ => None,
        }
    }

    /// If `self` is a directory, returns `Some` of the directory. Otherwise
    /// returns `None`.
    fn into_dir(self) -> Option<Self::Dir> {
        match self {
            Entry::Dir_(d) => Some(d),
            _ => None,
        }
    }
}
