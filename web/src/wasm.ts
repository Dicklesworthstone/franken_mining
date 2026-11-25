/**
 * `bun run wasm`: builds web/wasm (the bridge to the workspace's own Rust kernels) for wasm32-unknown-unknown,
 * shrinks it with `wasm-opt -Oz` and writes web/wasm/pkg/engine.wasm, which src/build.ts fingerprints into
 * out/assets/.
 *
 * The cargo build goes through RCH when it is installed (AGENTS.md: never bypass the rch cargo shim). RCH may run it
 * on a worker and copy the artefact back, so the script checks that the .wasm it reads was written by this build.
 * The target directory follows AGENTS.md: under ${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}, never a bare /tmp.
 *
 *   bun run wasm            build, optimise, write pkg/engine.wasm
 *   bun run wasm --no-opt   skip wasm-opt (copies the raw module)
 */
import { existsSync, mkdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const web = resolve(import.meta.dir, "..");
const crate = join(web, "wasm");
const pkg = join(crate, "pkg");
const out = join(pkg, "engine.wasm");
const base = process.env.RCH_TARGET_BASE || process.env.TMPDIR || tmpdir();
const targetDir = join(base, "rch_target_fmine_wasm");
const built = join(targetDir, "wasm32-unknown-unknown", "release", "fmine_wasm.wasm");

const which = (name: string) => Bun.which(name);
const wasmOpt = ["/opt/homebrew/bin/wasm-opt", which("wasm-opt")].find((p): p is string => !!p && existsSync(p));

async function sh(cmd: string[], env: Record<string, string> = {}) {
  console.log("$ " + cmd.join(" "));
  const p = Bun.spawn(cmd, { cwd: crate, env: { ...process.env, ...env }, stdout: "inherit", stderr: "inherit" });
  const code = await p.exited;
  if (code !== 0) throw Error(`${cmd[0]} exited with ${code}`);
}

const kb = (bytes: number) => (bytes / 1024).toFixed(1) + " KB";

const started = Date.now();
const cargo = ["cargo", "build", "--release", "--target", "wasm32-unknown-unknown"];
if (which("rch")) {
  await sh(["rch", "exec", "--", "env", `CARGO_TARGET_DIR=${targetDir}`, ...cargo]);
} else {
  await sh(cargo, { CARGO_TARGET_DIR: targetDir });
}
if (!existsSync(built)) throw Error(`the build reported success but ${built} is missing (did RCH copy it back?)`);
// A no-op rebuild leaves an older file in place; that is fine when the sources are older still.
const age = statSync(built).mtimeMs;
if (age < started - 1000) {
  const newest = (dir: string): number =>
    Math.max(...[...new Bun.Glob("**/*.{rs,toml}").scanSync({ cwd: dir, absolute: true })].map((f) => statSync(f).mtimeMs));
  const src = Math.max(newest(join(crate, "src")), statSync(join(crate, "Cargo.toml")).mtimeMs);
  if (src > age) throw Error(`${built} is older than the bridge sources; the build did not refresh it`);
  console.log("(cargo had nothing to rebuild)");
}

mkdirSync(pkg, { recursive: true });
const raw = statSync(built).size;
// rustc's wasm32-unknown-unknown defaults, every one in all current browsers. Named one by one rather than
// --all-features, so wasm-opt cannot introduce an instruction the input did not already need.
const features = [
  "--enable-bulk-memory",
  "--enable-bulk-memory-opt",
  "--enable-multivalue",
  "--enable-mutable-globals",
  "--enable-nontrapping-float-to-int",
  "--enable-reference-types",
  "--enable-sign-ext",
];
if (wasmOpt && !process.argv.includes("--no-opt")) {
  await sh([wasmOpt, "-Oz", ...features, "--strip-debug", "--strip-producers", built, "-o", out]);
} else {
  if (!process.argv.includes("--no-opt")) console.warn("wasm-opt not found; writing the unoptimised module");
  await Bun.write(out, Bun.file(built));
}
const bytes = await Bun.file(out).arrayBuffer();
const mod = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(mod);
if (imports.length) throw Error("the engine must import nothing, but imports " + JSON.stringify(imports));
const gz = Bun.gzipSync(new Uint8Array(bytes)).length;
console.log(`engine.wasm: ${kb(raw)} raw, ${kb(bytes.byteLength)} after wasm-opt, ${kb(gz)} gzipped -> ${out}`);
