import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { homedir, platform, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// UI-specific offline artifact builder. Keep the reflected ABI used by all
// backends, and reject a compiler/register mismatch before packaging bytecode.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

/* `cargo cove setup --shaders` records the pinned compiler in the shared
   toolchain state; mirror cove-native-build's cache_dir/state_path rules. */
function setupStatePath() {
    const os = platform();
    const name = { win32: 'windows', darwin: 'macos', linux: 'linux' }[os] ?? os;
    const arch = { x64: 'x86_64', arm64: 'aarch64' }[process.arch] ?? process.arch;
    let cache = process.env.COVE_CACHE_DIR;
    if (!cache) {
        if (os === 'win32') cache = join(process.env.LOCALAPPDATA ?? homedir(), 'Cove');
        else if (os === 'darwin') cache = join(homedir(), 'Library', 'Caches', 'Cove');
        else cache = join(process.env.XDG_CACHE_HOME ?? join(homedir(), '.cache'), 'Cove');
    }
    return join(cache, `setup-${name}-${arch}.json`);
}
function pinnedSlangc() {
    try { return JSON.parse(readFileSync(setupStatePath(), 'utf8')).environment?.SLANGC; }
    catch { return undefined; }
}

const compiler = process.env.SLANGC ?? pinnedSlangc();
if (!compiler) throw new Error('Set SLANGC to the pinned Slang 2026.19 executable, or run `cargo cove setup --shaders`');
const versionResult = spawnSync(compiler, ['-version'], { encoding: 'utf8' });
if (versionResult.status !== 0) throw new Error('Failed to query Slang version');
const version = (versionResult.stdout + versionResult.stderr).trim();
if (version !== '2026.19') throw new Error(`Expected Slang 2026.19, got ${version}`);
const shaders = join(root, 'crates/graphics/resources/Shaders');
const web = process.argv[2] === '--wgsl';
const target = web ? 'wgsl' : 'dxil';
const output = resolve(process.argv[web ? 3 : 2] ?? join(shaders, `Native/${target}`));
mkdirSync(output, { recursive: true });
const temporary = mkdtempSync(join(tmpdir(), `cove-ui-${target}-`));
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
      const code = join(temporary, `${name}.${target}`);
      const reflectionPath = join(temporary, `${name}.reflection.json`);
      execFileSync(compiler, [join(shaders, `Slang/${program}.slang`), '-entry',
        `${prefix}${stage === 'vertex' ? 'Vertex' : 'Fragment'}`, '-stage', stage,
        '-target', target, ...(web ? ['-DCOVE_WEBGPU=1'] : ['-profile', 'sm_6_6']),
        '-o', code, '-reflection-json', reflectionPath,
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
        if (actual.index !== expected.slot || (actual.space ?? 0) !== (web ? 1 : 0)
            || parameter.type?.elementVarLayout?.binding?.size !== expected.byteCount) {
          throw new Error(`${name}: constant ABI mismatch`);
        }
      }
      const bytes = web
        ? Buffer.from(readFileSync(code, 'utf8').replace(/\r\n/g, '\n'))
        : readFileSync(code);
      const entryPoint = web ? bytes.toString().match(new RegExp(`@${stage}\\s+fn\\s+(\\w+)`))?.[1] : reference.entryPoint;
      if (!entryPoint || (!web && bytes.subarray(0, 4).toString() !== 'DXBC')) {
        throw new Error(`${name}: invalid ${target} shader`);
      }
      const artifact = { ...reference, entryPoint, format: target, code: bytes.toString('base64'), compiler: `Slang ${version}` };
      writeFileSync(join(output, `${name}.json`), JSON.stringify(artifact, null, 2) + '\n');
      writeFileSync(join(output, `${name}.reflection.json`), JSON.stringify(reflection, null, 2) + '\n');
      console.log(`Built ${name}`);
    }
  }
} finally { rmSync(temporary, { recursive: true, force: true }); }
