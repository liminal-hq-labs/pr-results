// Reading an artefact zip: our own reader, for the two methods `upload-artifact` writes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A zip from a fork's CI is hostile input. This reader takes the least it can: it reads the
//! central directory only (never walks local headers to find entries), accepts stored and
//! deflated entries, and checks every offset and length with checked arithmetic before it
//! slices. It refuses what it does not need: zip64, encryption, other methods, multi-disk
//! archives, special files and any name that could leave the directory it is unpacked into.
//! Nothing is written to disk. Inflating stops one byte past the declared size, so a lying
//! header costs nothing.

use std::collections::BTreeSet;
use std::fmt;

use miniz_oxide::inflate::decompress_to_vec_with_limit;

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const EOCD64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const LOCAL_LEN: usize = 30;
/// The end-of-directory record is within this many bytes of the end: its length and the longest
/// comment a 16-bit field can hold.
const EOCD_SEARCH: usize = EOCD_LEN + 0xFFFF;
const MAX_NAME_BYTES: usize = 1024;

/// What an artefact may hold. The defaults are the settings' defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
	/// Entries in the directory, directories included.
	pub max_files: usize,
	/// One entry's inflated size.
	pub max_entry_bytes: u64,
	/// All entries' inflated sizes added together.
	pub max_total_bytes: u64,
}

impl Default for ArchiveLimits {
	fn default() -> Self {
		Self {
			max_files: 500,
			max_entry_bytes: 50 * 1024 * 1024,
			max_total_bytes: 100 * 1024 * 1024,
		}
	}
}

/// One regular file of the archive.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
	/// A relative path with `/` separators, free of `.` and `..` and of empty parts.
	pub name: String,
	pub data: Vec<u8>,
}

impl fmt::Debug for Entry {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Entry({} bytes)", self.data.len())
	}
}

/// Why an archive was refused. None of these carries a name from the archive, because a name is
/// hostile text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveError {
	/// There is no end-of-directory record, or the archive is cut short.
	NotAZip,
	Zip64,
	MultiDisk,
	Encrypted,
	/// A method other than stored (0) or deflate (8).
	Method(u16),
	/// An absolute path, a `..`, a backslash, a drive letter, a control character, a repeated or
	/// empty part, a name that is not UTF-8 or one that is too long.
	BadPath,
	Duplicate,
	/// A symbolic link, device or other entry that is not a regular file or a directory.
	SpecialFile,
	/// An offset or length that points outside the archive or into the directory itself.
	BadOffset,
	TooManyFiles,
	EntryTooLarge,
	TotalTooLarge,
	/// The data does not inflate, or does not fill exactly the size the directory declares.
	Corrupt,
	CrcMismatch,
}

impl fmt::Display for ArchiveError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NotAZip => f.write_str("not a zip archive, or cut short"),
			Self::Zip64 => f.write_str("zip64 archives are not supported"),
			Self::MultiDisk => f.write_str("archives split over several disks are not supported"),
			Self::Encrypted => f.write_str("encrypted entries are not supported"),
			Self::Method(m) => write!(f, "compression method {m} is not supported"),
			Self::BadPath => f.write_str("an entry has a name that is not allowed"),
			Self::Duplicate => f.write_str("two entries have the same name"),
			Self::SpecialFile => f.write_str("an entry is not a regular file"),
			Self::BadOffset => f.write_str("an entry points outside the archive"),
			Self::TooManyFiles => f.write_str("the archive has too many entries"),
			Self::EntryTooLarge => f.write_str("an entry is larger than the limit"),
			Self::TotalTooLarge => f.write_str("the archive is larger than the limit"),
			Self::Corrupt => f.write_str("an entry does not inflate to its declared size"),
			Self::CrcMismatch => f.write_str("an entry fails its checksum"),
		}
	}
}

impl std::error::Error for ArchiveError {}

/// Reads every regular file of the zip in `bytes`. Directory entries count towards
/// `max_files` and are not returned.
pub fn read_zip(bytes: &[u8], limits: &ArchiveLimits) -> Result<Vec<Entry>, ArchiveError> {
	read_zip_filtered(bytes, limits, &|_| true)
}

