<template>
    <div class="input-with-button">
        <input
            :value="modelValue"
            type="text"
            class="fused-input"
            :placeholder="placeholder"
            @input="handleInput"
            @change="handleChange"
            @keydown.enter="handleEnter"
        />
        <base-button theme="primary" class="fused-button" @click="emit('button-click')">
            {{ buttonLabel }}
        </base-button>
    </div>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: string | null;
  placeholder?: string;
  buttonLabel?: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
  (e: 'change', event: Event): void;
  (e: 'keydown.enter', event: Event): void;
  (e: 'button-click'): void;
}>();

const handleInput = (event: Event) => {
  const value = (event.target as HTMLInputElement).value;
  emit('update:modelValue', value);
};

const handleChange = (event: Event) => {
  emit('change', event);
};

const handleEnter = (event: Event) => {
  emit('keydown.enter', event);
};
</script>

<style scoped>
.input-with-button {
    display: flex;
    align-items: stretch;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.05);
    border-radius: 12px;
    overflow: hidden;
    border: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(255, 255, 255, 0.9);
    backdrop-filter: blur(20px);
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    min-height: 48px;
}

.input-with-button:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12), 0 2px 4px rgba(0, 0, 0, 0.08);
    border-color: rgba(90, 200, 250, 0.3);
}

.input-with-button:focus-within {
    box-shadow: 0 4px 16px rgba(90, 200, 250, 0.2), 0 2px 6px rgba(0, 0, 0, 0.1);
    border-color: rgba(90, 200, 250, 0.5);
}

.fused-input {
    flex-grow: 1;
    padding: 12px 20px;
    outline: none;
    border: none;
    background: transparent;
    font-size: clamp(14px, 2vh, 17px);
    color: #1d1d1f;
    font-weight: 400;
    -webkit-font-smoothing: antialiased;
    min-height: 100%;
    box-sizing: border-box;
}

.fused-input::placeholder {
    color: rgba(142, 142, 147, 0.6);
}

.fused-input:focus {
    background: transparent;
}

.fused-button {
    padding: 0 24px;
    border-radius: 0;
    border-left: 1px solid rgba(0, 0, 0, 0.06);
    min-width: 120px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
}
</style>
