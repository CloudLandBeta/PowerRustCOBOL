// SPDX-License-Identifier: Apache-2.0

pub struct Specialist {
    pub name: String,
}

impl Specialist {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
