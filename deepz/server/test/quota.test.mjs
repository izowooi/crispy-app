// `node --test` 로 돌린다.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHmac } from "node:crypto";

import { DailyCount, dayKey, systemPrompt, verifySignature } from "../src/quota.js";

const sign = (secret, ts, id, body) => createHmac("sha256", secret).update(`${ts}\n${id}\n`).update(body).digest("hex");
const body = new TextEncoder().encode('{"text":"hi","to":"Korean"}');
const id = "0123456789abcdef0123456789abcdef";
const now = 1_800_000_000_000;

test("한도는 한국 시간 자정에 바뀐다", () => {
  assert.equal(dayKey(Date.UTC(2026, 10, 28, 14, 59)), "2026-11-28");
  assert.equal(dayKey(Date.UTC(2026, 10, 28, 15, 0)), "2026-11-29");
});

test("올바른 서명만 통과한다", async () => {
  const ts = String(now / 1000);
  const good = sign("s3cret", ts, id, body);
  assert.equal(await verifySignature("s3cret", ts, id, body.buffer, good, now), true);
  assert.equal(await verifySignature("other", ts, id, body.buffer, good, now), false);
  assert.equal(await verifySignature("s3cret", ts, id.replace("0", "1"), body.buffer, good, now), false);
  assert.equal(await verifySignature("s3cret", ts, id, new TextEncoder().encode("x").buffer, good, now), false);
  assert.equal(await verifySignature("s3cret", ts, id, body.buffer, "zz", now), false);
  assert.equal(await verifySignature("s3cret", ts, id, body.buffer, undefined, now), false);
});

test("오래된 요청은 서명이 맞아도 거절한다", async () => {
  const old = String(now / 1000 - 16 * 60);
  assert.equal(await verifySignature("s3cret", old, id, body.buffer, sign("s3cret", old, id, body), now), false);
});

test("번역 대상 언어가 프롬프트에 들어간다", () => {
  assert.match(systemPrompt("Korean"), /fluent Korean\./);
});

test("하루 한도까지만 쓰고, 날이 바뀌면 다시 쓸 수 있다", async () => {
  const saved = new Map();
  const counter = new DailyCount({ get: async (k) => saved.get(k), put: (k, v) => saved.set(k, v) });
  assert.equal(await counter.take("d1", 2), 1);
  assert.equal(await counter.take("d1", 2), 0);
  assert.equal(await counter.take("d1", 2), -1);
  await counter.refund("d1");
  assert.equal(await counter.take("d1", 2), 0);
  assert.equal(await counter.take("d2", 2), 1);
  // 인스턴스가 새로 떠도 저장된 값에서 이어 간다.
  const again = new DailyCount({ get: async (k) => saved.get(k), put: (k, v) => saved.set(k, v) });
  assert.equal(await again.take("d2", 2), 0);
});
