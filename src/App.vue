<script setup lang="ts">
import { defineAsyncComponent } from "vue";
// Lazy-load the main board so the quick capture / floating note windows never
// download or parse its code.
const MainBoard = defineAsyncComponent(() => import("./views/MainBoard.vue"));
import FloatingNote from "./views/FloatingNote.vue";
import QuickCapture from "./views/QuickCapture.vue";

const params = new URLSearchParams(location.search);
</script>

<template>
  <FloatingNote v-if="params.has('note')" />
  <QuickCapture v-else-if="params.has('quick')" />
  <MainBoard v-else />
</template>
