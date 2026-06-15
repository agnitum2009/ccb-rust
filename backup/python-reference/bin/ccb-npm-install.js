#!/usr/bin/env node
"use strict";

const childProcess = require("child_process");
const crypto = require("crypto");
const fs = require("fs");
const https = require("https");
const os = require("os");
const path = require("path");

const root = path.resolve(__dirname, "..");
const manifest = require(path.join(root, "package.json"));
const version = manifest.version;
const vendorRoot = path.join(root, ".ccb-release");

function artifactForHost() {
  if (process.platform === "darwin") {
    return {
      directory: "ccb-macos-universal",
      file: "ccb-macos-universal.tar.gz",
    };
  }
  if (process.platform === "linux" && process.arch === "x64") {
    return {
      directory: "ccb-linux-x86_64",
      file: "ccb-linux-x86_64.tar.gz",
    };
  }
  throw new Error(
    `Unsupported platform for @seemseam/ccb: ${process.platform}/${process.arch}. ` +
      "Use Linux x64, macOS x64, macOS arm64, or install the GitHub release manually."
  );
}

function installDir(info) {
  return path.join(vendorRoot, info.directory);
}

function executablePath(command = "ccb") {
  const info = artifactForHost();
  const base = installDir(info);
  return command === "ccb" ? path.join(base, "ccb") : path.join(base, "bin", command);
}

function isInstalled(info) {
  const dir = installDir(info);
  const versionFile = path.join(dir, "VERSION");
  const ccbPath = path.join(dir, "ccb");
  if (!fs.existsSync(versionFile) || !fs.existsSync(ccbPath)) {
    return false;
  }
  return fs.readFileSync(versionFile, "utf8").trim() === version;
}

function download(url, destination, redirects = 0) {
  if (redirects > 5) {
    throw new Error(`Too many redirects while downloading ${url}`);
  }
  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      const status = response.statusCode || 0;
      if ([301, 302, 303, 307, 308].includes(status) && response.headers.location) {
        response.resume();
        const redirected = new URL(response.headers.location, url).toString();
        download(redirected, destination, redirects + 1).then(resolve, reject);
        return;
      }
      if (status < 200 || status >= 300) {
        response.resume();
        reject(new Error(`Download failed for ${url}: HTTP ${status}`));
        return;
      }
      const file = fs.createWriteStream(destination);
      response.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    });
    request.on("error", reject);
  });
}

function parseSha256Sums(text) {
  const checksums = new Map();
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }
    const match = trimmed.match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (match) {
      checksums.set(path.basename(match[2]), match[1].toLowerCase());
    }
  }
  return checksums;
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function run(command, args) {
  const completed = childProcess.spawnSync(command, args, {
    stdio: "inherit",
  });
  if (completed.error) {
    throw completed.error;
  }
  if (completed.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit ${completed.status}`);
  }
}

async function install() {
  const info = artifactForHost();
  if (isInstalled(info)) {
    return;
  }
  if (process.env.CCB_NPM_SKIP_DOWNLOAD === "1") {
    console.warn("Skipping CCB release download because CCB_NPM_SKIP_DOWNLOAD=1.");
    return;
  }

  const baseUrl =
    process.env.CCB_NPM_RELEASE_BASE_URL ||
    `https://github.com/SeemSeam/claude_codex_bridge/releases/download/v${version}`;
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ccb-npm-"));
  const archivePath = path.join(tmpDir, info.file);
  const sumsPath = path.join(tmpDir, "SHA256SUMS");

  try {
    await download(`${baseUrl}/${info.file}`, archivePath);
    await download(`${baseUrl}/SHA256SUMS`, sumsPath);
    const checksums = parseSha256Sums(fs.readFileSync(sumsPath, "utf8"));
    const expected = checksums.get(info.file);
    if (!expected) {
      throw new Error(`SHA256SUMS does not contain ${info.file}`);
    }
    const actual = sha256File(archivePath);
    if (actual !== expected) {
      throw new Error(`Checksum mismatch for ${info.file}: expected ${expected}, got ${actual}`);
    }

    fs.rmSync(vendorRoot, { recursive: true, force: true });
    fs.mkdirSync(vendorRoot, { recursive: true });
    run("tar", ["-xzf", archivePath, "-C", vendorRoot]);
    if (!fs.existsSync(executablePath("ccb"))) {
      throw new Error(`Installed CCB executable not found at ${executablePath("ccb")}`);
    }
    console.log(`Installed CCB v${version} from ${info.file}.`);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

if (require.main === module) {
  install().catch((error) => {
    console.error(error.message || error);
    process.exit(1);
  });
}

module.exports = {
  executablePath,
  install,
};
