//! The centralized management of video sub-system.

use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use utils::{HashValue, Rect};
use resource::{Location, Registery, ResourceSystemShared};

use super::*;
use super::errors::*;
use super::backend::frame::*;
use super::backend::device::Device;
use super::command::Command;
use super::window::Window;

use super::assets::texture_loader::{TextureLoader, TextureParser, TextureState};
use super::assets::mesh_loader::{MeshLoader, MeshParser, MeshState};
use super::assets::shader::ShaderState;

/// The centralized management of video sub-system.
pub struct GraphicsSystem {
    window: Arc<Window>,
    device: Device,
    frames: Arc<DoubleFrame>,
    shared: Arc<GraphicsSystemShared>,

    last_dimensions: (u32, u32),
    last_hidpi: f32,
}

impl GraphicsSystem {
    /// Create a new `GraphicsSystem` with one `Window` context.
    pub fn new(window: Arc<window::Window>, resource: Arc<ResourceSystemShared>) -> Result<Self> {
        let device = unsafe { Device::new() };
        let frames = Arc::new(DoubleFrame::with_capacity(64 * 1024));

        let err = ErrorKind::WindowNotExist;
        let dimensions = window.dimensions().ok_or(err)?;

        let err = ErrorKind::WindowNotExist;
        let dimensions_in_pixels = window.dimensions_in_pixels().ok_or(err)?;

        let shared =
            GraphicsSystemShared::new(resource, frames.clone(), dimensions, dimensions_in_pixels);

        Ok(GraphicsSystem {
            last_dimensions: dimensions,
            last_hidpi: window.hidpi_factor(),

            window: window,
            device: device,
            frames: frames,
            shared: Arc::new(shared),
        })
    }

    /// Returns the multi-thread friendly parts of `GraphicsSystem`.
    pub fn shared(&self) -> Arc<GraphicsSystemShared> {
        self.shared.clone()
    }

    /// Swap internal commands frame.
    #[inline]
    pub fn swap_frames(&self) {
        self.frames.swap_frames();
    }

    /// Advance to next frame.
    ///
    /// Notes that this method MUST be called at main thread, and will NOT return
    /// until all commands is finished by GPU.
    pub fn advance(&mut self) -> Result<GraphicsFrameInfo> {
        use std::time;

        unsafe {
            let ts = time::Instant::now();

            let err = ErrorKind::WindowNotExist;
            let dimensions = self.window.dimensions().ok_or(err)?;

            let err = ErrorKind::WindowNotExist;
            let dimensions_in_pixels = self.window.dimensions_in_pixels().ok_or(err)?;

            let hidpi = self.window.hidpi_factor();

            // Resize the window, which would recreate the underlying framebuffer.
            if dimensions != self.last_dimensions || self.last_hidpi != hidpi {
                self.last_dimensions = dimensions;
                self.last_hidpi = hidpi;
                self.window.resize(dimensions);
            }

            *self.shared.dimensions.write().unwrap() = (dimensions, dimensions_in_pixels);

            {
                self.device.run_one_frame()?;

                {
                    let mut frame = self.frames.back();
                    frame.dispatch(&mut self.device, dimensions, hidpi)?;
                    frame.clear();
                }
            }

            self.window.swap_buffers()?;
            let mut info = GraphicsFrameInfo::default();
            {
                let v = self.device.frame_info();
                info.drawcall = v.drawcall;
                info.triangles = v.triangles;
            }

            {
                let s = &self.shared;
                info.alive_surfaces = Self::clear(&mut s.surfaces.write().unwrap());
                info.alive_shaders = Self::clear(&mut s.shaders.write().unwrap());
                info.alive_frame_buffers = Self::clear(&mut s.framebuffers.write().unwrap());
                info.alive_meshes = Self::clear(&mut s.meshes.write().unwrap());
                info.alive_textures = Self::clear(&mut s.textures.write().unwrap());
                info.alive_render_buffers = Self::clear(&mut s.render_buffers.write().unwrap());
            }

            info.duration = time::Instant::now() - ts;
            Ok(info)
        }
    }

    fn clear<T>(v: &mut Registery<T>) -> u32
    where
        T: Sized,
    {
        v.clear();
        v.len() as u32
    }
}

/// The multi-thread friendly parts of `GraphicsSystem`.
pub struct GraphicsSystemShared {
    resource: Arc<ResourceSystemShared>,
    frames: Arc<DoubleFrame>,
    dimensions: RwLock<((u32, u32), (u32, u32))>,

