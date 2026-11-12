// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { divide } from '../../src/math.js';

let tries = 0;

describe('retries', () => {
	test('passes on the second try', () => {
		tries += 1;
		if (tries < 2) {
			throw new Error('not yet');
		}
		expect(tries).toBe(2);
	});

	test('never passes', () => {
		throw new Error('always broken');
	});

	test('fails with an empty message', () => {
		throw new Error('');
	});

	test('throws from the source', () => {
		expect(divide(1, 0)).toBe(Infinity);
	});
});
