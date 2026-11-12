// Points the hero's download button at this computer's build. Nothing here is
// sent anywhere: the guess stays in the page.
(() => {
  const ua = navigator.userAgent;
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  let os = null;
  if (/iPhone|iPad|iPod|Android/i.test(ua)) os = "web";
  else if (/Mac/i.test(platform) || /Macintosh/.test(ua)) os = "mac";
  else if (/Win/i.test(platform) || /Windows/.test(ua)) os = "windows";
  else if (/Linux|X11|CrOS/i.test(platform + ua)) os = /CrOS/.test(ua) ? "web" : "linux";

  const nav = document.querySelector(".nav");
  const onScroll = () => nav && nav.classList.toggle("scrolled", window.scrollY > 4);
  addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  if (!os) return;
  const button = document.querySelector("[data-primary]");
  const label = document.querySelector("[data-primary-label]");
  const note = document.querySelector("[data-primary-note]");
  const card = document.querySelector(`.platform[data-os="${os}"]`);
  if (card) card.classList.add("detected");
  const choices = {
    mac: ["/download/mac", "Download for macOS", "macOS 13 or later, Apple silicon and Intel."],
    windows: ["/download/windows", "Download for Windows", "For Windows on x86_64. It installs without an administrator prompt."],
    linux: ["/download/deb", "Download .deb for Linux", "Debian and Ubuntu. Fedora, AppImage and more are below."],
    web: ["https://karyalabs.dev/matty/", "Open Matty in your browser", "Matty runs on desktops and in the browser."],
  };
  const [href, text, sub] = choices[os];
  if (button && label) {
    button.href = href;
    button.classList.toggle("web", os === "web");
    label.textContent = text;
  }
  if (note) {
    note.innerHTML = "";
    note.append(sub + " ");
    const other = document.createElement("a");
    other.href = "#download";
    other.textContent = "Other platforms";
    note.append(other);
  }
})();
