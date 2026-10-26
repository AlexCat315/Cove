import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const directory = process.argv[2];
const release = JSON.parse(fs.readFileSync('Tools/Cove/Sources/CoveCLI/SDK.json'));
const required = process.argv.slice(3);
if (!required.length) required.push('macos-arm64', 'windows-x86_64', 'linux-x86_64', 'android-arm64', 'ios-arm64', 'ios-arm64-simulator');
const platforms = {};
for (const name of fs.readdirSync(directory).filter(name => /^native-.+\.json$/.test(name))) {
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, name)));
  if (manifest.version !== release.version) throw new Error('Native version mismatch');
  for (const [platform, archive] of Object.entries(manifest.platforms)) {
    if (platforms[platform]) throw new Error('Duplicate native platform: ' + platform);
    const filename = 'cove-native-' + platform + '.tar.gz';
    const digest = crypto.createHash('sha256').update(fs.readFileSync(path.join(directory, filename))).digest('hex');
    if (digest !== archive.sha256 || !archive.url.endsWith('/v' + release.version + '/' + filename)) throw new Error('Native archive mismatch: ' + platform);
    platforms[platform] = archive;
  }
}
for (const platform of required) if (!platforms[platform]) throw new Error('Missing native platform: ' + platform);
fs.writeFileSync(path.join(directory, 'native.json'), JSON.stringify({version: release.version, platforms}, null, 2) + '\n');
