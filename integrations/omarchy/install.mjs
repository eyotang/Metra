#!/usr/bin/env node
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { setTimeout } from "node:timers/promises";
import { pluginManifest } from "./manifest.mjs";

const source = dirname(fileURLToPath(import.meta.url));
const manifest = pluginManifest(source);
const binaryArg = process.argv.indexOf("--binary");
if (binaryArg >= 0 && !process.argv[binaryArg + 1]) throw new Error("--binary requires an executable path.");
const binary = binaryArg >= 0
  ? resolve(process.argv[binaryArg + 1] || "")
  : [join(source, "metra.usage/metra"), resolve(source, "../../target/release/metra")]
    .find(existsSync);
if (process.platform !== "linux") throw new Error("The Omarchy widget requires Linux.");
if (!binary || !existsSync(binary)) throw new Error("Build first: cargo build --release --locked");
const ping = spawnSync("omarchy-shell", ["shell", "ping"], { encoding: "utf8", timeout: 5000 });
if (ping.error || ping.status !== 0 || ping.stdout.trim() !== "ok") {
  throw new Error("A running Omarchy shell with plugin support is required.");
}

const config = process.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const destination = join(config, "omarchy/plugins/metra.usage");
mkdirSync(destination, { recursive: true });
// Replace the executable atomically so updating a running collector is safe.
copyFileSync(binary, join(destination, "metra.new"));
renameSync(join(destination, "metra.new"), join(destination, "metra"));
// Remove the pre-release singleton registration: the shell owns service lifetime.
rmSync(join(destination, "qmldir"), { force: true });
rmSync(join(destination, "Backend.qml"), { force: true });
for (const file of ["Model.js", "Service.qml", "Widget.qml"]) {
  copyFileSync(join(source, "metra.usage", file), join(destination, file));
}
writeFileSync(join(destination, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");

const shellConfig = join(config, "omarchy/shell.json");
if (existsSync(shellConfig)) {
  // Validate before enabling; the native CLI preserves the existing bar layout.
  JSON.parse(readFileSync(shellConfig, "utf8"));
  cpSync(shellConfig, shellConfig + ".metra-backup-" + Date.now());
}
function run(command, args) {
  const result = spawnSync(command, args, { stdio: "inherit", timeout: 30000 });
  if (result.error || result.status !== 0) {
    throw new Error(`Widget installed at ${destination}; could not run ${command}: ${result.error?.message || result.status}`);
  }
}
run("omarchy-shell", ["shell", "rescanPlugins"]);
// The rescan IPC returns before its manifest subprocess has finished.
let discovered = false;
for (let attempt = 0; attempt < 40; attempt++) {
  const result = spawnSync("omarchy-shell", ["shell", "listPlugins"], { encoding: "utf8", timeout: 5000 });
  try {
    discovered = JSON.parse(result.stdout).some(plugin => plugin.id === "metra.usage");
  } catch { /* The shell can briefly be reloading. */ }
  if (discovered) break;
  await setTimeout(250);
}
if (!discovered) throw new Error("Omarchy did not discover metra.usage; check the shell log.");
run("omarchy", ["plugin", "enable", "metra.usage"]);
console.log(`Metra ${manifest.version} installed: ${destination}\nHover over its bar label to see usage.`);
