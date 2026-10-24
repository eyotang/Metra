import { cpSync, copyFileSync, existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

if (process.platform !== "linux" || process.arch !== "x64") {
  throw new Error("The Omarchy x64 release package must be built on Linux x64.");
}
const root = resolve(import.meta.dirname, "..");
const version = readFileSync(join(root, "Cargo.toml"), "utf8").match(/^version\s*=\s*"([^\"]+)"/m)[1];
const binary = process.env.METRA_COLLECTOR_BINARY ? resolve(root, process.env.METRA_COLLECTOR_BINARY) : join(root, "target/release/metra");
if (!existsSync(binary)) throw new Error("Build the release binary first.");
const temporary = mkdtempSync(join(tmpdir(), "metra-omarchy-"));
try {
  const name = `Metra-${version}-omarchy-x64`;
  const staging = join(temporary, name);
  cpSync(join(root, "integrations/omarchy"), staging, { recursive: true });
  copyFileSync(binary, join(staging, "metra.usage/metra"));
  copyFileSync(join(root, "LICENSE"), join(staging, "LICENSE"));
  const output = join(root, "release-assets");
  mkdirSync(output, { recursive: true });
  const archive = name + ".tar.gz";
  execFileSync("tar", ["-czf", join(output, archive), "-C", temporary, name]);
  const digest = createHash("sha256").update(readFileSync(join(output, archive))).digest("hex");
  writeFileSync(join(output, archive + ".sha256"), `${digest}  ${archive}\n`);
  console.log(join(output, archive));
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
