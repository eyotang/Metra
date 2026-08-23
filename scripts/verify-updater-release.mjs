#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import {
  buildLatestManifest,
  generateLatestJson,
  readReleaseVersion,
  releaseAssetNames,
} from "./generate-latest-json.mjs";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

function parseArguments(argv) {
  const allowed = new Set(["assets-dir", "compare-dir", "repository", "tag", "pub-date"]);
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--") || !allowed.has(argument.slice(2))) {
      throw new Error(`Unknown option: ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for ${argument}`);
    }
    if (values[argument.slice(2)] !== undefined) {
      throw new Error(`Duplicate option: ${argument}`);
    }
    values[argument.slice(2)] = value;
    index += 1;
  }
  if (values["compare-dir"] && !values["assets-dir"]) {
    throw new Error("--compare-dir requires --assets-dir");
  }
  if (Object.keys(values).length > 0 && !values["assets-dir"]) {
    throw new Error("--assets-dir is required when release verification options are provided");
  }
  if (values["assets-dir"]) {
    for (const key of ["repository", "tag", "pub-date"]) {
      if (!values[key]) {
        throw new Error(`--${key} is required with --assets-dir`);
      }
    }
  }
  return values;
}

function verifyRepositoryContract() {
  const packageJson = JSON.parse(readFileSync(join(projectRoot, "package.json"), "utf8"));
  const overlay = JSON.parse(readFileSync(join(projectRoot, "src-tauri", "tauri.release.conf.json"), "utf8"));
  const workflow = readFileSync(join(projectRoot, ".github", "workflows", "ci.yml"), "utf8");
  const updaterSource = readFileSync(join(projectRoot, "src-tauri", "src", "updater.rs"), "utf8");
  const nativeAppSource = readFileSync(join(projectRoot, "src-tauri", "src", "app.rs"), "utf8");
  const platformSource = readFileSync(join(projectRoot, "src-tauri", "src", "platform.rs"), "utf8");
  const frontendSource = readFileSync(join(projectRoot, "src", "main.ts"), "utf8");
  const updateAction = frontendSource.match(/function bindAppUpdateAction\([\s\S]*?\n\}/)?.[0] ?? "";
  const manualAction = updateAction.match(/if \(manualDownload\) \{([\s\S]*?)\} else \{/)?.[1] ?? "";
  const releaseArtifactsJob = workflow.match(/release-artifacts:[\s\S]*?(?=\n  publish-release:)/)?.[0] ?? "";
  const macBuildStep = workflow.match(
    /- name: Build unsigned Universal macOS DMG[\s\S]*?(?=\n      - name:)/,
  )?.[0] ?? "";

  assert.equal(readReleaseVersion(projectRoot), packageJson.version);
  assert.equal(overlay.bundle?.createUpdaterArtifacts, true, "release overlay must create signed updater artifacts");
  assert.equal(
    packageJson.scripts["generate:updater-manifest"],
    "node scripts/generate-latest-json.mjs",
    "package.json must expose the production manifest generator",
  );
  assert.equal(
    packageJson.scripts["verify:updater-release"],
    "node scripts/verify-updater-release.mjs",
    "package.json must expose the deterministic updater release verifier",
  );
  assert.match(
    updaterSource,
    /\(DesktopPlatform::Windows, None\)\s*=>\s*AppUpdateMode::ManualDownload/,
    "Windows Portable must check for updates in manual-download mode",
  );
  assert.match(
    updaterSource,
    /fn finish_in_app_check_available\([\s\S]*?state\.status\.mode != AppUpdateMode::InApp[\s\S]*?state\.pending = Some\(update\)/,
    "only the in-app check path may retain an installable updater payload",
  );
  assert.match(
    updaterSource,
    /fn finish_manual_check_available\([\s\S]*?state\.status\.mode != AppUpdateMode::ManualDownload[\s\S]*?state\.pending = None/,
    "manual distributions must never retain an installable updater payload",
  );
  assert.match(
    updaterSource,
    /https:\/\/github\.com\/eyotang\/Metra\/releases\/tag\/v/,
    "manual downloads must open the backend-owned page for the detected release tag",
  );
  assert.match(platformSource, /ShellExecuteW/, "Windows must open fixed pages through the OS URL handler");
  assert.match(updaterSource, /crate::platform::open_url\(&download_page\)/);
  assert.match(nativeAppSource, /crate::platform::open_url\(CURSOR_SETTINGS_DEEP_LINK\)/);
  assert.doesNotMatch(
    `${updaterSource}\n${nativeAppSource}\n${platformSource}`,
    /Command::new\("rundll32\.exe"\)/,
    "Portable native links must not use a PATH-searchable helper executable",
  );
  assert.match(
    manualAction,
    /invokeWithTimeout<void>\(\s*"open_app_update_download_page",[\s\S]*?ACTION_TIMEOUT_MS/,
    "manual download page opening must use the bounded IPC wrapper",
  );
  assert.doesNotMatch(
    manualAction,
    /install_app_update/,
    "manual-download actions must never invoke in-app installation",
  );
  assert.match(
    updateAction,
    /else \{[\s\S]*invokeWithTimeout<AppUpdateStatus>\(\s*"install_app_update",[\s\S]*?UPDATE_INSTALL_TIMEOUT_MS/,
    "only the in-app branch may invoke installation with its long-running timeout",
  );

  assert.match(workflow, /concurrency:[\s\S]*metra-stable-release[\s\S]*cancel-in-progress/);
  assert.match(workflow, /release-artifacts:[\s\S]*if: startsWith\(github\.ref, 'refs\/tags\/v'\)/);
  assert.doesNotMatch(
    releaseArtifactsJob,
    /workflow_dispatch/,
    "manual branch dispatches must not publish a stable release",
  );
  assert.match(
    workflow,
    /Build Windows NSIS application[\s\S]*--bundles nsis[\s\S]*--config src-tauri\/tauri\.release\.conf\.json/,
  );
  assert.match(
    workflow,
    /Build unsigned Universal macOS DMG[\s\S]*--target universal-apple-darwin[\s\S]*--bundles app,dmg/,
  );
  assert.doesNotMatch(
    macBuildStep,
    /tauri\.release\.conf\.json|TAURI_SIGNING_PRIVATE_KEY|APPLE_/,
    "the manual macOS build must not generate updater artifacts or imply Apple signing",
  );
  assert.doesNotMatch(
    releaseArtifactsJob,
    /secrets\.APPLE_|APPLE_CERTIFICATE|APPLE_ID|APPLE_PASSWORD|APPLE_TEAM_ID/,
    "Apple credentials must not be required until signed macOS updates are enabled",
  );
  assert.match(workflow, /TAURI_SIGNING_PRIVATE_KEY_PASSWORD/);
  assert.equal(
    (releaseArtifactsJob.match(/^\s+TAURI_SIGNING_PRIVATE_KEY:\s*\$\{\{ secrets\./gm) ?? []).length,
    2,
    "only the Windows secret validator and NSIS builder may receive the updater signing key",
  );
  assert.match(workflow, /Verify rendered Windows bubble[\s\S]*src-tauri\/target\/release\/metra\.exe/);
  assert.match(
    workflow,
    /__TAURI_BUNDLE_TYPE_VAR_UNK[\s\S]*Portable executable does not contain exactly one unpatched Tauri bundle marker/,
    "the copied portable executable must retain Tauri's unbundled marker",
  );
  assert.doesNotMatch(
    releaseArtifactsJob,
    /Metra\.app\.tar\.gz|macos-universal\.app\.tar\.gz/,
    "macOS updater archives must not be generated before signed in-app updates are enabled",
  );
  assert.match(
    releaseArtifactsJob,
    /macOS DMG is intentionally unsigned and not notarized[\s\S]*manual download only/,
    "the workflow summary must state the macOS trust and update limitations",
  );
  assert.match(workflow, /bundle[\\/]nsis/);
  assert.equal(
    (workflow.match(
      /cargo test --locked --test updater_signature_contract release_artifact_matches_embedded_public_key -- --ignored --exact/g,
    ) ?? []).length,
    1,
    "the Windows NSIS updater artifact must be verified against the embedded public key exactly once",
  );
  assert.match(workflow, /Verify Windows updater artifact signature[\s\S]*METRA_UPDATER_ARTIFACT/);
  assert.doesNotMatch(workflow, /Verify macOS updater artifact signature/);
  assert.match(releaseArtifactsJob, /shasum -a 256/);
  assert.match(workflow, /publish-release:[\s\S]*needs: release-artifacts/);
  assert.match(workflow, /publish-release:[\s\S]*contents: write/);
  assert.match(
    workflow,
    /gh api --paginate "repos\/\$\{GITHUB_REPOSITORY\}\/releases\?per_page=100"/,
    "release ordering must inspect every GitHub Releases API page",
  );
  assert.match(
    workflow,
    /select\(\.draft == false and \.prerelease == false\)/,
    "release ordering must exclude drafts and prereleases",
  );
  assert.match(
    workflow,
    /const highestPublishedStable =[\s\S]*compareVersions\(currentVersion, parseVersion\(highestPublishedStable\)\) <= 0/,
    "the release tag must be strictly newer than the greatest published stable SemVer",
  );
  assert.doesNotMatch(
    workflow,
    /latest_tag=.*gh release view --json tagName/,
    "release ordering must not trust the mutable latest pointer",
  );
  assert.match(workflow, /actions\/upload-artifact@v7/, "release assets must use the Node 24 upload action runtime");
  assert.match(workflow, /actions\/download-artifact@v8[\s\S]*merge-multiple: true/, "release publishing must use the Node 24 download action runtime");
  assert.match(workflow, /gh release create[\s\S]*--draft/);
  assert.match(
    workflow,
    /metra-unsigned-macos-notice[\s\S]*--generate-notes/,
    "stable release notes must disclose that the macOS DMG is unsigned and not notarized",
  );
  assert.match(
    workflow,
    /sudo xattr -d com\.apple\.quarantine \/Applications\/Metra\.app/,
    "unsigned macOS release notes must remove only the quarantine attribute with the documented command",
  );
  assert.match(workflow, /generate:updater-manifest/);
  assert.match(workflow, /gh release upload[\s\S]*--clobber/);
  assert.match(workflow, /gh release download/);
  assert.match(workflow, /--compare-dir/);
  assert.match(workflow, /gh release edit[\s\S]*--draft=false[\s\S]*--latest/);
}

function writeFixtureAssets(directory, version) {
  mkdirSync(directory, { recursive: true });
  const names = releaseAssetNames(version);
  for (const [kind, filename] of Object.entries(names)) {
    if (kind === "macDmgChecksum") {
      continue;
    }
    const contents = kind === "windowsInstallerSignature"
      ? "deterministic-windows-signature\n"
      : `fixture:${filename}\n`;
    writeFileSync(join(directory, filename), contents);
  }
  const macDmg = readFileSync(join(directory, names.macDmg));
  const macDmgChecksum = createHash("sha256").update(macDmg).digest("hex");
  writeFileSync(
    join(directory, names.macDmgChecksum),
    `${macDmgChecksum}  ${names.macDmg}\n`,
  );
  return names;
}

function verifyDeterministicGeneration() {
  const version = readReleaseVersion(projectRoot);
  const temporaryRoot = mkdtempSync(join(tmpdir(), "metra-updater-release-"));
  const assetsDir = join(temporaryRoot, "assets");
  try {
    const names = writeFixtureAssets(assetsDir, version);
    const options = {
      assetsDir,
      output: join(assetsDir, "latest.json"),
      repository: "eyotang/Metra",
      tag: `v${version}`,
      pubDate: "2026-08-22T00:00:00Z",
      root: projectRoot,
    };
    generateLatestJson(options);
    const first = readFileSync(options.output, "utf8");
    generateLatestJson(options);
    const second = readFileSync(options.output, "utf8");
    assert.equal(second, first, "identical release inputs must produce byte-identical latest.json");

    const manifest = JSON.parse(first);
    assert.equal(manifest.version, version);
    assert.equal(manifest.pub_date, options.pubDate);
    assert.equal("notes" in manifest, false, "generic placeholder release notes must be omitted");
    assert.deepEqual(Object.keys(manifest.platforms), ["windows-x86_64"]);
    assert.equal(manifest.platforms["windows-x86_64"].signature, "deterministic-windows-signature");
    assert.match(manifest.platforms["windows-x86_64"].url, new RegExp(`${names.windowsInstaller.replaceAll(".", "\\.")}$`));
    assert.doesNotMatch(
      first,
      /darwin|portable\.exe|\.dmg"/i,
      "manual downloads must not be updater platforms",
    );

    writeFileSync(join(assetsDir, names.windowsInstallerSignature), "\n");
    assert.throws(
      () => buildLatestManifest({ ...options, version }),
      /signature is empty/,
      "an empty updater signature must fail closed",
    );

    writeFileSync(
      join(assetsDir, names.windowsInstallerSignature),
      "deterministic-windows-signature\n",
    );
    writeFileSync(
      join(assetsDir, names.macDmgChecksum),
      `${"0".repeat(64)}  ${names.macDmg}\n`,
    );
    assert.throws(
      () => buildLatestManifest({ ...options, version }),
      /does not match/,
      "a macOS DMG checksum mismatch must fail closed",
    );
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

function directoryFiles(directory) {
  return readdirSync(directory).sort().map((name) => {
    const path = join(directory, name);
    assert.ok(statSync(path).isFile(), `release directories may only contain files: ${path}`);
    return name;
  });
}

function verifyReleaseDirectory({ assetsDir, repository, tag, pubDate }) {
  const version = readReleaseVersion(projectRoot);
  const manifestPath = join(assetsDir, "latest.json");
  const actual = JSON.parse(readFileSync(manifestPath, "utf8"));
  const expected = buildLatestManifest({ assetsDir, repository, tag, pubDate, version });
  assert.deepEqual(actual, expected, "latest.json must exactly describe the staged updater artifacts");
}

function compareReleaseDirectories(expectedDirectory, actualDirectory) {
  const expectedFiles = directoryFiles(expectedDirectory);
  const actualFiles = directoryFiles(actualDirectory);
  assert.deepEqual(actualFiles, expectedFiles, "draft GitHub Release assets must exactly match staged assets");
  for (const filename of expectedFiles) {
    assert.ok(
      readFileSync(join(actualDirectory, filename)).equals(readFileSync(join(expectedDirectory, filename))),
      `draft GitHub Release asset differs from staged bytes: ${filename}`,
    );
  }
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  verifyRepositoryContract();
  verifyDeterministicGeneration();
  if (options["assets-dir"]) {
    const assetsDir = resolve(options["assets-dir"]);
    verifyReleaseDirectory({
      assetsDir,
      repository: options.repository,
      tag: options.tag,
      pubDate: options["pub-date"],
    });
    if (options["compare-dir"]) {
      compareReleaseDirectories(assetsDir, resolve(options["compare-dir"]));
    }
  }
  console.log("Updater release manifest, assets, and atomic publication contract verified");
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
