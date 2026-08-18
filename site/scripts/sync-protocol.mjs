import { copyFile } from "node:fs/promises";

const protocol = new URL("../../PROTOCOL.html", import.meta.url);
const publicProtocol = new URL("../public/PROTOCOL.html", import.meta.url);

await copyFile(protocol, publicProtocol);
