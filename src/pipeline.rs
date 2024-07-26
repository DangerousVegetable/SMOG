//! Demonstrates how to enqueue custom draw commands in a render phase.
//!
//! This example shows how to use the built-in
//! [`bevy_render::render_phase::BinnedRenderPhase`] functionality with a
//! custom [`RenderCommand`] to allow inserting arbitrary GPU drawing logic
//! into Bevy's pipeline. This is not the only way to add custom rendering code
//! into Bevy—render nodes are another, lower-level method—but it does allow
//! for better reuse of parts of Bevy's built-in mesh rendering logic.

use std::mem;

use bevy::{
    core_pipeline::{
        core_2d::Transparent2d,
        core_3d::{Opaque3d, Opaque3dBinKey, CORE_3D_DEPTH_FORMAT},
    },
    ecs::{
        query::{QueryItem, ROQueryItem, ReadOnlyQueryData},
        system::{
            lifetimeless::{Read, SQuery, SRes},
            SystemParamItem,
        },
    },
    math::{vec3, FloatOrd, Vec3A},
    prelude::*,
    render::{
        camera::{CameraProjection, ExtractedCamera},
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        primitives::Aabb,
        render_phase::{
            AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, PhaseItem, PhaseItemExtraIndex,
            RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
            ViewBinnedRenderPhases, ViewSortedRenderPhases,
        },
        render_resource::{
            BindGroup, BindGroupLayout, Buffer, BufferUsages, ColorTargetState, ColorWrites,
            CompareFunction, DepthStencilState, FragmentState, IndexFormat, MultisampleState,
            PipelineCache, PrimitiveState, RawBufferVec, RenderPipelineDescriptor,
            SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat, VertexAttribute,
            VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::BevyDefault as _,
        view::{self, ExtractedView, VisibilitySystems, VisibleEntities},
        Render, RenderApp, RenderSet,
    },
};
use bytemuck::{Pod, Zeroable};

pub mod particle;
mod vertex;

use vertex::Vertex;

use crate::Simulation;

/// A marker component that represents an entity that is to be rendered using
/// our custom phase item.
///
/// Note the [`ExtractComponent`] trait implementation. This is necessary to
/// tell Bevy that this object should be pulled into the render world.

/// Holds a reference to our shader.
///
/// This is loaded at app creation time.
#[derive(Resource)]
struct SimulationPipeline {
    shader: Handle<Shader>,
    uniform_bind_group_layout: BindGroupLayout,
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
        
        pass.set_bind_group(0, &simulation_buffers.uniform_bind_group, &[]);
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

    // uniform bindgroup
    uniform_bind_group: BindGroup,
    uniforms: Buffer,
}

/// The custom draw commands that Bevy executes for each entity we enqueue into
/// the render phase.
type DrawSimulationCommands = (SetItemPipeline, DrawSimulation);

impl ExtractComponent for Simulation {
    type QueryData = &'static Simulation;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(Simulation(item.0.clone()))
    }
}

pub struct RenderSimulationPlugin;

impl Plugin for RenderSimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<Simulation>::default());
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<SimulationPipeline>()
            .init_resource::<SpecializedRenderPipelines<SimulationPipeline>>()
            .add_render_command::<Transparent2d, DrawSimulationCommands>()
            .add_systems(
                Render,
                prepare_simulation_buffers.in_set(RenderSet::PrepareResources),
            )
            .add_systems(Render, queue_custom_phase_item.in_set(RenderSet::Queue));
    }
}

/// A render-world system that enqueues the entity with custom rendering into
/// the opaque render phases of each view.
fn queue_custom_phase_item(
    pipeline_cache: Res<PipelineCache>,
    custom_phase_pipeline: Res<SimulationPipeline>,
    msaa: Res<Msaa>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    transparent_draw_function: Res<DrawFunctions<Transparent2d>>,
    mut specialized_render_pipelines: ResMut<SpecializedRenderPipelines<SimulationPipeline>>,
    views: Query<Entity, With<ExtractedView>>,
    simulations: Query<Entity, With<Simulation>>,
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
        //println!("DRAWING ENTITY!!!");

        // Find all the custom rendered entities that are visible from this
        // view.
        for entity in simulations.iter() {
            // Ordinarily, the [`SpecializedRenderPipeline::Key`] would contain
            // some per-view settings, such as whether the view is HDR, but for
            // simplicity's sake we simply hard-code the view's characteristics,
            // with the exception of number of MSAA samples.
            let pipeline_id = specialized_render_pipelines.specialize(
                &pipeline_cache,
                &custom_phase_pipeline,
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
            layout: vec![self.uniform_bind_group_layout.clone()],
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
                    blend: None,
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

fn prepare_simulation_buffers(
    mut commands: Commands,
    views: Query<(Entity, &ExtractedView)>,
    simulations: Query<(Entity, &Simulation)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<SimulationPipeline>,
) {
    
    for (_, extracted_view) in views.iter() {
        let world_from_view = extracted_view.world_from_view.compute_matrix();
        let view_from_world = world_from_view.inverse();
        let clip_from_world = extracted_view.clip_from_view * view_from_world;

        for (entity, simulation) in &simulations {
            // handling particles
            let vertices = render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                label: Some("particle vertex buffer"),
                contents: bytemuck::cast_slice(&particle::Raw::vertices()),
                usage: BufferUsages::VERTEX,
            });
            
            let mut particles = RawBufferVec::new(BufferUsages::VERTEX);
            for p in simulation.0.particles.iter() {
                particles.push(particle::Raw::from_particle(p));
            }
            
            particles.write_buffer(&render_device, &render_queue);
            
            let indices = render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                label: Some("particle index buffer"),
                contents: bytemuck::cast_slice(&particle::Raw::indices()),
                usage: BufferUsages::INDEX,
            });
            
            // handling uniforms
            let uniforms = render_device.create_buffer_with_data(&wgpu::util::BufferInitDescriptor {
                label: Some("particles uniform buffer"),
                contents: bytemuck::bytes_of(&clip_from_world),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            
            let uniform_bind_group = render_device.create_bind_group(
                Some("particles uniform bind group"),
                &pipeline.uniform_bind_group_layout,
                &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                }],
            );
            
            commands.entity(entity).insert(SimulationBuffers {
                vertices,
                particles,
                indices,
                uniforms,
                uniform_bind_group
            });
        }
    }
}
    
    impl FromWorld for SimulationPipeline {
        fn from_world(world: &mut World) -> Self {
            // Load and compile the shader in the background.
            let asset_server = world.resource::<AssetServer>();
            let render_device = world.resource::<RenderDevice>();

        let uniform_bind_group_layout = render_device.create_bind_group_layout(
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

        SimulationPipeline {
            shader: asset_server.load("shaders/particles_lite.wgsl"),
            uniform_bind_group_layout,
        }
    }
}
