// SHA-256 and HMAC-SHA-256, written out so the anonymizer needs no crate.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! FIPS 180-4 SHA-256 and RFC 2104 HMAC over it. Only the anonymizer uses them, to turn a word
//! into a token that cannot be guessed without the key. The tests hold the NIST vectors for
//! SHA-256 and RFC 4231 test cases 1, 2, 3 and 6 for HMAC.

const K: [u32; 64] = [
	0x428a_2f98,
	0x7137_4491,
	0xb5c0_fbcf,
	0xe9b5_dba5,
	0x3956_c25b,
	0x59f1_11f1,
	0x923f_82a4,
	0xab1c_5ed5,
	0xd807_aa98,
	0x1283_5b01,
	0x2431_85be,
	0x550c_7dc3,
	0x72be_5d74,
	0x80de_b1fe,
	0x9bdc_06a7,
	0xc19b_f174,
	0xe49b_69c1,
	0xefbe_4786,
	0x0fc1_9dc6,
	0x240c_a1cc,
	0x2de9_2c6f,
	0x4a74_84aa,
	0x5cb0_a9dc,
	0x76f9_88da,
	0x983e_5152,
	0xa831_c66d,
	0xb003_27c8,
	0xbf59_7fc7,
	0xc6e0_0bf3,
	0xd5a7_9147,
	0x06ca_6351,
	0x1429_2967,
	0x27b7_0a85,
	0x2e1b_2138,
	0x4d2c_6dfc,
	0x5338_0d13,
	0x650a_7354,
	0x766a_0abb,
	0x81c2_c92e,
	0x9272_2c85,
	0xa2bf_e8a1,
	0xa81a_664b,
	0xc24b_8b70,
	0xc76c_51a3,
	0xd192_e819,
	0xd699_0624,
	0xf40e_3585,
	0x106a_a070,
	0x19a4_c116,
	0x1e37_6c08,
	0x2748_774c,
	0x34b0_bcb5,
	0x391c_0cb3,
	0x4ed8_aa4a,
	0x5b9c_ca4f,
	0x682e_6ff3,
	0x748f_82ee,
	0x78a5_636f,
	0x84c8_7814,
	0x8cc7_0208,
	0x90be_fffa,
	0xa450_6ceb,
	0xbef9_a3f7,
	0xc671_78f2,
];

const H0: [u32; 8] = [
	0x6a09_e667,
	0xbb67_ae85,
	0x3c6e_f372,
	0xa54f_f53a,
	0x510e_527f,
	0x9b05_688c,
	0x1f83_d9ab,
	0x5be0_cd19,
];

const BLOCK: usize = 64;

/// A word of the message schedule, or nothing (zero) past the ends, which no caller reaches.
fn at(w: &[u32], i: usize) -> u32 {
	w.get(i).copied().unwrap_or(0)
}

/// One 64-byte block into the state.
fn compress(state: &mut [u32; 8], chunk: &[u8]) {
	let mut w: Vec<u32> = chunk
		.chunks_exact(4)
		.map(|b| u32::from_be_bytes(<[u8; 4]>::try_from(b).unwrap_or_default()))
		.collect();
	for i in 16..64 {
		let s0 = at(&w, i - 15).rotate_right(7)
			^ at(&w, i - 15).rotate_right(18)
			^ (at(&w, i - 15) >> 3);
		let s1 =
			at(&w, i - 2).rotate_right(17) ^ at(&w, i - 2).rotate_right(19) ^ (at(&w, i - 2) >> 10);
		w.push(
			at(&w, i - 16)
				.wrapping_add(s0)
				.wrapping_add(at(&w, i - 7))
				.wrapping_add(s1),
		);
	}
	let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
	for (k, word) in K.iter().zip(&w) {
		let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
		let ch = (e & f) ^ (!e & g);
		let t1 = h
			.wrapping_add(s1)
			.wrapping_add(ch)
			.wrapping_add(*k)
			.wrapping_add(*word);
		let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
		let maj = (a & b) ^ (a & c) ^ (b & c);
		let t2 = s0.wrapping_add(maj);
		h = g;
		g = f;
		f = e;
		e = d.wrapping_add(t1);
		d = c;
		c = b;
		b = a;
		a = t1.wrapping_add(t2);
	}
	for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
		*s = s.wrapping_add(v);
	}
}

