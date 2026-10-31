// Serves Modbench from its own domain (modbench.nobrega.me), on top of what
// GitHub already hosts, so the links people and installed copies use do not
// depend on where the files live:
//
//   /latest.json                the updater's manifest, from the latest
//                               release, with its download links on this
//                               domain
//   /releases/download/<tag>/<file>
//                               a release file, through Cloudflare's cache
//   /download/<system>          the latest installer for a system (see
//                               DOWNLOADS), e.g. /download/windows
//   anything else               the website and the APT repository, from
//                               GitHub Pages
//
// Deployed with `wrangler deploy` in this folder.

const REPO = "nobrega8/modbench";
const PAGES = "https://nobrega8.github.io/modbench";
const GITHUB_DOWNLOAD = `https://github.com/${REPO}/releases/download/`;

/** Latest installer for each system, by the end of its file name. */
const DOWNLOADS = {
  windows: "_x64-setup.exe",
  "windows-msi": "_x64_en-US.msi",
  macos: "_universal.dmg",
  linux: "_amd64.AppImage",
  "linux-arm64": "_aarch64.AppImage",
  "deb-amd64": "_amd64.deb",
  "deb-arm64": "_arm64.deb",
  "deb-armhf": "_armhf.deb",
};

/** How long the manifest and the release list are kept, in seconds. */
const MANIFEST_TTL = 300;
const RELEASE_TTL = 600;
/** Release files never change under the same tag. */
const FILE_TTL = 30 * 24 * 3600;

export default {
  async fetch(request, env, ctx) {
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("Method not allowed", {
        status: 405,
        headers: { allow: "GET, HEAD" },
      });
    }
    const url = new URL(request.url);
    const path = url.pathname;
    try {
      if (path === "/latest.json") return await manifest(url, ctx);
      if (path.startsWith("/releases/download/")) return await file(path);
      if (path.startsWith("/download/")) return await latestDownload(url, ctx);
      return await pages(url);
    } catch (e) {
      return new Response(`Upstream error: ${e.message}\n`, { status: 502 });
    }
  },
};

/** A release from GitHub's API, kept in the cache for a while: the API allows
 * few unauthenticated calls, and Workers share their addresses. */
async function release(which, ctx) {
  const api = `https://api.github.com/repos/${REPO}/releases/${which}`;
  const cache = caches.default;
  const key = new Request(api);
  let res = await cache.match(key);
  if (!res) {
    res = await fetch(api, {
      headers: {
        "user-agent": "modbench-download-worker",
        accept: "application/vnd.github+json",
      },
    });
    if (!res.ok) throw new Error(`GitHub API ${res.status}`);
    res = new Response(res.body, res);
    res.headers.set("cache-control", `public, max-age=${RELEASE_TTL}`);
    ctx.waitUntil(cache.put(key, res.clone()));
  }
  return res.json();
}

/** The updater's manifest, with each platform's link on this domain. Without
 * the release list the links stay GitHub's, which work as well. */
async function manifest(url, ctx) {
  const res = await fetch(
    `https://github.com/${REPO}/releases/latest/download/latest.json`,
    { cf: { cacheTtl: MANIFEST_TTL, cacheEverything: true } },
  );
  if (!res.ok) return new Response("No release yet\n", { status: res.status });
  const data = await res.json();
  try {
    const rel = await release(`tags/v${data.version}`, ctx);
    const byApiUrl = new Map(rel.assets.map((a) => [a.url, a]));
    for (const p of Object.values(data.platforms ?? {})) {
      const asset = byApiUrl.get(p.url);
      if (asset) {
        p.url = `${url.origin}/releases/download/${rel.tag_name}/${asset.name}`;
      }
    }
  } catch {
    // Keep GitHub's links.
  }
  return new Response(JSON.stringify(data, null, 2), {
    headers: {
      "content-type": "application/json",
      "cache-control": `public, max-age=${MANIFEST_TTL}`,
      "access-control-allow-origin": "*",
    },
  });
}

/** A release file, fetched from GitHub once and then from Cloudflare's cache. */
async function file(path) {
  const rest = path.slice("/releases/download/".length);
  if (!/^v[\w.-]+\/[\w.+-]+$/.test(rest)) {
    return new Response("Not found\n", { status: 404 });
  }
  const res = await fetch(GITHUB_DOWNLOAD + rest, {
    cf: { cacheTtl: FILE_TTL, cacheEverything: true },
  });
  const out = new Response(res.body, res);
  if (res.ok) {
    out.headers.set("cache-control", `public, max-age=${FILE_TTL}, immutable`);
    out.headers.set(
      "content-disposition",
      `attachment; filename="${rest.split("/")[1]}"`,
    );
  }
  return out;
}

/** The latest installer for a system: a redirect to its release file, so the
 * browser saves it under its own name and version. */
async function latestDownload(url, ctx) {
  const system = url.pathname.slice("/download/".length).replace(/\/$/, "");
  const ending = DOWNLOADS[system];
  if (!ending) {
    return new Response(
      `Unknown system. Try: ${Object.keys(DOWNLOADS).join(", ")}\n`,
      { status: 404 },
    );
  }
  try {
    const rel = await release("latest", ctx);
    const asset = rel.assets.find((a) => a.name.endsWith(ending));
    if (asset) {
      return Response.redirect(
        `${url.origin}/releases/download/${rel.tag_name}/${asset.name}`,
        302,
      );
    }
  } catch {
    // Fall through to the release page.
  }
  return Response.redirect(`https://github.com/${REPO}/releases/latest`, 302);
}

/** The website and the APT repository, as GitHub Pages serves them, with
 * Pages' own redirects (a folder without its slash) kept on this domain. */
async function pages(url) {
  const res = await fetch(PAGES + url.pathname + url.search, {
    redirect: "manual",
    cf: { cacheTtl: 300, cacheEverything: true },
  });
  const out = new Response(res.body, res);
  const location = res.headers.get("location");
  if (location?.startsWith(PAGES)) {
    out.headers.set("location", url.origin + location.slice(PAGES.length));
  }
  return out;
}
