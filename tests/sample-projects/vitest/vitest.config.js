// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defineConfig } from 'vitest/config';

// The generator picks the variant with VARIANT: "default", "file" (addFileAttribute) or
// "projects" (two projects).
const variant = process.env.VARIANT ?? 'default';
const junit = (name) => [
	'junit',
	{ outputFile: `reports/${name}.xml`, addFileAttribute: variant === 'file' },
];

const shared = { retry: 2, include: ['test/**/*.test.js'] };

export default defineConfig({
	test:
		variant === 'projects'
			? {
					reporters: ['default', junit('projects')],
					projects: [
						{ test: { ...shared, name: 'unit', include: ['test/*.test.js'] } },
						{ test: { ...shared, name: 'nested', include: ['test/sub/*.test.js'] } },
					],
				}
			: { ...shared, reporters: ['default', junit(variant)] },
});
