import fs from 'node:fs';

// The workspace version in the root Cargo.toml is the single SDK version.
export function workspaceVersion(manifest = 'Cargo.toml') {
  const text = fs.readFileSync(manifest, 'utf8');
  const section = text.split(/^\[workspace\.package\]\s*$/m)[1]?.split(/^\[/m)[0];
  const version = section?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error('[workspace.package] version is missing from ' + manifest);
  return version;
}

if (import.meta.url === 'file://' + process.argv[1]) process.stdout.write(workspaceVersion() + '\n');
