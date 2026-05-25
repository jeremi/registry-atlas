import dns from "node:dns/promises";
import fs from "node:fs";
import http from "node:http";
import https from "node:https";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import express from "express";

const DEFAULT_PORT = 3717;
const DEFAULT_TIMEOUT_MS = 8_000;
const DEFAULT_MAX_BYTES = 2 * 1024 * 1024;
const DEFAULT_REDIRECT_LIMIT = 3;

const TEXT_DECODER = new TextDecoder();
const ALLOWED_CONTENT_TYPES = [
  "application/json",
  "application/ld+json",
  "application/schema+json",
  "application/problem+json",
  "application/geo+json",
  "application/vnd.oai.openapi+json",
  "application/x-yaml",
  "application/yaml",
  "text/",
];
const SEMANTIC_TEXT_EXTENSIONS = new Set([".json", ".jsonld", ".geojson", ".ttl", ".yaml", ".yml"]);

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");

function toPositiveInteger(value, fallback) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function isLocalMode(env) {
  return env.ATLAS_PROXY_ALLOW_LOCAL === "1" || env.NODE_ENV !== "production";
}

function redactUrl(value) {
  try {
    const url = new URL(value);
    if (url.username) {
      url.username = "redacted";
    }
    if (url.password) {
      url.password = "redacted";
    }
    for (const key of Array.from(url.searchParams.keys())) {
      if (isSensitiveQueryName(key)) {
        url.searchParams.set(key, "REDACTED");
      }
    }
    return url.toString();
  } catch {
    return "[invalid-url]";
  }
}

function isSensitiveQueryName(name) {
  return /(^|[_-])(token|access_token|id_token|refresh_token|api[_-]?key|apikey|key|secret|client_secret|password|signature|sig)([_-]|$)/i.test(
    name,
  );
}

function isAllowedContentType(contentType) {
  const normalized = contentType.toLowerCase().split(";")[0].trim();
  return ALLOWED_CONTENT_TYPES.some((allowed) =>
    allowed.endsWith("/") ? normalized.startsWith(allowed) : normalized === allowed,
  );
}

function hasSemanticTextExtension(url) {
  return SEMANTIC_TEXT_EXTENSIONS.has(path.extname(url.pathname).toLowerCase());
}

function isAllowedUpstreamBody(contentType, finalUrl) {
  if (isAllowedContentType(contentType)) {
    return true;
  }

  const normalized = contentType.toLowerCase().split(";")[0].trim();
  return normalized === "application/octet-stream" && hasSemanticTextExtension(finalUrl);
}

function isLocalDevHost(hostname, address) {
  const normalized = hostname.toLowerCase();
  return (
    normalized === "localhost" ||
    normalized === "127.0.0.1" ||
    normalized === "::1" ||
    address === "127.0.0.1" ||
    address === "::1"
  );
}

function isPrivateIpv4(address) {
  const parts = address.split(".").map((part) => Number.parseInt(part, 10));
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part))) {
    return true;
  }

  const [first, second] = parts;
  return (
    first === 0 ||
    first === 10 ||
    first === 127 ||
    (first === 169 && second === 254) ||
    (first === 172 && second >= 16 && second <= 31) ||
    (first === 192 && second === 168) ||
    first >= 224
  );
}

function isPrivateIpv6(address) {
  const normalized = address.toLowerCase();
  if (normalized.startsWith("::ffff:")) {
    return isPrivateIpv4(normalized.slice("::ffff:".length));
  }

  return (
    normalized === "::1" ||
    normalized === "::" ||
    normalized.startsWith("fc") ||
    normalized.startsWith("fd") ||
    normalized.startsWith("fe80:")
  );
}

function isPrivateAddress(address) {
  const family = net.isIP(address);
  if (family === 4) {
    return isPrivateIpv4(address);
  }
  if (family === 6) {
    return isPrivateIpv6(address);
  }
  return true;
}

function makeError(code, message) {
  return { code, message };
}

