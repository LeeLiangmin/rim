<template>
    <transition name="panel">
        <div v-if="props.show" class="panel-backdrop" @click.self="hide">
            <div class="panel-content" :style="{
                width: width,
                height: height,
            }">
                <div class="panel-close-btn" @click="emit('close')" title="close">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" viewBox="0 0 16 16">
                        <path
                            fill-rule="evenodd"
                            d="M4.28 3.22a.75.75 0 0 0-1.06 1.06L6.94 8l-3.72 3.72a.75.75 0 1 0 1.06 1.06L8 9.06l3.72 3.72a.75.75 0 1 0 1.06-1.06L9.06 8l3.72-3.72a.75.75 0 0 0-1.06-1.06L8 6.94z"
                            clip-rule="evenodd"
                        />
                    </svg>
                </div>
                <slot></slot>
            </div>
            <div v-if="clickToHide" class="panel-close-hint">{{ $t('close_panel_hint') }}</div>
        </div>
    </transition>
</template>

<script setup lang="ts">
const props = defineProps({
    show: {
        type: Boolean,
        default: true,
    },
    width: {
        type: String,
        default: 'auto'
    },
    height: {
        type: String,
        default: 'auto'
    },
    clickToHide: {
        type: Boolean,
        default: true,
    }
});

const emit = defineEmits(['close']);

function hide() {
    if (props.clickToHide) {
        emit('close');
    }
}
</script>

<style scoped>
.panel-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    backdrop-filter: blur(25px);
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    z-index: 999;
}

.panel-content {
    background: rgba(255, 255, 255, 0.85);
    margin-top: 6%;
    border-radius: 20px;
    box-shadow: 0 16px 32px rgba(0, 0, 0, .12);
    max-width: 90%;
    max-height: 75%;
    overflow: auto;
    padding: 2%;
    position: relative;
}

.panel-close-btn {
    position: absolute;
    top: 12px;
    right: 12px;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    cursor: pointer;
    fill: rgb(120, 120, 120);
}

.panel-close-btn:hover {
    background: rgba(0, 0, 0, 0.2);
    fill: white;
}

.panel-close-hint {
    margin-top: 2px;
    color: rgba(0, 0, 0, 0.3);
}

/* Enter/leave animations */
.panel-enter-active .panel-content,
.panel-leave-active .panel-content {
    transition: all 0.3s ease;
}

.panel-enter-from .panel-content,
.panel-leave-to .panel-content {
    transform: scale(0.7);
    opacity: 0;
}

.panel-enter-active,
.panel-leave-active {
    transition: opacity 0.3s ease;
}
</style>