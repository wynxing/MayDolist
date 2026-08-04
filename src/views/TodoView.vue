<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useTodoStore } from "../stores/todo";
const store = useTodoStore();
const newList = ref("");
const drafts = ref<Record<string, string>>({});
onMounted(() => store.init());
async function addList() { if (newList.value.trim()) { await store.createList(newList.value); newList.value = ""; } }
async function addItem(id: string) { const value = drafts.value[id]?.trim(); if (value) { await store.createItem(id, value); drafts.value[id] = ""; } }
async function moveList(id: string, delta: number) { const ids = store.lists.map((list) => list.id); const from = ids.indexOf(id); const to = from + delta; if (to < 0 || to >= ids.length) return; [ids[from], ids[to]] = [ids[to], ids[from]]; await store.reorderLists(ids); }
async function moveItem(listId: string, id: string, delta: number) { const list = store.lists.find((value) => value.id === listId); if (!list) return; const ids = list.items.map((item) => item.id); const from = ids.indexOf(id); const to = from + delta; if (to < 0 || to >= ids.length) return; [ids[from], ids[to]] = [ids[to], ids[from]]; await store.reorderItems(listId, ids); }
</script>
<template>
  <section>
    <div class="toolbar"><input v-model="newList" class="input" placeholder="新建列表" @keyup.enter="addList"><button class="btn primary" @click="addList">新建</button></div>
    <p v-if="store.error" class="error">{{ store.error }}</p>
    <div class="columns">
      <article v-for="list in store.lists" :key="list.id" class="column">
        <header class="snapshot-header">
          <input class="input column-title" :value="list.title" @change="store.updateList(list.id,{title:($event.target as HTMLInputElement).value})">
          <button class="btn" @click="moveList(list.id,-1)">←</button><button class="btn" @click="moveList(list.id,1)">→</button><button class="btn danger" @click="store.updateList(list.id,{deleted:true})">×</button>
        </header>
        <div class="toolbar"><input v-model="drafts[list.id]" class="input" placeholder="添加待办" @keyup.enter="addItem(list.id)"></div>
        <ul class="items">
          <li v-for="item in list.items" :key="item.id" class="item" :class="{done:item.completed}">
            <input type="checkbox" :checked="item.completed" @change="store.toggleItem(item.id,item.completed)">
            <input class="input item-edit" :value="item.title" @change="store.renameItem(item.id,($event.target as HTMLInputElement).value)">
            <select class="input move-select" :value="list.id" @change="store.moveItem(item.id,($event.target as HTMLSelectElement).value,9999)"><option v-for="target in store.lists" :key="target.id" :value="target.id">{{ target.title }}</option></select>
            <button class="btn" @click="moveItem(list.id,item.id,-1)">↑</button><button class="btn" @click="moveItem(list.id,item.id,1)">↓</button><button class="btn danger" @click="store.softDelete(item.id)">×</button>
          </li>
        </ul>
      </article>
    </div>
  </section>
</template>
