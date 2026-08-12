<script setup lang="ts">
import MainBoard from "./views/MainBoard.vue";
import FloatingNote from "./views/FloatingNote.vue";
import QuickCapture from "./views/QuickCapture.vue";
import CommandPalette from "./views/CommandPalette.vue";

const params = new URLSearchParams(location.search);
</script>

<template>
  <FloatingNote v-if="params.has('note')" />
  <QuickCapture v-else-if="params.has('quick')" />
  <CommandPalette v-else-if="params.has('palette')" />
  <MainBoard v-else />
</template>
