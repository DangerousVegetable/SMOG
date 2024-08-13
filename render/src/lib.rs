use std::num::NonZeroU32;

use bevy::{
    core_pipeline::core_2d::Transparent2d,
    ecs::{
        query::{QueryItem, ROQueryItem},
        system::{
            lifetimeless::Read,
            SystemParamItem,
        },
    },
    math::{vec2, FloatOrd},
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin}, render_asset::RenderAssets, render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex,
            RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
        }, render_resource::{
            binding_types::{sampler, texture_2d},
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, Buffer,
            BufferUsages, ColorTargetState, ColorWrites,
            FragmentState, MultisampleState, PipelineCache, PrimitiveState,
            RawBufferVec, RenderPipelineDescriptor, SpecializedRenderPipeline,
            SpecializedRenderPipelines, TextureFormat, VertexState,
        }, renderer::{RenderDevice, RenderQueue}, texture::{BevyDefault as _, GpuImage}, view::{ExtractedView}, MainWorld, Render, RenderApp, RenderSet
    },
};

pub mod particle;
mod vertex;

use solver::Solver;
use vertex::Vertex;
use wgpu::{SamplerBindingType, ShaderStages, TextureSampleType};

/// A marker component that represents an entity that is to be rendered using
/// our custom phase item.
///
/// Note the [`ExtractComponent`] trait implementation. This is necessary to
/// tell Bevy that this object should be pulled into the render world.
#[derive(Component)]
pub struct RenderedSimulation(pub Solver);

#[derive(Clone, Component, ExtractComponent)]
pub struct SimulationCamera;

/// Holds a reference to our shader.
///
/// This is loaded at app creation time.
#[derive(Resource)]
struct SimulationPipeline {
    shader: Handle<Shader>,
    uniforms_bind_group_layout: BindGroupLayout,
    textures_bind_group_layout: BindGroupLayout,
}

/// A [`RenderCommand`] that binds the vertex and index buffers and issues the
/// draw command for our custom phase item.
struct DrawSimulation;

impl<P> RenderCommand<P> for DrawSimulation
where
    P: PhaseItem,
{
    type Param = ();

    type ViewQuery = Read<ExtractedView>;

    type ItemQuery = Read<SimulationBuffers>;

    fn render<'w>(
        _: &P,
        _extracted_view: ROQueryItem<'w, Self::ViewQuery>,
        simulation_buffers: Option<&'w SimulationBuffers>,
        _: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(simulation_buffers) = simulation_buffers else {
            return RenderCommandResult::Failure;
        };

        if simulation_buffers.particles.len() == 0 {
            return RenderCommandResult::Success;
        }

        pass.set_bind_group(0, &simulation_buffers.uniforms_bind_group, &[]);
        pass.set_bind_group(1, &simulation_buffers.textures_bind_group, &[]);
        pass.set_vertex_buffer(0, simulation_buffers.vertices.slice(..));
        pass.set_vertex_buffer(1, simulation_buffers.particles.buffer().unwrap().slice(..));
        pass.set_index_buffer(
            simulation_buffers.indices.slice(..),
            0,
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..6, 0, 0..simulation_buffers.particles.len() as u32);

        RenderCommandResult::Success
    }
}

/// The GPU vertex and index buffers for our custom phase item.
///
/// As the custom phase item is a single triangle, these are uploaded once and
/// then left alone.
#[derive(Component)]
struct SimulationBuffers {
    // particles vertex buffer
    vertices: Buffer,

    // particles instance buffer
    particles: RawBufferVec<particle::Raw>,

    // particles index buffer
    indices: Buffer,

    // uniform bind group
    uniforms_bind_group: BindGroup,
    uniforms: Buffer,

    // textures bind group
    textures_bind_group: BindGroup,
}

#[derive(Component)]
struct SimulationBackground;

/// The custom draw commands that Bevy executes for each entity we enqueue into
/// the render phase.
type DrawSimulationCommands = (SetItemPipeline, DrawSimulation);

impl ExtractComponent for RenderedSimulation {
    type QueryData = &'static RenderedSimulation;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(RenderedSimulation(item.0.clone()))
    }
}

fn update_simulation_background(
    mut commands: Commands,
    query: Query<(Entity, &RenderedSimulation), Without<SimulationBackground>>,
) {
    for (entity, simulation) in &query {
        let (bl, tr) = simulation.0.constraint.bounds();
        let size = vec2(tr.x - bl.x, tr.y - bl.y);
        let pos = bl + size/2.;
        let sprite_bundle = SpriteBundle {
            sprite: Sprite {
                custom_size: Some(size),
                ..default()
            },
            visibility: Visibility::Hidden,
            transform: Transform::from_translation(pos.extend(-2.)),
            ..default()
        };
        commands.entity(entity).insert(SimulationBackground);
        commands.spawn(sprite_bundle)
            .insert(SimulationBackground);
    }
}
pub struct RenderSimulationPlugin;

