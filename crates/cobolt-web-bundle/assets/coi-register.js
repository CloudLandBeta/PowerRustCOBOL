// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Registers coi-serviceworker.js and reloads the page once so that it is served isolated.
// Loaded only when the bundle was built with the service-worker shim.

(function () {
  'use strict';
  if (self.crossOriginIsolated || !('serviceWorker' in navigator)) return;
  var KEY = 'pr-coi-reloaded';
  var tried = false;
  try {
    tried = sessionStorage.getItem(KEY) === '1';
  } catch (_) {
    tried = true; // no storage: do not risk a reload loop
  }
  var src = document.currentScript && document.currentScript.dataset && document.currentScript.dataset.worker;
  if (!src) return;
  navigator.serviceWorker.register(src).then(function (reg) {
    if (reg.active && !navigator.serviceWorker.controller && !tried) {
      try {
        sessionStorage.setItem(KEY, '1');
      } catch (_) {
        return;
      }
      location.reload();
    }
  });
})();
