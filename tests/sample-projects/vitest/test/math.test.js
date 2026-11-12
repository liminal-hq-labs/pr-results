// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'vitest';
import { add, divide, round } from '../src/math.js';

describe('math', () => {
	test('adds', () => {
		expect(add(2, 2)).toBe(4);
	});

	describe('division', () => {
		test('divides', () => {
			expect(divide(9, 3)).toBe(3);
		});

		test('rejects zero', () => {
			expect(() => divide(1, 0)).toThrow(RangeError);
		});

		describe('rounding', () => {
			test('rounds down', () => {
				expect(round(2.7)).toBe(2);
			});

			test('fails with a stack', () => {
				expect(add(2, 2)).toBe(5);
			});
		});
	});

	test.skip('is skipped', () => {
		expect(true).toBe(false);
	});

	test.todo('is still to do');
});

test('works outside any describe', () => {
	expect(add(1, 1)).toBe(2);
});
