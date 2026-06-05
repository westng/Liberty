#!/usr/bin/env node
import { createHash, createHmac, generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const DEFAULT_PRIVATE_KEY_PATH = ".liberty-secrets/redeem-private-key.pem";
const DEFAULT_PUBLIC_KEY_PATH = ".liberty-secrets/redeem-public-key.base64";
const DEFAULT_OUT_PATH = ".liberty-secrets/redeem-keys.csv";
const KEY_PREFIX = "LIB2";
const SHORT_KEY_SECRET_B64 = "B+6+O1eix4RItBiakvkqdGDRWlVK8I1m0ABDHgmJCXE=";
const SHORT_PAYLOAD_BYTES = 20;
const SHORT_MAC_BYTES = 12;
const SHORT_TOKEN_BYTES = SHORT_PAYLOAD_BYTES + SHORT_MAC_BYTES;
const SHORT_TOKEN_CHUNK_SIZE = 13;
const SHORT_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const COMPACT_PAYLOAD_BYTES = 28;
const COMPACT_SIGNATURE_BYTES = 64;
const COMPACT_KEY_LENGTH = 128;
const COMPACT_BASE_DATE = Date.UTC(2026, 0, 1);
const DAY_MS = 24 * 60 * 60 * 1000;

const command = process.argv[2] ?? "help";
const args = parseArgs(process.argv.slice(3));

if (command === "keypair") {
  generateKeyPair(args);
} else if (command === "generate") {
  generateRedeemKeys(args);
} else {
  printHelp();
}

function generateKeyPair(args) {
  const privateKeyPath = resolve(args.privateKey ?? DEFAULT_PRIVATE_KEY_PATH);
  const publicKeyPath = resolve(args.publicKey ?? DEFAULT_PUBLIC_KEY_PATH);
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const spki = publicKey.export({ type: "spki", format: "der" });
  const publicKeyRaw = spki.subarray(spki.length - 32);

  mkdirSync(dirname(privateKeyPath), { recursive: true });
  mkdirSync(dirname(publicKeyPath), { recursive: true });
  writeFileSync(privateKeyPath, privateKey.export({ type: "pkcs8", format: "pem" }));
  writeFileSync(publicKeyPath, `${publicKeyRaw.toString("base64")}\n`);

  console.log(`Private key: ${privateKeyPath}`);
  console.log(`Public key:  ${publicKeyPath}`);
  console.log(`Public key base64: ${publicKeyRaw.toString("base64")}`);
}

function generateRedeemKeys(args) {
  const campaignId = required(args.campaign, "--campaign");
  const displayPrefix = normalizeDisplayPrefix(args.prefix ?? campaignId);
  const count = positiveInteger(args.count ?? "1", "--count");
  const rewards = parseRewards(args);
  const outPath = resolve(args.out ?? DEFAULT_OUT_PATH);
  const rows = [["campaign_id", "key", "reward_summary"]];

  for (let index = 0; index < count; index += 1) {
    const key = encodeShortKey({ displayPrefix, rewards, expiresAt: args.expires ?? null });
    rows.push([
      campaignId,
      key,
      rewardSummary(rewards),
    ]);
  }

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, rows.map(csvRow).join("\n"));
  console.log(`Generated ${count} key(s): ${outPath}`);
}

function parseRewards(args) {
  const lp = nonNegativeInteger(args.lp ?? "0", "--lp");
  const growthValue = nonNegativeInteger(args.growth ?? "0", "--growth");
  const items = values(args.item).map((value) => {
    const [itemKey, quantityValue = "1"] = value.split(":");
    if (!itemKey?.trim()) {
      throw new Error("--item must be itemKey or itemKey:quantity");
    }
    return {
      itemKey: itemKey.trim(),
      quantity: positiveInteger(quantityValue, "--item quantity"),
    };
  });
  if (lp <= 0 && growthValue <= 0 && items.length === 0) {
    throw new Error("At least one reward is required: --lp, --growth, or --item");
  }
  return { lp, growthValue, items };
}

function encodeShortKey({ displayPrefix, rewards, expiresAt }) {
  const payload = encodeShortPayload({ rewards, expiresAt });
  const mac = createHmac("sha256", Buffer.from(SHORT_KEY_SECRET_B64, "base64"))
    .update(displayPrefix)
    .update(".")
    .update(payload)
    .digest()
    .subarray(0, SHORT_MAC_BYTES);
  const token = encodeCrockfordBase32(Buffer.concat([payload, mac]));
  const key = `${displayPrefix}-${chunkText(token, SHORT_TOKEN_CHUNK_SIZE).join("-")}`;
  if (key.length > 64) {
    throw new Error(`Unexpected short redeem key length: ${key.length}`);
  }
  return key;
}

function encodeShortPayload({ rewards, expiresAt }) {
  if (rewards.lp > 65_535) {
    throw new Error("--lp must be <= 65535 for short keys");
  }
  if (rewards.growthValue > 65_535) {
    throw new Error("--growth must be <= 65535 for short keys");
  }
  if (rewards.items.length > 2) {
    throw new Error("Short keys support at most 2 item reward types");
  }

  const buffer = Buffer.alloc(SHORT_PAYLOAD_BYTES);
  buffer.writeUInt8(3, 0);
  buffer.writeUInt8(rewards.items.length, 1);
  randomBytes(6).copy(buffer, 2);
  buffer.writeUInt16BE(expiresAt ? daysSinceBase(expiresAt, "--expires") : 0xffff, 8);
  buffer.writeUInt16BE(rewards.lp, 10);
  buffer.writeUInt16BE(rewards.growthValue, 12);

  rewards.items.forEach((item, index) => {
    if (item.quantity > 255) {
      throw new Error(`--item quantity for ${item.itemKey} must be <= 255 for short keys`);
    }
    const offset = index === 0 ? 14 : 17;
    buffer.writeUInt16BE(hashUInt16(item.itemKey), offset);
    buffer.writeUInt8(item.quantity, offset + 2);
  });

  return buffer;
}

