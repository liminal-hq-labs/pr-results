// Tests for the artefact zip reader: what it reads, and a built archive for every rejection.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]

mod common;

use common::zip::{build, build_with_comment, eocd, patch16, patch32, Spec};
use pr_results_github::{read_zip, ArchiveError, ArchiveLimits};

fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ArchiveError> {
	read_zip(bytes, &ArchiveLimits::default())
		.map(|v| v.into_iter().map(|e| (e.name, e.data)).collect())
}

fn refused(bytes: &[u8]) -> ArchiveError {
	read(bytes).unwrap_err()
}

#[test]
fn it_reads_the_real_artefact_that_upload_artifact_wrote() {
	let bytes = std::fs::read(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/tests/fixtures/real-artifact.zip"
	))
	.unwrap();
	let entries = read_zip(&bytes, &ArchiveLimits::default()).unwrap();
	assert_eq!(entries.len(), 1);
	assert_eq!(entries[0].name, "frames-native.tar");
	assert_eq!(entries[0].data.len(), 30_720);
	// A tar header: the name, then the `ustar` magic at byte 257.
	assert!(entries[0].data.starts_with(b"dist/shell-native-8/"));
	assert_eq!(&entries[0].data[257..262], b"ustar");
}

#[test]
fn it_reads_stored_and_deflated_entries() {
	let text = b"<testsuites/>\n".repeat(200);
	let bytes = build(&[
		Spec::stored("a.xml", b"<a/>"),
		Spec::deflated("dir/b.xml", &text),
		Spec::stored("empty.xml", b""),
		Spec::deflated("empty-deflated.xml", b""),
	]);
	let got = read(&bytes).unwrap();
	assert_eq!(got.len(), 4);
	assert_eq!(got[0], ("a.xml".to_owned(), b"<a/>".to_vec()));
	assert_eq!(got[1], ("dir/b.xml".to_owned(), text));
	assert!(got[2].1.is_empty() && got[3].1.is_empty());
}

#[test]
fn directories_count_but_are_not_returned() {
	let bytes = build(&[Spec::directory("dir/"), Spec::stored("dir/a.xml", b"x")]);
	let got = read(&bytes).unwrap();
	assert_eq!(got, vec![("dir/a.xml".to_owned(), b"x".to_vec())]);
	let limits = ArchiveLimits {
		max_files: 1,
		..ArchiveLimits::default()
	};
	assert_eq!(
		read_zip(&bytes, &limits).unwrap_err(),
		ArchiveError::TooManyFiles
	);
}

#[test]
fn an_archive_with_no_entries_is_empty() {
	assert!(read(&build(&[])).unwrap().is_empty());
}

#[test]
fn a_long_comment_does_not_hide_or_fake_the_directory() {
	let honest = build_with_comment(&[Spec::stored("a.xml", b"ok")], &vec![b'x'; 60_000]);
	assert_eq!(read(&honest).unwrap().len(), 1);
	// A comment that ends in a look-alike end record is found first, as in any reader that
	// searches from the end, and it is refused because it describes no directory.
	let mut fake = vec![0x50, 0x4b, 0x05, 0x06];
	fake.extend_from_slice(&[0; 18]);
	let tricky = build_with_comment(&[Spec::stored("a.xml", b"ok")], &fake);
	assert_eq!(refused(&tricky), ArchiveError::BadOffset);
	// One that sits earlier in the comment is skipped, because its own comment length does not
	// reach the end of the file.
	let mut early = vec![0x50, 0x4b, 0x05, 0x06];
	early.extend_from_slice(&[0; 18]);
	early.extend_from_slice(b"padding after it");
	let skipped = build_with_comment(&[Spec::stored("a.xml", b"ok")], &early);
	assert_eq!(read(&skipped).unwrap().len(), 1);
	// A comment longer than the field allows is not found at all.
	let mut far = build(&[Spec::stored("a.xml", b"ok")]);
	far.extend(vec![0u8; 70_000]);
	assert_eq!(refused(&far), ArchiveError::NotAZip);
}