    surfaces: RwLock<Registery<()>>,
    shaders: RwLock<Registery<ShaderState>>,
    framebuffers: RwLock<Registery<()>>,
    render_buffers: RwLock<Registery<()>>,
    meshes: RwLock<Registery<Arc<RwLock<MeshState>>>>,
    textures: RwLock<Registery<Arc<RwLock<TextureState>>>>,
}

impl GraphicsSystemShared {
    /// Create a new `GraphicsSystem` with one `Window` context.
    fn new(
        resource: Arc<ResourceSystemShared>,
        frames: Arc<DoubleFrame>,
        dimensions: (u32, u32),
        dimensions_in_pixels: (u32, u32),
    ) -> Self {
        GraphicsSystemShared {
            resource: resource,
            frames: frames,
            dimensions: RwLock::new((dimensions, dimensions_in_pixels)),

            surfaces: RwLock::new(Registery::new()),
            shaders: RwLock::new(Registery::new()),
            framebuffers: RwLock::new(Registery::new()),
            render_buffers: RwLock::new(Registery::new()),
            meshes: RwLock::new(Registery::new()),
            textures: RwLock::new(Registery::new()),
        }
    }

    /// Returns the size in points of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    #[inline]
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions.read().unwrap().0
    }

    /// Returns the size in pixels of the client area of the window.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    /// These are the dimensions of the frame buffer.
    #[inline]
    pub fn dimensions_in_pixels(&self) -> (u32, u32) {
        self.dimensions.read().unwrap().1
    }

    /// Submit a task into named bucket.
    ///
    /// Tasks inside bucket will be executed in sequential order.
    pub fn submit<'a, T1, T2>(&self, s: SurfaceHandle, o: T1, task: T2) -> Result<()>
    where
        T1: Into<u64>,
        T2: Into<Command<'a>>,
    {
        if !self.surfaces.read().unwrap().is_alive(s.into()) {
            bail!("Undefined surface handle.");
        }

        let o = o.into();
        match task.into() {
            Command::DrawCall(dc) => self.submit_drawcall(s, o, dc),
            Command::VertexBufferUpdate(vbu) => self.submit_update_vertex_buffer(s, o, vbu),
            Command::IndexBufferUpdate(ibu) => self.submit_update_index_buffer(s, o, ibu),
            Command::TextureUpdate(tu) => self.submit_update_texture(s, o, tu),
            Command::SetScissor(sc) => self.submit_set_scissor(s, o, sc),
        }
    }

    fn submit_drawcall<'a>(
        &self,
        surface: SurfaceHandle,
        order: u64,
        dc: command::SliceDrawCall<'a>,
    ) -> Result<()> {
        if !self.meshes.read().unwrap().is_alive(dc.mesh.into()) {
            bail!("Undefined mesh handle.");
        }

        let mut frame = self.frames.front();
        let uniforms = {
            let mut pack = Vec::new();
            if let Some(shader) = self.shaders.read().unwrap().get(dc.shader.into()) {
                for &(n, v) in dc.uniforms {
                    if let Some(&tt) = shader.uniform_variables.get(&n) {
                        if tt == v.variable_type() {
                            pack.push((n, frame.buf.extend(&v)));
                        } else {
                            let name = &shader.uniform_variable_names[&n];
                            bail!(format!("Unmatched uniform variable: {:?}.", name));
                        }
                    } else {
                        let name = &shader.uniform_variable_names[&n];
                        bail!(format!("Undefined uniform variable: {:?}.", name));
                    }
                }
            } else {
                bail!("Undefined shader state handle.");
            }

            frame.buf.extend_from_slice(&pack)
        };

        let dc = FrameDrawCall {
            shader: dc.shader,
            uniforms: uniforms,
            mesh: dc.mesh,
            index: dc.index,
        };

        frame.tasks.push((surface, order, FrameTask::DrawCall(dc)));
        Ok(())
    }

    fn submit_set_scissor(
        &self,
        surface: SurfaceHandle,
        order: u64,
        su: command::ScissorUpdate,
    ) -> Result<()> {
        if !self.surfaces.read().unwrap().is_alive(surface.into()) {
            bail!("Undefined surface handle.");
        }

        let mut frame = self.frames.front();
        let task = FrameTask::UpdateSurface(su.scissor);
        frame.tasks.push((surface, order, task));
        Ok(())
    }

    fn submit_update_vertex_buffer(
        &self,
        surface: SurfaceHandle,
        order: u64,
        vbu: command::VertexBufferUpdate,
    ) -> Result<()> {
        if !self.surfaces.read().unwrap().is_alive(surface.into()) {
            bail!("Undefined surface handle.");
        }

        if self.meshes.read().unwrap().is_alive(vbu.mesh.into()) {
            let mut frame = self.frames.front();
            let ptr = frame.buf.extend_from_slice(vbu.data);
            let task = FrameTask::UpdateVertexBuffer(vbu.mesh, vbu.offset, ptr);
            frame.tasks.push((surface, order, task));
            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }

    fn submit_update_index_buffer(
        &self,
        surface: SurfaceHandle,
        order: u64,
        ibu: command::IndexBufferUpdate,
    ) -> Result<()> {
        if !self.surfaces.read().unwrap().is_alive(surface.into()) {
            bail!("Undefined surface handle.");
        }

        if self.meshes.read().unwrap().is_alive(ibu.mesh.into()) {
            let mut frame = self.frames.front();
            let ptr = frame.buf.extend_from_slice(ibu.data);
            let task = FrameTask::UpdateIndexBuffer(ibu.mesh, ibu.offset, ptr);
            frame.tasks.push((surface, order, task));
            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }

    fn submit_update_texture(
        &self,
        surface: SurfaceHandle,
        order: u64,
        tu: command::TextureUpdate,
    ) -> Result<()> {
        if !self.surfaces.read().unwrap().is_alive(surface.into()) {
            bail!("Undefined surface handle.");
        }

        if let Some(state) = self.textures.read().unwrap().get(tu.texture.into()) {
            if TextureState::Ready == *state.read().unwrap() {
                let mut frame = self.frames.front();
                let ptr = frame.buf.extend_from_slice(tu.data);
                let task = FrameTask::UpdateTexture(tu.texture, tu.rect, ptr);
                frame.tasks.push((surface, order, task));
            }

            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }
}

impl GraphicsSystemShared {
    /// Creates an view with `SurfaceSetup`.
    pub fn create_surface(&self, setup: SurfaceSetup) -> Result<SurfaceHandle> {
        let location = Location::unique("");
        let handle = self.surfaces.write().unwrap().create(location, ()).into();

        {
            let task = PreFrameTask::CreateSurface(handle, setup);
            self.frames.front().pre.push(task);
        }

        Ok(handle)
    }

    /// Delete surface object.
    pub fn delete_surface(&self, handle: SurfaceHandle) {
        if self.surfaces
            .write()
            .unwrap()
            .dec_rc(handle.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeleteSurface(handle);
            self.frames.front().post.push(task);
        }
    }

    /// Lookup shader object from location.
    pub fn lookup_shader_from(&self, location: Location) -> Option<ShaderHandle> {
        self.shaders
            .read()
            .unwrap()
            .lookup(location)
            .map(|v| v.into())
    }

    /// Create a shader with initial shaders and render state. Pipeline encapusulate
    /// all the informations we need to configurate OpenGL before real drawing.
    pub fn create_shader(&self, location: Location, setup: ShaderSetup) -> Result<ShaderHandle> {
        if setup.uniform_variables.len() > MAX_UNIFORM_VARIABLES {
            bail!(
                "Too many uniform variables (>= {:?}).",
                MAX_UNIFORM_VARIABLES
            );
        }

        if setup.vs.len() == 0 {
            bail!("Vertex shader is required to describe a proper render pipeline.");
        }

        if setup.fs.len() == 0 {
            bail!("Fragment shader is required to describe a proper render pipeline.");
        }

        let handle = {
            let mut shaders = self.shaders.write().unwrap();
            if let Some(handle) = shaders.lookup(location) {
                shaders.inc_rc(handle);
                return Ok(handle.into());
            }

            let mut uniform_variable_names = HashMap::new();
            let mut uniform_variables = HashMap::new();
            for (name, v) in &setup.uniform_variables {
                let k: HashValue<str> = name.into();
                uniform_variables.insert(k, *v);
                uniform_variable_names.insert(k, name.clone());
            }

            let shader_state = ShaderState {
                render_state: setup.render_state,
                layout: setup.layout,
                uniform_variables: uniform_variables,
                uniform_variable_names: uniform_variable_names,
            };

            let handle = shaders.create(location, shader_state).into();
            handle
        };

        let task = PreFrameTask::CreatePipeline(handle, setup);
        self.frames.front().pre.push(task);
        Ok(handle)
    }

    /// Gets the shader state if exists.
    pub fn shader_state(&self, handle: ShaderHandle) -> Option<ShaderState> {
        self.shaders.read().unwrap().get(*handle).map(|v| v.clone())
    }

    /// Returns true if shader is exists.
    pub fn is_shader_alive(&self, handle: ShaderHandle) -> bool {
        self.shaders.read().unwrap().is_alive(handle.into())
    }

    /// Delete shader state object.
    pub fn delete_shader(&self, handle: ShaderHandle) {
        if self.shaders
            .write()
            .unwrap()
            .dec_rc(handle.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeletePipeline(handle);
            self.frames.front().post.push(task);
        }
    }

    /// Create a framebuffer object. A framebuffer allows you to render primitives directly to a texture,
    /// which can then be used in other rendering operations.
    ///
    /// At least one color attachment has been attached before you can use it.
    pub fn create_framebuffer(&self, setup: FrameBufferSetup) -> Result<FrameBufferHandle> {
        let location = Location::unique("");
        let handle = self.framebuffers
            .write()
            .unwrap()
            .create(location, ())
            .into();

        {
            let task = PreFrameTask::CreateFrameBuffer(handle, setup);
            self.frames.front().pre.push(task);
        }

        Ok(handle)
    }

    /// Delete frame buffer object.
    pub fn delete_framebuffer(&self, handle: FrameBufferHandle) {
        if self.framebuffers
            .write()
            .unwrap()
            .dec_rc(handle.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeleteFrameBuffer(handle);
            self.frames.front().post.push(task);
        }
    }

    /// Create a render buffer object, which could be attached to framebuffer.
    pub fn create_render_buffer(&self, setup: RenderBufferSetup) -> Result<RenderBufferHandle> {
        let location = Location::unique("");
        let handle = self.render_buffers
            .write()
            .unwrap()
            .create(location, ())
            .into();

        {
            let task = PreFrameTask::CreateRenderBuffer(handle, setup);
            self.frames.front().pre.push(task);
        }

        Ok(handle)
    }

    /// Delete frame buffer object.
    pub fn delete_render_buffer(&self, handle: RenderBufferHandle) {
        if self.render_buffers
            .write()
            .unwrap()
            .dec_rc(handle.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeleteRenderBuffer(handle);
            self.frames.front().post.push(task);
        }
    }
}

impl GraphicsSystemShared {
    /// Lookup mesh object from location.
    pub fn lookup_mesh_from(&self, location: Location) -> Option<MeshHandle> {
        self.meshes
            .read()
            .unwrap()
            .lookup(location)
            .map(|v| v.into())
    }

    /// Create a new mesh object from location.
    pub fn create_mesh_from<T>(&self, location: Location, setup: MeshSetup) -> Result<MeshHandle>
    where
        T: MeshParser + Send + Sync + 'static,
    {
        let (handle, state) = {
            let mut meshes = self.meshes.write().unwrap();
            if let Some(handle) = meshes.lookup(location) {
                meshes.inc_rc(handle);
                return Ok(handle.into());
            }

            let state = Arc::new(RwLock::new(MeshState::NotReady));
            let handle = meshes.create(location, state.clone()).into();
            (handle, state)
        };

        let loader = MeshLoader::<T>::new(handle, state, setup, self.frames.clone());
        self.resource.load_async(loader, location.uri());
        Ok(handle)
    }

    /// Create a new mesh object.
    pub fn create_mesh<'a, 'b, T1, T2>(
        &self,
        location: Location,
        setup: MeshSetup,
        verts: T1,
        idxes: T2,
    ) -> Result<MeshHandle>
    where
        T1: Into<Option<&'a [u8]>>,
        T2: Into<Option<&'b [u8]>>,
    {
        let verts = verts.into();
        let idxes = idxes.into();

        if let Some(buf) = verts.as_ref() {
            if buf.len() > setup.vertex_buffer_len() {
                bail!("Out of bounds!");
            }
        }

        if let Some(buf) = idxes.as_ref() {
            if buf.len() > setup.index_buffer_len() {
                bail!("Out of bounds!");
            }
        }

        setup.validate()?;
        
        let handle = {
            let mut meshes = self.meshes.write().unwrap();
            if let Some(handle) = meshes.lookup(location) {
                meshes.inc_rc(handle);
                return Ok(handle.into());
            }

            let state = Arc::new(RwLock::new(MeshState::Ready));
            let handle = meshes.create(location, state).into();
            handle
        };

        let mut frame = self.frames.front();
        let verts_ptr = verts.map(|v| frame.buf.extend_from_slice(v));
        let idxes_ptr = idxes.map(|v| frame.buf.extend_from_slice(v));
        let task = PreFrameTask::CreateMesh(handle, setup, verts_ptr, idxes_ptr);
        frame.pre.push(task);
        Ok(handle)
    }

    /// Update a subset of dynamic vertex buffer. Use `offset` specifies the offset
    /// into the buffer object's data store where data replacement will begin, measured
    /// in bytes.
    pub fn update_vertex_buffer(&self, mesh: MeshHandle, offset: usize, data: &[u8]) -> Result<()> {
        if self.meshes.read().unwrap().is_alive(mesh.into()) {
            let mut frame = self.frames.front();
            let ptr = frame.buf.extend_from_slice(data);
            let task = PreFrameTask::UpdateVertexBuffer(mesh, offset, ptr);
            frame.pre.push(task);
            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }

    /// Update a subset of dynamic index buffer. Use `offset` specifies the offset
    /// into the buffer object's data store where data replacement will begin, measured
    /// in bytes.
    pub fn update_index_buffer(&self, mesh: MeshHandle, offset: usize, data: &[u8]) -> Result<()> {
        if self.meshes.read().unwrap().is_alive(mesh.into()) {
            let mut frame = self.frames.front();
            let ptr = frame.buf.extend_from_slice(data);
            let task = PreFrameTask::UpdateIndexBuffer(mesh, offset, ptr);
            frame.pre.push(task);
            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }

    /// Delete mesh object.
    pub fn delete_mesh(&self, mesh: MeshHandle) {
        if self.meshes
            .write()
            .unwrap()
            .dec_rc(mesh.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeleteMesh(mesh);
            self.frames.front().post.push(task);
        }
    }
}

impl GraphicsSystemShared {
    /// Lookup texture object from location.
    pub fn lookup_texture_from(&self, location: Location) -> Option<TextureHandle> {
        self.textures
            .read()
            .unwrap()
            .lookup(location)
            .map(|v| v.into())
    }

    /// Create texture object from location.
    pub fn create_texture_from<T>(
        &self,
        location: Location,
        setup: TextureSetup,
    ) -> Result<TextureHandle>
    where
        T: TextureParser + Send + Sync + 'static,
    {
        let (handle, state) = {
            let mut textures = self.textures.write().unwrap();
            if let Some(handle) = textures.lookup(location) {
                textures.inc_rc(handle);
                return Ok(handle.into());
            }

            let state = Arc::new(RwLock::new(TextureState::NotReady));
            let handle = textures.create(location, state.clone()).into();
            (handle, state)
        };
    
        let loader = TextureLoader::<T>::new(handle, state, setup, self.frames.clone());
        self.resource.load_async(loader, location.uri());
        Ok(handle)
    }

    /// Create texture object. A texture is an image loaded in video memory,
    /// which can be sampled in shaders.
    pub fn create_texture<'a, T>(
        &self,
        location: Location,
        setup: TextureSetup,
        data: T,
    ) -> Result<TextureHandle>
    where
        T: Into<Option<&'a [u8]>>,
    {
        let handle = {
            let mut textures = self.textures.write().unwrap();
            if let Some(handle) = textures.lookup(location) {
                textures.inc_rc(handle);
                return Ok(handle.into());
            }

            let state = Arc::new(RwLock::new(TextureState::Ready));
            textures.create(location, state).into()
        };
    
        let mut frame = self.frames.front();
        let ptr = data.into().map(|v| frame.buf.extend_from_slice(v));
        let task = PreFrameTask::CreateTexture(handle, setup, ptr);
        frame.pre.push(task);
        Ok(handle)
    }

    /// Create render texture object, which could be attached with a framebuffer.
    pub fn create_render_texture(
        &self,
        setup: RenderTextureSetup,
    ) -> Result<TextureHandle> {
        let location = Location::unique("");
        let state = Arc::new(RwLock::new(TextureState::Ready));
        let handle = self.textures
            .write()
            .unwrap()
            .create(location, state)
            .into();

        {
            let task = PreFrameTask::CreateRenderTexture(handle, setup);
            self.frames.front().pre.push(task);
        }

        Ok(handle)
    }

    /// Update the texture object.
    ///
    /// Notes that this method might fails without any error when the texture is not
    /// ready for operating.
    pub fn update_texture(&self, texture: TextureHandle, rect: Rect, data: &[u8]) -> Result<()> {
        if let Some(state) = self.textures.read().unwrap().get(texture.into()) {
            if TextureState::Ready == *state.read().unwrap() {
                let mut frame = self.frames.front();
                let ptr = frame.buf.extend_from_slice(data);
                let task = PreFrameTask::UpdateTexture(texture, rect, ptr);
                frame.pre.push(task);
            }

            Ok(())
        } else {
            bail!(ErrorKind::InvalidHandle);
        }
    }

    /// Delete the texture object.
    pub fn delete_texture(&self, handle: TextureHandle) {
        if self.textures
            .write()
            .unwrap()
            .dec_rc(handle.into(), true)
            .is_some()
        {
            let task = PostFrameTask::DeleteTexture(handle);
            self.frames.front().post.push(task);
        }
    }
}
