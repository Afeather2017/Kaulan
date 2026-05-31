import { createRouter, createWebHistory } from "vue-router";
import Home from "../views/Home.vue";
import Library from "../views/Library.vue";
import Playlists from "../views/Playlists.vue";

const routes = [
  {
    path: "/",
    name: "Home",
    component: Home,
  },
  {
    path: "/library",
    name: "Library",
    component: Library,
  },
  {
    path: "/playlists",
    name: "Playlists",
    component: Playlists,
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
