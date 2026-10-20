// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The registry: every graphics object a program (or a form) has made, by handle
//! (WASM-018 R2, R15).
//!
//! A host makes one [`Registry`], hands it the services the crate must not reach for
//! itself - the assets ([`AssetHost`]), the fonts - and then
//!
//! * [`Registry::create`] makes an object of a [`ObjectKind`] for an **owner** (a form
//!   number) and returns its handle,
//! * [`Registry::invoke`] runs a method on it by name with [`Value`] arguments and
//!   returns a [`Value`] (the method tables are in the [crate docs](crate)),
//! * [`Registry::release`] releases one handle, and [`Registry::release_owner`]
//!   releases everything a form made when the form is destroyed.
//!
//! Things an object makes - the image `ToImage` returns, the series of a plot - are
//! handles too: images are objects of the registry (so any canvas can draw them by
//! handle), series, nodes and bodies live inside their object and die with it.
//!
//! # One object at a time
//!
//! While a method runs, its object is taken out of the table, so a method can use the
//! *other* objects (a plot draws into a canvas, a body moves a node) without aliasing
//! itself. A method that names its own handle gets the answer "busy", not a deadlock.

use std::collections::HashMap;
use std::sync::Arc;

use crate::api;
use crate::canvas2d::Canvas2D;
use crate::error::{GraphicsError, Result};
use crate::handles::{Handle, HandleTable};
use crate::limits::Limits;
use crate::pixels::PixelBuffer;
use crate::resources::{AssetHost, Resources};
use crate::textapi::TextEngine;
use crate::value::Value;

#[cfg(feature = "imagebuf")]
use crate::imagebuf::ImageBuffer;
#[cfg(feature = "physics")]
use crate::physics::{Physics2D, Physics3D};
#[cfg(feature = "plot")]
use crate::plot::Plot;
#[cfg(feature = "scene3d-core")]
use crate::scene3d::Scene3D;
#[cfg(feature = "scene3d")]
use crate::scene3d::{Canvas3D, Gpu};

/// Who owns an object: a form number. When the form is destroyed, everything it owns is released.
pub type Owner = u32;

/// The kinds of object a program can make.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    /// The 2D drawing surface.
    Canvas2D,
    /// The 3D view.
    Canvas3D,
    /// Pixels to load, change and save.
    ImageBuffer,
    /// A chart.
    Plot,
    /// A 3D scene.
    Scene3D,
    /// A 2D physics object (worlds of bodies).
    Physics2D,
    /// A 3D physics object.
    Physics3D,
}

impl ObjectKind {
    /// The name a program uses (`Canvas2D`).
    pub fn name(self) -> &'static str {
        match self {
            ObjectKind::Canvas2D => "Canvas2D",
            ObjectKind::Canvas3D => "Canvas3D",
            ObjectKind::ImageBuffer => "ImageBuffer",
            ObjectKind::Plot => "Plot",
            ObjectKind::Scene3D => "Scene3D",
            ObjectKind::Physics2D => "Physics2D",
            ObjectKind::Physics3D => "Physics3D",
        }
    }

    /// From a name, case-insensitive.
    pub fn from_name(name: &str) -> Option<ObjectKind> {
        Self::available().into_iter().find(|k| k.name().eq_ignore_ascii_case(name.trim()))
    }

    /// The kinds this build has (they follow the crate's features).
    pub fn available() -> Vec<ObjectKind> {
        let mut v = vec![ObjectKind::Canvas2D];
        #[cfg(feature = "scene3d")]
        v.push(ObjectKind::Canvas3D);
        #[cfg(feature = "imagebuf")]
        v.push(ObjectKind::ImageBuffer);
        #[cfg(feature = "plot")]
        v.push(ObjectKind::Plot);
        #[cfg(feature = "scene3d-core")]
        v.push(ObjectKind::Scene3D);
        #[cfg(feature = "physics")]
        {
            v.push(ObjectKind::Physics2D);
            v.push(ObjectKind::Physics3D);
        }
        v
    }
}

