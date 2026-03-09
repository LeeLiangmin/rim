import { installConf } from "./installConf";
import { AppInfo } from "./types/AppInfo";
import { RestrictedComponent } from "./types/Component";
import { invoke } from '@tauri-apps/api';
import { message } from '@tauri-apps/api/dialog';

type EnforceableOption = [string, boolean];

export interface BaseConfig {
  path: string;
  addToPath: boolean,
  insecure: boolean,
  rustupDistServer?: EnforceableOption,
  rustupUpdateRoot?: EnforceableOption,
  cargoRegistryName?: EnforceableOption,
  cargoRegistryValue?: EnforceableOption,
}

export const defaultBaseConfig: BaseConfig = {
  path: '',
  addToPath: false,
  insecure: false,
};

function normalizeInvokeError(error: unknown): string {
  if (typeof error === 'string' && error.trim()) {
    return error;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  try {
    const asString = String(error);
    if (asString && asString !== '[object Object]') {
      return asString;
    }
  } catch (_) {
    // ignore stringify failure
  }
  return '发生了一个未知错误';
}

// 使用 message invoke 显示错误信息
export async function invokeCommand(
  command: string,
  args = {},
  options?: { silent?: boolean; title?: string },
) {
  try {
    return await invoke(command, args);
  } catch (error: unknown) {
    if (options?.silent) {
      throw error;
    }

    await message(normalizeInvokeError(error), {
      title: options?.title || '错误',
      type: 'error',
    });
    throw error;
  }
}

/**
 * Handle the restricted components before installation,
 * as some components might need another package source.
 * 
 * @param onDefault The default callback where there aren't any restricted components.
 * @param onRestricted Callback when restricted components detected in `installConf`.
 */
export function handleRestrictedComponents(onDefault: () => void, onRestricted: () => void) {
  invokeCommand('get_restricted_components', { components: installConf.getCheckedComponents() }).then((res) => {
    const restricted = res as RestrictedComponent[];
    if (restricted.length > 0) {
      installConf.setRestrictedComponents(restricted);
      onRestricted();
    } else {
      onDefault();
    }
  });
}

/** The name and version of this application. */
export async function getAppNameWithVersion(): Promise<[string, string]> {
  const shortenVersion = (ver: string) => {
    return ver.split(' ')[0];
  };
  const info = await invokeCommand('app_info') as AppInfo;
  return [info.name, shortenVersion(info.version)];
}
