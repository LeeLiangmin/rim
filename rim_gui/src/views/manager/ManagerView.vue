<script setup lang="ts">
import { managerConf, ManagerOperation } from '@/utils';
import { computed, inject, onBeforeUnmount, onMounted, ref, type Ref } from 'vue';
import { event } from '@tauri-apps/api';
import { useCustomRouter } from '@/router';
import { CliPayload, UpdatePayload } from '@/utils/types/payloads';
import { useToolkitInstallation } from '@/composables/useToolkitInstallation';
import ToolkitItem from './ToolkitItem.vue';

const { routerPush } = useCustomRouter();
const { install, getInstallError, isInstallingToolkit } = useToolkitInstallation();

const kitsLoadError = inject<Ref<string | undefined>>('kitsLoadError', ref(undefined));
const isLoadingKits = inject<Ref<boolean>>('isLoadingKits', ref(false));
const retryLoadKits = inject<() => Promise<void>>('retryLoadKits', async () => {});

const installedKit = computed(() => managerConf.getInstalled());
const availableKits = computed(() => managerConf.getKits());
const latestToolkitUrl = ref('');

const displayFormat = ref<'list' | 'card'>('list');

function uninstall() {
  managerConf.setOperation(ManagerOperation.UninstallToolkit);
  routerPush('/manager/uninstall');
}

let unlistenChangeView: (() => void) | null = null;
let unlistenUpdateAvailable: (() => void) | null = null;

onMounted(async () => {
  unlistenChangeView = await event.listen('change-view', (event) => {
    let payload = event.payload as CliPayload;
    if (payload.command === 'Uninstall') {
      managerConf.setOperation(ManagerOperation.UninstallToolkit);
    }
    routerPush(payload.path);
  });

  unlistenUpdateAvailable = await event.listen('toolkit:update-available', (event) => {
    let payload = event.payload as UpdatePayload[];
    let maybeUrl = payload[1].data;
    if (maybeUrl) {
      latestToolkitUrl.value = maybeUrl;
    }
  });
});

onBeforeUnmount(() => {
  unlistenChangeView?.();
  unlistenUpdateAvailable?.();
});
</script>

<template>
  <div>
    <section>
      <div class="info-label" mb="1rem">{{ $t('current_toolkit') }}</div>
      <base-card v-if="installedKit" flex="~ justify-between items-center" ml="1rem" mr="1.2rem">
        <div flex="~ col">
          <span class="toolkit-name">{{ installedKit?.name }}</span>
          <span class="toolkit-version">{{ installedKit?.version }}</span>
          <span>{{ installedKit?.desc }}</span>
        </div>
        <div flex="~ justify-end" w="25%">
          <base-button w="45%" theme="secondary" @click="uninstall">{{ $t('uninstall') }}</base-button>
        </div>
      </base-card>
      <base-card v-else text="center" ml="1rem" mr="1.2rem">
        <p text="regular">{{ $t('no_toolkit_installed') }}</p>
      </base-card>
    </section>

    <section h="55%">
      <div class="info-label" mt="3vh" mb="1.5vh" flex="~ justify-between">
        {{ $t('available_toolkit') }}
        <!-- display format selector -->
        <div class="format-icons" mt="5px">
          <label :class="{ selected: displayFormat === 'card' }">
            <input type="radio" value="card" v-model="displayFormat" />
            <svg viewBox="0 0 24 24" class="icon">
              <rect x="0" y="0" width="6" height="6" />
              <rect x="9" y="0" width="6" height="6" />
              <rect x="0" y="9" width="6" height="6" />
              <rect x="9" y="9" width="6" height="6" />
            </svg>
          </label>
          <label :class="{ selected: displayFormat === 'list' }">
            <input type="radio" value="list" v-model="displayFormat" />
            <svg viewBox="0 0 24 24" class="icon">
              <rect x="0" y="0" width="15" height="3" />
              <rect x="0" y="6" width="15" height="3" />
              <rect x="0" y="12" width="15" height="3" />
            </svg>
          </label>
        </div>
      </div>

      <div :class="['toolkit-list', displayFormat]">
        <!-- Error state -->
        <base-card v-if="kitsLoadError" class="error-card" ml="1rem" mr="1.2rem" flex="~ col" gap="1rem">
          <div flex="~ col" gap="0.5rem">
            <span class="error-title">{{ $t('failed_to_load_toolkits') }}</span>
            <span class="error-message" c-regular>{{ kitsLoadError }}</span>
            <span class="error-hint" c-regular>{{ $t('failed_to_load_toolkits_hint') }}</span>
          </div>
          <div flex="~ gap-1rem" items-center>
            <base-button theme="primary" @click="retryLoadKits" :disabled="isLoadingKits">
              {{ isLoadingKits ? $t('loading') : $t('retry') }}
            </base-button>
            <spinner v-if="isLoadingKits" size="18px" color="blue" />
          </div>
        </base-card>

        <!-- Loading state -->
        <base-card v-else-if="isLoadingKits && availableKits.length === 0" class="loading-card" ml="1rem" mr="1.2rem" flex="~ col" gap="1rem">
          <base-progress w="full" kind="spinner" />
          <p text="regular">{{ $t('loading_toolkits') }}</p>
          <p text="regular" class="loading-hint">{{ $t('loading_toolkits_network_hint') }}</p>
        </base-card>

        <!-- Empty state -->
        <base-card v-else-if="!isLoadingKits && availableKits.length === 0" class="empty-card" ml="1rem" mr="1.2rem" text="center">
          <p text="regular">{{ $t('no_available_toolkits') }}</p>
        </base-card>

        <!-- Toolkit items -->
        <ToolkitItem
          v-for="toolkit in availableKits"
          :key="toolkit.manifestURL"
          :name="toolkit.name"
          :version="toolkit.version"
          :desc="toolkit.desc"
          :is-latest="toolkit.manifestURL === latestToolkitUrl"
          :display-format="displayFormat"
          :installing="isInstallingToolkit(toolkit.manifestURL)"
          :error="getInstallError(toolkit.manifestURL)"
          @install="install(toolkit.manifestURL)"
        />
      </div>
    </section>
  </div>
