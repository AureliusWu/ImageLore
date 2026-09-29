const status = document.querySelector("#status");
const state = await chrome.storage.local.get(["lastCapture", "lastError"]);
if (state.lastCapture) {
  const item = state.lastCapture;
  status.className = "status ok";
  status.textContent =
    "最近保存：" + item.imageName + (item.intent === "remix" ? " · Remix 参考" : "");
} else if (state.lastError) {
  status.className = "status err";
  status.textContent = "最近失败：" + state.lastError;
}
document
  .querySelector("#downloads")
  .addEventListener("click", () => chrome.downloads.showDefaultFolder());
