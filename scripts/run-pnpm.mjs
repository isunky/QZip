// Run the workspace's pinned pnpm version without requiring Corepack.
// This is used by Tauri lifecycle hooks, where a missing Corepack executable
// otherwise prevents the frontend build from starting at all.
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import process from "node:process";
import console from "node:console";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
const packageManager = String(manifest.packageManager ?? "");
const match = /^pnpm@(\d+\.\d+\.\d+(?:-.+)?)$/.exec(packageManager);
if (!match) {
  throw new Error(`package.json must declare a pinned pnpm packageManager, got '${packageManager}'.`);
}

const expectedVersion = match[1];
const commandArguments = process.argv.slice(2);
const executable = (command) => process.platform === "win32" ? `${command}.cmd` : command;

function probe(command, args) {
  return spawnSync(executable(command), args, {
    cwd: root,
    env: process.env,
    encoding: "utf8",
    shell: process.platform === "win32",
    stdio: ["ignore", "pipe", "pipe"]
  });
}

function hasVersion(result) {
  return result.status === 0 && String(result.stdout ?? "").trim() === expectedVersion;
}

function run(command, args) {
  const result = spawnSync(executable(command), args, {
    cwd: root,
    env: process.env,
    shell: process.platform === "win32",
    stdio: "inherit"
  });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}

const corepack = probe("corepack", [packageManager, "--version"]);
if (hasVersion(corepack)) {
  console.log(`Using ${packageManager} via Corepack.`);
  run("corepack", [packageManager, ...commandArguments]);
}

const pnpm = probe("pnpm", ["--version"]);
if (hasVersion(pnpm)) {
  console.log(`Using ${packageManager} via pnpm.`);
  run("pnpm", commandArguments);
}
if (pnpm.status === 0) {
  console.log(`Using ${packageManager} via pnpm dlx.`);
  run("pnpm", ["dlx", packageManager, ...commandArguments]);
}

const npx = probe("npx", ["--version"]);
if (npx.status === 0) {
  console.log(`Using ${packageManager} via npx.`);
  run("npx", ["--yes", packageManager, ...commandArguments]);
}

throw new Error("Unable to run the pinned workspace package manager. Install Node.js, pnpm, or npm.");
