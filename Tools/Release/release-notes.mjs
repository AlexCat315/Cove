import fs from 'node:fs';
import {workspaceVersion} from './version.mjs';
const version = workspaceVersion();
const sections = fs.readFileSync('CHANGELOG.md', 'utf8').split(/^## /m);
const section = sections.find(value => value.startsWith(version + ' '));
if (!section) throw new Error('Missing changelog for ' + version);
process.stdout.write('## ' + section.trim() + '\n');