/// SHA-256 of `data`.
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
	let mut state = H0;
	let mut chunks = data.chunks_exact(BLOCK);
	for chunk in &mut chunks {
		compress(&mut state, chunk);
	}
	let mut tail = chunks.remainder().to_vec();
	tail.push(0x80);
	while tail.len() % BLOCK != BLOCK - 8 {
		tail.push(0);
	}
	tail.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_be_bytes());
	for chunk in tail.chunks_exact(BLOCK) {
		compress(&mut state, chunk);
	}
	let mut out = [0u8; 32];
	for (dst, word) in out.chunks_exact_mut(4).zip(state) {
		dst.copy_from_slice(&word.to_be_bytes());
	}
	out
}

/// HMAC-SHA-256 of `message` under `key`.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
	let mut block = [0u8; BLOCK];
	if key.len() > BLOCK {
		for (dst, src) in block.iter_mut().zip(sha256(key)) {
			*dst = src;
		}
	} else {
		for (dst, src) in block.iter_mut().zip(key) {
			*dst = *src;
		}
	}
	let mut inner: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
	inner.extend_from_slice(message);
	let mut outer: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
	outer.extend_from_slice(&sha256(&inner));
	sha256(&outer)
}

#[cfg(test)]
#[allow(
	clippy::unwrap_used,
	clippy::expect_used,
	clippy::panic,
	clippy::indexing_slicing
)]
mod tests {
	use super::{hmac_sha256, sha256};
	use std::fmt::Write as _;

	fn hex(bytes: &[u8]) -> String {
		bytes.iter().fold(String::new(), |mut s, b| {
			let _ = write!(s, "{b:02x}");
			s
		})
	}

	#[test]
	fn sha256_matches_the_nist_vectors() {
		assert_eq!(
			hex(&sha256(b"")),
			"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
		);
		assert_eq!(
			hex(&sha256(b"abc")),
			"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
		);
		assert_eq!(
			hex(&sha256(
				b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
			)),
			"248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
		);
		assert_eq!(
			hex(&sha256(b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu")),
			"cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"
		);
		assert_eq!(
			hex(&sha256(&vec![b'a'; 1_000_000])),
			"cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
		);
	}

	#[test]
	fn sha256_handles_every_padding_boundary() {
		// The length where the padding spills into one more block (55, 56 and 64 bytes) must not
		// change which input gives which output.
		let mut seen = std::collections::BTreeSet::new();
		for n in 0..200usize {
			assert!(seen.insert(sha256(&vec![b'x'; n])), "{n}");
		}
		assert_eq!(
			hex(&sha256(&[b'a'; 55])),
			"9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
		);
		assert_eq!(
			hex(&sha256(&[b'a'; 56])),
			"b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
		);
		assert_eq!(
			hex(&sha256(&[b'a'; 64])),
			"ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
		);
	}

	#[test]
	fn hmac_matches_rfc_4231() {
		// Test case 1.
		assert_eq!(
			hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
			"b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
		);
		// Test case 2.
		assert_eq!(
			hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
			"5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
		);
		// Test case 3.
		assert_eq!(
			hex(&hmac_sha256(&[0xaa; 20], &[0xdd; 50])),
			"773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
		);
		// Test case 6: a key longer than the block is hashed first.
		assert_eq!(
			hex(&hmac_sha256(
				&[0xaa; 131],
				b"Test Using Larger Than Block-Size Key - Hash Key First"
			)),
			"60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
		);
	}
}
