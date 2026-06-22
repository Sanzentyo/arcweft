const rootUrl = new URL("../", import.meta.url);

const contentTypes = new Map([
  [".awfb", "application/octet-stream"],
  [".css", "text/css; charset=utf-8"],
  [".gif", "image/gif"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".ttf", "font/ttf"],
  [".wasm", "application/wasm"],
  [".webp", "image/webp"],
]);

function extension(pathname) {
  const index = pathname.lastIndexOf(".");
  return index < 0 ? "" : pathname.slice(index).toLowerCase();
}

async function serveStatic(request) {
  const url = new URL(request.url);
  let pathname = decodeURIComponent(url.pathname);
  if (pathname === "/") {
    pathname = "/index.html";
  }
  if (pathname === "/favicon.ico") {
    return new Response(null, { status: 204 });
  }
  if (pathname.includes("..") || pathname.includes("\0")) {
    return new Response("bad path", { status: 400 });
  }

  const file = await Deno.open(new URL(`.${pathname}`, rootUrl)).catch((error) => {
    if (error instanceof Deno.errors.NotFound) {
      return null;
    }
    throw error;
  });
  if (!file) {
    return new Response("not found", { status: 404 });
  }

  return new Response(file.readable, {
    headers: {
      "content-type": contentTypes.get(extension(pathname)) ?? "application/octet-stream",
    },
  });
}

Deno.serve({ hostname: "127.0.0.1", port: 4173 }, serveStatic);