async function resolveFetchableUrl(url, options) {
  if (!["http:", "https:"].includes(url.protocol)) {
    return { error: makeError("invalid_protocol", "Only http and https URLs can be fetched.") };
  }

  if (url.username || url.password) {
    return { error: makeError("url_credentials_blocked", "URLs with embedded credentials are not allowed.") };
  }

  let addresses;
  const literalFamily = net.isIP(url.hostname);
  if (literalFamily) {
    addresses = [{ address: url.hostname, family: literalFamily }];
  } else {
    try {
      addresses = await dns.lookup(url.hostname, { all: true, verbatim: true });
    } catch {
      return { error: makeError("dns_lookup_failed", "The target host could not be resolved.") };
    }
  }

  if (addresses.length === 0) {
    return { error: makeError("dns_lookup_failed", "The target host could not be resolved.") };
  }

  const blocked = addresses.find(({ address }) => {
    if (options.allowLocalhost && isLocalDevHost(url.hostname, address)) {
      return false;
    }
    return isPrivateAddress(address);
  });

  if (blocked) {
    return { error: makeError("private_network_blocked", "Private-network targets are blocked by this proxy.") };
  }

  return { addresses };
}

async function readLimitedBody(stream, maxBytes) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;

    stream.on("data", (value) => {
      const chunk = Buffer.isBuffer(value) ? value : Buffer.from(value);
      size += chunk.byteLength;
      if (size > maxBytes) {
        stream.destroy();
        reject(
          Object.assign(new Error("The upstream response exceeded the proxy size limit."), {
            code: "response_too_large",
          }),
        );
        return;
      }
      chunks.push(chunk);
    });
    stream.on("end", () => {
      resolve(TEXT_DECODER.decode(Buffer.concat(chunks, size)));
    });
    stream.on("error", reject);
  });
}

function parseJsonIfPossible(body) {
  if (!body.trim()) {
    return undefined;
  }
  try {
    return JSON.parse(body);
  } catch {
    return undefined;
  }
}

function bearerHeaders(req) {
  const raw = req.get("x-atlas-bearer");
  if (!raw) {
    return {};
  }

  const token = raw.replace(/^Bearer\s+/i, "").trim();
  if (!token || /[\r\n]/.test(token)) {
    return {};
  }

  return { Authorization: `Bearer ${token}` };
}

function sameOrigin(left, right) {
  return (
    left.protocol === right.protocol &&
    left.hostname.toLowerCase() === right.hostname.toLowerCase() &&
    (left.port || defaultPort(left.protocol)) === (right.port || defaultPort(right.protocol))
  );
}

function defaultPort(protocol) {
  return protocol === "https:" ? "443" : "80";
}

function fetchPinned(url, req, options, addresses, sendCredentials) {
  const client = url.protocol === "https:" ? https : http;
  const selectedAddress = addresses[0];

  return new Promise((resolve, reject) => {
    const request = client.request(
      url,
      {
        method: "GET",
        headers: {
          Accept:
            "application/ld+json, application/json, application/geo+json, text/plain, text/turtle, application/yaml;q=0.9, text/*;q=0.8",
          ...(sendCredentials ? bearerHeaders(req) : {}),
        },
        lookup: (_hostname, _lookupOptions, callback) => {
          callback(null, selectedAddress.address, selectedAddress.family);
        },
        servername: url.hostname,
        timeout: options.timeoutMs,
      },
      (response) => {
        resolve({
          body: response,
          headers: {
            get(name) {
              const value = response.headers[name.toLowerCase()];
              return Array.isArray(value) ? value.join(", ") : value ?? null;
            },
          },
          status: response.statusCode ?? 0,
          statusText: response.statusMessage ?? "",
        });
      },
    );

    request.on("timeout", () => {
      request.destroy(Object.assign(new Error("The upstream request timed out."), { name: "AbortError" }));
    });
    request.on("error", reject);
    request.end();
  });
}

async function fetchWithRedirects(startUrl, req, options) {
  let currentUrl = startUrl;

  for (let redirectCount = 0; redirectCount <= options.redirectLimit; redirectCount += 1) {
    const resolution = await resolveFetchableUrl(currentUrl, options);
    if (resolution.error) {
      return {
        type: "blocked",
        statusCode: 400,
        payload: {
          ok: false,
          url: redactUrl(startUrl),
          finalUrl: redactUrl(currentUrl),
          error: resolution.error,
        },
      };
    }

    let response;
    try {
      response = await fetchPinned(currentUrl, req, options, resolution.addresses, sameOrigin(startUrl, currentUrl));
    } catch (error) {
      return {
        type: "error",
        statusCode: error.name === "AbortError" ? 504 : 502,
        payload: {
          ok: false,
          url: redactUrl(startUrl),
          finalUrl: redactUrl(currentUrl),
          error: makeError(
            error.name === "AbortError" ? "upstream_timeout" : "upstream_fetch_failed",
            error.name === "AbortError"
              ? "The upstream request timed out."
              : "The upstream request failed.",
          ),
        },
      };
    }

    const location = response.headers.get("location");
    if ([301, 302, 303, 307, 308].includes(response.status) && location) {
      if (redirectCount === options.redirectLimit) {
        return {
          type: "error",
          statusCode: 508,
          payload: {
            ok: false,
            status: response.status,
            statusText: response.statusText,
            url: redactUrl(startUrl),
            finalUrl: redactUrl(currentUrl),
            error: makeError("redirect_limit_exceeded", "The upstream response redirected too many times."),
          },
        };
      }
      currentUrl = new URL(location, currentUrl);
      continue;
    }

    return { type: "response", response, finalUrl: currentUrl };
  }

  throw new Error("Unreachable redirect state.");
}

