import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/global.css";
import { useUpdateStore } from "./stores/update";

const pinia = createPinia();
createApp(App).use(pinia).mount("#app");

if (!new URLSearchParams(location.search).has("note")) {
  void useUpdateStore(pinia).init();
}
