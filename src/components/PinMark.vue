<script setup lang="ts">
defineProps<{
  on?: boolean;
}>();
</script>

<template>
  <span class="pin-mark" :class="{ on }" aria-hidden="true"></span>
</template>
