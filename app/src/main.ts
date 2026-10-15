import { invoke } from "@tauri-apps/api/core";

invoke("get_settings").then((s) => {
  document.getElementById("out")!.textContent = JSON.stringify(s, null, 2);
});
