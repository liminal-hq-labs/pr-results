// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expect, test } from '@playwright/test';
import { add, divide, round } from '../src/math.js';

test.describe('math', () => {
	test('adds', () => {
		expect(add(2, 2)).toBe(4);
	});

	test.describe('division', () => {
		test('divides', () => {
			expect(divide(9, 3)).toBe(3);
		});

		test.describe('rounding', () => {
			test('rounds down', () => {
				expect(round(2.7)).toBe(2);
			});

			test('fails with a stack', async ({}, testInfo) => {
				await testInfo.attach('note', { body: 'a note for the report', contentType: 'text/plain' });
				expect(add(2, 2)).toBe(5);
			});
		});
	});

	test.skip('is skipped', () => {
		expect(true).toBe(false);
	});

	test.fixme('is still to do', () => {});
});

test('works outside any describe', () => {
	expect(add(1, 1)).toBe(2);
});
