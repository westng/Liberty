import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const tauriConfig = JSON.parse(
  readFileSync("apps/desktop/src-tauri/tauri.conf.json", "utf8"),
);
const runtimeManifest = JSON.parse(
  readFileSync("apps/desktop/src-tauri/resources/runtime-manifest.json", "utf8"),
);
const systemSource = readFileSync(
  "apps/desktop/src-tauri/src/system.rs",
  "utf8",
);
const archiveSource = readFileSync(
  "apps/desktop/src-tauri/src/local_runtime/archive.rs",
  "utf8",
);
const buildSource = readFileSync(
  "apps/desktop/src-tauri/build.rs",
  "utf8",
);
const handlerSource = readFileSync(
  "apps/desktop/src-tauri/src/lib.rs",
  "utf8",
);
const localJobsSource = readFileSync(
  "apps/desktop/src-tauri/src/local_jobs.rs",
  "utf8",
);
const localAiSource = readFileSync(
  "apps/desktop/src-tauri/src/local_ai.rs",
  "utf8",
);
const windowScopeSource = readFileSync(
  "apps/desktop/src-tauri/src/window_scope.rs",
  "utf8",
);
const capabilitiesDir = "apps/desktop/src-tauri/capabilities";
const capabilities = readdirSync(capabilitiesDir)
  .filter((fileName) => fileName.endsWith(".json"))
  .map((fileName) => ({
    fileName,
    data: JSON.parse(readFileSync(join(capabilitiesDir, fileName), "utf8")),
  }));

const errors = [];
const csp = tauriConfig?.app?.security?.csp;
const sha256Pattern = /^[a-fA-F0-9]{64}$/;
const permissionForCommand = (command) => `allow-${command.replaceAll("_", "-")}`;
const commandForPermission = (permission) => permission.slice("allow-".length).replaceAll("-", "_");

function extractBuildCommands(source) {
  const block = source.match(/const COMMANDS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/)?.[1] ?? "";
  return [...block.matchAll(/"([a-z0-9_]+)"/g)].map((match) => match[1]);
}

function extractHandlerCommands(source) {
  const block = source.match(/invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/)?.[1] ?? "";
  return [...block.matchAll(/(?:^|\n)\s*(?:[a-z0-9_]+::)+([a-z0-9_]+),?(?=\s*(?:\n|$))/g)]
    .map((match) => match[1]);
}

function difference(left, right) {
  return [...left].filter((item) => !right.has(item)).sort();
}

