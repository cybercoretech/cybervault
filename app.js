const CMDS = {
  linux: "curl -fsSL https://raw.githubusercontent.com/cybercoretech/cybervault/main/install.sh | sh",
  macos: "curl -fsSL https://raw.githubusercontent.com/cybercoretech/cybervault/main/install.sh | sh"
};

function cvCopy(button) {
  const text = document.querySelector("#cv-cmd")?.textContent || "";
  navigator.clipboard?.writeText(text).then(() => {
    button.textContent = "OK";
    button.classList.add("copied");
    window.setTimeout(() => {
      button.textContent = "COPY";
      button.classList.remove("copied");
    }, 1400);
  }).catch(() => {});
}

function cvSetOs(os) {
  const command = document.querySelector("#cv-cmd");
  if (command) command.textContent = CMDS[os] || CMDS.linux;
  document.querySelectorAll("[data-os]").forEach((tab) => {
    tab.classList.toggle("active", tab.dataset.os === os);
  });
}

document.querySelectorAll("[data-os]").forEach((tab) => {
  tab.addEventListener("click", () => cvSetOs(tab.dataset.os));
});
document.querySelector("#cv-copy")?.addEventListener("click", (event) => cvCopy(event.currentTarget));

fetch("https://api.github.com/repos/cybercoretech/cybervault/releases/latest", {
  headers: { Accept: "application/vnd.github+json" }
}).then((response) => {
  if (!response.ok) throw new Error(String(response.status));
  return response.json();
}).then((release) => {
  const downloads = (release.assets || [])
    .filter((asset) => !asset.name.endsWith(".sha256"))
    .reduce((total, asset) => total + (asset.download_count || 0), 0);
  const target = document.querySelector("#dl-count");
  if (target) target.textContent = `${downloads.toLocaleString()} downloads · ${release.tag_name}`;
}).catch(() => {});
