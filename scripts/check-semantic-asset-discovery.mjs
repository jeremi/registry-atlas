import { spawnSync } from "node:child_process";

const commands = [
  ["guard-release-checklist", []],
  ["cargo", ["fmt", "--all", "--check"]],
  ["cargo", ["test", "--workspace"]],
  [
    "cargo",
    [
      "run",
      "-q",
      "-p",
      "semantic-asset-discovery-cli",
      "--",
      "validate-report",
      "fixtures/reports/dcat-ap-report.json",
      "fixtures/reports/semantic-package-report.json",
    ],
  ],
  ["guard-core-network", []],
  ["guard-core-registry-relay", []],
  ["wasm-target-build", []],
];

for (const [command, args] of commands) {
  if (command === "guard-core-network") {
    runShell("cargo tree -p semantic-asset-discovery-core | rg 'reqwest|hyper|ureq|isahc' && exit 1 || true");
    continue;
  }
  if (command === "guard-core-registry-relay") {
    runShell("rg -n 'Registry Relay|registry-relay|registry_relay' crates/semantic-asset-discovery-core/src && exit 1 || true");
    continue;
  }
  if (command === "guard-release-checklist") {
    runReleaseChecklistGuard();
    continue;
  }
  if (command === "wasm-target-build") {
    runWasmBuild();
    continue;
  }
  run(command, args);
}

function runReleaseChecklistGuard() {
  console.log("\n$ release checklist guard");
  const result = spawnSync(
    "node",
    [
      "--input-type=module",
      "-e",
      `
        import { readFileSync } from "node:fs";
        const text = readFileSync("SEMANTIC_ASSET_DISCOVERY_V0.1_RELEASE.md", "utf8");
        const unchecked = text.split("\\n").filter((line) => line.includes("- [ ]"));
        const blankSignoffs = text
          .split("\\n")
          .filter((line) => line.startsWith("| ") && line.endsWith(" |"))
          .filter((line) => !line.includes("---") && !line.includes("Reviewer"));
        if (unchecked.length > 0 || blankSignoffs.some((line) => line.includes("|  |"))) {
          console.error("Release checklist is not fully checked or reviewer sign-off rows are incomplete.");
          for (const line of unchecked) console.error(line);
          process.exit(1);
        }
      `,
    ],
    { stdio: "inherit", shell: false },
  );
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function run(command, args, options = {}) {
  console.log(`\n$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, {
    stdio: "inherit",
    shell: false,
    ...options,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function runShell(command) {
  console.log(`\n$ ${command}`);
  const result = spawnSync(command, {
    stdio: "inherit",
    shell: true,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function runWasmBuild() {
  const rustupCargo = spawnSync("rustup", ["which", "cargo"], { encoding: "utf8" });
  const rustupRustc = spawnSync("rustup", ["which", "rustc"], { encoding: "utf8" });

  if (rustupCargo.status === 0 && rustupRustc.status === 0) {
    run("rustup", ["target", "add", "wasm32-unknown-unknown"]);
    run(
      rustupCargo.stdout.trim(),
      ["build", "-p", "semantic-asset-discovery-wasm", "--target", "wasm32-unknown-unknown"],
      {
        env: { ...process.env, RUSTC: rustupRustc.stdout.trim() },
      },
    );
    return;
  }

  run("cargo", ["build", "-p", "semantic-asset-discovery-wasm", "--target", "wasm32-unknown-unknown"]);
}
