// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export function add(a, b) {
	return a + b;
}

export function divide(a, b) {
	if (b === 0) {
		throw new RangeError('division by zero');
	}
	return a / b;
}

export function round(x) {
	return Math.floor(x);
}
