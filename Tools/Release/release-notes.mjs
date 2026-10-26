import fs from 'node:fs';
const {version} = JSON.parse(fs.readFileSync('Tools/Cove/Sources/CoveCLI/SDK.json'));
const sections = fs.readFileSync('CHANGELOG.md', 'utf8').split(/^## /m);
const section = sections.find(value => value.startsWith(version + ' '));
if (!section) throw new Error('Missing changelog for ' + version);
process.stdout.write('## ' + section.trim() + '\n');