#[test]
fn not_a_zip() {
	assert_eq!(refused(b""), ArchiveError::NotAZip);
	assert_eq!(refused(b"PK"), ArchiveError::NotAZip);
	assert_eq!(refused(&[0u8; 100]), ArchiveError::NotAZip);
	assert_eq!(
		refused(b"just some text, long enough to hold a record"),
		ArchiveError::NotAZip
	);
	let whole = build(&[Spec::stored("a.xml", b"ok")]);
	assert_eq!(refused(&whole[..whole.len() - 1]), ArchiveError::NotAZip);
	assert_eq!(refused(&whole[10..]), ArchiveError::BadOffset);
	let mut trailing = whole.clone();
	trailing.push(0);
	assert_eq!(refused(&trailing), ArchiveError::NotAZip);
}

#[test]
fn zip64_is_refused_wherever_it_shows() {
	let base = build(&[Spec::stored("a.xml", b"ok")]);

	let mut e = base.clone();
	let at = eocd(&e);
	patch16(&mut e, at + 10, 0xFFFF);
	patch16(&mut e, at + 8, 0xFFFF);
	assert_eq!(refused(&e), ArchiveError::Zip64);

	let mut e = base.clone();
	patch32(&mut e, at + 16, 0xFFFF_FFFF);
	assert_eq!(refused(&e), ArchiveError::Zip64);

	// A zip64 locator just before the end record.
	let mut e = base[..at].to_vec();
	e.extend_from_slice(&[0x50, 0x4b, 0x06, 0x07]);
	e.extend_from_slice(&[0u8; 16]);
	e.extend_from_slice(&base[at..]);
	assert_eq!(refused(&e), ArchiveError::Zip64);

	let sizes = [
		Spec {
			size: Some(u32::MAX),
			..Spec::stored("a.xml", b"ok")
		},
		Spec {
			compressed: Some(u32::MAX),
			..Spec::stored("a.xml", b"ok")
		},
	];
	for s in sizes {
		assert_eq!(refused(&build(&[s])), ArchiveError::Zip64);
	}
}

