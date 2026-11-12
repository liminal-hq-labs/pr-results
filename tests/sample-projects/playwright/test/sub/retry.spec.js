// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expect, test } from '@playwright/test';
import { divide } from '../../src/math.js';

test.describe('retries', () => {
	test('passes on the second try', async ({}, testInfo) => {
		expect(testInfo.retry).toBeGreaterThanOrEqual(1);
	});

	test('never passes', () => {
		throw new Error('always broken');
	});

	test('throws from the source', () => {
		expect(divide(1, 0)).toBe(Infinity);
	});
});
