#!/usr/bin/env node

import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const stableVersionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function cargoPackageVersion(path) {
  const cargoToml = readFileSync(path, "utf8");
  const packageSection = cargoToml.match(/^\[package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1];
  const version = packageSection?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) {
    throw new Error(`Unable to read [package].version from ${path}`);
  }
  return version;
}

export function readReleaseVersion(root = projectRoot) {
  const versions = {
    packageJson: readJson(join(root, "package.json")).version,
    tauriConfig: readJson(join(root, "src-tauri", "tauri.conf.json")).version,
    cargoPackage: cargoPackageVersion(join(root, "src-tauri", "Cargo.toml")),
  };
  const uniqueVersions = new Set(Object.values(versions));
  if (uniqueVersions.size !== 1) {
    throw new Error(`Release versions disagree: ${JSON.stringify(versions)}`);
  }
  const [version] = uniqueVersions;
  if (!stableVersionPattern.test(version)) {
    throw new Error(`Release version must be stable SemVer (x.y.z), received ${version}`);
  }
  return version;
}

export function releaseAssetNames(version) {
  return {
    macDmg: `Metra-${version}-macos-universal.dmg`,
    macDmgChecksum: `Metra-${version}-macos-universal.dmg.sha256`,
    windowsInstaller: `Metra-${version}-windows-x64-setup.exe`,
    windowsInstallerSignature: `Metra-${version}-windows-x64-setup.exe.sig`,
    windowsPortable: `Metra-${version}-windows-x64-portable.exe`,
  };
}

function parseRfc3339(value) {
  if (!value || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value)) {
    throw new Error(`--pub-date must be an RFC 3339 timestamp, received ${value ?? "nothing"}`);
  }
  if (Number.isNaN(Date.parse(value))) {
    throw new Error(`--pub-date is not a real timestamp: ${value}`);
  }
  return value;
}

function releaseDownloadUrl(repository, tag, filename) {
  return `https://github.com/${repository}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(filename)}`;
}

function assertRepository(repository) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error(`--repository must use owner/name format, received ${repository}`);
  }
}

function assertTag(tag, version) {
  if (tag !== `v${version}`) {
    throw new Error(`Release tag ${tag} must exactly match v${version}`);
  }
}

function assertReleaseAssets(assetsDir, names, allowManifest) {
  const expected = new Set(Object.values(names));
  if (allowManifest) {
    expected.add("latest.json");
  }
  const actual = readdirSync(assetsDir).sort();
  const expectedSorted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expectedSorted)) {
    throw new Error(
      `Release assets must match exactly. Expected ${expectedSorted.join(", ")}; received ${actual.join(", ")}`,
    );
  }
  for (const filename of Object.values(names)) {
    const path = join(assetsDir, filename);
    const stats = statSync(path);
    if (!stats.isFile() || stats.size === 0) {
      throw new Error(`Release asset must be a non-empty file: ${path}`);
    }
  }

  const checksumPath = join(assetsDir, names.macDmgChecksum);
  const checksum = readFileSync(checksumPath, "utf8").trim();
  const checksumMatch = checksum.match(/^([a-fA-F0-9]{64}) [ *](.+)$/);
  if (!checksumMatch || checksumMatch[2] !== names.macDmg) {
    throw new Error(
      `${names.macDmgChecksum} must contain one SHA-256 checksum for ${names.macDmg}`,
    );
  }
  const actualChecksum = createHash("sha256")
    .update(readFileSync(join(assetsDir, names.macDmg)))
    .digest("hex");
  if (checksumMatch[1].toLowerCase() !== actualChecksum) {
    throw new Error(`${names.macDmgChecksum} does not match ${names.macDmg}`);
  }
}

function readSignature(path) {
  const signature = readFileSync(path, "utf8").trim();
  if (!signature) {
    throw new Error(`Updater signature is empty: ${path}`);
  }
  return signature;
}

export function buildLatestManifest({ assetsDir, repository, tag, pubDate, version }) {
  assertRepository(repository);
  assertTag(tag, version);
  const normalizedPubDate = parseRfc3339(pubDate);
  const names = releaseAssetNames(version);
  let manifestExists = false;
  try {
    manifestExists = statSync(join(assetsDir, "latest.json")).isFile();
  } catch {
    // The first generation intentionally starts without latest.json.
  }
  assertReleaseAssets(assetsDir, names, manifestExists);

  const windowsUpdate = {
    signature: readSignature(join(assetsDir, names.windowsInstallerSignature)),
    url: releaseDownloadUrl(repository, tag, names.windowsInstaller),
  };

  return {
    version,
    pub_date: normalizedPubDate,
    platforms: {
      "windows-x86_64": windowsUpdate,
    },
  };
}

export function generateLatestJson({ assetsDir, output, repository, tag, pubDate, root = projectRoot }) {
  const version = readReleaseVersion(root);
  const outputPath = resolve(output);
  const normalizedAssetsDir = resolve(assetsDir);
  if (dirname(outputPath) !== normalizedAssetsDir || basename(outputPath) !== "latest.json") {
    throw new Error("latest.json must be written directly inside --assets-dir");
  }

  const names = releaseAssetNames(version);
  const outputAlreadyExists = (() => {
    try {
      return statSync(outputPath).isFile();
    } catch {
      return false;
    }
  })();
  assertReleaseAssets(normalizedAssetsDir, names, outputAlreadyExists);

  const manifest = buildLatestManifest({
    assetsDir: normalizedAssetsDir,
    repository,
    tag,
    pubDate,
    version,
  });
  writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return { manifest, outputPath, version };
}

function parseArguments(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      throw new Error(`Unexpected positional argument: ${argument}`);
    }
    const key = argument.slice(2);
    if (!new Set(["assets-dir", "output", "repository", "tag", "pub-date"]).has(key)) {
      throw new Error(`Unknown option: ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for ${argument}`);
    }
    if (values[key] !== undefined) {
      throw new Error(`Duplicate option: ${argument}`);
    }
    values[key] = value;
    index += 1;
  }
  for (const key of ["assets-dir", "output", "repository", "tag", "pub-date"]) {
    if (!values[key]) {
      throw new Error(`Missing required option: --${key}`);
    }
  }
  return values;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const result = generateLatestJson({
    assetsDir: options["assets-dir"],
    output: options.output,
    repository: options.repository,
    tag: options.tag,
    pubDate: options["pub-date"],
  });
  console.log(`Generated ${result.outputPath} for v${result.version}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
