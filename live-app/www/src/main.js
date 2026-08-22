async function main() {
  const boot = document.querySelector("#boot");
  const loading = document.querySelector("#loading");
  const pathMatch = /^\/(?:f\/)?([a-z2-7]{13})\/?$/.exec(location.pathname);
  const query = new URLSearchParams(location.search);
  const queryFeed = query.get("feed");
  const feedId = queryFeed && /^[a-z2-7]{13}$/.test(queryFeed) ? queryFeed : pathMatch?.[1];
  if (!feedId) {
    showLanding(boot);
    return;
  }
  if (!("gpu" in navigator)) {
    showFailure(loading, "WEBGPU IS REQUIRED", "Open this link in a current browser with WebGPU support.");
    return;
  }
  try {
    const wasm = await import("./wasm/superiority_live_app.js");
    await wasm.default();

    const backend = (query.get("backend") ?? location.origin).replace(/\/$/, "");
    await wasm.run(feedId, backend, location.origin);
    boot?.remove();
  } catch (error) {
    console.error("Superiority Live failed to start", error);
    showFailure(loading, "FAILED TO START", error?.message ?? String(error));
  }
}

function showLanding(boot) {
  if (!boot) return;
  boot.className = "landing";
  boot.replaceChildren();

  const card = document.createElement("main");
  card.className = "landing-card";
  const icon = document.createElement("img");
  icon.src = "/ui/app-icon.png";
  icon.alt = "";
  const title = document.createElement("h1");
  title.textContent = "SUPERIORITY LIVE";
  const copy = document.createElement("p");
  copy.textContent = "Watch a Battle.net channel shared from Superiority.";
  const form = document.createElement("form");
  const input = document.createElement("input");
  input.type = "text";
  input.autocomplete = "off";
  input.placeholder = "Paste a Superiority Live link";
  input.setAttribute("aria-label", "Superiority Live link");
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.textContent = "WATCH";
  const error = document.createElement("div");
  error.className = "landing-error";
  form.append(input, submit);
  form.addEventListener("submit", event => {
    event.preventDefault();
    const feed = parseFeed(input.value);
    if (feed) {
      location.href = `/${feed}`;
    } else {
      error.textContent = "That is not a valid Superiority Live link.";
    }
  });
  card.append(icon, title, copy, form, error);
  boot.append(card);
}

function parseFeed(value) {
  const raw = value.trim();
  if (/^[a-z2-7]{13}$/.test(raw)) return raw;
  try {
    return /^\/(?:f\/)?([a-z2-7]{13})\/?$/.exec(new URL(raw).pathname)?.[1] ?? null;
  } catch {
    return null;
  }
}

function showFailure(loading, title, detail) {
  if (!loading) return;
  loading.replaceChildren();
  const heading = document.createElement("strong");
  heading.textContent = title;
  const message = document.createElement("small");
  message.textContent = detail;
  loading.append(heading, message);
}

void main();
