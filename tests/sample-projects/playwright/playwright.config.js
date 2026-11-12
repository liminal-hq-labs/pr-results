// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defineConfig } from '@playwright/test';

// The tests never open a browser: two projects exist so that the same spec runs twice.
export default defineConfig({
	testDir: 'test',
	retries: 2,
	workers: 1,
	reporter: [['list'], ['junit', { outputFile: 'reports/' + (process.env.VARIANT ?? 'default') + '.xml' }]],
	projects: [{ name: 'chromium' }, { name: 'firefox' }],
});
