// A tiny mock "DDT" order API for trying the `tools-http` plugin.
// No dependencies; needs Node 18+.
//
//   node examples/http-api/server.mjs            (listens on 127.0.0.1:8787)
//   (the token defaults to "demo-token"; override it with DDT_TOKEN)
//
// Endpoints (all require `Authorization: Bearer <DDT_TOKEN>`):
//   GET  /orders/{id}         one order
//   GET  /orders?q=&limit=    search orders by customer or status
//   POST /orders/{id}/note    {"text": "..."} appends a note (a write)
import http from "node:http";

const PORT = Number(process.env.PORT ?? 8787);
const TOKEN = process.env.DDT_TOKEN ?? "demo-token";

const orders = new Map([
  ["1001", { id: "1001", customer: "Acme Corp", status: "shipped", total: 249.5, notes: [] }],
  ["1002", { id: "1002", customer: "Globex", status: "pending", total: 80, notes: [] }],
  ["1003", { id: "1003", customer: "Acme Corp", status: "cancelled", total: 15.25, notes: [] }],
  ["1004", { id: "1004", customer: "Initech", status: "pending", total: 1200, notes: [] }],
]);

function send(res, status, body) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
}

async function readJson(req) {
  let raw = "";
  for await (const chunk of req) raw += chunk;
  return raw ? JSON.parse(raw) : {};
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  console.log(`${req.method} ${url.pathname}${url.search}`);

  if (req.headers.authorization !== `Bearer ${TOKEN}`) {
    return send(res, 401, { error: "missing or wrong bearer token" });
  }

  const one = url.pathname.match(/^\/orders\/([^/]+)$/);
  const note = url.pathname.match(/^\/orders\/([^/]+)\/note$/);

  try {
    if (req.method === "GET" && url.pathname === "/orders") {
      const q = (url.searchParams.get("q") ?? "").toLowerCase();
      const limit = Number(url.searchParams.get("limit") ?? 10);
      const hits = [...orders.values()]
        .filter((o) => `${o.customer} ${o.status}`.toLowerCase().includes(q))
        .slice(0, limit);
      return send(res, 200, { count: hits.length, orders: hits });
    }
    if (req.method === "GET" && one) {
      const order = orders.get(decodeURIComponent(one[1]));
      return order ? send(res, 200, order) : send(res, 404, { error: "no such order" });
    }
    if (req.method === "POST" && note) {
      const order = orders.get(decodeURIComponent(note[1]));
      if (!order) return send(res, 404, { error: "no such order" });
      const { text } = await readJson(req);
      if (typeof text !== "string" || !text) return send(res, 400, { error: "text is required" });
      order.notes.push(text);
      return send(res, 200, order);
    }
    send(res, 404, { error: "not found" });
  } catch (err) {
    send(res, 400, { error: String(err.message ?? err) });
  }
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`mock DDT API on http://127.0.0.1:${PORT} (token: ${TOKEN === "demo-token" ? "demo-token" : "from DDT_TOKEN"})`);
});
