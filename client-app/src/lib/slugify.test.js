// Run with `npm test`. The Rust port (backend crates/web/src/slug.rs) asserts
// the same fixtures, so a link built on either side reaches the same URL.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { slugify } from './slugify.js';

const fixtures = JSON.parse(
    readFileSync(new URL('./slugify.fixtures.json', import.meta.url), 'utf8'),
);

test('slugify matches the shared fixtures', () => {
    for (const [input, expected] of fixtures) {
        assert.equal(slugify(input), expected, JSON.stringify(input));
    }
});
