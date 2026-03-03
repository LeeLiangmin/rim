import { ref } from 'vue';
import { invokeCommand, KitItem, managerConf, ManagerOperation } from '@/utils';
import { useCustomRouter } from '@/router';

export function useToolkitInstallation() {
  const { routerPush } = useCustomRouter();

  const installErrors = ref<Map<string, string>>(new Map());
  const isInstalling = ref<Map<string, boolean>>(new Map());

  async function install(url: string) {
    const isRetry = installErrors.value.has(url);

    installErrors.value.delete(url);
    isInstalling.value.set(url, true);

    try {
      const toolkit = (await invokeCommand(
        'get_toolkit_from_url',
        { url: url, force_refresh: isRetry },
        { silent: true },
      )) as KitItem;
      await managerConf.setCurrent(toolkit);
      managerConf.setOperation(ManagerOperation.Update);
      routerPush('/manager/change');
    } catch (error: unknown) {
      const errorMessage =
        error instanceof Error ? error.message : String(error) || 'Unknown error occurred';
      installErrors.value.set(url, errorMessage);
    } finally {
      isInstalling.value.delete(url);
    }
  }

  function getInstallError(url: string): string | undefined {
    return installErrors.value.get(url);
  }

  function isInstallingToolkit(url: string): boolean {
    return isInstalling.value.get(url) || false;
  }

  return {
    install,
    getInstallError,
    isInstallingToolkit,
  };
}
