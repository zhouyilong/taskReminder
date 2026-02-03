import { createApp } from "vue";
import NotificationApp from "./NotificationApp.vue";
import "./styles.css";

document.documentElement.classList.add("notification-mode");
document.body.classList.add("notification-mode");

createApp(NotificationApp).mount("#notification");
