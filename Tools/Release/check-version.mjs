import fs from 'node:fs';
import {workspaceVersion} from './version.mjs';
const version = workspaceVersion();
if (process.argv[2] !== 'v' + version) throw new Error('Tag and workspace version must agree');
if (process.argv.includes('--release') && !fs.readFileSync('CHANGELOG.md', 'utf8').includes('## ' + version + ' ')) {
  throw new Error('Every SDK release requires a versioned changelog entry');
}
