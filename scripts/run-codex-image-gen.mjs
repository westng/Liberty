#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const CODEX_HOME = process.env.CODEX_HOME || path.join(os.homedir(), ".codex");
const CODEX_CONFIG = process.env.CODEX_CONFIG || path.join(CODEX_HOME, "config.toml");
const CODEX_AUTH = process.env.CODEX_AUTH || path.join(CODEX_HOME, "auth.json");
const IMAGE_GEN =
  process.env.IMAGE_GEN ||
  path.join(CODEX_HOME, "skills", ".system", "imagegen", "scripts", "image_gen.py");
const PYTHON = process.env.PYTHON || "python3";

function fail(message) {
  console.error(`Error: ${message}`);
  process.exit(1);
}

function unquoteToml(value) {
  const trimmed = value.trim();
  const match = trimmed.match(/^"((?:[^"\\]|\\.)*)"/);
  if (!match) return trimmed;
  return match[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
}

function readCodexProviderConfig(configPath) {
  if (!fs.existsSync(configPath)) {
    fail(`Codex config not found: ${configPath}`);
  }

  const text = fs.readFileSync(configPath, "utf8");
  const providers = new Map();
  let modelProvider = null;
  let currentProvider = null;

  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, "").trim();
    if (!line) continue;

    const topProvider = line.match(/^model_provider\s*=\s*(.+)$/);
    if (topProvider && currentProvider === null) {
      modelProvider = unquoteToml(topProvider[1]);
      continue;
    }

    const section = line.match(/^\[model_providers\.(?:"([^"]+)"|([^\]]+))\]$/);
    if (section) {
      currentProvider = section[1] || section[2];
      if (!providers.has(currentProvider)) providers.set(currentProvider, {});
      continue;
    }

    if (line.startsWith("[") && line.endsWith("]")) {
      currentProvider = null;
      continue;
    }

    if (currentProvider) {
      const kv = line.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$/);
      if (!kv) continue;
      const [, key, value] = kv;
      providers.get(currentProvider)[key] = unquoteToml(value);
    }
  }

  if (!modelProvider) fail(`model_provider is not set in ${configPath}`);
  const provider = providers.get(modelProvider);
  if (!provider) fail(`model_provider "${modelProvider}" has no [model_providers.${modelProvider}] block`);
  if (!provider.base_url) fail(`model_provider "${modelProvider}" has no base_url`);

  return {
    providerName: modelProvider,
    baseUrl: provider.base_url,
    requiresOpenAIAuth: String(provider.requires_openai_auth || "").trim() === "true",
  };
}

function findOpenAIKey(value) {
  if (!value || typeof value !== "object") return null;
  if (typeof value.OPENAI_API_KEY === "string" && value.OPENAI_API_KEY.trim()) {
    return value.OPENAI_API_KEY.trim();
  }
  if (typeof value.api_key === "string" && value.api_key.trim()) {
    return value.api_key.trim();
  }
  for (const nested of Object.values(value)) {
    const found = findOpenAIKey(nested);
    if (found) return found;
  }
  return null;
}

function readAuthKey(authPath, requiresOpenAIAuth) {
  if (fs.existsSync(authPath)) {
    const auth = JSON.parse(fs.readFileSync(authPath, "utf8"));
    const key = findOpenAIKey(auth);
    if (key) return key;
  }

  if (process.env.OPENAI_API_KEY) return process.env.OPENAI_API_KEY;
  if (requiresOpenAIAuth) fail(`OPENAI_API_KEY not found in ${authPath} or environment`);
  return "not-needed";
}

function buildEnv({ baseUrl, apiKey }) {
  const env = {
    ...process.env,
    OPENAI_BASE_URL: baseUrl,
    OPENAI_API_KEY: apiKey,
  };

  const localPythonPath = process.env.IMAGEGEN_PYTHONPATH || path.join(process.cwd(), "tmp", "imagegen-py");
  if (fs.existsSync(localPythonPath)) {
    env.PYTHONPATH = env.PYTHONPATH
      ? `${localPythonPath}${path.delimiter}${env.PYTHONPATH}`
      : localPythonPath;
  }

  return env;
}

const printConfig = process.argv.includes("--print-effective-config");
const passthroughArgs = process.argv
  .slice(2)
  .filter((arg) => arg !== "--print-effective-config");

const provider = readCodexProviderConfig(CODEX_CONFIG);
const apiKey = readAuthKey(CODEX_AUTH, provider.requiresOpenAIAuth);

if (printConfig) {
  console.error(`Codex model_provider: ${provider.providerName}`);
  console.error(`Codex base_url: ${provider.baseUrl}`);
  console.error(`Codex auth: ${apiKey ? "OPENAI_API_KEY loaded" : "not loaded"}`);
  console.error(`Image CLI: ${IMAGE_GEN}`);
}

if (!passthroughArgs.length) {
  if (printConfig) process.exit(0);
  console.error("Usage: node scripts/run-codex-image-gen.mjs [--print-effective-config] <image_gen.py args...>");
  console.error("Example: node scripts/run-codex-image-gen.mjs generate --model gpt-image-2 --prompt-file prompt.txt --out output.png");
  process.exit(1);
}

if (!fs.existsSync(IMAGE_GEN)) {
  fail(`image_gen.py not found: ${IMAGE_GEN}`);
}

const result = spawnSync(PYTHON, [IMAGE_GEN, ...passthroughArgs], {
  stdio: "inherit",
  env: buildEnv({ baseUrl: provider.baseUrl, apiKey }),
});

if (result.error) fail(result.error.message);
if (typeof result.status === "number") process.exit(result.status);
process.exit(result.signal ? 1 : 0);
