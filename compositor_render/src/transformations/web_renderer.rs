use bytes::{Bytes, BytesMut};
use nalgebra_glm::Mat4;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::renderer::render_graph::NodeId;
use crate::renderer::{RegisterCtx, RenderCtx};
use crate::wgpu::shader::shader_params::ParamsBuffer;
use crate::wgpu::shader::{CreateShaderError, WgpuShader};
use crate::wgpu::texture::{BGRATexture, NodeTexture, NodeTextureState, RGBATexture, Texture};

use crate::transformations::web_renderer::browser_client::BrowserClient;
use crate::transformations::web_renderer::chromium_sender::ChromiumSender;
use crate::transformations::web_renderer::embedder::{EmbedError, EmbeddingHelper, TextureInfo};
use compositor_common::renderer_spec::WebEmbeddingMethod;
use compositor_common::{
    renderer_spec::{FallbackStrategy, WebRendererSpec},
    scene::{constraints::NodeConstraints, Resolution},
};
use log::{error, info};

pub mod browser_client;
pub mod chromium_context;
mod chromium_sender;
mod chromium_sender_thread;
mod embedder;
pub(crate) mod node;
mod shared_memory;

pub const EMBED_SOURCE_FRAMES_MESSAGE: &str = "EMBED_SOURCE_FRAMES";
pub const UNEMBED_SOURCE_FRAMES_MESSAGE: &str = "UNEMBED_SOURCE_FRAMES";
pub const GET_FRAME_POSITIONS_MESSAGE: &str = "GET_FRAME_POSITIONS";

pub(super) type FrameData = Arc<Mutex<Bytes>>;
pub(super) type SourceTransforms = Arc<Mutex<Vec<Mat4>>>;

pub struct WebRendererOptions {
    pub init: bool,
    pub disable_gpu: bool,
}

impl Default for WebRendererOptions {
    fn default() -> Self {
        Self {
            init: true,
            disable_gpu: false,
        }
    }
}

#[derive(Debug)]
pub struct WebRenderer {
    spec: WebRendererSpec,
    frame_data: FrameData,
    source_transforms: SourceTransforms,
    embedding_helper: EmbeddingHelper,

    website_texture: BGRATexture,
    render_website_shader: WgpuShader,
    shader_params: Mutex<ParamsBuffer>,
}

impl WebRenderer {
    pub fn new(ctx: &RegisterCtx, spec: WebRendererSpec) -> Result<Self, CreateWebRendererError> {
        info!("Starting web renderer for {}", &spec.url);

        let frame_data = Arc::new(Mutex::new(Bytes::new()));
        let source_transforms = Arc::new(Mutex::new(Vec::new()));

        let client = BrowserClient::new(
            frame_data.clone(),
            source_transforms.clone(),
            spec.resolution,
        );
        let chromium_sender = ChromiumSender::new(ctx, spec.url.clone(), client);
        let embedding_helper = EmbeddingHelper::new(ctx, chromium_sender, spec.embedding_method);

        let render_website_shader = WgpuShader::new(
            &ctx.wgpu_ctx,
            include_str!("web_renderer/render_website.wgsl").into(),
        )?;
        let shader_params = Mutex::new(ParamsBuffer::new(Bytes::new(), &ctx.wgpu_ctx));
        let website_texture = BGRATexture::new(&ctx.wgpu_ctx, spec.resolution);

        Ok(Self {
            spec,
            frame_data,
            source_transforms,
            embedding_helper,
            website_texture,
            render_website_shader,
            shader_params,
        })
    }

    pub fn render(
        &self,
        ctx: &RenderCtx,
        node_id: &NodeId,
        sources: &[(&NodeId, &NodeTexture)],
        buffers: &[Arc<wgpu::Buffer>],
        target: &mut NodeTexture,
    ) -> Result<(), RenderWebsiteError> {
        self.embedding_helper
            .prepare_embedding(node_id, sources, buffers)
            .map_err(|err| RenderWebsiteError::EmbeddingFailed(self.spec.url.clone(), err))?;

        if let Some(frame) = self.retrieve_frame() {
            let target = target.ensure_size(ctx.wgpu_ctx, self.spec.resolution);
            self.website_texture.upload(ctx.wgpu_ctx, &frame);

            let (textures, textures_info) = self.prepare_textures(sources);
            let mut shader_params = self.shader_params.lock().unwrap();
            shader_params.update(textures_info, ctx.wgpu_ctx);

            self.render_website_shader.render_with_textures(
                shader_params.bind_group(),
                &textures,
                target,
                Default::default(),
                None,
            );
        }

        Ok(())
    }

    fn prepare_textures<'a>(
        &'a self,
        sources: &'a [(&NodeId, &NodeTexture)],
    ) -> (Vec<Option<&'a Texture>>, Bytes) {
        let source_transforms = TextureInfo::sources(&self.source_transforms.lock().unwrap());
        let mut source_textures = sources
            .iter()
            .map(|(_, texture)| {
                texture
                    .state()
                    .map(NodeTextureState::rgba_texture)
                    .map(RGBATexture::texture)
            })
            .collect::<Vec<_>>();

        let mut textures = Vec::new();
        match self.spec.embedding_method {
            WebEmbeddingMethod::NativeEmbeddingOverContent => {
                textures.push(Some(self.website_texture.texture()));
                textures.append(&mut source_textures);
            }
            WebEmbeddingMethod::NativeEmbeddingUnderContent => {
                textures.append(&mut source_textures);
                textures.push(Some(self.website_texture.texture()));
            }
            WebEmbeddingMethod::ChromiumEmbedding => {
                textures.push(Some(self.website_texture.texture()));
            }
        };

        let mut textures_info = BytesMut::new();
        match self.spec.embedding_method {
            WebEmbeddingMethod::NativeEmbeddingOverContent => {
                textures_info.extend(TextureInfo::website());
                textures_info.extend(source_transforms);
            }
            WebEmbeddingMethod::NativeEmbeddingUnderContent => {
                textures_info.extend(source_transforms);
                textures_info.extend(TextureInfo::website());
            }
            WebEmbeddingMethod::ChromiumEmbedding => textures_info.extend(TextureInfo::website()),
        };

        (textures, textures_info.freeze())
    }

    fn retrieve_frame(&self) -> Option<Bytes> {
        let frame_data = self.frame_data.lock().unwrap();
        if frame_data.is_empty() {
            return None;
        }
        Some(frame_data.clone())
    }

    pub fn resolution(&self) -> Resolution {
        self.spec.resolution
    }

    pub fn shared_memory_root_path(renderer_id: &str) -> PathBuf {
        env::temp_dir()
            .join("video_compositor")
            .join(format!("instance_{}", renderer_id))
    }

    pub fn fallback_strategy(&self) -> FallbackStrategy {
        self.spec.fallback_strategy
    }

    pub fn constraints(&self) -> &NodeConstraints {
        &self.spec.constraints
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreateWebRendererError {
    #[error(transparent)]
    CreateShaderFailed(#[from] CreateShaderError),
}

#[derive(Debug, thiserror::Error)]
pub enum RenderWebsiteError {
    #[error("Failed to embed source on the website \"{0}\": {1}")]
    EmbeddingFailed(String, #[source] EmbedError),
}
