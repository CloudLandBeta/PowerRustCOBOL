// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// OPTIONAL shim (off by default; WASM-017 R19c, Q10): for a static host that cannot set response
// headers (GitHub Pages is the usual case), this service worker re-serves every same-origin response
// with the cross-origin-isolation headers, so threads become available. Trade-offs, stated:
//
//   * the first visit is not isolated: the page registers this worker and reloads once;
//   * a service worker is a second place where the application's responses are produced, so it is a
//     second place to review; it adds no header but the three below and changes no body;
//   * `credentialless` is used so that images, fonts and tiles from other origins keep loading
//     without CORS; a browser that does not support `credentialless` ignores the isolation and the
//     application runs on the plain player and the main thread, which always works.

'use strict';

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
  const request = event.request;
  // A cache-only request that is not same-origin cannot be answered by fetch(); leave it alone.
  if (request.cache === 'only-if-cached' && request.mode !== 'same-origin') return;
  event.respondWith(
    fetch(request).then((response) => {
      if (response.status === 0) return response; // an opaque response cannot be rewritten
      const headers = new Headers(response.headers);
      headers.set('Cross-Origin-Opener-Policy', 'same-origin');
      headers.set('Cross-Origin-Embedder-Policy', 'credentialless');
      headers.set('Cross-Origin-Resource-Policy', 'same-origin');
      return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
    }),
  );
});
