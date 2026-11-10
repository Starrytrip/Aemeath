import { cpSync, mkdirSync } from "node:fs";
mkdirSync("public/vendor/vditor", { recursive: true });
cpSync("node_modules/vditor/dist", "public/vendor/vditor/dist", { recursive: true });
