import type { SavedConnection } from "../../types";

export function createProfileCopy(
  source: SavedConnection,
  profiles: SavedConnection[],
  baseName: string
): SavedConnection {
  const reserved = new Set(
    profiles
      .filter((profile) => profile.connection_type === source.connection_type)
      .map((profile) => profile.id)
  );
  if (source.connection_type === "ssh" && source.jump_profile_id)
    reserved.add(source.jump_profile_id);
  const normalizedName = baseName.trim();
  let id = normalizedName;
  let suffix = 2;
  while (reserved.has(id)) {
    id = `${normalizedName} (${suffix})`;
    suffix += 1;
  }
  const copy: SavedConnection = {
    id,
    connection_type: source.connection_type,
    group: source.group,
    host: source.host,
    port: source.port,
    encoding: source.encoding,
    terminal_mode: source.terminal_mode,
    memo: source.memo,
    external_control_enabled: source.external_control_enabled,
  };
  if (source.connection_type === "ssh") {
    copy.username = source.username;
    copy.auth_method = source.auth_method;
    copy.private_key_path = source.private_key_path;
    copy.jump_profile_id = source.jump_profile_id;
  }
  return copy;
}
