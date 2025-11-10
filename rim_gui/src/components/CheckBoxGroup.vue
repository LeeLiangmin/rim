<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { CheckGroup, CheckGroupItem } from '@/utils';

type Props<T> = {
  expand: boolean;
  group: CheckGroup<T>;
  radio?: boolean; // 单选模式
};

const { expand, group, radio = false } = defineProps<Props<unknown>>();
const emit = defineEmits(['itemClick', 'change']);

const groupExpand = ref(expand);
const isCheckedAll = computed(() => group.items.every((item) => item.checked));
const isCheckedEmpty = computed(() =>
  group.items.every((item) => !item.checked)
);

function handleExpandClick() {
  groupExpand.value = !groupExpand.value;
}

function handleCheckAllClick(newValue: boolean) {
  if (radio) {
    // 单选模式下，全选功能禁用
    return;
  }
  group.items.forEach((checkItem) => {
    if (checkItem.disabled) return;
    checkItem.checked = newValue;
  });
}

function handleItemToggle(item: CheckGroupItem<unknown>) {
  if (radio && !item.checked) {
    // 单选模式：选中当前项时，取消同组其他项的选中
    group.items.forEach((otherItem) => {
      if (otherItem !== item && !otherItem.disabled) {
        otherItem.checked = false;
      }
    });
    item.checked = true;
  } else {
    // 多选模式：正常切换
    item.checked = !item.checked;
  }
}

function handleItemClick<T>(item: CheckGroupItem<T>) {
  emit('itemClick', item);
}

watch(group.items, (newValue) => {
  emit(
    'change',
    newValue.filter((item) => item.checked)
  );
});
</script>

<template>
  <div>
    <div flex="~ items-center">
      <i
        class="i-mdi:menu-up"
        w="1.5rem"
        h="1.5rem"
        transition="all"
        cursor="pointer"
        c="secondary"
        :class="{ 'rotate-180': groupExpand }"
        @click="handleExpandClick"
      />
      <base-check-box 
        w="full" 
        @titleClick="handleExpandClick" 
        :title="group.label" 
        :isGroup="true"
        :modelValue="isCheckedAll"
        @update:modelValue="handleCheckAllClick"
      >
        <template #icon>
          <span
            v-if="!radio"
            flex="~ items-center justify-center"
            h="full"
          >
            <i class="i-mdi:check" v-show="isCheckedAll" c="active" />
            <i
              class="i-mdi:minus"
              v-show="!isCheckedAll && !isCheckedEmpty"
              c="active"
            />
          </span>
        </template>
      </base-check-box>
    </div>
    <transition name="group">
      <div v-if="groupExpand" ml="3rem">
        <base-check-box
          flex="~ items-center"
          v-for="item of group.items"
          :key="item.label"
          :modelValue="item.checked"
          @update:modelValue="handleItemToggle(item)"
          :title="item.label"
          :disabled="item.disabled"
          :label-component="item.labelComponent"
          :label-component-props="item.labelComponentProps"
          decoration="hover:underline"
          :class="{
            'decoration-underline': item.focused,
          }"
          @titleClick="handleItemClick(item)"
        />
      </div>
    </transition>
  </div>
</template>

<style scoped>
.group-enter-active {
  transition: all 150ms ease;
}
/* 菜单进出 */
.group-enter-from {
  transform: scaleY(0.5) translateY(-50%);
}

.group-enter-to {
  transform: scaleY(1) translateY(0);
}
</style>
