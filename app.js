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

(() => {
  const root = document.documentElement;
  const site = window.SITE || { mashups: [] };
  const dataRoot = "/data";
  const store = "cybercore-theme";
  const fallback = {
    bg: "07090e", white: "e9f1f7", panel: "111821", line: "3a4656",
    muted: "8a98a8", cyan: "2ee6f6", hot_pink: "ff2e97", purple: "9b6bff",
    acid_green: "6ee7a8", orange: "ffb347", red: "f1142d"
  };
  const vars = {
    bg: "--void", white: "--text", panel: "--surface", line: "--line",
    muted: "--muted", cyan: "--cyan", hot_pink: "--pink", purple: "--purple",
    acid_green: "--green", orange: "--orange", red: "--red"
  };
  const labels = {
    mashup: "SIGNATURE MASHUPS", cyberdyne: "CYBERDYNE", cyberpunk: "CYBERPUNK",
    dystopian: "DYSTOPIAN", neosynth: "NEOSYNTH", synthwave: "SYNTHWAVE",
    default: "DEFAULT / CLASSICS"
  };
  const button = document.querySelector("#themeButton");
  const menu = document.querySelector("#themeMenu");
  let families = {};
  const cache = {};
  const display = (value) => value.replaceAll("-", " ").toUpperCase();
  const familyOf = (name) => Object.keys(families).find((family) => families[family].includes(name));

  async function palette(name) {
    if (cache[name]) return cache[name];
    const family = familyOf(name);
    if (!family) throw new Error(`Unknown theme ${name}`);
    const response = await fetch(`${dataRoot}/themes/${family}/${name}.json`);
    if (!response.ok) throw new Error(`${name}: HTTP ${response.status}`);
    cache[name] = await response.json();
    return cache[name];
  }

  async function resolve(name) {
    const mashup = site.mashups.find((item) => item.id === name);
    if (!mashup) return palette(name);
    const [base, accent] = await Promise.all([palette(mashup.base), palette(mashup.accent)]);
    return { ...accent, bg: base.bg, white: base.white, panel: base.panel, line: base.line, muted: base.muted };
  }

  function paint(name, paletteValue) {
    Object.entries(vars).forEach(([key, variable]) => {
      if (paletteValue[key]) root.style.setProperty(variable, `#${paletteValue[key]}`);
    });
    root.dataset.theme = name;
    const mashup = site.mashups.find((item) => item.id === name);
    if (button) button.textContent = mashup ? mashup.label : display(name);
    menu?.querySelectorAll(".theme-option").forEach((option) => {
      option.setAttribute("aria-selected", String(option.dataset.value === name));
    });
  }

  async function apply(name, persist = true) {
    try { paint(name, await resolve(name)); }
    catch (error) { console.warn("theme", error); paint("cybercore-tech", fallback); return; }
    if (persist) {
      try { localStorage.setItem(store, name); } catch {}
      history.replaceState(null, "", `?theme=${name}${location.hash}`);
    }
  }

  function buildMenu() {
    if (!menu) return;
    menu.innerHTML = "";
    const groups = [["mashup", site.mashups.map((item) => ({ value: item.id, text: item.label }))]]
      .concat(Object.entries(families).map(([family, names]) => [family, names.map((name) => ({ value: name, text: display(name) }))]));
    groups.forEach(([family, items]) => {
      if (!items.length) return;
      const heading = document.createElement("div");
      heading.className = "theme-group-label";
      heading.textContent = labels[family] || family.toUpperCase();
      menu.append(heading);
      items.forEach((item) => {
        const option = document.createElement("button");
        option.type = "button";
        option.className = "theme-option";
        option.setAttribute("role", "option");
        option.dataset.value = item.value;
        option.textContent = item.text;
        option.addEventListener("click", () => { apply(item.value); menu.hidden = true; button?.focus(); });
        menu.append(option);
      });
    });
  }

  if (button && menu) {
    button.addEventListener("click", () => {
      const opening = menu.hidden;
      menu.hidden = !opening;
      button.setAttribute("aria-expanded", String(opening));
    });
    document.addEventListener("pointerdown", (event) => {
      if (!event.target.closest(".theme-picker")) {
        menu.hidden = true;
        button.setAttribute("aria-expanded", "false");
      }
    });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        menu.hidden = true;
        button.setAttribute("aria-expanded", "false");
      }
    });
  }

  (async () => {
    try {
      const response = await fetch(`${dataRoot}/cybergrid.json`);
      families = (await response.json()).families || {};
      buildMenu();
      const requested = new URLSearchParams(location.search).get("theme");
      let saved = null;
      try { saved = localStorage.getItem(store); } catch {}
      const known = (name) => name && (site.mashups.some((item) => item.id === name) || familyOf(name));
      const start = [requested, saved, site.defaultTheme, "cybercore-tech"].find(known);
      if (start) apply(start, Boolean(requested));
    } catch (error) {
      console.warn("theme registry offline", error);
      if (button) button.textContent = "DEFAULT";
      paint(site.defaultTheme, fallback);
    }
  })();
})();
