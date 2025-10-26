<script setup lang="ts">
import { RouterView } from 'vue-router';
</script>

<template>
  <div>
    <RouterView />
  </div>
</template>

<style>
:root {
  margin: 0;
  padding: 0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue",
             "Noto Sans", Ubuntu, Cantarell, "Liberation Sans", "DejaVu Sans",
             "Helvetica", "Arial", system-ui, "Apple Color Emoji", "Segoe UI Emoji",
             "Segoe UI Symbol", "Noto Color Emoji", serif, sans-serif;
  line-height: 24px;
  letter-spacing: normal;
  font-weight: 400;
  --uno: bg-back c-header;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}
</style>
