<script setup lang="ts">
defineProps<{
  title: string;
  subtitle?: string;
  headingId?: string;
}>();
</script>

<template>
  <header class="page-header">
    <div>
      <h1 :id="headingId">{{ title }}</h1>
      <p v-if="subtitle">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" class="page-header-actions">
      <slot name="actions" />
    </div>
  </header>
</template>
