<script setup lang="ts">
defineProps<{
  title?: string;
  text: string;
  actionLabel?: string;
}>();

const emit = defineEmits<{ action: [] }>();
</script>

<template>
  <div class="empty-state">
    <p v-if="title" class="empty-state-title">{{ title }}</p>
    <p class="empty-state-text">{{ text }}</p>
    <button v-if="actionLabel" class="btn primary" type="button" @click="emit('action')">
      {{ actionLabel }}
    </button>
  </div>
</template>