function encodeCompactPayload({ campaignId, rewards, expiresAt }) {
  if (rewards.lp > 65_535) {
    throw new Error("--lp must be <= 65535 for compact keys");
  }
  if (rewards.growthValue > 65_535) {
    throw new Error("--growth must be <= 65535 for compact keys");
  }
  if (rewards.items.length > 2) {
    throw new Error("Compact keys support at most 2 item reward types");
  }

  const buffer = Buffer.alloc(COMPACT_PAYLOAD_BYTES);
  buffer.writeUInt8(2, 0);
  buffer.writeUInt8(rewards.items.length, 1);
  buffer.writeUInt32BE(hashUInt32(campaignId), 2);
  randomBytes(8).copy(buffer, 6);
  buffer.writeUInt16BE(daysSinceBase(new Date().toISOString().slice(0, 10), "--issuedAt"), 14);
  buffer.writeUInt16BE(expiresAt ? daysSinceBase(expiresAt, "--expires") : 0xffff, 16);
  buffer.writeUInt16BE(rewards.lp, 18);
  buffer.writeUInt16BE(rewards.growthValue, 20);

  rewards.items.forEach((item, index) => {
    if (item.quantity > 255) {
      throw new Error(`--item quantity for ${item.itemKey} must be <= 255 for compact keys`);
    }
    const offset = index === 0 ? 22 : 25;
    buffer.writeUInt16BE(hashUInt16(item.itemKey), offset);
    buffer.writeUInt8(item.quantity, offset + 2);
  });

  return buffer;
}

function normalizeDisplayPrefix(value) {
  const normalized = String(value).toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 8);
  if (!normalized) {
    throw new Error("--prefix must contain at least one letter or number");
  }
  return normalized;
}

function chunkText(value, size) {
  const chunks = [];
  for (let index = 0; index < value.length; index += size) {
    chunks.push(value.slice(index, index + size));
  }
  return chunks;
}

function encodeCrockfordBase32(buffer) {
  let output = "";
  let value = 0;
  let bits = 0;
  for (const byte of buffer) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      output += SHORT_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) {
    output += SHORT_ALPHABET[(value << (5 - bits)) & 31];
  }
  return output;
}

function hashUInt32(value) {
  return createHash("sha256").update(value).digest().readUInt32BE(0);
}

function hashUInt16(value) {
  const code = createHash("sha256").update(value).digest().readUInt16BE(0);
  if (code === 0) {
    throw new Error(`Item key ${value} produced a reserved compact code`);
  }
  return code;
}

function daysSinceBase(value, flag) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    throw new Error(`${flag} must use YYYY-MM-DD`);
  }
  const date = Date.parse(`${value}T00:00:00.000Z`);
  if (!Number.isFinite(date)) {
    throw new Error(`${flag} is invalid`);
  }
  const days = Math.floor((date - COMPACT_BASE_DATE) / DAY_MS);
  if (days < 0 || days > 65_534) {
    throw new Error(`${flag} must be between 2026-01-01 and 2205-06-05`);
  }
  return days;
}

function parseArgs(values) {
  const result = {};
  for (let index = 0; index < values.length; index += 1) {
    const current = values[index];
    if (!current.startsWith("--")) {
      continue;
    }
    const key = current.slice(2);
    const next = values[index + 1];
    const value = next && !next.startsWith("--") ? next : "true";
    if (Object.prototype.hasOwnProperty.call(result, key)) {
      result[key] = Array.isArray(result[key]) ? [...result[key], value] : [result[key], value];
    } else {
      result[key] = value;
    }
    if (value !== "true") {
      index += 1;
    }
  }
  return result;
}

function values(value) {
  if (!value) {
    return [];
  }
  return Array.isArray(value) ? value : [value];
}

function required(value, flag) {
  if (!value || value === "true") {
    throw new Error(`${flag} is required`);
  }
  return value;
}

function positiveInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return parsed;
}

function nonNegativeInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${flag} must be a non-negative integer`);
  }
  return parsed;
}

function base64UrlEncode(buffer) {
  return buffer.toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}

function rewardSummary(rewards) {
  const parts = [
    rewards.lp > 0 ? `${rewards.lp}LP` : "",
    rewards.growthValue > 0 ? `growth+${rewards.growthValue}` : "",
    ...rewards.items.map((item) => `${item.itemKey}x${item.quantity}`),
  ].filter(Boolean);
  return parts.join(" + ");
}

function csvRow(values) {
  return values.map((value) => `"${String(value).replace(/"/g, '""')}"`).join(",");
}

function printHelp() {
  console.log(`
Usage:
  node scripts/redeem-key.mjs keypair
  node scripts/redeem-key.mjs generate --campaign 2026-gift --prefix LP2026 --lp 120 --growth 20 --item gem-ticket-tool:2 --count 10 --expires 2026-12-31

Generated redeem keys use the short offline format and are no more than 64 characters long.

Options:
  --privateKey <path>   Private key PEM path. Default: ${DEFAULT_PRIVATE_KEY_PATH}
  --publicKey <path>    Public key output path for keypair. Default: ${DEFAULT_PUBLIC_KEY_PATH}
  --prefix <text>       User-facing key prefix, max 8 letters/numbers. Example: LP2026
  --out <path>          CSV output path. Default: ${DEFAULT_OUT_PATH}
`);
}
