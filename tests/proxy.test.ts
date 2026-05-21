import http from "node:http";
import request from "supertest";
import { afterEach, describe, expect, it } from "vitest";
import { createApp } from "../server/index.mjs";

type FixtureRoute = (req: http.IncomingMessage, res: http.ServerResponse) => void;

function listen(server: http.Server): Promise<number> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address && typeof address === "object") {
        resolve(address.port);
      } else {
        reject(new Error("Fixture server did not expose a TCP port."));
      }
    });
  });
}

function close(server: http.Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    });
  });
}

describe("server proxy", () => {
  const servers: http.Server[] = [];

  async function startFixture(route: FixtureRoute) {
    const server = http.createServer(route);
    servers.push(server);
    const port = await listen(server);
    return `http://127.0.0.1:${port}`;
  }

  afterEach(async () => {
    await Promise.all(servers.splice(0).map((server) => close(server)));
  });

  it("returns health status", async () => {
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/health")
      .expect(200)
      .expect(({ body }) => {
        expect(body).toEqual({ ok: true, status: "ok" });
      });
  });

  it("blocks private-network targets when localhost is not allowed", async () => {
    const app = createApp({ allowLocalhost: false });

    await request(app)
      .get("/api/proxy")
      .query({ url: "http://127.0.0.1:8080/metadata/dcat/bregdcat-ap" })
      .expect(400)
      .expect(({ body }) => {
        expect(body.ok).toBe(false);
        expect(body.error.code).toBe("private_network_blocked");
      });
  });

  it("forwards a session bearer token without requiring the caller to prefix it", async () => {
    let authorization = "";
    const upstream = await startFixture((req, res) => {
      authorization = req.headers.authorization ?? "";
      res.setHeader("content-type", "application/ld+json");
      res.end(JSON.stringify({ "@id": "catalog" }));
    });
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/proxy")
      .set("x-atlas-bearer", "secret-token")
      .query({ url: `${upstream}/metadata/dcat/bregdcat-ap` })
      .expect(200)
      .expect(({ body }) => {
        expect(body.ok).toBe(true);
        expect(body.json).toEqual({ "@id": "catalog" });
      });

    expect(authorization).toBe("Bearer secret-token");
  });

  it("drops bearer credentials on cross-origin redirects", async () => {
    let redirectedAuthorization = "";
    const redirected = await startFixture((req, res) => {
      redirectedAuthorization = req.headers.authorization ?? "";
      res.setHeader("content-type", "application/json");
      res.end("{}");
    });
    const upstream = await startFixture((_req, res) => {
      res.statusCode = 302;
      res.setHeader("location", `${redirected}/catalog.json`);
      res.end();
    });
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/proxy")
      .set("x-atlas-bearer", "secret-token")
      .query({ url: `${upstream}/redirect` })
      .expect(200)
      .expect(({ body }) => {
        expect(body.ok).toBe(true);
      });

    expect(redirectedAuthorization).toBe("");
  });

  it("redacts query-string secrets from proxy envelopes", async () => {
    const upstream = await startFixture((_req, res) => {
      res.setHeader("content-type", "application/json");
      res.end("{}");
    });
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/proxy")
      .query({ url: `${upstream}/catalog.json?api_key=secret&ok=true` })
      .expect(200)
      .expect(({ body }) => {
        expect(body.url).toContain("api_key=REDACTED");
        expect(body.finalUrl).toContain("api_key=REDACTED");
        expect(body.url).not.toContain("secret");
        expect(body.finalUrl).not.toContain("secret");
      });
  });

  it("passes through auth-required upstream status and response details", async () => {
    const upstream = await startFixture((_req, res) => {
      res.statusCode = 401;
      res.statusMessage = "Unauthorized";
      res.setHeader("content-type", "application/problem+json");
      res.end(JSON.stringify({ error: "token required" }));
    });
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/proxy")
      .query({ url: `${upstream}/protected/openapi.json` })
      .expect(401)
      .expect(({ body }) => {
        expect(body.ok).toBe(false);
        expect(body.status).toBe(401);
        expect(body.statusText).toBe("Unauthorized");
        expect(body.json).toEqual({ error: "token required" });
      });
  });

  it("rejects blocked content types before returning a body", async () => {
    const upstream = await startFixture((_req, res) => {
      res.setHeader("content-type", "application/octet-stream");
      res.end("binary-ish");
    });
    const app = createApp({ allowLocalhost: true });

    await request(app)
      .get("/api/proxy")
      .query({ url: `${upstream}/artifact.bin` })
      .expect(415)
      .expect(({ body }) => {
        expect(body.ok).toBe(false);
        expect(body.error.code).toBe("content_type_blocked");
        expect(body.body).toBeUndefined();
      });
  });

  it("stops following redirects after the configured limit", async () => {
    const upstream = await startFixture((req, res) => {
      res.statusCode = 302;
      res.setHeader("location", req.url === "/one" ? "/two" : "/three");
      res.end();
    });
    const app = createApp({ allowLocalhost: true, redirectLimit: 1 });

    await request(app)
      .get("/api/proxy")
      .query({ url: `${upstream}/one` })
      .expect(508)
      .expect(({ body }) => {
        expect(body.ok).toBe(false);
        expect(body.error.code).toBe("redirect_limit_exceeded");
      });
  });

  it("enforces the configured response size limit", async () => {
    const upstream = await startFixture((_req, res) => {
      res.setHeader("content-type", "text/plain");
      res.end("0123456789");
    });
    const app = createApp({ allowLocalhost: true, maxBytes: 4 });

    await request(app)
      .get("/api/proxy")
      .query({ url: `${upstream}/large.txt` })
      .expect(413)
      .expect(({ body }) => {
        expect(body.ok).toBe(false);
        expect(body.error.code).toBe("response_too_large");
      });
  });
});
