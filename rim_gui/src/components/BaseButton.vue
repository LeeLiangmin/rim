<script setup lang="ts">
import { computed } from 'vue';

// Define props for the component
const props = defineProps({
  theme: {
    type: String,
    default: 'default', // Default theme
  },
  disabled: {
    type: Boolean,
    default: false, // Button is enabled by default
  },
});

// Computed class for dynamic theme application
const themeClasses = computed(() => {
  switch (props.theme) {
    case 'primary':
      return 'btn-primary';
    case 'secondary':
      return 'btn-secondary';
    // Add more themes as needed
    default:
      return 'btn-default';
  }
});
</script>

<template>
  <button :class="[themeClasses, disabled ? 'button-disabled' : 'button-active']" :disabled="disabled">
    <slot></slot>
  </button>
</template>

<style scoped>
button {
  padding: 8px 20px;
  font-size: clamp(10px, 2.6vh, 20px);
  border-radius: 10px;
  min-width: 100px;
  min-height: 2rem;
  border: none;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
  outline: none;
  -webkit-font-smoothing: antialiased;
}

/* Primary button - Apple blue style */
.btn-primary {
  background: linear-gradient(180deg, #5AC8FA 0%, #4A9EFF 100%);
  color: white;
  box-shadow: 0 1px 3px rgba(90, 200, 250, 0.2), 0 1px 2px rgba(0, 0, 0, 0.06);
}

.btn-primary:hover:not(.button-disabled) {
  background: linear-gradient(180deg, #4A9EFF 0%, #3A8EFF 100%);
  box-shadow: 0 2px 6px rgba(90, 200, 250, 0.25), 0 1px 3px rgba(0, 0, 0, 0.1);
  transform: translateY(-0.5px);
}

.btn-primary:active:not(.button-disabled) {
  background: linear-gradient(180deg, #3A8EFF 0%, #2A7EFF 100%);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  transform: translateY(0);
}

/* Secondary button - Apple gray style */
.btn-secondary {
  background: rgba(174, 174, 178, 0.08);
  color: #3a3a3c;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
}

.btn-secondary:hover:not(.button-disabled) {
  background: rgba(174, 174, 178, 0.15);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.08);
  transform: translateY(-0.5px);
}

.btn-secondary:active:not(.button-disabled) {
  background: rgba(174, 174, 178, 0.2);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  transform: translateY(0);
}

/* Default button */
.btn-default {
  background: rgba(174, 174, 178, 0.08);
  color: #3a3a3c;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
}

.btn-default:hover:not(.button-disabled) {
  background: rgba(174, 174, 178, 0.15);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.08);
}

.button-active {
  cursor: pointer;
}

.button-disabled {
  cursor: not-allowed;
  opacity: 0.5;
  transform: none !important;
}
</style>
