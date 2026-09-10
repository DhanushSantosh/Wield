const { existsSync, unlinkSync, writeFileSync } = require("node:fs");
const { spawnSync } = require("node:child_process");
const os = require("node:os");
const path = require("node:path");

const cargoArgs = process.argv.slice(2);

if (cargoArgs.length === 0) {
  console.error("Usage: node scripts/run-cargo.js <cargo args...>");
  process.exit(1);
}

if (process.platform !== "win32") {
  const result = spawnSync("cargo", cargoArgs, { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

const cargoBin = path.join(process.env.USERPROFILE || "", ".cargo", "bin", "cargo.exe");
const vswhere = path.join(
  "C:",
  "Program Files (x86)",
  "Microsoft Visual Studio",
  "Installer",
  "vswhere.exe"
);

if (!existsSync(cargoBin)) {
  console.error("Cargo was not found. Install Rustup so Wield can run Cargo checks on Windows.");
  process.exit(1);
}

if (!existsSync(vswhere)) {
  runCargoDirectly(cargoBin);
}

const vswhereResult = spawnSync(
  vswhere,
  [
    "-latest",
    "-products",
    "*",
    "-requires",
    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
    "-property",
    "installationPath",
  ],
  { encoding: "utf8" }
);

const installationPath = (vswhereResult.stdout || "").trim();
const vsDevCmd = installationPath
  ? path.join(installationPath, "Common7", "Tools", "VsDevCmd.bat")
  : null;

if (!vsDevCmd || !existsSync(vsDevCmd)) {
  runCargoDirectly(cargoBin);
}

const quotedArgs = cargoArgs.map(quoteForCmd).join(" ");
const scriptPath = path.join(os.tmpdir(), "wield-run-cargo.cmd");
writeFileSync(
  scriptPath,
  [
    "@echo off",
    `call "${vsDevCmd}" -no_logo`,
    'set "PATH=%USERPROFILE%\\.cargo\\bin;%PATH%"',
    `cargo ${quotedArgs}`,
    "",
  ].join("\r\n"),
  "ascii"
);

const result = spawnSync("cmd.exe", ["/d", "/s", "/c", scriptPath], {
  stdio: "inherit",
});

try {
  unlinkSync(scriptPath);
} catch {}

process.exit(result.status ?? 1);

function runCargoDirectly(cargoPath) {
  const result = spawnSync(cargoPath, cargoArgs, { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

function quoteForCmd(value) {
  if (!value.includes(" ") && !value.includes('"')) {
    return value;
  }
  return `"${value.replace(/"/g, '\\"')}"`;
}
