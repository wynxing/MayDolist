import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/global.css";
import { useUpdateStore } from "./stores/update";

const pinia = createPinia();
createApp(App).use(pinia).mount("#app");

const params = new URLSearchParams(location.search);
if (!params.has("note") && !params.has("quick")) {
  void useUpdateStore(pinia).init();
}
