#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { dirname, delimiter, join } from "node:path";
import { fileURLToPath } from "node:url";

const toolchain = "stable";
const targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
const tauriArgs = ["build", "--target", "universal-apple-darwin"];
const dryRun = process.argv.includes("--dry-run");

if (dryRun) {
  console.log(JSON.stringify({
    toolchain,
    targets,
    tauriArgs,
    usesRustupToolchain: true,
    usesSccache: false,
  }));
  process.exit(0);
}

if (process.platform !== "darwin") {
  console.error("Universal macOS builds must run on macOS.");
  process.exit(1);
}

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

function capture(command, args) {
  const result = spawnSync(command, args, { cwd: projectRoot, encoding: "utf8" });
  if (result.error || result.status !== 0) {
    console.error(result.stderr?.trim() || result.error?.message || `${command} failed`);
    process.exit(result.status ?? 1);
  }
  return result.stdout.trim();
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: projectRoot, env, stdio: "inherit" });
  if (result.error || result.status !== 0) {
    console.error(result.error?.message || `${command} failed`);
    process.exit(result.status ?? 1);
  }
}

const cargoPath = capture("rustup", ["which", "--toolchain", toolchain, "cargo"]);
const rustcPath = capture("rustup", ["which", "--toolchain", toolchain, "rustc"]);
const toolchainBin = dirname(cargoPath);

run("rustup", ["target", "add", "--toolchain", toolchain, ...targets]);

const buildEnv = {
  ...process.env,
  PATH: `${toolchainBin}${delimiter}${process.env.PATH ?? ""}`,
  CARGO: cargoPath,
  RUSTC: rustcPath,
  RUSTUP_TOOLCHAIN: toolchain,
  CARGO_BUILD_RUSTC_WRAPPER: "/usr/bin/env",
};
delete buildEnv.RUSTC_WRAPPER;

run(join(projectRoot, "node_modules", ".bin", "tauri"), tauriArgs, buildEnv);
