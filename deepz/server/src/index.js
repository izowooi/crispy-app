// DeepZ 무료 번역 서버 (Cloudflare Workers).
// 번역할 글은 기록하지 않는다. 저장하는 것은 설치 ID·IP 별 "오늘 몇 번 썼는지" 숫자뿐이다.
import { DurableObject } from "cloudflare:workers";
import { DailyCount, ID_PATTERN, TARGETS, dayKey, systemPrompt, verifySignature } from "./quota.js";

function fail(status, code, message) {
  return Response.json({ error: { code, message } }, { status, headers: { "cache-control": "no-store" } });
}

/** 이름(설치 ID, IP, 전체)마다 하나씩 생기는 하루 횟수 저장소. */
export class Counter extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.daily = new DailyCount(ctx.storage);
  }
  take(day, limit) {
    return this.daily.take(day, limit);
  }
  refund(day) {
    return this.daily.refund(day);
  }
}

// 오늘 한도를 넘긴 설치 ID·IP. 같은 인스턴스가 받는 다음 요청부터는 번역 요청을 보내지 않고 바로 거절한다.
const denied = { day: "", names: new Set(), everyone: false };

function claudeRequest(env, target, text) {
  return fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-api-key": env.ANTHROPIC_API_KEY,
      "anthropic-version": "2026-10-21",
    },
    body: JSON.stringify({
      model: env.MODEL,
      max_tokens: 8000,
      stream: true,
      system: systemPrompt(target),
      messages: [{ role: "user", content: text }],
      // 번역에는 추론이 필요 없다. 끄면 첫 글자가 가장 빨리 나온다.
      thinking: { type: "disabled" },
      output_config: { effort: "low" },
    }),
  });
}

function openAIRequest(env, target, text) {
  return fetch("https://api.openai.com/v1/chat/completions", {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${env.OPENAI_API_KEY}` },
    body: JSON.stringify({
      model: env.OPENAI_MODEL,
      stream: true,
      reasoning_effort: "low",
      messages: [{ role: "system", content: systemPrompt(target) }, { role: "user", content: text }],
    }),
  });
}

async function translate(request, env, ctx) {
  // 앱은 Origin 을 보내지 않는다. 웹 페이지에서 부르는 요청은 받지 않는다.
  if (request.headers.has("origin")) return fail(403, "forbidden", "허용되지 않은 요청입니다.");
  if (!env.APP_SECRET || !env.ANTHROPIC_API_KEY) return fail(503, "not_configured", "무료 번역 서버가 아직 준비되지 않았습니다.");

  const id = request.headers.get("x-deepz-id") ?? "";
  const maxChars = Number(env.MAX_CHARS);
  // 한글은 UTF-8 로 글자당 3바이트. 그보다 큰 본문은 읽기 전에 거절한다.
  if (Number(request.headers.get("content-length") ?? 0) > maxChars * 4 + 200) {
    return fail(413, "too_long", `무료 번역은 한 번에 ${maxChars}자까지입니다. API 키를 넣으면 20000자까지 번역할 수 있습니다.`);
  }
  const body = await request.arrayBuffer();
  const now = Date.now();
  const signed = ID_PATTERN.test(id) && await verifySignature(
    env.APP_SECRET, request.headers.get("x-deepz-ts") ?? "", id, body, request.headers.get("x-deepz-sig"), now);
  if (!signed) return fail(401, "unauthorized", "요청을 확인하지 못했습니다. 컴퓨터 시계가 맞는지 확인하고, DeepZ 를 최신 버전으로 바꿔 주세요.");

  let text, target;
  try {
    ({ text, to: target } = JSON.parse(new TextDecoder().decode(body)));
  } catch {
    return fail(400, "bad_request", "요청 형식이 잘못됐습니다.");
  }
  if (typeof text !== "string" || !text.trim() || !TARGETS.has(target)) return fail(400, "bad_request", "요청 형식이 잘못됐습니다.");
  if ([...text].length > maxChars) {
    return fail(413, "too_long", `무료 번역은 한 번에 ${maxChars}자까지입니다. API 키를 넣으면 20000자까지 번역할 수 있습니다.`);
  }

  const day = dayKey(now);
  const ip = request.headers.get("cf-connecting-ip") ?? "unknown";
  const limitMessage = `오늘 무료 번역 ${env.DAILY_LIMIT}회를 모두 썼습니다. 내일 다시 쓸 수 있습니다.\nAPI 키를 넣으면 횟수 제한 없이 쓸 수 있습니다.`;
  if (denied.day !== day) {
    denied.day = day;
    denied.names.clear();
    denied.everyone = false;
  }
  if (denied.everyone) return fail(429, "busy", "오늘 무료 번역이 모두 소진됐습니다. API 키를 넣으면 바로 쓸 수 있습니다.");
  if (denied.names.has(id) || denied.names.has(ip)) return fail(429, "daily_limit", limitMessage);

  const counter = (name) => env.COUNTER.get(env.COUNTER.idFromName(name));
  const byId = counter(`id:${id}`);
  const byIp = counter(`ip:${ip}`);
  // 횟수 확인과 번역 요청을 동시에 보낸다. 확인을 먼저 기다리면 그만큼 첫 글자가 늦어진다.
  const [leftForId, leftForIp, first] = await Promise.all([
    byId.take(day, Number(env.DAILY_LIMIT)),
    byIp.take(day, Number(env.IP_DAILY_LIMIT)),
    claudeRequest(env, target, text).catch(() => null),
  ]);
  const refund = () => Promise.all([leftForId >= 0 && byId.refund(day), leftForIp >= 0 && byIp.refund(day)]);
  if (leftForId < 0 || leftForIp < 0) {
    ctx.waitUntil(Promise.all([refund(), first?.body?.cancel()]));
    if (denied.names.size < 10_000) denied.names.add(leftForId < 0 ? id : ip);
    return fail(429, "daily_limit", limitMessage);
  }
  // 전체 한도는 응답을 기다리게 하지 않고 뒤에서 센다.
  ctx.waitUntil(counter("global").take(day, Number(env.GLOBAL_DAILY_LIMIT)).then((left) => {
    if (left < 0) denied.everyone = true;
  }));

  let format = "claude", model = env.MODEL, upstream = first;
  if (!upstream?.ok && env.OPENAI_API_KEY) {
    console.log(`claude failed: ${upstream?.status ?? "network"}`);
    format = "openai";
    model = env.OPENAI_MODEL;
    upstream = await openAIRequest(env, target, text).catch(() => null);
  }
  if (!upstream?.ok) {
    console.log(`upstream failed: ${upstream?.status ?? "network"}`);
    ctx.waitUntil(refund());
    return fail(502, "upstream", "번역 서버가 잠시 응답하지 않습니다. 잠시 뒤 다시 시도해 주세요.");
  }
  // 받은 스트림을 그대로 흘려보낸다. 중간에서 풀어 보거나 모으지 않는다.
  return new Response(upstream.body, {
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-store",
      "x-deepz-format": format,
      "x-deepz-model": model,
      "x-deepz-remaining": String(leftForId),
    },
  });
}

export default {
  async fetch(request, env, ctx) {
    const { pathname } = new URL(request.url);
    if (request.method === "POST" && pathname === "/v1/translate") return translate(request, env, ctx);
    return fail(404, "not_found", "없는 주소입니다.");
  },
};
