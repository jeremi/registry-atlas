import { spawnSync } from "node:child_process";
import { dirname, delimiter } from "node:path";

const rustupRustc = spawnSync("rustup", ["which", "rustc"], { encoding: "utf8" });
const rustupEnv =
  rustupRustc.status === 0
    ? {
        PATH: `${dirname(rustupRustc.stdout.trim())}${delimiter}${process.env.PATH ?? ""}`,
        RUSTC: rustupRustc.stdout.trim(),
      }
    : {};
if (rustupRustc.status === 0) {
  run("rustup", ["target", "add", "wasm32-unknown-unknown"]);
}

run(
  "wasm-pack",
  [
    "build",
    "crates/semantic-asset-discovery-wasm",
    "--target",
    "web",
    "--out-dir",
    "../../src/wasm/semantic-asset-discovery",
    "--out-name",
    "semantic_asset_discovery",
  ],
  rustupEnv,
);

function run(command, args, env = {}) {
  console.log(`\n$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
