// macOS bootstrap/build entry point. No Homebrew or global pnpm required.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, mkdtemp, copyFile, chmod, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { execFileSync } from 'node:child_process';
import process from 'node:process';
import { Buffer } from 'node:buffer';
import console from 'node:console';

if (process.platform !== 'darwin') throw new Error('Run this script on macOS');
const root = fileURLToPath(new URL('../', import.meta.url));
const runtime = resolve(root, 'third_party/7zip/bin/macos');
const run = (command, args, options = {}) => execFileSync(command, args, { cwd: root, stdio: 'inherit', ...options });
const hash = async (file) => createHash('sha256').update(await readFile(file)).digest('hex');
const command = process.argv[2] ?? 'fetch';
if (command === 'fetch') {
  const manifest = JSON.parse(await readFile(resolve(root, 'third_party/7zip/macos.json'), 'utf8'));
  const staging = await mkdtemp(join(tmpdir(), 'qzip-macos-'));
  const archive = join(staging, 'engine.tar.xz');
  const response = await globalThis.fetch(manifest.url);
  if (!response.ok) throw new Error(`Engine download failed: ${response.status}`);
  await writeFile(archive, Buffer.from(await response.arrayBuffer()));
  if (await hash(archive) !== manifest.sha256) throw new Error('Official engine archive SHA256 mismatch');
  run('/usr/bin/tar', ['-xf', archive, '-C', staging]);
  await mkdir(runtime, { recursive: true });
  await copyFile(join(staging, '7zz'), join(runtime, '7zz'));
  await chmod(join(runtime, '7zz'), 0o755);
  for (const name of await readdir(staging)) {
    if (/^(license|copying|readme|history).*\.txt$/i.test(name)) await copyFile(join(staging, name), join(runtime, name));
  }
  run('/usr/bin/lipo', ['-verify_arch', 'arm64', 'x86_64', join(runtime, '7zz')]);
  run('/usr/bin/codesign', ['--force', '--sign', '-', join(runtime, '7zz')]);
  run(join(runtime, '7zz'), ['i']);
  const digest = await hash(join(runtime, '7zz'));
  await writeFile(join(runtime, 'engine.env'), `QZIP_MAC_ENGINE_SHA256=${digest}\n`);
  const iconset = join(staging, 'QZip.iconset');
  await mkdir(iconset);
  const source = resolve(root, 'apps/desktop/src-tauri/icons/128x128@2x.png');
  for (const size of [16, 32, 128, 256, 512]) {
    for (const scale of [1, 2]) run('/usr/bin/sips', ['-z', String(size * scale), String(size * scale), source, '--out', join(iconset, `icon_${size}x${size}${scale === 2 ? '@2x' : ''}.png`)]);
  }
  run('/usr/bin/iconutil', ['-c', 'icns', iconset, '-o', join(runtime, 'icon.icns')]);
  console.log(`Prepared verified engine. Temporary download retained at ${staging}`);
} else {
  const envFile = await readFile(join(runtime, 'engine.env'), 'utf8');
  const digest = envFile.trim().split('=')[1];
  if (await hash(join(runtime, '7zz')) !== digest) throw new Error('Prepared engine changed; rerun fetch');
  const env = { ...process.env, QZIP_MAC_ENGINE_SHA256: digest, MACOSX_DEPLOYMENT_TARGET: '12.0' };
  if (command === 'check') {
    run('corepack', ['pnpm', 'check'], { env });
    run('cargo', ['build', '-p', 'qzip-cli'], { env });
    const cli = resolve(root, 'target/debug/qzip-cli');
    run(cli, ['capabilities'], { env });
    const work = await mkdtemp(join(tmpdir(), 'qzip-mac-smoke-'));
    const source = join(work, '中文 source');
    await mkdir(source);
    const name = '文件 📦.txt';
    await writeFile(join(source, name), 'QZip macOS Unicode round trip\n');
    for (const format of ['zip', '7z', 'tar', 'tar.gz', 'tar.xz']) {
      const archive = join(work, `archive.${format}`);
      const output = join(work, `output-${format}`);
      run(cli, ['create', '--format', format, '--output', archive, source], { env });
      run(cli, ['list', '--archive', archive], { env });
      run(cli, ['test', '--archive', archive], { env });
      run(cli, ['extract', '--archive', archive, '--output', output], { env });
      if (await hash(join(output, '中文 source', name)) !== await hash(join(source, name))) throw new Error(`${format} round trip mismatch`);
      if (format === 'tar.gz' || format === 'tar.xz') {
        const alias = join(work, format === 'tar.gz' ? 'alias.tgz' : 'alias.txz');
        await copyFile(archive, alias);
        run(cli, ['list', '--archive', alias], { env });
      }
    }
    console.log(`macOS backend smoke passed; fixtures retained at ${work}`);
  }
  else if (command === 'build' || command === 'dev') {
    const args = ['pnpm', '--filter', '@qzip/desktop', 'tauri', command];
    if (command === 'build') args.push('--target', process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin');
    run('corepack', args, { env });
    if (await hash(join(runtime, '7zz')) !== digest) throw new Error('Bundler rewrote engine');
    if (command === 'build') {
      const target = process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
      const bundled = resolve(root, `target/${target}/release/bundle/macos/QZip.app/Contents/Resources/7zip/7zz`);
      if (await hash(bundled) !== digest) throw new Error('Bundled engine differs from embedded runtime hash');
      run(bundled, ['i']);
      const appBundle = resolve(root, `target/${target}/release/bundle/macos/QZip.app`);
      run('/usr/bin/codesign', ['--verify', '--deep', '--strict', appBundle]);
    }
  } else throw new Error(`Unknown command: ${command}`);
}