export function createApp(config = {}) {
  const env = config.env ?? process.env;
  const app = express();
  const options = {
    allowLocalhost: config.allowLocalhost ?? isLocalMode(env),
    maxBytes: config.maxBytes ?? toPositiveInteger(env.ATLAS_PROXY_MAX_BYTES, DEFAULT_MAX_BYTES),
    redirectLimit: config.redirectLimit ?? toPositiveInteger(env.ATLAS_PROXY_REDIRECT_LIMIT, DEFAULT_REDIRECT_LIMIT),
    timeoutMs: config.timeoutMs ?? toPositiveInteger(env.ATLAS_PROXY_TIMEOUT_MS, DEFAULT_TIMEOUT_MS),
  };

  app.disable("x-powered-by");

  app.get("/api/health", (_req, res) => {
    res.json({ ok: true, status: "ok" });
  });

  app.get("/api/proxy", async (req, res) => {
    const rawUrl = String(req.query.url ?? "");
    let targetUrl;

    try {
      targetUrl = new URL(rawUrl);
    } catch {
      res.status(400).json({
        ok: false,
        error: makeError("invalid_url", "A valid url query parameter is required."),
      });
      return;
    }

    const result = await fetchWithRedirects(targetUrl, req, options);
    if (result.type !== "response") {
      res.status(result.statusCode).json(result.payload);
      return;
    }

    const { response, finalUrl } = result;
    const contentType = response.headers.get("content-type") ?? "";

    if (!contentType || !isAllowedUpstreamBody(contentType, finalUrl)) {
      res.status(415).json({
        ok: false,
        status: response.status,
        statusText: response.statusText,
        url: redactUrl(targetUrl),
        finalUrl: redactUrl(finalUrl),
        contentType,
        error: makeError(
          contentType ? "content_type_blocked" : "content_type_missing",
          contentType
            ? "The upstream content type is not allowed."
            : "The upstream response did not include a content type.",
        ),
      });
      return;
    }

    try {
      const body = await readLimitedBody(response.body, options.maxBytes);
      const json = parseJsonIfPossible(body);
      const payload = {
        ok: response.status >= 200 && response.status < 300,
        status: response.status,
        statusText: response.statusText,
        url: redactUrl(targetUrl),
        finalUrl: redactUrl(finalUrl),
        contentType,
        body,
        ...(json === undefined ? {} : { json }),
      };

      res.status(response.status).json(payload);
    } catch (error) {
      res.status(error.code === "response_too_large" ? 413 : 502).json({
        ok: false,
        status: response.status,
        statusText: response.statusText,
        url: redactUrl(targetUrl),
        finalUrl: redactUrl(finalUrl),
        contentType,
        error: makeError(
          error.code === "response_too_large" ? "response_too_large" : "upstream_body_failed",
          error.code === "response_too_large"
            ? "The upstream response exceeded the proxy size limit."
            : "The upstream response body could not be read.",
        ),
      });
    }
  });

  const distDir = path.join(projectRoot, "dist");
  const indexPath = path.join(distDir, "index.html");
  if (env.NODE_ENV === "production" && fs.existsSync(indexPath)) {
    app.use(express.static(distDir));
    app.use((_req, res) => {
      res.sendFile(indexPath);
    });
  }

  return app;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const port = toPositiveInteger(process.env.PORT, DEFAULT_PORT);
  createApp().listen(port, "127.0.0.1", () => {
    console.log(`Registry Atlas server listening on http://127.0.0.1:${port}`);
  });
}
