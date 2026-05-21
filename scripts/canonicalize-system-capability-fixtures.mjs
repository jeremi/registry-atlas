import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const fixtureRoot = path.join(projectRoot, "fixtures", "system-capability");

const volatileKeys = new Set([
]);

const volatileTimestampKeys = new Set(["analyzed_at", "fetched_at"]);
const volatileNumberKeys = new Set(["total_elapsed_ms"]);
const STABLE_TIMESTAMP = "1970-01-01T00:00:00Z";

function canonicalize(value) {
  if (Array.isArray(value)) {
    return value.map(canonicalize);
  }
  if (!value || typeof value !== "object") {
    return value;
  }

  const output = {};
  for (const key of Object.keys(value).sort()) {
    if (volatileKeys.has(key)) {
      continue;
    }
    if (volatileTimestampKeys.has(key)) {
      output[key] = STABLE_TIMESTAMP;
      continue;
    }
    if (volatileNumberKeys.has(key)) {
      output[key] = 0;
      continue;
    }
    output[key] = canonicalize(value[key]);
  }
  return output;
}

function canonicalizeFile(filePath) {
  const raw = fs.readFileSync(filePath, "utf8");
  const parsed = JSON.parse(raw);
  fs.writeFileSync(filePath, `${JSON.stringify(canonicalize(parsed), null, 2)}\n`);
}

function walk(dir) {
  if (!fs.existsSync(dir)) {
    return [];
  }

  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return walk(absolute);
    }
    return entry.isFile() && entry.name.endsWith(".json") ? [absolute] : [];
  });
}

fs.mkdirSync(fixtureRoot, { recursive: true });
for (const filePath of walk(fixtureRoot)) {
  canonicalizeFile(filePath);
}
