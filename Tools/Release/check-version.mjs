import fs from 'node:fs';
const sdk = JSON.parse(fs.readFileSync('Tools/Guava/Sources/GuavaCLI/SDK.json'));
const native = JSON.parse(fs.readFileSync('Artifacts/native.json'));
if (process.argv[2] !== 'v' + sdk.version || native.version !== sdk.version || Object.keys(native.platforms).length !== 3) {
  throw new Error('Tag, SDK version and committed native metadata must agree');
}
