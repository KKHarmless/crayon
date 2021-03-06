use std::sync::{Mutex, MutexGuard, RwLock};

use super::super::*;
use super::errors::*;
use super::device::Device;

use utils::{DataBuffer, DataBufferPtr, HashValue, Rect};

#[derive(Debug, Clone)]
pub(crate) enum PreFrameTask {
    CreateSurface(SurfaceHandle, SurfaceSetup),
    CreatePipeline(ShaderHandle, ShaderSetup),
    CreateFrameBuffer(FrameBufferHandle, FrameBufferSetup),
    CreateTexture(TextureHandle, TextureSetup, Option<DataBufferPtr<[u8]>>),
    UpdateTexture(TextureHandle, Rect, DataBufferPtr<[u8]>),
    CreateRenderTexture(TextureHandle, RenderTextureSetup),
    CreateRenderBuffer(RenderBufferHandle, RenderBufferSetup),
    CreateMesh(
        MeshHandle,
        MeshSetup,
        Option<DataBufferPtr<[u8]>>,
        Option<DataBufferPtr<[u8]>>,
    ),
    UpdateVertexBuffer(MeshHandle, usize, DataBufferPtr<[u8]>),
    UpdateIndexBuffer(MeshHandle, usize, DataBufferPtr<[u8]>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FrameTask {
    DrawCall(FrameDrawCall),
    UpdateSurface(Scissor),
    UpdateVertexBuffer(MeshHandle, usize, DataBufferPtr<[u8]>),
    UpdateIndexBuffer(MeshHandle, usize, DataBufferPtr<[u8]>),
    UpdateTexture(TextureHandle, Rect, DataBufferPtr<[u8]>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameDrawCall {
    pub shader: ShaderHandle,
    pub uniforms: DataBufferPtr<[(HashValue<str>, DataBufferPtr<UniformVariable>)]>,
    pub mesh: MeshHandle,
    pub index: MeshIndex,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PostFrameTask {
    DeleteSurface(SurfaceHandle),
    DeletePipeline(ShaderHandle),
    DeleteMesh(MeshHandle),
    DeleteTexture(TextureHandle),
    DeleteRenderBuffer(RenderBufferHandle),
    DeleteFrameBuffer(FrameBufferHandle),
}

#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub pre: Vec<PreFrameTask>,
    pub tasks: Vec<(SurfaceHandle, u64, FrameTask)>,
    pub post: Vec<PostFrameTask>,
    pub buf: DataBuffer,
}

unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    /// Creates a new frame with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Frame {
            pre: Vec::new(),
            post: Vec::new(),
            tasks: Vec::new(),
            buf: DataBuffer::with_capacity(capacity),
        }
    }

    /// Clear the frame, removing all data.
    pub unsafe fn clear(&mut self) {
        self.pre.clear();
        self.tasks.clear();
        self.post.clear();
        self.buf.clear();
    }

    /// Dispatch frame tasks and draw calls to the backend context.
    pub unsafe fn dispatch(
        &mut self,
        device: &mut Device,
        dimensions: (u32, u32),
        hidpi: f32,
    ) -> Result<()> {
        for v in self.pre.drain(..) {
            match v {
                PreFrameTask::CreateSurface(handle, setup) => {
                    device.create_surface(handle, setup)?;
                }
                PreFrameTask::CreatePipeline(handle, setup) => {
                    device.create_shader(handle, setup)?;
                }
                PreFrameTask::CreateMesh(handle, setup, verts, idxes) => {
                    let field = &self.buf;
                    let verts = verts.map(|v| field.as_slice(v));
                    let idxes = idxes.map(|v| field.as_slice(v));
                    device.create_mesh(handle, setup, verts, idxes)?;
                }
                PreFrameTask::UpdateVertexBuffer(handle, offset, data) => {
                    let data = self.buf.as_slice(data);
                    device.update_vertex_buffer(handle, offset, data)?;
                }
                PreFrameTask::UpdateIndexBuffer(handle, offset, data) => {
                    let data = self.buf.as_slice(data);
                    device.update_index_buffer(handle, offset, data)?;
                }
                PreFrameTask::CreateTexture(handle, setup, data) => {
                    let field = &self.buf;
                    let buf = data.map(|v| field.as_slice(v));
                    device.create_texture(handle, setup, buf)?;
                }
                PreFrameTask::UpdateTexture(handle, rect, data) => {
                    let data = self.buf.as_slice(data);
                    device.update_texture(handle, rect, data)?;
                }
                PreFrameTask::CreateRenderTexture(handle, setup) => {
                    device.create_render_texture(handle, setup)?;
                }
                PreFrameTask::CreateRenderBuffer(handle, setup) => {
                    device.create_render_buffer(handle, setup)?;
                }
                PreFrameTask::CreateFrameBuffer(handle, setup) => {
                    device.create_framebuffer(handle)?;

                    // Update framebuffer's attachments.
                    for (i, attachment) in setup.attachments().iter().enumerate() {
                        if let &Some(v) = attachment {
                            let i = i as u32;
                            match v {
                                FrameBufferAttachment::RenderBuffer(rb) => {
                                    device.update_framebuffer_with_renderbuffer(handle, rb, i)?;
                                }
                                FrameBufferAttachment::Texture(texture) => {
                                    device.update_framebuffer_with_texture(handle, texture, i)?;
                                }
                            };
                        }
                    }
                }
            }
        }

        device.flush(&mut self.tasks, &self.buf, dimensions, hidpi)?;

        for v in self.post.drain(..) {
            match v {
                PostFrameTask::DeleteSurface(handle) => {
                    device.delete_surface(handle)?;
                }
                PostFrameTask::DeletePipeline(handle) => {
                    device.delete_shader(handle)?;
                }
                PostFrameTask::DeleteMesh(handle) => {
                    device.delete_mesh(handle)?;
                }
                PostFrameTask::DeleteTexture(handle) => {
                    device.delete_texture(handle)?;
                }
                PostFrameTask::DeleteRenderBuffer(handle) => {
                    device.delete_render_buffer(handle)?;
                }
                PostFrameTask::DeleteFrameBuffer(handle) => {
                    device.delete_framebuffer(handle)?;
                }
            }
        }

        Ok(())
    }
}

pub(crate) struct DoubleFrame {
    idx: RwLock<usize>,
    frames: [Mutex<Frame>; 2],
}

impl DoubleFrame {
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleFrame {
            idx: RwLock::new(0),
            frames: [
                Mutex::new(Frame::with_capacity(capacity)),
                Mutex::new(Frame::with_capacity(capacity)),
            ],
        }
    }

    #[inline]
    pub fn front(&self) -> MutexGuard<Frame> {
        self.frames[*self.idx.read().unwrap()].lock().unwrap()
    }

    #[inline]
    pub fn back(&self) -> MutexGuard<Frame> {
        self.frames[(*self.idx.read().unwrap() + 1) % 2]
            .lock()
            .unwrap()
    }

    #[inline]
    pub fn swap_frames(&self) {
        let mut idx = self.idx.write().unwrap();
        *idx = (*idx + 1) % 2;
    }
}