const expectedPermissionsByWindow = {
  main: [
    "core:default",
    "core:webview:allow-create-webview-window",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-set-focus",
    "core:window:allow-set-decorations",
    "core:window:allow-start-dragging",
    "core:window:allow-toggle-maximize",
    "dialog:default",
    "dialog:allow-confirm",
    "fs:allow-write-text-file",
    ...[
      "export_desktop_pet_diagnostic_log",
      "get_diagnostics",
      "delete_ai_model",
      "delete_ai_summary_run",
      "delete_ai_template",
      "get_ai_summary_options",
      "list_ai_models",
      "list_ai_summary_runs",
      "list_ai_templates",
      "save_ai_model",
      "save_ai_template",
      "start_or_resume_ai_summary_run",
      "set_active_ai_summary_run",
      "export_job_summary_docx",
      "get_farm_state",
      "get_work_market_state",
      "harvest_farm_plot",
      "list_farm_harvest_ledger",
      "plant_farm_crop",
      "water_farm_plot",
      "care_work_game_task",
      "claim_work_game_task",
      "get_work_game_state",
      "start_work_game_task",
      "create_job",
      "delete_job",
      "get_job",
      "get_job_result",
      "list_jobs",
      "rename_job_speaker",
      "retry_job",
      "get_remote_capabilities",
      "remote_list_jobs",
      "remote_get_job",
      "remote_get_job_result",
      "remote_retry_job",
      "remote_delete_job",
      "remote_rename_job_speaker",
      "delete_meeting_member",
      "export_meeting_members_excel",
      "import_meeting_members_excel",
      "list_meeting_members",
      "save_meeting_member",
      "apply_pet_interaction",
      "apply_pet_workflow_event",
      "claim_pet_daily_check_in",
      "draw_pet_blind_box",
      "get_pet_daily_check_in_state",
      "get_pet_blind_box_state",
      "get_pet_profile",
      "get_pet_store_state",
      "get_pet_settings",
      "get_desktop_pet_status",
      "hide_desktop_pet",
      "list_pet_cosmetic_unlocks",
      "list_pet_event_ledger",
      "list_pet_redeem_key_redemptions",
      "open_extra_desktop_pet",
      "open_pet_gift_box",
      "purchase_pet_store_item",
      "repair_pet_daily_check_in",
      "redeem_pet_key",
      "save_pet_profile",
      "show_desktop_pet",
      "start_desktop_pet_drag",
      "equip_pet_inventory_item",
      "unequip_pet_inventory_slot",
      "use_pet_inventory_item",
      "detect_runtime_component",
      "get_runtime_component_log",
      "get_runtime_install_log",
      "get_runtime_status",
      "install_runtime",
      "install_runtime_component",
      "list_runtime_download_sources",
      "set_runtime_component_source",
      "save_pet_settings",
      "get_settings",
      "get_ui_preferences",
      "save_settings",
      "get_process_metrics",
      "open_external_url",
      "prompt_pet_name",
      "issue_job_window_scope",
      "set_current_window_theme",
    ].map(permissionForCommand),
  ],
  "ai-summary": [
    "core:event:allow-emit",
    "dialog:allow-confirm",
    ...[
      "close_current_window",
      "set_current_window_theme",
      "get_ui_preferences",
      "get_job_result",
      "get_ai_summary_options",
      "start_or_resume_ai_summary_run",
      "set_active_ai_summary_run",
      "delete_ai_summary_run",
    ].map(permissionForCommand),
  ],
  "meeting-notes": [
    ...[
      "close_current_window",
      "set_current_window_theme",
      "get_ui_preferences",
      "get_job_result",
    ].map(permissionForCommand),
  ],
  "model-editor": [
    "core:event:allow-emit",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "dialog:allow-confirm",
    "dialog:allow-message",
    ...[
      "destroy_current_window",
      "set_current_window_theme",
      "set_current_window_title",
      "get_ui_preferences",
      "list_ai_models",
      "save_ai_model",
      "delete_ai_model",
    ]
      .map(permissionForCommand),
  ],
  "template-editor": [
    "core:event:allow-emit",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "dialog:allow-confirm",
    ...[
      "destroy_current_window",
      "set_current_window_theme",
      "set_current_window_title",
      "get_ui_preferences",
      "list_ai_templates",
      "save_ai_template",
      "delete_ai_template",
    ]
      .map(permissionForCommand),
  ],
  "member-editor": [
    "core:event:allow-emit",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "dialog:allow-confirm",
    ...[
      "destroy_current_window",
      "set_current_window_theme",
      "set_current_window_title",
      "get_ui_preferences",
      "list_meeting_members",
      "save_meeting_member",
      "delete_meeting_member",
    ]
      .map(permissionForCommand),
  ],
  "pet-store-item-detail": [
    ...[
      "set_current_window_theme",
      "get_ui_preferences",
      "get_pet_store_state",
    ].map(permissionForCommand),
  ],
};

const buildCommands = new Set(extractBuildCommands(buildSource));
const handlerCommands = new Set(extractHandlerCommands(handlerSource));
if (buildCommands.size === 0) {
  errors.push("Tauri AppManifest command extraction returned no commands.");
}
if (handlerCommands.size === 0) {
  errors.push("Tauri invoke_handler command extraction returned no commands.");
}
const capabilityByWindow = new Map();
for (const { fileName, data } of capabilities) {
  for (const windowLabel of data.windows ?? []) {
    if (capabilityByWindow.has(windowLabel)) {
      errors.push(`Window ${windowLabel} is assigned by more than one capability.`);
    }
    capabilityByWindow.set(windowLabel, { fileName, data });
  }
}

