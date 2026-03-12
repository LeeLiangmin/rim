<template>
    <div :class="['nav-buttons-container', { 'manager-mode': isManagerMode }]">
      <div v-if="backLabel" :class="['button-wrapper', { 'manager-mode': isManagerMode }]" @click="backClicked">
        <base-button theme="secondary" @click.stop="backClicked">{{ backLabel }}</base-button>
      </div>
      <div class="spacer"></div>
      <div v-if="nextLabel" :class="['button-wrapper', { 'manager-mode': isManagerMode }]" @click="nextClicked">
        <base-button theme="primary" @click.stop="nextClicked">{{ nextLabel }}</base-button>
      </div>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';

defineProps<{
    backLabel?: string,
    nextLabel?: string,
}>();

const emit = defineEmits<{
  (e: 'back-clicked'): void;
  (e: 'next-clicked'): void;
}>();

const route = useRoute();
// Only apply enhanced click area in manager mode
const isManagerMode = computed(() => route.path.startsWith('/manager'));

const backClicked = () => emit('back-clicked');
const nextClicked = () => emit('next-clicked');
</script>

<style scoped>
.nav-buttons-container {
  position: fixed;
  bottom: 7%;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  /* Ensure container doesn't block clicks on elements beneath it */
  pointer-events: none;
  z-index: 10;
}

.spacer {
  flex: 1;
}

.button-wrapper {
  /* Re-enable pointer events for button area */
  pointer-events: auto;
  display: flex;
  align-items: center;
}

/* Left button offset */
.button-wrapper:first-child {
  margin-left: 8%;
}

/* Right button offset */
.button-wrapper:last-child {
  margin-right: 8%;
}

/* Enhanced click area only for manager mode */
.button-wrapper.manager-mode {
  padding: 8px 6px;
  cursor: pointer;
}

/* Installer mode: no extra padding, keep original behavior */
.button-wrapper:not(.manager-mode) {
  padding: 0;
}
</style>
