// Phextron web UI glue — the entire JS surface of the WASM web UI.
//
// The module (phextron_web.wasm, built from crates/phextron-web) is a pure
// function: panel JSON bytes in, HTML bytes out, through three exports
// (pw_alloc / pw_render / pw_free). This file only:
//   1. instantiates the module,
//   2. fetches the same /api/panels/nodes JSON the desktop shell fetches,
//   3. hands the bytes to the module and places the returned HTML,
//   4. wires Refresh and the theme toggle (same storage key and cycle as
//      the desktop shell, so the choice is shared across surfaces).
// All parsing and rendering happen in Rust, in the module.
(function () {
  var panelEl = document.getElementById("panel");

  function fail(msg) {
    panelEl.innerHTML = '<p class="err">Web UI failed to load: ' + msg + "</p>";
  }

  function instantiate(url) {
    if (WebAssembly.instantiateStreaming) {
      return WebAssembly.instantiateStreaming(fetch(url), {}).then(
        function (r) { return r.instance; },
        function () {
          // Streaming needs the application/wasm MIME; fall back to the
          // buffered path (the console server serves the right type, but
          // a static host in front of it might not).
          return fetch(url)
            .then(function (r) { return r.arrayBuffer(); })
            .then(function (b) { return WebAssembly.instantiate(b, {}); })
            .then(function (r) { return r.instance; });
        }
      );
    }
    return fetch(url)
      .then(function (r) { return r.arrayBuffer(); })
      .then(function (b) { return WebAssembly.instantiate(b, {}); })
      .then(function (r) { return r.instance; });
  }

  instantiate("/web/phextron_web.wasm")
    .then(function (instance) {
      var ex = instance.exports;
      var enc = new TextEncoder();
      var dec = new TextDecoder();

      function load() {
        fetch("/api/panels/nodes")
          .then(function (r) { return r.text(); })
          .then(function (text) {
            var bytes = enc.encode(text);
            var ptr = ex.pw_alloc(bytes.length);
            // Fresh view after alloc: allocation may have grown memory.
            new Uint8Array(ex.memory.buffer).set(bytes, ptr);
            var packed = ex.pw_render(ptr, bytes.length);
            var outPtr = Number(packed >> 32n);
            var outLen = Number(packed & 0xffffffffn);
            var html = dec.decode(new Uint8Array(ex.memory.buffer, outPtr, outLen));
            panelEl.innerHTML = html;
            panelEl.dataset.renderedBy = "phextron-web-wasm";
            document.documentElement.dataset.renderer = "wasm";
            ex.pw_free(ptr, bytes.length);
            ex.pw_free(outPtr, outLen);
          })
          .catch(function (e) { fail(e.message); });
      }

      document.getElementById("refreshBtn").addEventListener("click", load);
      load();
    })
    .catch(function (e) { fail(e.message); });

  // Theme toggle — the desktop shell's exact behavior: cycle
  // system → light → dark, persist the choice, resolve against the OS.
  var themeBtn = document.getElementById("themeBtn");
  function themeLabel() {
    var c = document.documentElement.dataset.themeChoice || "system";
    themeBtn.textContent = "Theme: " + c.charAt(0).toUpperCase() + c.slice(1);
  }
  themeBtn.addEventListener("click", function () {
    var order = ["system", "light", "dark"];
    var c = document.documentElement.dataset.themeChoice || "system";
    var next = order[(order.indexOf(c) + 1) % order.length];
    try { localStorage.setItem("phextron.theme", next); } catch (e) {}
    var dark = next === "dark" ||
      (next === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    document.documentElement.dataset.themeChoice = next;
    themeLabel();
  });
  themeLabel();
})();
