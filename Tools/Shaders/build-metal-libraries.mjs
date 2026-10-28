import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const shaders = resolve(process.argv[2] ?? 'Graphics/Sources/CoveUIGraphics/Resources/Shaders');
const selected = process.argv[3] ?? 'all';
const targets = [
  ['macos', 'macosx', 'air64-apple-macosx26.0.0'],
  ['ios', 'iphoneos', 'air64-apple-ios26.0.0'],
  ['ios-simulator', 'iphonesimulator', 'air64-apple-ios26.0.0-simulator'],
];
if (selected !== 'all' && !targets.some(([name]) => name === selected)) throw Error('Unknown Metal platform');
const temporary = mkdtempSync(join(tmpdir(), 'cove-metallib-'));
try {
  for (const [platform, sdk, target] of targets) {
    if (selected !== 'all' && selected !== platform) continue;
    const output = join(shaders, `Native/metal-${platform}`);
    mkdirSync(output, { recursive: true });
    for (const file of readdirSync(join(shaders, 'Native/metal')).filter(x => x.endsWith('.json') && !x.endsWith('.reflection.json') && !x.endsWith('.workgroups.json'))) {
      const artifact = JSON.parse(readFileSync(join(shaders, 'Native/metal', file)));
      if (artifact.format !== 'mslSource') throw Error(`${file}: expected MSL source`);
      const source = Buffer.from(artifact.code, 'base64');
      const input = join(temporary, 'shader.metal'), air = join(temporary, 'shader.air'), library = join(temporary, 'shader.metallib');
      writeFileSync(input, source);
      execFileSync('xcrun', ['-sdk', sdk, 'metal', '-target', target, '-std=metal3.2', '-c', input, '-o', air], { stdio: 'inherit' });
      execFileSync('xcrun', ['-sdk', sdk, 'metallib', air, '-o', library], { stdio: 'inherit' });
      const bytes = readFileSync(library);
      if (bytes.subarray(0, 4).toString() !== 'MTLB') throw Error(`${file}: invalid metallib`);
      writeFileSync(join(output, file), JSON.stringify({ ...artifact, format: 'metallib', code: bytes.toString('base64'),
        compiler: artifact.compiler + '; Apple Metal', sourceFingerprint: createHash('sha256').update(source).digest('hex') }, null, 2) + '\n');
      console.log(`${platform}/${file}`);
    }
  }
} finally { rmSync(temporary, { recursive: true, force: true }); }
