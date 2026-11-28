// Auth tool composition root (FE-012). Keeps auth.html as markup only.

import { initFalcon } from "./falcon.js";
import { initKeygen } from "./keygen.js";
import { initSession } from "./session.js";
import { initApiPanel } from "./api-panel.js";

initKeygen();
initSession();
initApiPanel();

// Warm the WASM bundle in the background; UI awaits it on first use.
initFalcon();
