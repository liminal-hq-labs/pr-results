// SAMPLE: a tiny project whose JUnit output becomes the real-*.xml fixtures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// VARIANT picks the jest-junit options: "default", "file" (addFileAttribute) or "templates"
// (suiteName, classNameTemplate and titleTemplate set).
const variant = process.env.VARIANT ?? 'default';
const options = {
	default: { outputDirectory: 'reports', outputName: 'default.xml' },
	file: { outputDirectory: 'reports', outputName: 'file.xml', addFileAttribute: 'true' },
	templates: {
		outputDirectory: 'reports',
		outputName: 'templates.xml',
		suiteName: 'sample jest',
		classNameTemplate: '{filepath} {classname}',
		titleTemplate: '{title}',
		addFileAttribute: 'true',
	},
}[variant];

module.exports = {
	testMatch: ['<rootDir>/test/**/*.test.js'],
	reporters: ['default', ['jest-junit', options]],
};
