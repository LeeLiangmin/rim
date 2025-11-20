<template>
    <div flex="~ items-center" position="fixed" bottom="7%" :class="['nav-buttons-container', { 'manager-mode': isManagerMode }]">
      <div v-if="backLabel" :class="['button-wrapper', 'cancel-button-wrapper', { 'manager-mode': isManagerMode }]" style="left: 0; transform: translateX(30%);" @click="backClicked">
        <base-button theme="secondary" @click.stop="backClicked">{{ backLabel }}</base-button>
      </div>
      <div v-if="nextLabel" :class="['button-wrapper', { 'manager-mode': isManagerMode }]" style="right: 0; transform: translateX(-30%);" @click="nextClicked">
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
  /* Ensure container doesn't block clicks */
  pointer-events: none;
}

.button-wrapper {
  position: fixed;
  bottom: 7%;
  /* Re-enable pointer events for button wrapper */
  pointer-events: auto;
  /* Ensure wrapper doesn't interfere with layout */
  z-index: 10;
  /* Ensure wrapper captures clicks */
  display: flex;
  align-items: center;
}

/* Enhanced click area only for manager mode */
.button-wrapper.manager-mode {
  /* Expand clickable area for better sensitivity - add padding around button */
  padding: 8px 6px; /* Increased top/bottom padding (8px) extends clickable area significantly */
  /* Make entire wrapper clickable for better sensitivity */
  cursor: pointer;
}
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        
/* Align cancel button with uninstall button in manager mode */
.cancel-button-wrapper.manager-mode {
  /* Adjust cancel button position downward to align with uninstall button */
  bottom: calc(7% - 8px); /* Compensate for padding to align button centers */
}

/* Installer mode: no extra padding, keep original behavior */
.button-wrapper:not(.manager-mode) {
  padding: 0;
}
</style>
