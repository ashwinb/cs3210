use core::fmt;

use crate::traits;

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

impl Date {
    const MONTHS: [&'static str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    fn year(&self) -> usize {
        1980 + ((self.0 & (0x7F << 9)) >> 9) as usize
    }

    fn month(&self) -> u8 {
        ((self.0 & (0xF << 5)) >> 5) as u8
    }

    fn day(&self) -> u8 {
        (self.0 & 0x1F) as u8
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{} {}, {}",
            self.day(),
            Date::MONTHS[self.month() as usize - 1],
            self.year()
        )
    }
}

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

impl Time {
    fn hour(&self) -> u8 {
        ((self.0 & (0x1F << 11)) >> 11) as u8
    }

    fn minute(&self) -> u8 {
        ((self.0 & (0x3F << 5)) >> 5) as u8
    }

    fn second(&self) -> u8 {
        2 * (self.0 & 0x1F) as u8
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}:{}:{}", self.hour(), self.minute(), self.second())
    }
}

/// File attributes as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(pub u8);

impl Attributes {
    fn read_only(&self) -> bool {
        (self.0 & 0x1) == 0x1
    }

    fn hidden(&self) -> bool {
        (self.0 & 0x2) == 0x2
    }

    fn is_lfn(&self) -> bool {
        self.0 != 0xF
    }

    pub fn is_dir(&self) -> bool {
        (self.0 & 0x10) == 0x10
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir() & !self.is_lfn()
    }
}

/// A structure containing a date and time.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.date, self.time)
    }
}

/// Metadata for a directory entry.
#[derive(Default, Debug, Clone)]
pub struct Metadata {
    // FIXME: Fill me in.
    attributes: Attributes,
    created: Timestamp,
    modified: Timestamp,
    accessed: Date,
}

impl Metadata {
    pub fn from(attr: Attributes, created: Timestamp, modified: Timestamp, accessed: Date) -> Self {
        Metadata {
            attributes: attr,
            created,
            modified,
            accessed,
        }
    }
}

// FIXME: Implement `traits::Timestamp` for `Timestamp`.
impl traits::Timestamp for Timestamp {
    fn year(&self) -> usize {
        self.date.year()
    }

    /// The calendar month, starting at 1 for January. Always in range [1, 12].
    ///
    /// January is 1, Feburary is 2, ..., December is 12.
    fn month(&self) -> u8 {
        self.date.month()
    }

    /// The calendar day, starting at 1. Always in range [1, 31].
    fn day(&self) -> u8 {
        self.date.day()
    }

    /// The 24-hour hour. Always in range [0, 24).
    fn hour(&self) -> u8 {
        self.time.hour()
    }

    /// The minute. Always in range [0, 60).
    fn minute(&self) -> u8 {
        self.time.minute()
    }

    /// The second. Always in range [0, 60).
    /// TODO: still doesn't use the slightly higher resolution "seconds" information
    fn second(&self) -> u8 {
        self.time.second()
    }
}

// FIXME: Implement `traits::Metadata` for `Metadata`.
impl traits::Metadata for Metadata {
    type Timestamp = self::Timestamp;

    /// Type corresponding to a point in time.
    /// Whether the associated entry is read only.
    fn read_only(&self) -> bool {
        self.attributes.read_only()
    }

    /// Whether the entry should be "hidden" from directory traversals.
    fn hidden(&self) -> bool {
        self.attributes.hidden()
    }

    /// The timestamp when the entry was created.
    fn created(&self) -> Self::Timestamp {
        self.created
    }

    /// The timestamp for the entry's last access.
    fn accessed(&self) -> Self::Timestamp {
        Timestamp {
            date: self.accessed,
            time: Time(0),
        }
    }

    /// The timestamp for the entry's last modification.
    fn modified(&self) -> Self::Timestamp {
        self.modified
    }
}

// FIXME: Implement `fmt::Display` (to your liking) for `Metadata`.
impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Created: {}", self.created)?;
        if self.attributes.read_only() {
            write!(f, " RO")?;
        }
        if self.attributes.hidden() {
            write!(f, " <hidden>")?;
        }
        Ok(())
    }
}