/// One method as the documentation and the host's tooling (IntelliSense, the System KB, the generator) see it.
#[derive(Clone, Debug, PartialEq)]
pub struct MethodInfo {
    /// The object kind.
    pub object: ObjectKind,
    /// The method name.
    pub name: String,
    /// `(name, kind word, optional)` for each parameter.
    pub params: Vec<(String, &'static str, bool)>,
    /// The kind word of the answer (`-` for none).
    pub returns: &'static str,
    /// One line.
    pub doc: String,
}

impl MethodInfo {
    /// `Name(a, b, [c])` - the shape of the call.
    pub fn signature(&self) -> String {
        let ps: Vec<String> = self.params.iter().map(|(n, t, opt)| if *opt { format!("[{n}: {t}]") } else { format!("{n}: {t}") }).collect();
        format!("{}({})", self.name, ps.join(", "))
    }
}

pub(crate) enum Object {
    Canvas2D(Canvas2D),
    #[cfg(feature = "imagebuf")]
    ImageBuffer(ImageBuffer),
    #[cfg(feature = "plot")]
    Plot(Plot),
    #[cfg(feature = "physics")]
    Physics2D(Physics2D),
    #[cfg(feature = "physics")]
    Physics3D(Physics3D),
    #[cfg(feature = "scene3d-core")]
    Scene3D(Box<Scene3D>),
    #[cfg(feature = "scene3d")]
    Canvas3D(Box<Canvas3D>),
}

impl Object {
    fn memory_bytes(&self) -> u64 {
        match self {
            Object::Canvas2D(o) => o.memory_bytes(),
            #[cfg(feature = "imagebuf")]
            Object::ImageBuffer(o) => o.memory_bytes(),
            #[cfg(feature = "plot")]
            Object::Plot(o) => o.memory_bytes(),
            #[cfg(feature = "physics")]
            Object::Physics2D(_) | Object::Physics3D(_) => 4096,
            #[cfg(feature = "scene3d-core")]
            Object::Scene3D(o) => o.memory_bytes() + o.node_count() as u64 * 256,
            #[cfg(feature = "scene3d")]
            Object::Canvas3D(o) => {
                let (w, h) = o.size();
                if o.renderer().is_some() {
                    w as u64 * h as u64 * 4 * 3
                } else {
                    0
                }
            }
        }
    }

    fn call(&mut self, reg: &mut Registry, method: &str, args: &[Value]) -> Result<Value> {
        match self {
            Object::Canvas2D(o) => api::canvas2d::call(o, reg, method, args),
            #[cfg(feature = "imagebuf")]
            Object::ImageBuffer(o) => api::image::call(o, reg, method, args),
            #[cfg(feature = "plot")]
            Object::Plot(o) => api::plot::call(o, reg, method, args),
            #[cfg(feature = "physics")]
            Object::Physics2D(o) => api::physics2d::call(o, reg, method, args),
            #[cfg(feature = "physics")]
            Object::Physics3D(o) => api::physics3d::call(o, reg, method, args),
            #[cfg(feature = "scene3d-core")]
            Object::Scene3D(o) => api::scene3d::call(o, reg, method, args),
            #[cfg(feature = "scene3d")]
            Object::Canvas3D(o) => api::canvas3d::call(o, reg, method, args),
        }
    }
}

struct Entry {
    owner: Owner,
    kind: ObjectKind,
    /// `None` while one of its methods runs.
    obj: Option<Object>,
    memory: u64,
}

/// A snapshot of what a canvas may refer to by number while it draws.
pub(crate) struct RegistryResources {
    images: HashMap<u32, Arc<PixelBuffer>>,
    engine: Option<Arc<dyn TextEngine>>,
}

impl Resources for RegistryResources {
    fn image(&self, id: u32) -> Option<Arc<PixelBuffer>> {
        self.images.get(&id).cloned()
    }

    fn text_engine(&self) -> Option<&dyn TextEngine> {
        self.engine.as_deref()
    }
}

/// Every graphics object, by handle.
pub struct Registry {
    limits: Limits,
    objects: HandleTable<Entry>,
    assets: Option<Arc<dyn AssetHost + Send + Sync>>,
    engine: Option<Arc<dyn TextEngine>>,
    #[cfg(feature = "text")]
    fonts: Option<Arc<crate::text::FontSet>>,
    #[cfg(feature = "scene3d")]
    gpu: Option<Gpu>,
    current: Option<(Handle, Owner)>,
    memory: u64,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Registry({} object(s), {} bytes)", self.objects.len(), self.memory)
    }
}

