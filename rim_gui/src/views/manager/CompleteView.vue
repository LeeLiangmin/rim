<script setup lang="ts">
import { useCustomRouter } from '@/router';
import { invokeCommand, managerConf, ManagerOperation } from '@/utils';
import { computed } from 'vue';

const { routerPush } = useCustomRouter();
const isUninstallManger = computed(() => managerConf.getOperation() === ManagerOperation.UninstallAll);

function closeOrReturn() {
  if (isUninstallManger.value) {
    invokeCommand('close_window', { code: 0 });
  } else {
    // FIXME: refresh `kit` list, since the `installed` kit should no longer exist after uninstallation.
    routerPush('/manager');
  }
}
</script>

<template>
  <section flex="~ col">
    <h4 ml="12px">{{ $t('uninstall_finished') }}</h4>
    <div flex="1" p="12px">
      <p>{{ $t('uninstall_product_removed') }}</p>
    </div>
    <div basis="60px" flex="~ justify-end items-center">
      <base-button theme="primary" mr="12px" @click="closeOrReturn"
        >{{ isUninstallManger ? $t('close') : $t('back') }}</base-button
      >
    </div>
  </section>
</template>
