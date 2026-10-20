// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// The host page's glue (WASM-002 R13). It exposes the whole JavaScript surface a host page may use,
// and nothing that evaluates script:
//
//   PowerRustCOBOL.mount(element, params)   mount the application into an element; resolves to the app
//   PowerRustCOBOL.unmount(element)         unmount it and free everything it held
//   PowerRustCOBOL.versions                 { loader, abi }
//
//   params: { config, locale, theme: { '--pr-accent': '#…' }, backend: 'wss://…' }
//
// Events come out on the element as DOM events: pr-ready, pr-error, pr-navigated, pr-signed-in,
// pr-signed-out (each a CustomEvent with a `detail`).
//
// A page that only wants the application fills the window: the element `#pr-app` carries
// `data-pr-config` and is mounted on load.

import { mount as loaderMount } from './__LOADER__';

const apps = new WeakMap();

async function mount(element, params = {}) {
  if (!(element instanceof Element)) throw new TypeError('PowerRustCOBOL.mount needs a DOM element');
  if (apps.has(element)) return apps.get(element);
  // Theme values flow INTO the application as CSS custom properties; they inherit through the
  // shadow root, and nothing flows back out.
  if (params.theme && typeof params.theme === 'object') {
    for (const [name, value] of Object.entries(params.theme)) {
      if (/^--[a-zA-Z0-9_-]+$/.test(name)) element.style.setProperty(name, String(value));
    }
  }
  const options = {};
  if (params.config) options.configUrl = params.config;
  if (params.config && typeof params.config === 'object') {
    options.config = params.config;
    delete options.configUrl;
  }
  options.startForm = element.getAttribute('data-pr-start') || params.startForm || '';
  const promise = loaderMount(element, options).then((app) => {
    if (params.locale && app.player && typeof app.player.set_locale === 'function') app.player.set_locale(String(params.locale));
    return app;
  });
  apps.set(element, promise);
  try {
    return await promise;
  } catch (e) {
    apps.delete(element);
    throw e;
  }
}

async function unmount(element) {
  const p = apps.get(element);
  if (!p) return;
  apps.delete(element);
  const app = await p.catch(() => null);
  if (app) await app.unmount();
}

Object.defineProperty(globalThis, 'PowerRustCOBOL', {
  value: Object.freeze({ mount, unmount, versions: Object.freeze({ loader: 1, abi: 1 }) }),
  enumerable: true,
});

function autoMount() {
  for (const el of document.querySelectorAll('[data-pr-config]')) {
    mount(el).catch(() => {
      /* the loader has shown the message and raised pr-error */
    });
  }
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', autoMount);
else autoMount();
