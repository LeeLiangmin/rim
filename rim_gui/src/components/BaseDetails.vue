<script setup lang="ts">
import { ref, watch, onMounted, nextTick } from 'vue';

const props = defineProps({
    title: {
        type: String,
        default: 'Advanced options'
    },
    open: {
        type: Boolean,
        default: false
    }
});

const emit = defineEmits(['toggle']);

const isOpen = ref(props.open);
const contentHeight = ref(0);
const contentRef = ref<HTMLElement | null>(null);

const toggle = () => {
    isOpen.value = !isOpen.value;
    emit('toggle', isOpen.value);
};

// Watch for prop changes
watch(() => props.open, (newValue) => {
    isOpen.value = newValue;
});

// Calculate content height when opened
watch(isOpen, (newValue) => {
    if (newValue && contentRef.value) {
        // When opening, calculate the height after DOM update
        requestAnimationFrame(() => {
            if (contentRef.value) {
                contentHeight.value = contentRef.value.scrollHeight;
            }
        });
    } else {
        // When closing, ensure we have the current height for smooth transition
        if (contentRef.value && contentHeight.value === 0) {
            contentHeight.value = contentRef.value.scrollHeight;
        }
    }
});

// Initialize height if component starts open
onMounted(() => {
    if (props.open && contentRef.value) {
        nextTick(() => {
            requestAnimationFrame(() => {
                if (contentRef.value) {
                    contentHeight.value = contentRef.value.scrollHeight;
                }
            });
        });
    }
});
</script>

<template>
    <div class="details-container">
        <button class="summary" @click="toggle" :aria-expanded="isOpen">
            <span class="title">{{ title }}</span>
            <span class="icon" :class="{ open: isOpen }">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                    <path d="M7 10l5 5 5-5z" />
                </svg>
            </span>
        </button>

        <div class="content" :class="{ open: isOpen }" ref="contentRef"
            :style="{ '--content-height': contentHeight + 'px' }">
            <slot></slot>
        </div>
    </div>
</template>

<style scoped>
.details-container {
    transition: all 0.3s ease;
}

.summary {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0);
    border: none;
    cursor: pointer;
}

.title {
    --uno: "c-regular";
    font-size: clamp(8px, 2.6vh, 22px);
    font-weight: bold;
}

.icon {
    transition: transform 0.3s ease;
    width: 4vw;
    height: 4vh;
}

.icon svg {
    width: 100%;
    height: 100%;
    fill: #64748b;
}

.icon.open {
    transform: rotate(180deg);
}

.content {
    max-height: 0;
    overflow: hidden;
    opacity: 0;
    transition: 
        max-height 0.35s cubic-bezier(0.4, 0, 0.2, 1),
        opacity 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.content.open {
    max-height: var(--content-height, 2000px);
    opacity: 1;
}
</style>
