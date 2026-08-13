<script setup lang="ts">
defineProps<{
  message: string;
  confirmLabel?: string;
  danger?: boolean;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();
</script>

<template>
  <div class="confirm-bar" role="alertdialog" aria-live="polite">
    <p class="confirm-bar-message">{{ message }}</p>
    <div class="confirm-bar-actions">
      <button class="btn ghost compact" type="button" @click="emit('cancel')">取消</button>
      <button
        class="btn compact"
        :class="danger ? 'danger' : 'primary'"
        type="button"
        @click="emit('confirm')"
      >
        {{ confirmLabel ?? "确认" }}
      </button>
    </div>
  </div>
</template>
