// 画面（dist/）を組み立てる：node build.mjs
// 先に WebAssembly（pkg/）を作っておく（../build_wasm.sh）
import * as esbuild from 'esbuild';
import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';

const out = 'dist';
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
if (!existsSync('pkg/egword_wasm_bg.wasm')) {
  console.error('pkg/ がありません。先に ../build_wasm.sh を実行してください。');
  process.exit(1);
}
await esbuild.build({
  entryPoints: ['src/main.ts'],
  bundle: true,
  format: 'esm',
  target: ['safari15', 'chrome100', 'firefox100'],
  minify: true,
  sourcemap: false,
  outfile: `${out}/app.js`,
  legalComments: 'none',
});
cpSync('static', out, { recursive: true });
cpSync('pkg/egword_wasm_bg.wasm', `${out}/egword_wasm_bg.wasm`);
console.log('dist/ を作りました');