impl Plugin for RenderSimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GpuFeatureSupportChecker)
            .add_plugins(ExtractComponentPlugin::<RenderedSimulation>::default())
            .add_plugins(ExtractComponentPlugin::<SimulationCamera>::default())
            .add_systems(Update, update_simulation_background);
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<SimulationTextures>()
            .init_resource::<SimulationPipeline>()
            .init_resource::<SpecializedRenderPipelines<SimulationPipeline>>()
            .add_render_command::<Transparent2d, DrawSimulationCommands>()
            .add_systems(
                Render,
                (prepare_simulation_buffers.run_if(textures_prepared))
                    .in_set(RenderSet::PrepareResources),
            )
            .add_systems(Render, queue_simulation.in_set(RenderSet::Queue))
            .add_systems(ExtractSchedule, update_simulation_textures);
    }
}

struct GpuFeatureSupportChecker;

impl Plugin for GpuFeatureSupportChecker {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        let render_device = render_app.world().resource::<RenderDevice>();

        if !render_device
            .features()
            .contains(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING)
        {
            error!(
                "Render device doesn't support feature \
                SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING, \
                which is required for texture binding arrays"
            );
            std::process::exit(1);
        }
    }
}

/// A render-world system that enqueues the entity with custom rendering into
/// the transparent render phases of each view.
fn queue_simulation(
    pipeline_cache: Res<PipelineCache>,
    simulation_pipeline: Res<SimulationPipeline>,
    msaa: Res<Msaa>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    transparent_draw_function: Res<DrawFunctions<Transparent2d>>,
    mut specialized_render_pipelines: ResMut<SpecializedRenderPipelines<SimulationPipeline>>,
    views: Query<Entity, (With<ExtractedView> /*With<SimulationCamera>*/,)>,
    simulations: Query<Entity, With<RenderedSimulation>>,
) {
    let draw_simulation = transparent_draw_function
        .read()
        .id::<DrawSimulationCommands>();

    // Render phases are per-view, so we need to iterate over all views so that
    // the entity appears in them. (In this example, we have only one view, but
    // it's good practice to loop over all views anyway.)
    for view_entity in views.iter() {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view_entity) else {
            continue;
        };

        // Find all the custom rendered entities that are visible from this
        // view.
        for entity in simulations.iter() {
            // Ordinarily, the [`SpecializedRenderPipeline::Key`] would contain
            // some per-view settings, such as whether the view is HDR, but for
            // simplicity's sake we simply hard-code the view's characteristics,
            // with the exception of number of MSAA samples.
            let pipeline_id = specialized_render_pipelines.specialize(
                &pipeline_cache,
                &simulation_pipeline,
                *msaa,
            );

            transparent_phase.add(Transparent2d {
                entity,
                pipeline: pipeline_id,
                draw_function: draw_simulation,
                sort_key: FloatOrd(-1.),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::NONE,
            });
        }
    }
}

impl SpecializedRenderPipeline for SimulationPipeline {
    type Key = Msaa;

    fn specialize(&self, msaa: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("simulation render pipeline".into()),
            layout: vec![
                self.uniforms_bind_group_layout.clone(),
                self.textures_bind_group_layout.clone(),
            ],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "vs_main".into(),
                buffers: vec![Vertex::desc(), particle::Raw::desc()],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: "fs_main".into(),
                targets: vec![Some(ColorTargetState {
                    // Ordinarily, you'd want to check whether the view has the
                    // HDR format and substitute the appropriate texture format
                    // here, but we omit that for simplicity.
                    format: TextureFormat::bevy_default(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Max,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            // Note that if your view has no depth buffer this will need to be
            // changed.
            depth_stencil: None,
            multisample: MultisampleState {
                count: msaa.samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }
    }
}

fn textures_prepared(
    simulation_textures: Res<SimulationTextures>,
    image_assets: Res<RenderAssets<GpuImage>>,
) -> bool {
    simulation_textures.textures.iter().all(|handle| {
        //println!("{:?}", handle.path());
        image_assets.get(handle).is_some()
    })
}

fn prepare_simulation_buffers(
    mut commands: Commands,
    views: Query<(Entity, &ExtractedView), With<SimulationCamera>>,
    //view_uniforms: Res<ViewUniforms>,
    simulations: Query<(Entity, &RenderedSimulation)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    image_assets: Res<RenderAssets<GpuImage>>,
    simulation_textures: Res<SimulationTextures>,
    pipeline: Res<SimulationPipeline>,
) {
    for (_, extracted_view) in views.iter() {
        let world_from_view = extracted_view.world_from_view.compute_matrix(); // TODO: replace with Res<ViewUniforms>
        let view_from_world = world_from_view.inverse();
        let clip_from_world = extracted_view.clip_from_view * view_from_world;

        for (entity, simulation) in &simulations {
            // handling particles
            let vertices =
                render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                    label: Some("simulation vertex buffer"),
                    contents: bytemuck::cast_slice(&particle::Raw::vertices()),
                    usage: BufferUsages::VERTEX,
                });

            let mut particles = RawBufferVec::new(BufferUsages::VERTEX);
            for p in simulation.0.particles.iter() {
                particles.push(particle::Raw::from_particle(p));
            }

            particles.write_buffer(&render_device, &render_queue);

            let indices =
                render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                    label: Some("simulation index buffer"),
                    contents: bytemuck::cast_slice(&particle::Raw::indices()),
                    usage: BufferUsages::INDEX,
                });

            // handling uniforms
            let uniforms =
                render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                    label: Some("simulation uniform buffer"),
                    contents: bytemuck::bytes_of(&clip_from_world),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let uniforms_bind_group = render_device.create_bind_group(
                Some("simulation uniform bind group"),
                &pipeline.uniforms_bind_group_layout,
                &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                }],
            );