impl Registry {
    /// An empty registry with these limits.
    pub fn new(limits: Limits) -> Registry {
        Registry {
            limits,
            objects: HandleTable::new("object"),
            assets: None,
            engine: None,
            #[cfg(feature = "text")]
            fonts: None,
            #[cfg(feature = "scene3d")]
            gpu: None,
            current: None,
            memory: 0,
        }
    }

    /// The limits in force.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Give the registry the host's assets (`LoadAsset`, `Save`, models read through it).
    pub fn set_assets(&mut self, assets: Arc<dyn AssetHost + Send + Sync>) {
        self.assets = Some(assets);
    }

    /// The assets, if the host gave any.
    pub fn assets(&self) -> Option<&dyn AssetHost> {
        self.assets.as_deref().map(|a| a as &dyn AssetHost)
    }

    /// Use this text engine (any [`TextEngine`]; see [`Registry::fonts`] for the built-in one).
    pub fn set_text_engine(&mut self, engine: Arc<dyn TextEngine>) {
        self.engine = Some(engine);
    }

    /// The text engine canvases and plots use, if there is one.
    pub fn text_engine(&self) -> Option<Arc<dyn TextEngine>> {
        self.engine.clone()
    }

    /// The registry's font set, made on first use; `LoadFont` registers fonts in it and it is the
    /// text engine unless the host set another.
    #[cfg(feature = "text")]
    pub fn fonts(&mut self) -> Arc<crate::text::FontSet> {
        if self.fonts.is_none() {
            let f = Arc::new(crate::text::FontSet::with_limits(self.limits.clone()));
            self.fonts = Some(f.clone());
            if self.engine.is_none() {
                self.engine = Some(f);
            }
        }
        self.fonts.clone().expect("just made")
    }

