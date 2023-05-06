import { nip19, validateEvent, Event } from "nostr-tools"

export interface Env {
  KIND4_ARCHIVE: KVNamespace;
}

type ParsedRequest = {
  method: string;
  params: URLSearchParams;
  body: any;
};

async function parseRequest(request: Request): Promise<ParsedRequest> {
  const method = request.method;
  const url = new URL(request.url);
  const params = new URLSearchParams(url.searchParams);
  let body;
  try {
    body = await request.json();
  } catch (err) {
    console.log(err)
    body = {};
  }
  return { method, params, body };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const parsed = await parseRequest(request.clone());

    if (parsed.method === "OPTIONS") {
      return new Response(null, {
        status: 200,
        headers: { allow: "OPTIONS, PUT" },
      });
    }

    if (parsed.method === "PUT") {
      const parsed = await parseRequest(request);
      let event: Event = parsed.body;
      if (!validateEvent(event)) {
        return new Response("Body is not a valid nostr event", { status: 400 });
      }
      if (event.kind !== 4) {
        return new Response("Event is not kind 4", { status: 400 });
      }
      const created_at = event.created_at;
      let sender = event.pubkey;
      let receiver = "";
      for (const tag of event.tags) {
        if (tag[0] === "p") {
          receiver = tag[1]
          break
        }
      }
      try {
        sender = nip19.npubEncode(sender);
        receiver = nip19.npubEncode(receiver);
      } catch {
        return new Response("Unable to npub encode sender or receiver", { status: 400 });
      }
      await env.KIND4_ARCHIVE.put(`${sender}:${receiver}:${created_at}`, JSON.stringify(event));
      return new Response(null, { status: 200 });
    }

    return new Response(null, { status: 405 });
  },
};