            // TODO: binding textures every frame is not optimal, need to move this code into another function
            // handling textures
            let mut images = vec![];
            for handle in simulation_textures.textures.iter() {
                match image_assets.get(handle) {
                    Some(image) => images.push(image),
                    None => panic!("No image {handle:?} found in assets folder!"),
                }
            }

            let sampler = &images[0].sampler;
            let textures: Vec<&wgpu::TextureView> = images
                .into_iter()
                .map(|image| &*image.texture_view)
                .collect();

            let textures_bind_group = render_device.create_bind_group(
                "simulation textures bind group",
                &pipeline.textures_bind_group_layout,
                &BindGroupEntries::sequential((&textures[..], sampler)),
            );

            commands.entity(entity).insert(SimulationBuffers {
                vertices,
                particles,
                indices,
                uniforms,
                uniforms_bind_group,
                textures_bind_group,
            });
        }
    }
}

#[derive(Resource)]
pub struct SimulationTextures {
    pub textures: Vec<Handle<Image>>,
    pub background: Option<Handle<Image>>,
}

impl SimulationTextures {
    pub const SIMULATION_TEXTURES: [&'static str; 5] = [
        "particle-empty.png",
        "particle-sand.png",
        "particle-metal.png",
        "particle-motor.png",
        "particle-spike.png",
    ];
}

fn update_simulation_textures(mut commands: Commands, mut main_world: ResMut<MainWorld>) {
    let mut simulations = main_world.query::<(&mut Handle<Image>, &mut Visibility, &SimulationBackground)>();
    let Some(textures) = main_world.remove_resource::<SimulationTextures>() else {
        return;
    };    

    for (mut handle, mut visibility, _) in simulations.iter_mut(&mut main_world) {
        *handle = textures.background.as_ref().map_or(default(), |handle| handle.clone());
        *visibility = textures.background.as_ref().map_or(Visibility::Hidden, |_| Visibility::Visible);
    }

    commands.remove_resource::<SimulationPipeline>();
    commands.remove_resource::<SpecializedRenderPipelines<SimulationPipeline>>();

    commands.insert_resource(textures);
    commands.init_resource::<SimulationPipeline>();
    commands.init_resource::<SpecializedRenderPipelines<SimulationPipeline>>();

}

impl FromWorld for SimulationTextures {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let textures = SimulationTextures::SIMULATION_TEXTURES
            .iter()
            .map(|&name| asset_server.load(name))
            .collect();
        Self {
            textures,
            background: None,
        }
    }
}

impl FromWorld for SimulationPipeline {
    fn from_world(world: &mut World) -> Self {
        // Load and compile the shader in the background.
        let asset_server = world.resource::<AssetServer>();
        let render_device = world.resource::<RenderDevice>();

        let uniforms_bind_group_layout = render_device.create_bind_group_layout(
            Some("particles uniform bind group layout"),
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        let textures = &world.resource::<SimulationTextures>().textures;

        let textures_bind_group_layout = render_device.create_bind_group_layout(
            Some("particles textures bind group layout"),
            // particle textures
            &BindGroupLayoutEntries::with_indices(
                ShaderStages::FRAGMENT,
                (
                    (
                        0,
                        texture_2d(TextureSampleType::Float { filterable: true })
                            .count(NonZeroU32::new(textures.len() as u32).unwrap()),
                    ),
                    (1, sampler(SamplerBindingType::Filtering)),
                ),
            )
            .to_vec(),
        );

        SimulationPipeline {
            shader: asset_server.load("shaders/simulation.wgsl"),
            uniforms_bind_group_layout,
            textures_bind_group_layout,
        }
    }
}
