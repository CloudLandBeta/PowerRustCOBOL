// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 075 T1 — the project's memory limit for file searches reaches every
//! tool set built after a host publishes it. Its own test binary: the limit is
//! process-wide, and no other test here may see it change.

use cobolt_runtime::mcp_tool::{
    file_memory_limit, publish_file_memory_limit, IndexedToolSet, DEFAULT_MEMORY_LIMIT_BYTES,
};

#[test]
fn a_published_limit_reaches_every_new_tool_set() {
    assert_eq!(file_memory_limit(), DEFAULT_MEMORY_LIMIT_BYTES, "64 MiB until published");
    assert_eq!(IndexedToolSet::new().memory_limit(), 64 * 1024 * 1024);

    publish_file_memory_limit(200 * 1024 * 1024);
    assert_eq!(IndexedToolSet::new().memory_limit(), 200 * 1024 * 1024);

    publish_file_memory_limit(0);
    assert_eq!(IndexedToolSet::new().memory_limit(), DEFAULT_MEMORY_LIMIT_BYTES, "0 restores the default");
    println!("file memory limit: default 64 MiB; published 200 MiB reached a new tool set; 0 restored 64 MiB");
}