if (typeof csp !== "string" || csp.trim().length === 0) {
  errors.push("Tauri CSP must be a non-empty string.");
}

if (csp === null) {
  errors.push("Tauri CSP must not be null.");
}

if (capabilities.length < 2) {
  errors.push("Tauri capabilities must be split by window role, not kept as one broad capability.");
}

const capabilitiesWithFsWrite = capabilities.filter(({ data }) =>
  (data.permissions ?? []).includes("fs:allow-write-text-file"),
);

if (capabilitiesWithFsWrite.length !== 1 || capabilitiesWithFsWrite[0]?.fileName !== "main.json") {
  errors.push("Dynamic text export permission must exist only in main.json.");
}

for (const { fileName, data } of capabilities) {
  const permissions = data.permissions ?? [];
  if ((data.webviews ?? []).length > 0) {
    errors.push(`${fileName}: webview-bound capabilities require an explicit security review.`);
  }
  if (data.remote != null || data.local === false) {
    errors.push(`${fileName}: capabilities must remain limited to local application content.`);
  }
  const hasFsPermission = permissions.some(
    (permission) => typeof permission === "string" && permission.startsWith("fs:"),
  );
  const hasFsScope = permissions.some(
    (permission) =>
      typeof permission === "object" &&
      permission !== null &&
      permission.identifier === "fs:scope" &&
      Array.isArray(permission.allow) &&
      permission.allow.length > 0,
  );
  const fsScopes = permissions
    .filter(
      (permission) =>
        typeof permission === "object" &&
        permission !== null &&
        permission.identifier === "fs:scope",
    )
    .flatMap((permission) => permission.allow ?? []);

  if (hasFsPermission && fileName !== "main.json") {
    errors.push(`${fileName}: file-system permissions must stay limited to the main window.`);
  }

  if (permissions.includes("fs:allow-write")) {
    errors.push(`${fileName}: broad fs:allow-write is not allowed.`);
  }

  if (fsScopes.some((scope) => scope.includes("$RESOURCE"))) {
    errors.push(`${fileName}: application resources must never be writable.`);
  }

  if (fileName !== "main.json" && permissions.includes("core:webview:allow-create-webview-window")) {
    errors.push(`${fileName}: child-window creation must stay limited to the main window.`);
  }

  const targetWindowPermissions = [
    "core:window:allow-close",
    "core:window:allow-destroy",
    "core:window:allow-set-theme",
    "core:window:allow-set-title",
  ];
  if (
    fileName !== "main.json"
    && targetWindowPermissions.some((permission) => permissions.includes(permission))
  ) {
    errors.push(`${fileName}: child windows must use caller-bound Rust window commands.`);
  }

  if (fileName !== "main.json" && permissions.includes("core:default")) {
    errors.push(`${fileName}: child windows must not inherit core:default.`);
  }

  if (hasFsScope) {
    errors.push(`${fileName}: static file scopes are forbidden; paths must come from dialog-granted runtime scopes.`);
  }
}

const missingFromBuild = difference(handlerCommands, buildCommands);
const missingFromHandler = difference(buildCommands, handlerCommands);
if (missingFromBuild.length > 0) {
  errors.push(`Tauri AppManifest is missing handler commands: ${missingFromBuild.join(", ")}.`);
}
if (missingFromHandler.length > 0) {
  errors.push(`Tauri AppManifest contains commands absent from invoke_handler: ${missingFromHandler.join(", ")}.`);
}

if (
  !windowScopeSource.includes("pub fn issue_job_window_scope")
  || !windowScopeSource.includes("pub(crate) fn authorize_job_window")
  || !localJobsSource.includes("window_scope::authorize_job_window")
  || !localAiSource.includes("authorize_ai_summary_job")
) {
  errors.push("Standalone task windows must keep Rust-enforced window, token, and job scope binding.");
}

