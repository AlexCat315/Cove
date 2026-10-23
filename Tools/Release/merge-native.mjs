import fs from 'node:fs';
import path from 'node:path';
const directory = process.argv[2];
const release = JSON.parse(fs.readFileSync('Tools/Guava/Sources/GuavaCLI/SDK.json'));
const platforms = {};
for (const name of fs.readdirSync(directory).filter(name => /^native-.+\.json$/.test(name))) {
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, name)));
  if (manifest.version !== release.version) throw new Error('Native version mismatch');
  for (const [platform, archives] of Object.entries(manifest.platforms)) {
    if (platforms[platform]) throw new Error('Duplicate native platform: ' + platform);
    platforms[platform] = archives;
  }
}
if (Object.keys(platforms).length !== 3) throw new Error('Missing native platform');
fs.writeFileSync(path.join(directory, 'native.json'), JSON.stringify({version: release.version, platforms}, null, 2) + '\n');
