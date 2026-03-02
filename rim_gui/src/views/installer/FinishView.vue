<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { installConf, invokeCommand } from '@/utils/index';
import Spinner from '@/components/Spinner.vue';

const runApp = ref(true);
const createShortcut = ref(true);
const isLoading = ref(false);
const isLinux = ref(false);

onMounted(async () => {
  isLinux.value = (await invokeCommand('is_linux')) as boolean;
  if (isLinux.value) {
    runApp.value = false;
    createShortcut.value = false;
  }
});

async function closeWindow() {
  isLoading.value = true;
  
  try {
    await invokeCommand('post_installation_opts', {
      installDir: installConf.config.value.path,
      open: runApp.value,
      shortcut: createShortcut.value
    });
  } finally {
    isLoading.value = false;
  }

  await invokeCommand('close_window');
}
</script>

<template>
  <div flex="~ col items-center">
    <base-card class="info-card">
      <div flex="~ col items-center" h="full" :class="{ 'linux-layout': isLinux }">
        <div text="center" class="finish-info">
          <div c="darker-secondary" font="bold" text="4vh">{{ $t('install_finish_info') }}</div>
          <div v-if="!isLinux" c="secondary" text="3vh">{{ $t('post_installation_hint') }}</div>
        </div>
        <div v-if="!isLinux" flex="~ col" gap="4vh" :class="{ 'loading-dimmed': isLoading }">
          <base-check-box v-model="runApp" :title="$t('post_installation_open')" @titleClick="runApp = !runApp" />
          <base-check-box v-model="createShortcut" :title="$t('post_installation_create_shortcut')" @titleClick="createShortcut = !createShortcut" />
        </div>
        <base-button theme="primary" w="20vw" :class="['finish-btn-anchor', { 'finish-btn': !isLinux, 'finish-btn-linux': isLinux }]" @click="closeWindow()" :disabled="isLoading">
          <div flex="~ items-center justify-center" gap="2" class="btn-content">
            <Spinner v-if="isLoading" size="16px" color="white" />
            <span>{{ isLoading ? $t('post_installation_processing') : $t('finish') }}</span>
          </div>
        </base-button>
      </div>
    </base-card>
  </div>
</template>

<style lang="css" scoped>
.finish-info {
  margin-top: 5vh;
  margin-bottom: 10vh;
  display: flex;
  flex-direction: column;
  gap: 3vh;
}

.info-card {
  position: absolute;
  left: 10%;
  right: 10%;
  top: 10%;
  bottom: 10%;
}

.finish-btn-anchor {
  min-width: 160px;
}

.finish-btn {
  position: fixed;
  bottom: 5vh;
}

.finish-btn-linux {
  margin-top: auto;
  margin-bottom: 5vh;
}

.linux-layout {
  justify-content: center;
}

.loading-dimmed {
  opacity: 0.6;
  pointer-events: none;
  transition: opacity 0.2s ease;
}
</style>