for (const [windowLabel, expectedPermissions] of Object.entries(expectedPermissionsByWindow)) {
  const capability = capabilityByWindow.get(windowLabel);
  if (!capability) {
    errors.push(`Missing capability for window ${windowLabel}.`);
    continue;
  }
  const permissions = capability.data.permissions ?? [];
  const nonStringPermissions = permissions.filter((permission) => typeof permission !== "string");
  if (nonStringPermissions.length > 0) {
    errors.push(`${capability.fileName} contains permissions outside the explicit string allowlist.`);
  }
  const actual = new Set(permissions.filter((permission) => typeof permission === "string"));
  const expected = new Set(expectedPermissions);
  const missing = difference(expected, actual);
  const excessive = difference(actual, expected);
  if (missing.length > 0) {
    errors.push(`${capability.fileName} is missing permissions: ${missing.join(", ")}.`);
  }
  if (excessive.length > 0) {
    errors.push(`${capability.fileName} grants excessive permissions: ${excessive.join(", ")}.`);
  }
  if (actual.size !== permissions.length) {
    errors.push(`${capability.fileName} contains duplicate permissions.`);
  }
}

for (const [windowLabel, capability] of capabilityByWindow) {
  if (!(windowLabel in expectedPermissionsByWindow)) {
    errors.push(`${capability.fileName} grants permissions to unreviewed window ${windowLabel}.`);
  }
}

for (const { fileName, data } of capabilities) {
  const grantedCommands = new Set(
    (data.permissions ?? [])
      .filter((permission) => typeof permission === "string" && permission.startsWith("allow-"))
      .map(commandForPermission),
  );
  const unknownCommands = difference(grantedCommands, handlerCommands);
  if (unknownCommands.length > 0) {
    errors.push(`${fileName} grants commands absent from invoke_handler: ${unknownCommands.join(", ")}.`);
  }
}

for (const [sourcePath, requiredChecks] of Object.entries({
  "apps/desktop/src-tauri/src/local_export.rs": 1,
  "apps/desktop/src-tauri/src/local_members.rs": 1,
  "apps/desktop/src-tauri/src/local_jobs.rs": 1,
  "apps/desktop/src-tauri/src/commands/diagnostics.rs": 1,
})) {
  const source = readFileSync(sourcePath, "utf8");
  const checks = source.match(/fs_scope\(\)\.is_allowed\(/g)?.length ?? 0;
  if (checks < requiredChecks) {
    errors.push(`${sourcePath}: user-selected paths must be checked against the dialog-granted FS scope.`);
  }
}


if (/Command::new\(["']cmd["']\)/.test(systemSource) || systemSource.includes('"/C"')) {
  errors.push("Windows external URLs must not pass through cmd.exe.");
}

if (!systemSource.includes("ShellExecuteW")) {
  errors.push("Windows external URLs must use the native ShellExecuteW API.");
}

for (const platform of runtimeManifest.platforms ?? []) {
  for (const [assetKind, asset] of [
    ["pythonBundle", platform.pythonBundle],
    ["ffmpegBundle", platform.ffmpegBundle],
  ]) {
    if (!asset) continue;
    if (!sha256Pattern.test(asset.sha256 ?? "")) {
      errors.push(`${platform.platformId}.${assetKind}: sha256 must be 64 hexadecimal characters.`);
    }
    for (const [sourceId, url] of Object.entries(asset.urls ?? {})) {
      if (typeof url !== "string" || !url.startsWith("https://")) {
        errors.push(`${platform.platformId}.${assetKind}.${sourceId}: runtime URLs must use HTTPS.`);
      }
      if (/\/latest\/|\/getrelease\//i.test(url)) {
        errors.push(`${platform.platformId}.${assetKind}.${sourceId}: runtime URLs must be immutable.`);
      }
    }
  }
}

for (const requiredGuard of [
  "verify_bundled_asset_sha256(target_path",
  "verify_bundled_asset_sha256(&temp_path",
  "preflight_zip(archive_path)",
  "preflight_tar_gz(archive_path)",
]) {
  if (!archiveSource.includes(requiredGuard)) {
    errors.push(`Runtime archive guard is missing: ${requiredGuard}`);
  }
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Security baseline is configured.");
