import fs from 'node:fs';
const sdk = JSON.parse(fs.readFileSync('Tools/Guava/Sources/GuavaCLI/SDK.json'));
if (process.argv[2] !== 'v' + sdk.version) throw new Error('Tag and SDK version must agree');
if (process.argv.includes('--release') && !fs.readFileSync('CHANGELOG.md', 'utf8').includes('## ' + sdk.version + ' ')) {
  throw new Error('Every SDK release requires a versioned changelog entry');
}
