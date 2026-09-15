import type { Encoding, SavedConnection, SshAuthMethod, TerminalMode } from "../../types";
import type { PreparationContext } from "./connectionAttemptController";
import {
  normalizeSshAuthMethod,
  resolveSshAuthentication,
  usesPrivateKeyAuthentication,
} from "./connectionProfileUtils";

export type ConnectionInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export interface SshAttemptInput {
  host: string;
  port: string;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  jumpProfileId: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  defaultPrivateKeyPath: string;
}

export async function prepareSshConnection(
  ssh: SshAttemptInput,
  profiles: SavedConnection[],
  context: PreparationContext,
  invoke: ConnectionInvoke
) {
  const target = resolveSshAuthentication(
    ssh.authMethod,
    ssh.privateKeyPath,
    ssh.defaultPrivateKeyPath
  );
  const jump = profiles.find((profile) => profile.id === ssh.jumpProfileId);
  let jumpCredential = "";
  let targetCredential = "";
  const promptIfNeeded = async (
    phase: "jump" | "target",
    host: string,
    port: number,
    username: string,
    authMethod: SshAuthMethod,
    privateKeyPath: string
  ) => {
    if (!usesPrivateKeyAuthentication(authMethod) || (authMethod === "auto" && !privateKeyPath))
      return "";
    let requiresPassphrase: boolean;
    try {
      requiresPassphrase = await invoke<boolean>("ssh_private_key_requires_passphrase", {
        privateKeyPath,
      });
    } catch (error) {
      if (authMethod !== "auto") throw error;
      requiresPassphrase = false;
    }
    context.assertActive();
    return requiresPassphrase
      ? context.credential({ phase, host, port, username, authMethod, privateKeyPath })
      : "";
  };
  try {
    if (ssh.jumpProfileId && !jump)
      throw {
        code: "ssh.jump_profile_not_found",
        message: "SSH jump profile is unavailable.",
        params: { profile: ssh.jumpProfileId },
      };
    if (jump) {
      if (!jump.host)
        throw {
          code: "ssh.jump_profile_host_missing",
          message: "SSH jump profile host is missing.",
        };
      if (!jump.username)
        throw {
          code: "ssh.jump_profile_username_missing",
          message: "SSH jump profile username is missing.",
        };
      const authMethod = normalizeSshAuthMethod(jump.auth_method);
      const auth = resolveSshAuthentication(
        authMethod,
        jump.private_key_path ?? "",
        ssh.defaultPrivateKeyPath
      );
      jumpCredential = await promptIfNeeded(
        "jump",
        jump.host,
        jump.port ?? 22,
        jump.username,
        authMethod,
        auth.privateKeyPath
      );
      context.assertActive();
    }
    targetCredential = await promptIfNeeded(
      "target",
      ssh.host,
      Number(ssh.port),
      ssh.username,
      ssh.authMethod,
      target.privateKeyPath
    );
    context.assertActive();
  } catch (error) {
    jumpCredential = "";
    targetCredential = "";
    throw error;
  }
  return {
    authMethod: target.authMethod,
    clearCredentials: () => {
      jumpCredential = "";
      targetCredential = "";
    },
    connect: async (requestId: string) => {
      const jumpAuthMethod = normalizeSshAuthMethod(jump?.auth_method);
      try {
        const pending = invoke<{ session_id: string }>("ssh_connect", {
          options: {
            host: ssh.host,
            port: Number(ssh.port),
            username: ssh.username,
            password: target.authMethod === "password" ? targetCredential : "",
            authMethod: target.authMethod,
            privateKeyPath: target.privateKeyPath || null,
            keyPassphrase: usesPrivateKeyAuthentication(target.authMethod) ? targetCredential : "",
            jumpProfileId: ssh.jumpProfileId || null,
            jumpPassword: jumpAuthMethod === "password" ? jumpCredential : "",
            jumpKeyPassphrase: usesPrivateKeyAuthentication(jumpAuthMethod) ? jumpCredential : "",
            cols: 120,
            rows: 30,
            encoding: ssh.encoding,
            requestId,
          },
        });
        jumpCredential = "";
        targetCredential = "";
        return (await pending).session_id;
      } finally {
        jumpCredential = "";
        targetCredential = "";
      }
    },
  };
}