</template>

<style lang="css" scoped>
.toolkit-name {
  --uno: 'c-regular';
  font-weight: bold;
  font-size: clamp(20px, 2.6vh, 35px);
}

.toolkit-version {
  --uno: 'c-regular';
  font-weight: 600;
  margin-top: 1rem;
  font-size: 2.2vh;
}

.format-icons {
  display: flex;
  gap: 0.5rem;
}

.format-icons label {
  cursor: pointer;
  opacity: 0.3;
  transition: opacity 0.2s;
  display: flex;
  align-items: center;
}

.format-icons label.selected,
.format-icons label:hover {
  opacity: 1;
}

.format-icons input {
  display: none;
}

.icon {
  width: 24px;
  height: 24px;
  fill: #666;
}

.toolkit-list {
  height: 100%;
  padding: 1rem;
  overflow-y: auto;
  box-sizing: border-box;
  scrollbar-gutter: stable;
}

.toolkit-list.list :deep(.toolkit-item) {
  display: flex;
  justify-content: space-between;
  margin-bottom: 3vh;
  align-items: center;
}

.toolkit-list.list :deep(.toolkit-item .toolkit-name) {
  display: flex;
  justify-content: left;
}

.toolkit-list.card {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 2rem;
}

.toolkit-list.card :deep(.toolkit-item) {
  padding: 5%;
  max-height: 180px;
  display: flex;
  flex-direction: column;
  text-align: center;
  justify-content: center;
}

.toolkit-list.card :deep(.toolkit-item .toolkit-name) {
  display: flex;
  justify-content: center;
}

.toolkit-list.card :deep(.toolkit-item *) {
  cursor: pointer;
}

.error-card {
  padding: 2rem;
  border: 2px solid rgba(255, 59, 48, 0.3);
  background: rgba(255, 59, 48, 0.05);
}

.error-title {
  font-weight: 600;
  font-size: clamp(16px, 2.2vh, 24px);
  color: #ff3b30;
}

.error-message {
  font-size: clamp(12px, 1.8vh, 16px);
  color: rgba(0, 0, 0, 0.6);
  word-break: break-word;
}

.loading-card,
.empty-card {
  padding: 2rem;
  text-align: center;
}

.loading-hint {
  font-size: clamp(11px, 1.6vh, 14px);
  opacity: 0.75;
}

.error-hint {
  font-size: clamp(11px, 1.6vh, 14px);
  opacity: 0.85;
}
</style>