#[test]
fn archives_split_over_disks_are_refused() {
	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	patch16(&mut e, at + 4, 1);
	assert_eq!(refused(&e), ArchiveError::MultiDisk);

	let e = build(&[Spec {
		disk: 1,
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::MultiDisk);

	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	patch16(&mut e, at + 8, 2);
	assert_eq!(refused(&e), ArchiveError::MultiDisk);
}

#[test]
fn encryption_is_refused() {
	for flags in [0x0001u16, 0x0040, 0x2000, 0x0009] {
		let e = build(&[Spec {
			flags,
			..Spec::stored("a.xml", b"ok")
		}]);
		assert_eq!(refused(&e), ArchiveError::Encrypted, "flags {flags:#x}");
	}
}

#[test]
fn only_stored_and_deflate_are_read() {
	for method in [1u16, 6, 9, 12, 14, 93, 95, 99] {
		let e = build(&[Spec {
			method,
			..Spec::stored("a.xml", b"ok")
		}]);
		assert_eq!(refused(&e), ArchiveError::Method(method));
	}
}

#[test]
fn names_that_could_escape_are_refused() {
	let bad = [
		"../x.xml",
		"a/../../x.xml",
		"a/..",
		"..",
		".",
		"./a.xml",
		"a/./b.xml",
		"/etc/passwd",
		"\\windows\\a.xml",
		"a\\b.xml",
		"C:\\a.xml",
		"c:a.xml",
		"a//b.xml",
		"a\0b.xml",
		"a\nb.xml",
		"a\u{1b}[31mb.xml",
		"",
	];
	for name in bad {
		let e = build(&[Spec::stored(name, b"ok")]);
		assert_eq!(refused(&e), ArchiveError::BadPath, "{name:?}");
	}
	let long = format!("{}.xml", "a".repeat(1100));
	assert_eq!(
		refused(&build(&[Spec::stored(&long, b"ok")])),
		ArchiveError::BadPath
	);
	let invalid = Spec {
		name: vec![b'a', 0xff, 0xfe, b'.', b'x'],
		..Spec::stored("x", b"ok")
	};
	assert_eq!(refused(&build(&[invalid])), ArchiveError::BadPath);
	// A directory with a bad name is as bad as a file.
	assert_eq!(
		refused(&build(&[Spec::directory("../d/")])),
		ArchiveError::BadPath
	);
	// Names that only look odd are fine.
	for name in [
		"ab:c.xml",
		"a b.xml",
		"..a.xml",
		"a..b/c.xml",
		"\u{e9}t\u{e9}.xml",
	] {
		assert_eq!(
			read(&build(&[Spec::stored(name, b"ok")])).unwrap().len(),
			1,
			"{name:?}"
		);
	}
}

#[test]
fn a_name_may_not_appear_twice() {
	let e = build(&[Spec::stored("a.xml", b"1"), Spec::stored("a.xml", b"2")]);
	assert_eq!(refused(&e), ArchiveError::Duplicate);
}

#[test]
fn only_regular_files_and_directories_are_read() {
	for mode in [0o120_777u32, 0o060_644, 0o020_644, 0o010_644, 0o140_644] {
		let e = build(&[Spec {
			attrs: mode << 16,
			..Spec::stored("link", b"/etc/passwd")
		}]);
		assert_eq!(refused(&e), ArchiveError::SpecialFile, "mode {mode:o}");
	}
	// A directory mode on a name that is not a directory, and the reverse.
	let e = build(&[Spec {
		attrs: 0o040_755 << 16,
		..Spec::stored("looks-like-a-file", b"")
	}]);
	assert_eq!(refused(&e), ArchiveError::SpecialFile);
	let e = build(&[Spec {
		attrs: 0o100_644 << 16,
		..Spec::stored("dir/", b"")
	}]);
	assert_eq!(
		read(&e).unwrap().len(),
		0,
		"a trailing slash is a directory"
	);
	// A directory that holds data.
	let e = build(&[Spec {
		data: b"hidden".to_vec(),
		..Spec::directory("dir/")
	}]);
	assert_eq!(refused(&e), ArchiveError::SpecialFile);
	// A FAT-style directory bit on a name without a slash.
	let e = build(&[Spec {
		made_by: 0x0014,
		attrs: 0x10,
		..Spec::stored("dir", b"")
	}]);
	assert_eq!(refused(&e), ArchiveError::SpecialFile);
	// A FAT-style file is fine.
	let e = build(&[Spec {
		made_by: 0x0014,
		attrs: 0x20,
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(read(&e).unwrap().len(), 1);
}

#[test]
fn offsets_that_point_outside_are_refused() {
	// A local header past the end.
	let e = build(&[Spec {
		offset_delta: 1_000_000,
		..Spec::stored("a.xml", b"ok")
	}]);
	assert!(matches!(
		refused(&e),
		ArchiveError::BadOffset | ArchiveError::NotAZip
	));
	// A local header pointing into the middle of nothing.
	let e = build(&[Spec {
		offset_delta: 3,
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
	// A local header that names a different file than the directory.
	let e = build(&[Spec {
		local_name: Some(b"b.xml".to_vec()),
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
	// Data that runs on into the central directory.
	let e = build(&[Spec {
		compressed: Some(500),
		size: Some(500),
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
	// A directory that does not end where the end record starts.
	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	let size = u32::from_le_bytes(e[at + 12..at + 16].try_into().unwrap());
	patch32(&mut e, at + 12, size - 1);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
	// A directory that starts beyond its end record.
	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	patch32(&mut e, at + 16, 1_000_000);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
	// More entries declared than the directory holds.
	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	patch16(&mut e, at + 8, 2);
	patch16(&mut e, at + 10, 2);
	assert!(matches!(
		refused(&e),
		ArchiveError::NotAZip | ArchiveError::BadOffset
	));
	// A directory entry that does not start with the signature.
	let mut e = build(&[Spec::stored("a.xml", b"ok")]);
	let at = eocd(&e);
	let dir = u32::from_le_bytes(e[at + 16..at + 20].try_into().unwrap()) as usize;
	e[dir] = 0;
	assert_eq!(refused(&e), ArchiveError::NotAZip);
	// Fewer entries declared than the directory holds.
	let mut e = build(&[Spec::stored("a.xml", b"ok"), Spec::stored("b.xml", b"ok")]);
	let at = eocd(&e);
	patch16(&mut e, at + 8, 1);
	patch16(&mut e, at + 10, 1);
	assert_eq!(refused(&e), ArchiveError::BadOffset);
}

#[test]
fn too_many_entries() {
	let specs: Vec<Spec> = (0..501)
		.map(|i| Spec::stored(&format!("f{i}.xml"), b"x"))
		.collect();
	let e = build(&specs);
	assert_eq!(refused(&e), ArchiveError::TooManyFiles);
	let ok = build(&specs[..500]);
	assert_eq!(read(&ok).unwrap().len(), 500);
}

#[test]
fn a_zip_bomb_is_refused_before_it_is_inflated() {
	let zeros = vec![0u8; 10 * 1024 * 1024];
	let e = build(&[Spec::deflated("zeros.xml", &zeros)]);
	assert!(e.len() < 100_000, "{} bytes", e.len());
	let limits = ArchiveLimits {
		max_entry_bytes: 1024 * 1024,
		..ArchiveLimits::default()
	};
	assert_eq!(
		read_zip(&e, &limits).unwrap_err(),
		ArchiveError::EntryTooLarge
	);
	// With room it is read, so the refusal above was the limit and not a fault.
	assert_eq!(read(&e).unwrap()[0].1.len(), zeros.len());
}

#[test]
fn the_total_is_limited_as_well_as_each_entry() {
	let e = build(&[
		Spec::stored("a.xml", &[b'a'; 600]),
		Spec::stored("b.xml", &[b'b'; 600]),
	]);
	let limits = ArchiveLimits {
		max_entry_bytes: 1000,
		max_total_bytes: 1000,
		..ArchiveLimits::default()
	};
	assert_eq!(
		read_zip(&e, &limits).unwrap_err(),
		ArchiveError::TotalTooLarge
	);
	let enough = ArchiveLimits {
		max_total_bytes: 1200,
		..limits
	};
	assert_eq!(read_zip(&e, &enough).unwrap().len(), 2);
}

#[test]
fn a_header_that_lies_about_its_size_costs_nothing() {
	// The directory says 100 bytes, the stream inflates to 10 000: inflating stops at 101.
	let big = vec![b'z'; 10_000];
	let e = build(&[Spec {
		size: Some(100),
		..Spec::deflated("lie.xml", &big)
	}]);
	assert_eq!(refused(&e), ArchiveError::Corrupt);
	// The directory says more than the stream holds.
	let e = build(&[Spec {
		size: Some(20_000),
		..Spec::deflated("lie.xml", &big)
	}]);
	assert_eq!(refused(&e), ArchiveError::Corrupt);
	// Stored data whose two sizes disagree.
	let e = build(&[Spec {
		size: Some(1),
		..Spec::stored("lie.xml", b"two")
	}]);
	assert_eq!(refused(&e), ArchiveError::Corrupt);
	// A deflate stream that is not one.
	let e = build(&[Spec {
		method: 8,
		compressed: Some(4),
		size: Some(4),
		data: b"\xff\xff\xff\xff".to_vec(),
		..Spec::stored("junk.xml", b"")
	}]);
	assert_eq!(refused(&e), ArchiveError::Corrupt);
}

#[test]
fn the_checksum_is_checked() {
	let e = build(&[Spec {
		crc: Some(0xdead_beef),
		..Spec::stored("a.xml", b"ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::CrcMismatch);
	let e = build(&[Spec {
		crc: Some(1),
		..Spec::deflated("a.xml", b"ok ok ok ok ok")
	}]);
	assert_eq!(refused(&e), ArchiveError::CrcMismatch);
	// The same bytes with the right checksum are read: the reader's CRC-32 is the standard one.
	let honest = build(&[Spec::deflated("a.xml", b"ok ok ok ok ok")]);
	assert_eq!(read(&honest).unwrap()[0].1, b"ok ok ok ok ok");
}

#[test]
fn errors_name_no_entry() {
	let all = [
		ArchiveError::NotAZip,
		ArchiveError::Zip64,
		ArchiveError::MultiDisk,
		ArchiveError::Encrypted,
		ArchiveError::Method(9),
		ArchiveError::BadPath,
		ArchiveError::Duplicate,
		ArchiveError::SpecialFile,
		ArchiveError::BadOffset,
		ArchiveError::TooManyFiles,
		ArchiveError::EntryTooLarge,
		ArchiveError::TotalTooLarge,
		ArchiveError::Corrupt,
		ArchiveError::CrcMismatch,
	];
	for e in all {
		assert!(!e.to_string().is_empty());
	}
	let e = build(&[Spec::stored("secret-name.xml", b"ok")]);
	let entries = read_zip(&e, &ArchiveLimits::default()).unwrap();
	assert!(
		!format!("{:?}", entries[0]).contains("ok"),
		"Debug prints sizes only"
	);
}
