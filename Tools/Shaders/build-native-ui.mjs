import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// UI-specific offline artifact builder. Keep the reflected ABI used by all
// backends, and reject a compiler/register mismatch before packaging bytecode.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const compiler = process.env.SLANGC;
if (!compiler) throw new Error('Set SLANGC to the pinned Slang 2026.19 executable');
const version = execFileSync(compiler, ['-version'], { encoding: 'utf8' }).trim();
if (version !== '2026.19') throw new Error(`Expected Slang 2026.19, got ${version}`);
const shaders = join(root, 'Sources/CoveUIRuntime/Resources/Shaders');
const output = resolve(process.argv[2] ?? join(shaders, 'Native/dxil'));
mkdirSync(output, { recursive: true });
const temporary = mkdtempSync(join(tmpdir(), 'cove-ui-dxil-'));
function binding(parameter) {
  const bindings = [parameter.binding, ...(parameter.bindings ?? [])].filter(Boolean);
  for (const kind of ['descriptorTableSlot', 'constantBuffer', 'shaderResource', 'unorderedAccess', 'samplerState']) {
    const match = bindings.find(value => value.kind === kind);
    if (match) return match;
  }
  throw new Error(`Missing register for ${parameter.name}`);
}
try {
  for (const [program, prefix, define] of [
    ['ui', 'ui', 'UI_VERTEX_SHADER'], ['ui_backdrop', 'backdrop', 'BACKDROP_VERTEX_SHADER'],
  ]) {
    for (const stage of ['vertex', 'fragment']) {
      const name = `${program}.${stage}`;
      const reference = JSON.parse(readFileSync(join(shaders, `Native/spirv/${name}.json`)));
      const code = join(temporary, `${name}.dxil`);
      const reflectionPath = join(temporary, `${name}.reflection.json`);
      execFileSync(compiler, [join(shaders, `Slang/${program}.slang`), '-entry',
        `${prefix}${stage === 'vertex' ? 'Vertex' : 'Fragment'}`, '-stage', stage,
        '-target', 'dxil', '-profile', 'sm_6_6', '-o', code, '-reflection-json', reflectionPath,
        '-line-directive-mode', 'none', `-D${define}=${stage === 'vertex' ? 1 : 0}`], { stdio: 'inherit' });
      const reflection = JSON.parse(readFileSync(reflectionPath));
      const parameters = reflection.parameters ?? [];
      for (const expected of reference.interface.bindings) {
        const parameter = parameters.find(value => value.name === expected.name);
        if (!parameter) throw new Error(`${name}: missing binding ${expected.name}`);
        const actual = binding(parameter);
        if (actual.index !== expected.slot || (actual.space ?? 0) !== expected.space) {
          throw new Error(`${name}: register mismatch for ${expected.name}`);
        }
      }
      const names = new Set(reference.interface.bindings.map(value => value.name));
      const constants = parameters.filter(value => !names.has(value.name));
      if (constants.length !== reference.interface.pushConstants.length) throw new Error(`${name}: constant count mismatch`);
      for (const [index, parameter] of constants.entries()) {
        const expected = reference.interface.pushConstants[index];
        const actual = binding(parameter);
        if (actual.index !== expected.slot || (actual.space ?? 0) !== 0
            || parameter.type?.elementVarLayout?.binding?.size !== expected.byteCount) {
          throw new Error(`${name}: constant ABI mismatch`);
        }
      }
      const bytes = readFileSync(code);
      if (bytes.subarray(0, 4).toString() !== 'DXBC') throw new Error(`${name}: invalid DXIL container`);
      const artifact = { ...reference, format: 'dxil', code: bytes.toString('base64'), compiler: `Slang ${version}` };
      writeFileSync(join(output, `${name}.json`), JSON.stringify(artifact, null, 2) + '\n');
      writeFileSync(join(output, `${name}.reflection.json`), JSON.stringify(reflection, null, 2) + '\n');
      console.log(`Built ${name}`);
    }
  }
} finally { rmSync(temporary, { recursive: true, force: true }); }
