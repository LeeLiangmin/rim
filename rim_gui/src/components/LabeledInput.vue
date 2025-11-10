<template>
    <div flex="~ col">
        <label :for="id" class="input-label">
            {{ label }}
            <lock-indicator v-if="disabled" :hint="disabledReason"/>
        </label>
        <div class="input-wrapper">
            <input :disabled="disabled" :id="id" :value="modelValue" @input="handleInput" v-bind="$attrs" class="input-field" :class="{
                'bg-disabled-bg': disabled,
                'cursor-not-allowed': disabled,
            }" @mouseenter="showHint = true" @mouseleave="showHint = false" />
            <Transition name="fade">
                <div v-if="showHint && hint" class="tooltip">
                    {{ hint }}
                </div>
            </Transition>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

defineProps<{
    label?: string;
    modelValue: string | null;
    hint?: string;
    disabled?: boolean;
    disabledReason?: string;
}>();

const emit = defineEmits<{
    (e: 'update:modelValue', value: string | number | null): void
}>()

const id = `input-${Math.random().toString(36).slice(2, 11)}`
const showHint = ref(false)

const handleInput = (event: Event) => {
    emit('update:modelValue', (event.target as HTMLInputElement).value)
}
</script>

<style scoped>
.input-label {
    --uno: 'c-regular';
    margin-bottom: 0.75rem;
    font-weight: 500;
    font-size: clamp(0.875rem, 2.4vh, 1.125rem);
    flex-shrink: 0;
    display: flex;
    gap: 0.5rem;
    color: #1d1d1f;
    letter-spacing: -0.01em;
}

.input-wrapper {
    position: relative;
    flex-grow: 1;
    margin-bottom: 16px;
}

.input-field {
    width: 100%;
    background: rgba(255, 255, 255, 0.9);
    border: 1px solid rgba(0, 0, 0, 0.08);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06), 0 1px 3px rgba(0, 0, 0, 0.04);
    font-size: clamp(14px, 2vh, 18px);
    padding: 14px 18px;
    box-sizing: border-box;
    border-radius: 12px;
    color: #1d1d1f;
    font-weight: 400;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    -webkit-font-smoothing: antialiased;
    backdrop-filter: blur(20px);
}

.input-field::placeholder {
    color: rgba(142, 142, 147, 0.6);
}

.input-field:hover:not(:disabled) {
    border-color: rgba(0, 0, 0, 0.12);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08), 0 2px 4px rgba(0, 0, 0, 0.06);
}

.input-field:focus {
    outline: none;
    border-color: rgba(90, 200, 250, 0.5);
    box-shadow: 0 4px 16px rgba(90, 200, 250, 0.15), 0 2px 6px rgba(0, 0, 0, 0.08);
    background: rgba(255, 255, 255, 0.95);
}

.input-field:disabled {
    background: rgba(142, 142, 147, 0.08);
    color: rgba(142, 142, 147, 0.6);
    cursor: not-allowed;
}

.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}
</style>