    /// The graphics device every Canvas3D shares, opened on first use.
    #[cfg(feature = "scene3d")]
    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.gpu = Some(gpu);
    }

    #[cfg(feature = "scene3d")]
    pub(crate) fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    pub(crate) fn current_handle(&self) -> Option<Handle> {
        self.current.map(|c| c.0)
    }

    pub(crate) fn current_owner(&self) -> Owner {
        self.current.map(|c| c.1).unwrap_or(0)
    }

    // ---- making and releasing -----------------------------------------------

    /// Make an object. `args` are the constructor's: `Canvas2D` and `Canvas3D` take `(width, height)`
    /// (a 2D canvas also an optional pixel ratio); the others take none.
    pub fn create(&mut self, kind: ObjectKind, owner: Owner, args: &[Value]) -> Result<Handle> {
        let a = crate::value::Args::new("Registry", "Create", args);
        let obj = match kind {
            ObjectKind::Canvas2D => {
                let (w, h) = (a.u32(0)?, a.u32(1)?);
                let ratio = a.num_or(2, 1.0)? as f32;
                Object::Canvas2D(Canvas2D::new(w, h, ratio, self.limits.clone())?)
            }
            #[cfg(feature = "scene3d")]
            ObjectKind::Canvas3D => Object::Canvas3D(Box::new(Canvas3D::new(a.u32(0)?, a.u32(1)?, self.limits.clone())?)),
            #[cfg(feature = "imagebuf")]
            ObjectKind::ImageBuffer => Object::ImageBuffer(ImageBuffer::create(1, 1, crate::Rgba::TRANSPARENT, &self.limits)?),
            #[cfg(feature = "plot")]
            ObjectKind::Plot => Object::Plot(Plot::new(self.limits.clone())),
            #[cfg(feature = "scene3d-core")]
            ObjectKind::Scene3D => Object::Scene3D(Box::new(Scene3D::new(self.limits.clone()))),
            #[cfg(feature = "physics")]
            ObjectKind::Physics2D => Object::Physics2D(Physics2D::new(self.limits.clone())),
            #[cfg(feature = "physics")]
            ObjectKind::Physics3D => Object::Physics3D(Physics3D::new(self.limits.clone())),
            #[allow(unreachable_patterns)]
            other => return Err(GraphicsError::Unsupported(format!("{} is not in this build (it needs a crate feature that is off)", other.name()))),
        };
        self.insert(obj, kind, owner)
    }

    fn insert(&mut self, obj: Object, kind: ObjectKind, owner: Owner) -> Result<Handle> {
        let memory = obj.memory_bytes();
        self.limits.check_total(self.memory + memory)?;
        let h = self.objects.insert(Entry { owner, kind, obj: Some(obj), memory })?;
        self.memory += memory;
        Ok(h)
    }

    /// Make an image from pixels, for the object that is running a method (it belongs to the same owner).
    pub(crate) fn add_image(&mut self, px: PixelBuffer) -> Result<Handle> {
        #[cfg(feature = "imagebuf")]
        {
            let owner = self.current_owner();
            self.insert(Object::ImageBuffer(ImageBuffer::from_pixels(px)), ObjectKind::ImageBuffer, owner)
        }
        #[cfg(not(feature = "imagebuf"))]
        {
            let _ = px;
            Err(GraphicsError::Unsupported("ImageBuffer is not in this build".into()))
        }
    }

    /// Release one object.
    pub fn release(&mut self, handle: Handle) -> Result<()> {
        let e = self.objects.remove(handle)?;
        self.memory = self.memory.saturating_sub(e.memory);
        Ok(())
    }

    /// Release everything an owner made (its form was destroyed). Returns how many.
    pub fn release_owner(&mut self, owner: Owner) -> usize {
        let doomed: Vec<Handle> = self.objects.iter().filter(|(_, e)| e.owner == owner).map(|(h, _)| h).collect();
        let n = doomed.len();
        for h in doomed {
            let _ = self.release(h);
        }
        n
    }

    /// How many objects are live.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether no object is live.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// The memory the live objects hold (pixels, vertices, textures), as they account for it.
    pub fn memory_bytes(&self) -> u64 {
        self.memory
    }

    /// The live objects, in handle order, with their owners.
    pub fn handles(&self) -> Vec<(Handle, ObjectKind, Owner)> {
        self.objects.iter().map(|(h, e)| (h, e.kind, e.owner)).collect()
    }

    /// The kind of an object.
    pub fn kind_of(&self, handle: Handle) -> Result<ObjectKind> {
        Ok(self.objects.get(handle)?.kind)
    }

    /// The owner of an object.
    pub fn owner_of(&self, handle: Handle) -> Result<Owner> {
        Ok(self.objects.get(handle)?.owner)
    }

    // ---- calling ----------------------------------------------------------------

    /// Run a method (matched without regard to case) on an object.
    pub fn invoke(&mut self, handle: Handle, method: &str, args: &[Value]) -> Result<Value> {
        let (kind, owner) = {
            let e = self.objects.get(handle)?;
            (e.kind, e.owner)
        };
        if method.eq_ignore_ascii_case("Release") && args.is_empty() {
            self.release(handle)?;
            return Ok(Value::Nothing);
        }
        let mut obj = self.objects.get_mut(handle)?.obj.take().ok_or_else(|| GraphicsError::invalid(format!("that {} is busy: one of its own methods used its handle", kind.name())))?;
        let saved = self.current.replace((handle, owner));
        let result = obj.call(self, method, args);
        self.current = saved;
        let after = obj.memory_bytes();
        match self.objects.get_mut(handle) {
            Ok(e) => {
                self.memory = self.memory - e.memory + after;
                e.memory = after;
                e.obj = Some(obj);
            }
            Err(_) => {
                // The method released its own handle: the object goes with it.
                drop(obj);
            }
        }
        if result.is_ok() {
            self.limits.check_total(self.memory)?;
        }
        result
    }

    /// Run `f` on an object of the right type with the registry free for `f` to use the others.
    /// The generic helper behind the `with_*` methods.
    fn with_object<R>(&mut self, handle: Handle, want: ObjectKind, f: impl FnOnce(&mut Object, &mut Registry) -> Result<R>) -> Result<R> {
        let found = self.objects.get(handle)?.kind;
        if found != want {
            return Err(GraphicsError::WrongHandleKind { handle, found: found.name(), expected: want.name() });
        }
        let mut obj = self.objects.get_mut(handle)?.obj.take().ok_or_else(|| GraphicsError::invalid(format!("that {} is busy", want.name())))?;
        let r = f(&mut obj, self);
        let after = obj.memory_bytes();
        if let Ok(e) = self.objects.get_mut(handle) {
            self.memory = self.memory - e.memory + after;
            e.memory = after;
            e.obj = Some(obj);
        }
        r
    }

    /// Use a Canvas2D.
    pub fn with_canvas2d<R>(&mut self, handle: Handle, f: impl FnOnce(&mut Canvas2D, &mut Registry) -> Result<R>) -> Result<R> {
        self.with_object(handle, ObjectKind::Canvas2D, |o, r| match o {
            Object::Canvas2D(c) => f(c, r),
            _ => unreachable!("with_object checked the kind"),
        })
    }

    /// Use an ImageBuffer.
    #[cfg(feature = "imagebuf")]
    pub fn with_image<R>(&mut self, handle: Handle, f: impl FnOnce(&mut ImageBuffer, &mut Registry) -> Result<R>) -> Result<R> {
        self.with_object(handle, ObjectKind::ImageBuffer, |o, r| match o {
            Object::ImageBuffer(c) => f(c, r),
            _ => unreachable!("with_object checked the kind"),
        })
    }

    /// Use a Scene3D.
    #[cfg(feature = "scene3d-core")]
    pub fn with_scene3d<R>(&mut self, handle: Handle, f: impl FnOnce(&mut Scene3D, &mut Registry) -> Result<R>) -> Result<R> {
        self.with_object(handle, ObjectKind::Scene3D, |o, r| match o {
            Object::Scene3D(c) => f(c, r),
            _ => unreachable!("with_object checked the kind"),
        })
    }

    /// Use a Canvas3D.
    #[cfg(feature = "scene3d")]
    pub fn with_canvas3d<R>(&mut self, handle: Handle, f: impl FnOnce(&mut Canvas3D, &mut Registry) -> Result<R>) -> Result<R> {
        self.with_object(handle, ObjectKind::Canvas3D, |o, r| match o {
            Object::Canvas3D(c) => f(c, r),
            _ => unreachable!("with_object checked the kind"),
        })
    }

    /// The pixels of an ImageBuffer, shared (no copy).
    #[cfg(feature = "imagebuf")]
    pub fn image_pixels(&self, handle: Handle) -> Result<Arc<PixelBuffer>> {
        let e = self.objects.get(handle)?;
        match &e.obj {
            Some(Object::ImageBuffer(i)) => Ok(i.pixels().clone()),
            None if e.kind == ObjectKind::ImageBuffer => Err(GraphicsError::invalid("that ImageBuffer is busy")),
            _ => Err(GraphicsError::WrongHandleKind { handle, found: e.kind.name(), expected: "ImageBuffer" }),
        }
    }

    /// What a canvas may refer to by number while it draws: every ImageBuffer by handle, and the text engine.
    pub(crate) fn resources(&self) -> RegistryResources {
        #[allow(unused_mut)]
        let mut images = HashMap::new();
        #[cfg(feature = "imagebuf")]
        for (h, e) in self.objects.iter() {
            if let Some(Object::ImageBuffer(i)) = &e.obj {
                images.insert(h, i.pixels().clone());
            }
        }
        RegistryResources { images, engine: self.engine.clone() }
    }

    /// The methods of one kind, for documentation and tooling.
    pub fn methods(kind: ObjectKind) -> Vec<MethodInfo> {
        match kind {
            ObjectKind::Canvas2D => api::canvas2d::infos(),
            #[cfg(feature = "imagebuf")]
            ObjectKind::ImageBuffer => api::image::infos(),
            #[cfg(feature = "plot")]
            ObjectKind::Plot => api::plot::infos(),
            #[cfg(feature = "physics")]
            ObjectKind::Physics2D => api::physics2d::infos(),
            #[cfg(feature = "physics")]
            ObjectKind::Physics3D => api::physics3d::infos(),
            #[cfg(feature = "scene3d-core")]
            ObjectKind::Scene3D => api::scene3d::infos(),
            #[cfg(feature = "scene3d")]
            ObjectKind::Canvas3D => api::canvas3d::infos(),
            #[allow(unreachable_patterns)]
            _ => Vec::new(),
        }
    }

    /// Every method of every kind this build has.
    pub fn all_methods() -> Vec<MethodInfo> {
        ObjectKind::available().into_iter().flat_map(Self::methods).collect()
    }
}
