import { readFileSync } from 'node:fs';

// Historical commit controls must not disable the new release commit/tag.
const notes = readFileSync(0, 'utf8')
  .replace(/\[(?:skip\s+ci|ci\s+skip|no\s+ci|skip\s+actions|actions\s+skip)\]/gi, '')
  .replace(/skip-checks:\s*true/gi, '')
  .split('\n').map(line => line.trimEnd()).join('\n');
process.stdout.write(notes);
