import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { parseTag, checkVersions, branchForTag } from './release-check.mjs';

test('stable and beta tags map to channels', () => {
  assert.deepEqual(parseTag('v1.2.3'), { version: '1.2.3', channel: 'stable' });
  assert.deepEqual(parseTag('v1.2.3-beta.4'), { version: '1.2.3-beta.4', channel: 'beta' });
});

test('beta tags release from master and stable tags release from release', () => {
  assert.equal(branchForTag('v1.2.3-beta.4'), 'master');
  assert.equal(branchForTag('v1.2.3'), 'release');
});

test('invalid or ambiguous tags are rejected', () => {
  for (const tag of ['v1.2', 'v01.2.3', 'v1.2.3-beta.0', 'v1.2.3-rc.1',
    'v1.2.3-beta.01', 'v1.2.3+build', 'v1.2.3-beta.1-extra']) {
    assert.throws(() => parseTag(tag));
  }
});

test('current manifest versions agree', () => {
  const version = JSON.parse(readFileSync('apps/wield/package.json', 'utf8')).version;
  assert.equal(checkVersions(`v${version}`).version, version);
  assert.throws(() => checkVersions('v999.999.999'), /must match/);
});
