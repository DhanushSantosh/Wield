#!/usr/bin/env node
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export function parseTag(tag) {
  const match = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-beta\.([1-9]\d*))?$/.exec(tag);
  if (!match) throw new Error(`Invalid release tag ${tag}; expected vX.Y.Z or vX.Y.Z-beta.N`);
  return { version: tag.slice(1), channel: match[4] ? 'beta' : 'stable' };
}

export function branchForTag(tag) {
  return parseTag(tag).channel === 'beta' ? 'master' : 'release';
}

function json(path) { return JSON.parse(readFileSync(path, 'utf8')); }

export function checkVersions(tag, root = '.') {
  const { version, channel } = parseTag(tag);
  const paths = [
    'apps/wield/package.json',
    'apps/wield/src-tauri/tauri.conf.json',
  ];
  for (const path of paths) {
    const actual = json(join(root, path)).version;
    assert.equal(actual, version, `${path} version must match ${tag}`);
  }
  assert.equal(json(join(root, 'package-lock.json')).packages['apps/wield'].version,
    version, `package-lock.json workspace version must match ${tag}`);
  for (const path of [
    'crates/wield-core/Cargo.toml', 'crates/wield-native/Cargo.toml',
    'crates/wield-portal/Cargo.toml', 'crates/wield-tools/Cargo.toml',
    'crates/wield-cli/Cargo.toml', 'apps/wield/src-tauri/Cargo.toml',
  ]) {
    const actual = /^version\s*=\s*"([^"]+)"/m.exec(readFileSync(join(root, path), 'utf8'))?.[1];
    assert.equal(actual, version, `${path} version must match ${tag}`);
  }
  return { version, channel };
}

export function checkArtifact(tag, directory) {
  const { version, channel } = parseTag(tag);
  const files = readdirSync(directory).filter(name => name.endsWith('.AppImage'));
  assert.equal(files.length, 1, `Expected exactly one AppImage in ${directory}`);
  const name = files[0];
  assert.match(name, new RegExp(`^Wield_${version.replaceAll('.', '\\.')}_(amd64|x86_64)\\.AppImage$`));
  const path = join(directory, name);
  assert.ok(statSync(path).size > 1_000_000, 'AppImage is unexpectedly small');
  const bytes = readFileSync(path);
  assert.equal(bytes.toString('ascii', 1, 4), 'ELF', 'AppImage must be an ELF executable');
  assert.equal(bytes.toString('ascii', 8, 10), 'AI', 'AppImage magic is missing');
  assert.equal(bytes[10], 2, 'Expected AppImage type 2');
  const digest = createHash('sha256').update(bytes).digest('hex');
  const checksum = `${digest}  ${name}\n`;
  writeFileSync(join(directory, 'SHA256SUMS'), checksum);
  return { version, channel, name, path, checksum };
}

if (process.argv[1]?.endsWith('release-check.mjs')) {
  try {
    const [, , mode, tag, directory] = process.argv;
    if (mode === 'manifest') {
      const result = checkVersions(tag);
      process.stdout.write(`${result.channel} ${result.version}\n`);
    } else if (mode === 'branch') {
      process.stdout.write(`${branchForTag(tag)}\n`);
    } else if (mode === 'artifact') {
      const result = checkArtifact(tag, directory);
      process.stdout.write(`${result.path}\n${result.checksum}`);
    } else {
      throw new Error('Usage: node scripts/release-check.mjs manifest <tag> | branch <tag> | artifact <tag> <directory>');
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
