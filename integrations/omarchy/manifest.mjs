import { existsSync, readFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

// Source installs read the application's version. Extracted release archives
// have no Cargo.toml and retain the version stamped during packaging.
export function pluginManifest(source) {
  const manifest = JSON.parse(readFileSync(join(source, "metra.usage/manifest.json"), "utf8"));
  const cargo = resolve(source, "../../Cargo.toml");
  if (basename(source) === "omarchy" && basename(dirname(source)) === "integrations" && existsSync(cargo)) {
    const text = readFileSync(cargo, "utf8");
    const section = text.match(/^\[package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1];
    const version = section?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (!version) throw new Error(`Unable to read [package].version from ${cargo}`);
    manifest.version = version;
  }
  return manifest;
}
