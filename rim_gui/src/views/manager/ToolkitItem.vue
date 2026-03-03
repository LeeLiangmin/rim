<script setup lang="ts">
defineProps<{
  name: string;
  version: string;
  desc: string;
  isLatest: boolean;
  displayFormat: 'list' | 'card';
  installing: boolean;
  error?: string;
}>();

const emit = defineEmits<{
  install: [];
}>();
</script>

<template>
  <base-card
    class="toolkit-item"
    :interactive="displayFormat === 'card' && !error"
    @click="displayFormat === 'card' && !error ? emit('install') : undefined"
  >
    <div flex="~ col">
      <span class="toolkit-name">
        {{ name }}
        <div class="latest-indicator" v-if="isLatest">New</div>
      </span>
      <span class="toolkit-version">{{ version }}</span>
      <span mt="1rem" c-regular>{{ desc }}</span>

      <!-- Error message -->
      <div v-if="error" class="install-error" mt="1rem" flex="~ col" gap="0.5rem">
        <span class="error-text" c-regular>{{ error }}</span>
        <base-button
          v-if="displayFormat === 'card'"
          theme="primary"
          :disabled="installing"
          @click.stop="emit('install')"
          mt="0.5rem"
        >
          {{ installing ? $t('retrying') : $t('retry') }}
        </base-button>
      </div>
    </div>

    <div class="button-container" v-if="displayFormat === 'list'">
      <base-button
        v-if="!error"
        class="button"
        theme="primary"
        :disabled="installing"
        @click="emit('install')"
      >
        {{ installing ? $t('installing') : $t('install') }}
      </base-button>
      <div v-else flex="~ col" gap="0.5rem">
        <base-button
          class="button"
          theme="primary"
          :disabled="installing"
          @click="emit('install')"
        >
          {{ installing ? $t('retrying') : $t('retry') }}
        </base-button>
      </div>
    </div>
  </base-card>
</template>

<style lang="css" scoped>
.toolkit-name {
  --uno: 'c-regular';
  font-weight: bold;
  font-size: clamp(20px, 2.6vh, 35px);
}

.latest-indicator {
  background-color: red;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.6), 0 12px 16px rgba(0, 0, 0, 0.12);
  border-radius: 20vh;
  color: white;
  text-align: center;
  width: 6vw;
  font-size: 2.3vh;
  margin-left: 1rem;
}

.toolkit-version {
  --uno: 'c-regular';
  font-weight: 600;
  margin-top: 1rem;
  font-size: 2.2vh;
}

.button-container {
  width: 25%;
  text-align: end;
}

.button-container .button {
  width: 45%;
}

.install-error {
  padding: 0.75rem;
  background: rgba(255, 59, 48, 0.05);
  border: 1px solid rgba(255, 59, 48, 0.2);
  border-radius: 4px;
}

.error-text {
  font-size: clamp(11px, 1.5vh, 14px);
  color: #ff3b30;
  word-break: break-word;
  line-height: 1.4;
}
</style>
