// A small zip writer for tests, with a field for everything a hostile archive might lie about.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

/// One entry. `None` means "what an honest writer would put".
#[derive(Clone)]
pub struct Spec {
	pub name: Vec<u8>,
	pub data: Vec<u8>,
	pub method: u16,
	pub flags: u16,
	pub made_by: u16,
	pub attrs: u32,
	pub size: Option<u32>,
	pub compressed: Option<u32>,
	pub crc: Option<u32>,
	/// The name written in the local header, if it should differ.
	pub local_name: Option<Vec<u8>>,
	/// Added to the local header offset written in the directory.
	pub offset_delta: i64,
	pub disk: u16,
}

impl Spec {
	/// A regular file made on Unix, stored.
	pub fn stored(name: &str, data: &[u8]) -> Self {
		Self {
			name: name.as_bytes().to_vec(),
			data: data.to_vec(),
			method: 0,
			flags: 0,
			made_by: 0x031e,
			attrs: 0o100_644 << 16,
			size: None,
			compressed: None,
			crc: None,
			local_name: None,
			offset_delta: 0,
			disk: 0,
		}
	}

	pub fn deflated(name: &str, data: &[u8]) -> Self {
		Self {
			method: 8,
			..Self::stored(name, data)
		}
	}

	pub fn directory(name: &str) -> Self {
		Self {
			attrs: (0o040_755 << 16) | 0x10,
			..Self::stored(name, b"")
		}
	}
}

pub fn crc32(data: &[u8]) -> u32 {
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

fn le16(out: &mut Vec<u8>, v: u16) {
	out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
	out.extend_from_slice(&v.to_le_bytes());
}

pub fn build(specs: &[Spec]) -> Vec<u8> {
	build_with_comment(specs, b"")
}

pub fn build_with_comment(specs: &[Spec], comment: &[u8]) -> Vec<u8> {
	let mut out = Vec::new();
	let mut central = Vec::new();
	for s in specs {
		let payload = if s.method == 8 {
			miniz_oxide::deflate::compress_to_vec(&s.data, 6)
		} else {
			s.data.clone()
		};
		let crc = s.crc.unwrap_or_else(|| crc32(&s.data));
		let size = s.size.unwrap_or(s.data.len() as u32);
		let compressed = s.compressed.unwrap_or(payload.len() as u32);
		let offset = out.len() as u32;
		let local_name = s.local_name.clone().unwrap_or_else(|| s.name.clone());

		le32(&mut out, 0x0403_4b50);
		le16(&mut out, 20);
		le16(&mut out, s.flags);
		le16(&mut out, s.method);
		le32(&mut out, 0);
		le32(&mut out, crc);
		le32(&mut out, compressed);
		le32(&mut out, size);
		le16(&mut out, local_name.len() as u16);
		le16(&mut out, 0);
		out.extend_from_slice(&local_name);
		out.extend_from_slice(&payload);

		le32(&mut central, 0x0201_4b50);
		le16(&mut central, s.made_by);
		le16(&mut central, 20);
		le16(&mut central, s.flags);
		le16(&mut central, s.method);
		le32(&mut central, 0);
		le32(&mut central, crc);
		le32(&mut central, compressed);
		le32(&mut central, size);
		le16(&mut central, s.name.len() as u16);
		le16(&mut central, 0);
		le16(&mut central, 0);
		le16(&mut central, s.disk);
		le16(&mut central, 0);
		le32(&mut central, s.attrs);
		le32(&mut central, (i64::from(offset) + s.offset_delta) as u32);
		central.extend_from_slice(&s.name);
	}
	let cd_offset = out.len() as u32;
	let cd_size = central.len() as u32;
	out.extend_from_slice(&central);
	le32(&mut out, 0x0605_4b50);
	le16(&mut out, 0);
	le16(&mut out, 0);
	le16(&mut out, specs.len() as u16);
	le16(&mut out, specs.len() as u16);
	le32(&mut out, cd_size);
	le32(&mut out, cd_offset);
	le16(&mut out, comment.len() as u16);
	out.extend_from_slice(comment);
	out
}

/// Where the end-of-directory record starts.
pub fn eocd(bytes: &[u8]) -> usize {
	bytes
		.windows(4)
		.rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
		.unwrap()
}

pub fn patch16(bytes: &mut [u8], at: usize, v: u16) {
	bytes[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn patch32(bytes: &mut [u8], at: usize, v: u32) {
	bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
