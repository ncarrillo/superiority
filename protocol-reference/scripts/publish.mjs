import { copyFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const builtDocument = new URL("../dist/PROTOCOL.html", import.meta.url);
const publishedDocument = new URL("../../PROTOCOL.html", import.meta.url);

await copyFile(builtDocument, publishedDocument);
console.log(`Published ${publishedDocument.pathname}`);
