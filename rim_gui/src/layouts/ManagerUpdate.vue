<template>
    <div text="center" v-if="!isUpdating">
        <h2 class="title">{{ $t('self_update_available') }}</h2>

        <span>{{ $t('ask_self_update') }}</span>
        <p mb="2rem">
            {{ $t('current') }}:
            <base-tag min-w="5rem">{{ curVer }}</base-tag>
            ➜
            {{ $t('latest') }}:
            <base-tag min-w="5rem" type="success">{{ newVer }}</base-tag>
        </p>

        <p v-if="updateError" style="color: #d03050; margin-bottom: 1rem; word-break: break-word;">
            {{ updateError }}
        </p>

        <div flex="~ justify-between" style="gap: 2rem;">
            <base-button flex="1" theme="secondary" @click="onClose">{{ $t('cancel') }}</base-button>
            <base-button flex="1" theme="primary" @click="onUpdate">
                {{ updateError ? $t('retry') : $t('update') }}
            </base-button>
        </div>
    </div>
    <div v-else flex="~ col justify-center items-center">
        <span class="info-label">{{ mainProgressPayload?.message }}</span>
        <base-progress
            w="full"
            h="4vh"
            :value="mainProgress"
            :kind="mainProgressPayload?.style?.toString() || 'len'"
            :length="mainProgressPayload?.length"
            :transition="false"
        />

        <span class="sub-info-label">{{ subProgressPayload?.message }}</span>
        <base-progress
            mt="0.5rem"
            w="full"
            h="4vh"
            :value="subProgress"
            :kind="subProgressPayload?.style?.toString() || 'len'"
            :length="subProgressPayload?.length"
            :transition="false"
        />
    </div>
</template>

<script setup lang="ts">
import { invokeCommand } from "@/utils";
import { onBeforeUnmount, onMounted, ref } from "vue";
import { event, UnlistenFn } from "@tauri-apps/api";
import { ProgressPayload } from "@/utils/types/payloads";

const isUpdating = ref(false);
const updateError = ref<string | null>(null);

const mainProgressPayload = ref<ProgressPayload | null>(null);
const mainProgress = ref(0);
const subProgress = ref(0);
const subProgressPayload = ref<ProgressPayload | null>(null);

const unlistenFns: UnlistenFn[] = [];

defineProps<{
    curVer: string;
    newVer: string;
}>();

const emit = defineEmits(['close']);

function onClose() {
    emit('close');
}

function toErrorMessage(error: unknown): string {
    if (typeof error === 'string' && error.trim()) {
        return error;
    }
    if (error instanceof Error && error.message.trim()) {
        return error.message;
    }
    return '更新失败，请重试';
}

async function onUpdate() {
    updateError.value = null;
    isUpdating.value = true;
    try {
        await invokeCommand('self_update', {}, { silent: true });
        emit('close');
    } catch (error: unknown) {
        updateError.value = toErrorMessage(error);
    } finally {
        isUpdating.value = false;
    }
}

onMounted(async () => {
    unlistenFns.push(
        await event.listen('progress:main-start', (event) => {
            const payload = event.payload as ProgressPayload;
            mainProgressPayload.value = payload;
            mainProgress.value = 0;
        })
    );

    unlistenFns.push(
        await event.listen('progress:main-update', (event) => {
            if (typeof event.payload === 'number') {
                mainProgress.value += event.payload;
            }
        })
    );

    unlistenFns.push(
        await event.listen('progress:main-end', (event) => {
            if (typeof event.payload === 'string' && mainProgressPayload.value) {
                mainProgressPayload.value = {
                    ...mainProgressPayload.value,
                    message: event.payload
                };
            }
        })
    );

    unlistenFns.push(
        await event.listen('progress:sub-start', (event) => {
            const payload = event.payload as ProgressPayload;
            subProgress.value = 0;
            subProgressPayload.value = payload;
        })
    );

    unlistenFns.push(
        await event.listen('progress:sub-update', (event) => {
            if (typeof event.payload === 'number') {
                subProgress.value += event.payload;
            }
        })
    );

    unlistenFns.push(
        await event.listen('progress:sub-end', (event) => {
            if (typeof event.payload === 'string' && subProgressPayload.value) {
                subProgressPayload.value = {
                    ...subProgressPayload.value,
                    message: event.payload
                };
            }
        })
    );
});

onBeforeUnmount(() => {
    while (unlistenFns.length > 0) {
        const unlisten = unlistenFns.pop();
        if (unlisten) {
            unlisten();
        }
    }
});
</script>
