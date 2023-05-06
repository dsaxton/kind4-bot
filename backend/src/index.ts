import { Event, nip19, validateEvent } from "nostr-tools";

export interface Env {
  KIND4_ARCHIVE: KVNamespace;
}

type ParsedRequest = {
  method: string;
  path: string;
  params: URLSearchParams;
  body: any;
};

async function parseRequest(request: Request): Promise<ParsedRequest> {
  const method = request.method;
  const url = new URL(request.url);
  const params = new URLSearchParams(url.searchParams);
  const path = url.pathname;
  let body;
  try {
    body = await request.json();
  } catch (err) {
    body = {};
  }
  return { method, path, params, body };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const parsed = await parseRequest(request.clone());

    if (parsed.method === "OPTIONS") {
      return new Response(null, {
        status: 200,
        headers: { allow: "OPTIONS, GET, PUT" },
      });
    }

    if (parsed.method === "PUT") {
      const parsed = await parseRequest(request);
      let event: Event = parsed.body;
      if (!validateEvent(event)) {
        return new Response(
          JSON.stringify({ error: "Body is not a valid nostr event" }),
          { status: 400 }
        );
      }
      if (event.kind !== 4) {
        return new Response(
          JSON.stringify({ error: "Event is not a kind 4" }),
          { status: 400 }
        );
      }
      const created_at = event.created_at;
      let sender = event.pubkey;
      let receiver = "";
      for (const tag of event.tags) {
        if (tag[0] === "p") {
          receiver = tag[1];
          break;
        }
      }
      try {
        sender = nip19.npubEncode(sender);
        receiver = nip19.npubEncode(receiver);
      } catch {
        return new Response(
          JSON.stringify({ error: "Unable to npub encode sender or receiver" }),
          {
            status: 400,
          }
        );
      }
      await env.KIND4_ARCHIVE.put(
        `${sender}:${receiver}:${created_at}`,
        JSON.stringify(event)
      );
      return new Response(null, { status: 200 });
    }

    if (parsed.method === "GET") {
      if (parsed.path === "/counts") {
        const sender = parsed.params.get("sender");
        const receiver = parsed.params.get("receiver");
        const since = parsed.params.get("since");
        if (!sender) {
          return new Response(
            JSON.stringify({ error: "Must specify a sender" }),
            { status: 400 }
          );
        }
        const list_result = await env.KIND4_ARCHIVE.list({ prefix: sender });
        let keys = list_result.keys.map((key) => key.name);
        if (receiver) {
          keys = keys.filter((key) => key.split(":")[1] === receiver);
        }
        if (since) {
          keys = keys.filter(
            (key) => parseInt(key.split(":")[2]) >= parseInt(since)
          );
        }
        let counts: any = {};
        for (const key of keys) {
          const receiver = key.split(":")[1];
          counts[receiver] = (counts[receiver] || 0) + 1;
        }
        return new Response(JSON.stringify(counts), { status: 200 });
      }
      return new Response(JSON.stringify({ error: "Invalid route" }), {
        status: 400,
      });
    }

    return new Response(null, { status: 405 });
  },
};