/// Like [`read_zip`], but only a file that `keep` accepts is inflated, checked against the byte
/// limits and returned. The names of all entries are still checked, so an archive with a hostile
/// name is refused whatever it holds, but a large entry nobody asked for costs nothing.
pub fn read_zip_filtered(
	bytes: &[u8],
	limits: &ArchiveLimits,
	keep: &dyn Fn(&str) -> bool,
) -> Result<Vec<Entry>, ArchiveError> {
	let eocd = find_eocd(bytes)?;
	let dir = Directory::read(bytes, eocd)?;
	if usize::from(dir.entries) > limits.max_files {
		return Err(ArchiveError::TooManyFiles);
	}

	let mut at = dir.offset;
	let mut total: u64 = 0;
	let mut seen: BTreeSet<&str> = BTreeSet::new();
	let mut out = Vec::new();
	for _ in 0..dir.entries {
		let header = slice(bytes, at, CENTRAL_LEN)?;
		if u32_at(header, 0)? != CENTRAL_SIGNATURE {
			return Err(ArchiveError::NotAZip);
		}
		let made_by = u16_at(header, 4)?;
		let flags = u16_at(header, 8)?;
		let method = u16_at(header, 10)?;
		let crc = u32_at(header, 16)?;
		let compressed = u32_at(header, 20)?;
		let size = u32_at(header, 24)?;
		let name_len = usize::from(u16_at(header, 28)?);
		let extra_len = usize::from(u16_at(header, 30)?);
		let comment_len = usize::from(u16_at(header, 32)?);
		let disk = u16_at(header, 34)?;
		let attrs = u32_at(header, 38)?;
		let local = u32_at(header, 42)?;

		let name_at = at.checked_add(CENTRAL_LEN).ok_or(ArchiveError::BadOffset)?;
		let name_bytes = slice(bytes, name_at, name_len)?;
		let next = name_at
			.checked_add(name_len)
			.and_then(|n| n.checked_add(extra_len))
			.and_then(|n| n.checked_add(comment_len))
			.ok_or(ArchiveError::BadOffset)?;
		if next > dir.end {
			return Err(ArchiveError::BadOffset);
		}

		if flags & 0x0001 != 0 || flags & 0x0040 != 0 || flags & 0x2000 != 0 {
			return Err(ArchiveError::Encrypted);
		}
		if compressed == u32::MAX || size == u32::MAX || local == u32::MAX {
			return Err(ArchiveError::Zip64);
		}
		if disk != 0 {
			return Err(ArchiveError::MultiDisk);
		}
		let name = std::str::from_utf8(name_bytes).map_err(|_| ArchiveError::BadPath)?;
		let kind = kind_of(made_by, attrs, name)?;
		check_name(name, kind == Kind::Directory)?;
		if !seen.insert(name) {
			return Err(ArchiveError::Duplicate);
		}
		if method != 0 && method != 8 {
			return Err(ArchiveError::Method(method));
		}

		if kind == Kind::File && keep(name) {
			if u64::from(size) > limits.max_entry_bytes {
				return Err(ArchiveError::EntryTooLarge);
			}
			total = total
				.checked_add(u64::from(size))
				.filter(|t| *t <= limits.max_total_bytes)
				.ok_or(ArchiveError::TotalTooLarge)?;
			let data = entry_data(bytes, &dir, name, method, crc, compressed, size, local)?;
			out.push(Entry {
				name: name.to_owned(),
				data,
			});
		} else if kind == Kind::Directory && (size != 0 || compressed > 2) {
			// A directory holds nothing. (An empty deflate stream is two bytes.)
			return Err(ArchiveError::SpecialFile);
		}
		at = next;
	}
	if at != dir.end {
		// The directory declares its own length, and the entries have to fill it.
		return Err(ArchiveError::BadOffset);
	}
	Ok(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
	File,
	Directory,
}

/// Regular files and directories only. The mode is the high half of the external attributes when
/// the archive was made on Unix (host 3); FAT-style attributes (host 0) mark a directory with
/// bit 4.
fn kind_of(made_by: u16, attrs: u32, name: &str) -> Result<Kind, ArchiveError> {
	let host = made_by >> 8;
	let dir_name = name.ends_with('/');
	let kind = match host {
		3 => match (attrs >> 16) & 0o170_000 {
			0 | 0o100_000 => {
				if dir_name {
					Kind::Directory
				} else {
					Kind::File
				}
			}
			0o040_000 => Kind::Directory,
			_ => return Err(ArchiveError::SpecialFile),
		},
		_ => {
			if attrs & 0x10 != 0 || dir_name {
				Kind::Directory
			} else {
				Kind::File
			}
		}
	};
	// A name that ends in `/` is a directory whatever the attributes say, and the reverse is a
	// contradiction.
	if (kind == Kind::Directory) != dir_name {
		return Err(ArchiveError::SpecialFile);
	}
	Ok(kind)
}

/// A name that stays inside the directory it is unpacked into, on any platform.
fn check_name(name: &str, directory: bool) -> Result<(), ArchiveError> {
	let name = if directory {
		name.strip_suffix('/').unwrap_or(name)
	} else {
		name
	};
	if name.is_empty() || name.len() > MAX_NAME_BYTES || name.starts_with('/') {
		return Err(ArchiveError::BadPath);
	}
	if name.chars().any(|c| c.is_control() || c == '\\') {
		return Err(ArchiveError::BadPath);
	}
	let mut chars = name.chars();
	if let (Some(first), Some(':')) = (chars.next(), chars.next()) {
		if first.is_ascii_alphabetic() {
			return Err(ArchiveError::BadPath);
		}
	}
	if name
		.split('/')
		.any(|part| part.is_empty() || part == "." || part == "..")
	{
		return Err(ArchiveError::BadPath);
	}
	Ok(())
}

#[allow(clippy::too_many_arguments)]
fn entry_data(
	bytes: &[u8],
	dir: &Directory,
	name: &str,
	method: u16,
	crc: u32,
	compressed: u32,
	size: u32,
	local: u32,
) -> Result<Vec<u8>, ArchiveError> {
	let local = usize::try_from(local).map_err(|_| ArchiveError::BadOffset)?;
	let header = slice(bytes, local, LOCAL_LEN)?;
	if u32_at(header, 0)? != LOCAL_SIGNATURE {
		return Err(ArchiveError::BadOffset);
	}
	let name_len = usize::from(u16_at(header, 26)?);
	let extra_len = usize::from(u16_at(header, 28)?);
	let name_at = local
		.checked_add(LOCAL_LEN)
		.ok_or(ArchiveError::BadOffset)?;
	if slice(bytes, name_at, name_len)? != name.as_bytes() {
		return Err(ArchiveError::BadOffset);
	}
	let start = name_at
		.checked_add(name_len)
		.and_then(|n| n.checked_add(extra_len))
		.ok_or(ArchiveError::BadOffset)?;
	let compressed = usize::try_from(compressed).map_err(|_| ArchiveError::BadOffset)?;
	let end = start
		.checked_add(compressed)
		.ok_or(ArchiveError::BadOffset)?;
	// The data lies before the central directory, never inside it.
	if end > dir.offset {
		return Err(ArchiveError::BadOffset);
	}
	let raw = slice(bytes, start, compressed)?;
	let size = usize::try_from(size).map_err(|_| ArchiveError::EntryTooLarge)?;

	let data = if method == 0 {
		if compressed != size {
			return Err(ArchiveError::Corrupt);
		}
		raw.to_vec()
	} else {
		// One byte past the declared size is enough to tell a stream that is too long.
		let limit = size.checked_add(1).ok_or(ArchiveError::EntryTooLarge)?;
		let inflated =
			decompress_to_vec_with_limit(raw, limit).map_err(|_| ArchiveError::Corrupt)?;
		if inflated.len() != size {
			return Err(ArchiveError::Corrupt);
		}
		inflated
	};
	if crc32(&data) != crc {
		return Err(ArchiveError::CrcMismatch);
	}
	Ok(data)
}

/// Where the central directory is and how many entries it holds.
struct Directory {
	offset: usize,
	/// One past its last byte: where the end-of-directory record starts.
	end: usize,
	entries: u16,
}

impl Directory {
	fn read(bytes: &[u8], eocd: usize) -> Result<Self, ArchiveError> {
		let rec = slice(bytes, eocd, EOCD_LEN)?;
		let disk = u16_at(rec, 4)?;
		let dir_disk = u16_at(rec, 6)?;
		let on_disk = u16_at(rec, 8)?;
		let entries = u16_at(rec, 10)?;
		let size = u32_at(rec, 12)?;
		let offset = u32_at(rec, 16)?;
		if let Some(locator_at) = eocd.checked_sub(20) {
			if let Ok(loc) = slice(bytes, locator_at, 4) {
				if u32_at(loc, 0)? == EOCD64_LOCATOR_SIGNATURE {
					return Err(ArchiveError::Zip64);
				}
			}
		}
		if entries == u16::MAX || size == u32::MAX || offset == u32::MAX || on_disk == u16::MAX {
			return Err(ArchiveError::Zip64);
		}
		if disk != 0 || dir_disk != 0 || on_disk != entries {
			return Err(ArchiveError::MultiDisk);
		}
		let offset = usize::try_from(offset).map_err(|_| ArchiveError::BadOffset)?;
		let size = usize::try_from(size).map_err(|_| ArchiveError::BadOffset)?;
		// The directory ends exactly where the end record begins.
		if offset.checked_add(size) != Some(eocd) {
			return Err(ArchiveError::BadOffset);
		}
		Ok(Self {
			offset,
			end: eocd,
			entries,
		})
	}
}

/// The last end-of-directory record whose comment length reaches the end of the file, searching
/// only the final 64 KiB and a bit.
fn find_eocd(bytes: &[u8]) -> Result<usize, ArchiveError> {
	let len = bytes.len();
	if len < EOCD_LEN {
		return Err(ArchiveError::NotAZip);
	}
	let lowest = len.saturating_sub(EOCD_SEARCH);
	let mut at = len - EOCD_LEN;
	loop {
		if let Ok(rec) = slice(bytes, at, EOCD_LEN) {
			if u32_at(rec, 0)? == EOCD_SIGNATURE {
				let comment = usize::from(u16_at(rec, 20)?);
				if at
					.checked_add(EOCD_LEN)
					.and_then(|n| n.checked_add(comment))
					== Some(len)
				{
					return Ok(at);
				}
			}
		}
		if at <= lowest {
			return Err(ArchiveError::NotAZip);
		}
		at -= 1;
	}
}

fn slice(bytes: &[u8], at: usize, len: usize) -> Result<&[u8], ArchiveError> {
	let end = at.checked_add(len).ok_or(ArchiveError::BadOffset)?;
	bytes.get(at..end).ok_or(ArchiveError::NotAZip)
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, ArchiveError> {
	let end = at.checked_add(2).ok_or(ArchiveError::BadOffset)?;
	let b: [u8; 2] = bytes
		.get(at..end)
		.and_then(|b| b.try_into().ok())
		.ok_or(ArchiveError::NotAZip)?;
	Ok(u16::from_le_bytes(b))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, ArchiveError> {
	let end = at.checked_add(4).ok_or(ArchiveError::BadOffset)?;
	let b: [u8; 4] = bytes
		.get(at..end)
		.and_then(|b| b.try_into().ok())
		.ok_or(ArchiveError::NotAZip)?;
	Ok(u32::from_le_bytes(b))
}

/// CRC-32 (the zip and PNG polynomial), one bit at a time: a table would need indexing, and the
/// archive is capped well below the size where the difference matters.
fn crc32(data: &[u8]) -> u32 {
	let mut crc = !0u32;
	for &byte in data {
		crc ^= u32::from(byte);
		for _ in 0..8 {
			crc = if crc & 1 != 0 {
				0xEDB8_8320 ^ (crc >> 1)
			} else {
				crc >> 1
			};
		}
	}
	!crc
}
